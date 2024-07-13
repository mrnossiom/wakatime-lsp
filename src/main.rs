//! Wakatime LSP

// TODO: check options for additional ideas <https://github.com/wakatime/wakatime-cli/blob/develop/USAGE.md#ini-config-file>

// TODO: implement debounding ourselves to avoid wkcli roundtrips
// TODO: read wakatime config
// TODO: do not log when out of dev folder

use reqwest::Client as HttpClient;
use serde_json::Value;
use std::env;
use std::fs::File;
use std::io::{copy, Cursor};
use std::{
	io::BufRead,
	panic::{self, PanicInfo},
};
use tokio::{process::Command, sync::RwLock};
use tower_lsp::{
	jsonrpc::{Error, Result},
	lsp_types::{
		DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
		DidSaveTextDocumentParams, ExecuteCommandOptions, ExecuteCommandParams, InitializeParams,
		InitializeResult, InitializedParams, MessageType, ServerCapabilities, ServerInfo,
		ShowDocumentParams, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
		WorkDoneProgressOptions,
	},
	Client, LanguageServer, LspService, Server,
};
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
use zip::read::ZipArchive;

#[cfg(target_os = "macos")]
static WAKATIME_CLI_RELEASE_FILE_PLATFORM: &str = "darwin";

#[cfg(target_os = "linux")]
static WAKATIME_CLI_RELEASE_FILE_PLATFORM: &str = "linux";

#[cfg(target_os = "windows")]
static WAKATIME_CLI_RELEASE_FILE_PLATFORM: &str = "windows";

#[cfg(target_arch = "x86_64")]
static WAKATIME_CLI_RELEASE_FILE_ARCH: &str = "amd64";

#[cfg(target_arch = "aarch64")]
static WAKATIME_CLI_RELEASE_FILE_ARCH: &str = "arm64";

/// Open the Wakatime web dashboard in a browser
const OPEN_DASHBOARD_ACTION: &str = "open_dashboard";
/// Log the time past today in an editor
const SHOW_TIME_PAST_ACTION: &str = "show_time_past";

/// Base plugin user agent
const PLUGIN_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Implements [`LanguageServer`] to interact with an editor
#[derive(Debug)]
struct Backend {
	/// Interface for sending LSP notifications to the client
	client: Client,

	/// Editor and LSP user agent for `wakatime-cli`
	user_agent: RwLock<String>,
}

impl Backend {
	/// Creates a new [`Backend`]
	fn new(client: Client) -> Self {
		Self {
			client,
			user_agent: RwLock::new(PLUGIN_USER_AGENT.into()),
		}
	}

	#[tracing::instrument(skip_all)]
	async fn on_change(&self, uri: Url, is_write: bool) {
		let mut cmd = Command::new("wakatime-cli");

		let user_agent = self.user_agent.read().await;
		cmd.args(["--plugin", &user_agent]);

		cmd.args(["--entity", uri.path()]);

		// cmd.args(["--lineno", ""]);
		// cmd.args(["--cursorno", ""]);
		// cmd.args(["--lines-in-file", ""]);
		// cmd.args(["--category", ""]);

		// cmd.args(["--alternate-project", ""]);
		// cmd.args(["--project-folder", ""]);

		if is_write {
			cmd.arg("--write");
		}

		tracing::debug!(cmd = ?cmd.as_std());

		match cmd.status().await {
			Err(e) => tracing::error!(?e),
			Ok(exit) if !exit.success() => {
				tracing::error!(
					"`wakatime-cli` exited with error code: {}",
					exit.code().map_or("<none>".into(), |c| c.to_string())
				);
			}
			_ => {}
		};
	}
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
	#[tracing::instrument(skip_all)]
	async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
		install_wakatime_cli_if_missing().await?;

		if let Some(info) = params.client_info {
			let mut ua = self.user_agent.write().await;

			*ua = format!(
				"{}/{} {} {}-wakatime/{}",
				// Editor part
				info.name,
				info.version
					.as_ref()
					.map_or_else(|| "unknown", |version| version),
				// Plugin part
				PLUGIN_USER_AGENT,
				// Last part is the one parsed by `wakatime` servers
				// It follows `{editor}-wakatime/{version}` where `editor` is
				// registered in intern. Works when `info.name` matches what the
				// wakatime dev choose.
				// IDEA: rely less on luck
				info.name,
				env!("CARGO_PKG_VERSION"),
			);
		};

		Ok(InitializeResult {
			server_info: Some(ServerInfo {
				name: env!("CARGO_PKG_NAME").into(),
				version: Some(env!("CARGO_PKG_VERSION").into()),
			}),
			capabilities: ServerCapabilities {
				text_document_sync: Some(TextDocumentSyncCapability::Kind(
					TextDocumentSyncKind::NONE,
				)),
				execute_command_provider: Some(ExecuteCommandOptions {
					commands: vec![OPEN_DASHBOARD_ACTION.into(), SHOW_TIME_PAST_ACTION.into()],
					work_done_progress_options: WorkDoneProgressOptions::default(),
				}),
				..Default::default()
			},
		})
	}

	async fn initialized(&self, _: InitializedParams) {
		tracing::info!("client `{}` initialized", PLUGIN_USER_AGENT);
	}

	#[tracing::instrument(skip_all)]
	async fn shutdown(&self) -> Result<()> {
		Ok(())
	}

	async fn did_open(&self, params: DidOpenTextDocumentParams) {
		self.on_change(params.text_document.uri, false).await;
	}

	async fn did_change(&self, params: DidChangeTextDocumentParams) {
		self.on_change(params.text_document.uri, false).await;
	}

	async fn did_close(&self, params: DidCloseTextDocumentParams) {
		self.on_change(params.text_document.uri, false).await;
	}

	async fn did_save(&self, params: DidSaveTextDocumentParams) {
		self.on_change(params.text_document.uri, true).await;
	}

	async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
		match params.command.as_str() {
			OPEN_DASHBOARD_ACTION => {
				self.client
					.show_document(ShowDocumentParams {
						uri: "https://wakatime.com/dashboard"
							.try_into()
							.expect("the url is valid"),
						external: Some(true),
						take_focus: None,
						selection: None,
					})
					.await?;
			}
			SHOW_TIME_PAST_ACTION => {
				let output = Command::new("wakatime-cli")
					.arg("--today")
					.output()
					.await
					.map_err(|e| {
						tracing::error!("While executing `wakatime-cli`: {e}");
						Error::internal_error()
					})?;

				let Some(Ok(time_past)) = output.stdout.lines().next() else {
					tracing::error!("");
					return Err(Error::internal_error());
				};

				self.client.show_message(MessageType::INFO, time_past).await;
			}
			unknown_cmd_id => {
				let message = format!("Unknown workspace command received: `{unknown_cmd_id}`");
				tracing::error!(message);
				self.client.log_message(MessageType::ERROR, &message).await;
			}
		};

		Ok(None)
	}
}

/// Transfers panic messages to the tracing logging pipeline
fn tracing_panic_hook(panic_info: &PanicInfo) {
	let payload = panic_info
		.payload()
		.downcast_ref::<&'static str>()
		.map_or_else(
			|| {
				panic_info
					.payload()
					.downcast_ref::<String>()
					.map_or("Box<dyn Any>", |s| &s[..])
			},
			|s| *s,
		);

	let location = panic_info.location().map(ToString::to_string);

	tracing::error!(
		panic.payload = payload,
		panic.location = location,
		"A panic occurred",
	);
}

async  fn install_wakatime_cli_if_missing() -> Result<()> {
	if which::which("wakatime-cli").is_ok() {
		tracing::debug!("wakatime-cli is already installed");
		return Ok(());
	}

	let download_url = format!("https://github.com/wakatime/wakatime-cli/releases/latest/download/wakatime-cli-{WAKATIME_CLI_RELEASE_FILE_PLATFORM}-{WAKATIME_CLI_RELEASE_FILE_ARCH}.zip");
	tracing::info!("Downloading wakatime-cli from {download_url}");

	// Create a HTTP client
	let client = HttpClient::new();

	// Send a GET request to download the file
	let response = client.get(&download_url).send().await.map_err(|e| Error {
		code: tower_lsp::jsonrpc::ErrorCode::InternalError,
		data: None,
		message: std::borrow::Cow::Owned(format!("Failed to download wakatime-cli: {e}")),
	})?;

	// Ensure the request was successful
	if !response.status().is_success() {
		let status_code = response.status();
		return Err(Error {
			code: tower_lsp::jsonrpc::ErrorCode::InternalError,
			message: std::borrow::Cow::Owned(format!(
				"Failed to download wakatime-cli: got HTTP {status_code} on {download_url}"
			)),
			data: None,
		});
	}

	// Read the response body into a Vec<u8>
	let bytes = response.bytes().await.map_err(|e| Error {
		code: tower_lsp::jsonrpc::ErrorCode::InternalError,
		data: None,
		message: std::borrow::Cow::Owned(format!(
			"While downloading wakatime-cli: failed to read response body: {e}"
		)),
	})?;
	let reader = Cursor::new(bytes);

	tracing::debug!("Extracting wakatime-cli from zip archive");

	// Create a ZipArchive from the response body
	let mut archive = ZipArchive::new(reader).map_err(|e| Error {
		code: tower_lsp::jsonrpc::ErrorCode::InternalError,
		data: None,
		message: std::borrow::Cow::Owned(format!(
			"While downloading wakatime-cli: could not parse zip archive: {e}"
		)),
	})?;

	// Extract the first (and presumably only) file from the archive
	let mut file = archive.by_index(0).map_err(|e| Error {
		code: tower_lsp::jsonrpc::ErrorCode::InternalError,
		data: None,
		message: std::borrow::Cow::Owned(format!(
			"While downloading wakatime-cli: archive has no files (or is corrupted): {e}"
		)),
	})?;

	let output_filepath = env::current_exe()
		.expect("Could not get current executable path")
		.parent()
		.expect("Could not get parent directory of current executable path")
		.join("wakatime-cli");

	// Create the output file, store it next to the current executable
	let mut output_file = File::create(&output_filepath).map_err(|e| Error {
		code: tower_lsp::jsonrpc::ErrorCode::InternalError,
		data: None,
		message: std::borrow::Cow::Owned(format!(
			"While downloading wakatime-cli: could not create output file: {e}"
		)),
	})?;

	tracing::debug!("Writing binary to {output_filepath:?}");

	// Copy the contents of the zip file to the output file
	copy(&mut file, &mut output_file).map_err(|e| Error {
		code: tower_lsp::jsonrpc::ErrorCode::InternalError,
		data: None,
		message: std::borrow::Cow::Owned(format!(
			"While downloading wakatime-cli: could not write output file: {e}"
		)),
	})?;

	Ok(())
}

// We really don't need much power with what we are doing
#[tokio::main(flavor = "current_thread")]
async fn main() {
	panic::set_hook(Box::new(tracing_panic_hook));

	let file_appender = tracing_appender::rolling::never("/tmp", "wakatime-lsp.log");
	let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
	tracing_subscriber::fmt()
		.with_writer(non_blocking)
		.with_span_events(FmtSpan::NEW)
		.with_env_filter(EnvFilter::from_default_env())
		.init();

	let stdin = tokio::io::stdin();
	let stdout = tokio::io::stdout();

	let (service, socket) = LspService::new(Backend::new);
	Server::new(stdin, stdout, socket).serve(service).await;
}
