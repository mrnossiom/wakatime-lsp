//! Wakatime LSP

// TODO: check options for additional ideas <https://github.com/wakatime/wakatime-cli/blob/develop/USAGE.md#ini-config-file>

// TODO: implement debounding ourselves to avoid wkcli roundtrips
// TODO: read wakatime config
// TODO: do not log when out of dev folder

use serde_json::Value;
use std::{
	io::BufRead,
	panic::{self, PanicInfo},
};
use tokio::process::Command;
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

/// Open the Wakatime web dashboard in a browser
const OPEN_DASHBOARD_ACTION: &str = "open_dashboard";
/// Log the time past today in an editor
const SHOW_TIME_PAST_ACTION: &str = "show_time_past";

/// Implements [`LanguageServer`] to interact with an editor
#[derive(Debug)]
struct Backend {
	/// Interface for sending LSP notifications to the client
	client: Client,
}

impl Backend {
	/// Creates a new [`Backend`]
	const fn new(client: Client) -> Self {
		Self { client }
	}

	#[tracing::instrument(skip_all)]
	async fn on_change(&self, uri: Url, is_write: bool) {
		let mut cmd = Command::new("wakatime-cli");

		let user_agent = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
		cmd.args(["--plugin", user_agent]);

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
	async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
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

	#[tracing::instrument(skip_all)]
	async fn initialized(&self, _: InitializedParams) {}

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
