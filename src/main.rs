//! Wakatime LS

use lsp_server::Connection;
use wakatime_ls::LanguageServer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Create the transport. Includes the stdio (stdin and stdout) versions but this could
	// also be implemented to use sockets or HTTP.
	let (connection, io_threads) = Connection::stdio();

	LanguageServer::new(connection).start()?;

	io_threads.join()?;

	Ok(())
}
