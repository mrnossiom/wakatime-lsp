//! Wakatime LS implementation
//!
//! Entrypoint is [`Backend::new`]

// TODO: check options for additional ideas <https://github.com/wakatime/wakatime-cli/blob/develop/USAGE.md#ini-config-file>

// TODO: implement debounding ourselves to avoid wkcli roundtrips
// TODO: read wakatime config
// TODO: do not log when out of dev folder

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId};
use lsp_types::{notification::Notification as _, request::Request as _, *};

/// Open the Wakatime web dashboard in a browser
const OPEN_DASHBOARD_ACTION: &str = "open_dashboard";
/// Log the time past today in an editor
const SHOW_TIME_PAST_ACTION: &str = "show_time_past";

/// Base plugin user agent
const PLUGIN_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

pub struct LanguageServer {
	connection: Connection,

	user_agent: String,
}

impl LanguageServer {
	#[must_use]
	pub fn new(connection: Connection) -> Self {
		Self {
			connection,
			user_agent: PLUGIN_USER_AGENT.into(),
		}
	}

	fn capabilities() -> ServerCapabilities {
		ServerCapabilities {
			text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::NONE)),
			execute_command_provider: Some(ExecuteCommandOptions {
				commands: vec![OPEN_DASHBOARD_ACTION.into(), SHOW_TIME_PAST_ACTION.into()],
				work_done_progress_options: WorkDoneProgressOptions::default(),
			}),
			..Default::default()
		}
	}

	/// Entrypoint
	///
	/// # Errors
	///
	/// - For kindof everything that went wrong
	pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
		let server_capabilities = serde_json::to_value(Self::capabilities())?;
		let init_params = self.connection.initialize(server_capabilities)?;
		let init_params = serde_json::from_value::<InitializeParams>(init_params)?;

		if let Some(info) = &init_params.client_info {
			self.user_agent = format!(
				"{}/{} {} {}-wakatime/{}",
				// Editor part
				info.name,
				info.version
					.as_ref()
					.map_or_else(|| "unknown", |version| version),
				// Plugin part
				self.user_agent,
				// Last part is the one parsed by `wakatime` servers
				// It follows `{editor}-wakatime/{version}` where `editor` is
				// registered in intern. Works when `info.name` matches what the
				// wakatime dev choose.
				// IDEA: rely less on luck
				info.name,
				env!("CARGO_PKG_VERSION"),
			);
		}

		self.main_loop()?;

		Ok(())
	}

	fn main_loop(&self) -> Result<(), Box<dyn std::error::Error>> {
		for msg in &self.connection.receiver {
			match msg {
				Message::Request(req) => {
					if self.connection.handle_shutdown(&req)? {
						return Ok(());
					}

					self.handle_request(req)?;

					continue;
				}

				Message::Notification(notification) => {
					self.handle_notification(notification)?;

					continue;
				}

				Message::Response(response) => {
					eprintln!("{:?}", response.result);
					continue;
				}
			}
		}

		Ok(())
	}

	fn handle_request(&self, req: Request) -> Result<(), Box<dyn std::error::Error>> {
		let req = match cast_r::<request::ExecuteCommand>(req) {
			Ok((id, params)) => {
				self.execute_command(id, &params)?;
				return Ok(());
			}
			Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
			Err(ExtractError::MethodMismatch(req)) => req,
		};

		let _ = req;

		Ok(())
	}

	fn handle_notification(
		&self,
		notification: Notification,
	) -> Result<(), Box<dyn std::error::Error>> {
		let notification = match cast_n::<notification::DidOpenTextDocument>(notification) {
			Ok((_id, params)) => {
				self.on_change(&params.text_document.uri, false)?;
				return Ok(());
			}
			Err(err @ ExtractError::JsonError { .. }) => {
				eprintln!("{err:?}");
				return Ok(());
			}
			Err(ExtractError::MethodMismatch(req)) => req,
		};

		let notification = match cast_n::<notification::DidChangeTextDocument>(notification) {
			Ok((_id, params)) => {
				self.on_change(&params.text_document.uri, false)?;
				return Ok(());
			}
			Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
			Err(ExtractError::MethodMismatch(req)) => req,
		};

		let notification = match cast_n::<notification::DidCloseTextDocument>(notification) {
			Ok((_id, params)) => {
				self.on_change(&params.text_document.uri, false)?;
				return Ok(());
			}
			Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
			Err(ExtractError::MethodMismatch(req)) => req,
		};

		let notification = match cast_n::<notification::DidSaveTextDocument>(notification) {
			Ok((_id, params)) => {
				self.on_change(&params.text_document.uri, true)?;
				return Ok(());
			}
			Err(err @ ExtractError::JsonError { .. }) => panic!("{err:?}"),
			Err(ExtractError::MethodMismatch(req)) => req,
		};

		let _ = notification;

		Ok(())
	}

	fn on_change(&self, uri: &Uri, is_write: bool) -> Result<(), Box<dyn std::error::Error>> {
		let mut cmd = std::process::Command::new("wakatime-cli");

		cmd.args(["--plugin", &self.user_agent]);

		cmd.args(["--entity", uri.path().as_str()]);

		// cmd.args(["--lineno", ""]);
		// cmd.args(["--cursorno", ""]);
		// cmd.args(["--lines-in-file", ""]);
		// cmd.args(["--category", ""]);

		// cmd.args(["--alternate-project", ""]);
		// cmd.args(["--project-folder", ""]);

		if is_write {
			cmd.arg("--write");
		}

		match cmd.status() {
			Err(err) => Err(err.into()),
			Ok(exit) => {
				assert!(
					exit.success(),
					"`wakatime-cli` exited with error code: {}",
					exit.code().map_or("<none>".into(), |c| c.to_string())
				);
				Ok(())
			}
		}
	}

	fn execute_command(
		&self,
		id: RequestId,
		params: &ExecuteCommandParams,
	) -> Result<(), Box<dyn std::error::Error>> {
		match params.command.as_str() {
			OPEN_DASHBOARD_ACTION => {
				let show_documents_params = ShowDocumentParams {
					uri: "https://wakatime.com/dashboard"
						.parse()
						.expect("url is valid"),
					external: Some(true),
					take_focus: None,
					selection: None,
				};

				let req = Message::Request(Request::new(
					id,
					request::ShowMessageRequest::METHOD.into(),
					show_documents_params,
				));
				self.connection.sender.send(req)?;
			}
			SHOW_TIME_PAST_ACTION => {
				let output = std::process::Command::new("wakatime-cli")
					.arg("--today")
					.output()?;

				let time_past = String::from_utf8_lossy(&output.stdout);

				let notification = Message::Notification(Notification::new(
					notification::LogMessage::METHOD.into(),
					LogMessageParams {
						typ: MessageType::INFO,
						message: time_past.to_string(),
					},
				));
				self.connection.sender.send(notification)?;
			}
			unknown_cmd_id => {
				let message = format!("Unknown workspace command received: `{unknown_cmd_id}`");

				let notification = Message::Notification(Notification::new(
					notification::LogMessage::METHOD.into(),
					LogMessageParams {
						typ: MessageType::ERROR,
						message,
					},
				));
				self.connection.sender.send(notification)?;
			}
		};

		Ok(())
	}
}

fn cast_r<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
	R: lsp_types::request::Request,
	R::Params: serde::de::DeserializeOwned,
{
	req.extract(R::METHOD)
}

fn cast_n<N>(req: Notification) -> Result<(RequestId, N::Params), ExtractError<Notification>>
where
	N: lsp_types::notification::Notification,
	N::Params: serde::de::DeserializeOwned,
{
	req.extract(N::METHOD)
}
