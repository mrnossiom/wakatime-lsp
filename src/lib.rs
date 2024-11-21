//! Wakatime LS implementation
//!
//! Entrypoint is [`Backend::new`]

// TODO: check options for additional ideas <https://github.com/wakatime/wakatime-cli/blob/develop/USAGE.md#ini-config-file>

// TODO: implement debounding ourselves to avoid wkcli roundtrips
// TODO: read wakatime config
// TODO: do not log when out of dev folder

use serde_json::Value;
use std::io::BufRead;
use tokio::{process::Command, sync::RwLock};
use tower_lsp::{
	jsonrpc::{Error, Result},
	lsp_types::*,
	Client, LanguageServer,
};

/// Open the Wakatime web dashboard in a browser
const OPEN_DASHBOARD_ACTION: &str = "open_dashboard";
/// Log the time past today in an editor
const SHOW_TIME_PAST_ACTION: &str = "show_time_past";

/// Base plugin user agent
const PLUGIN_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Implements [`LanguageServer`] to interact with an editor
#[derive(Debug)]
pub struct Backend {
	/// Interface for sending LSP notifications to the client
	client: Client,

	/// Editor and LS user agent for `wakatime-cli`
	user_agent: RwLock<String>,
}

impl Backend {
	/// Creates a new [`Backend`]
	#[must_use]
	pub fn new(client: Client) -> Self {
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
