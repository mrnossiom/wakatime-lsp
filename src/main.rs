//! Wakatime LS

use std::panic::{self, PanicInfo};
use tower_lsp::{LspService, Server};
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
use wakatime_ls::Backend;

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

	let file_appender = tracing_appender::rolling::never("/tmp", "wakatime-ls.log");
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
