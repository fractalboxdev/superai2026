//! superai2026 local-first sync — a single binary that runs as both server and
//! client.
//!
//! Internals are placeholders; the real sync protocol (transport, conflict
//! resolution, persistence) will be specified later.

mod client;
mod server;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Local-first sync daemon and client.
#[derive(Debug, Parser)]
#[command(name = "sync", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the sync server (the authoritative peer).
    Serve {
        /// Address to bind, e.g. 127.0.0.1:7878
        #[arg(long, default_value = "127.0.0.1:7878")]
        addr: String,
    },
    /// Run the sync client and connect to a server.
    Client {
        /// Server address to connect to.
        #[arg(long, default_value = "127.0.0.1:7878")]
        addr: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli.command {
        Command::Serve { addr } => server::run(&addr).await,
        Command::Client { addr } => client::run(&addr).await,
    }
}
