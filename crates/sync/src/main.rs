//! Contextful — local-first company brain. One Rust binary, several subcommands:
//!
//!   serve   — Loro CRDT sync relay (authoritative peer)         [spec 01]
//!   client  — headless file-sync peer                           [spec 01]
//!   ingest  — run a connector → raw events → synthesis          [spec 02/05]
//!   cron    — scheduled ingest pipelines                        [spec 05]
//!   mcp     — brain over MCP (stdio)                            [spec 06]
//!   agent   — agent runtime loop (MCP-only tool surface)        [spec 04]
//!   ctl     — control plane: seed / mint / grant / revoke / show / audit  [spec 07]

use anyhow::Result;
use clap::{Parser, Subcommand};

use sync::config::Config;
use sync::sync as rooms; // the relay module (crate is also named `sync`)
use sync::{agent, brain, controlplane, cron, scenario};

#[derive(Debug, Parser)]
#[command(name = "sync", version, about = "Contextful local-first company brain")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the sync relay (the authoritative peer).
    Serve {
        #[arg(long, default_value = "127.0.0.1:7878")]
        addr: String,
        /// Also co-host the brain MCP server over streamable HTTP (spec 06 §4).
        #[arg(long)]
        with_mcp: bool,
        /// Bind address for the co-hosted MCP HTTP endpoint.
        #[arg(long, default_value = "127.0.0.1:7979")]
        mcp_addr: String,
        /// Also co-host the cron scheduler (spec 05 §3) so the host keeps
        /// the brain fresh without a separate `sync cron` process.
        #[arg(long)]
        with_cron: bool,
        /// Also co-host the editor agent (spec 04 §1) so `Q:` lines in the
        /// relay doc get answered without a separate `sync agent` process.
        #[arg(long)]
        with_editor_agent: bool,
        /// Document the co-hosted editor agent watches.
        #[arg(long, default_value = "finops")]
        agent_doc: String,
        /// Principal the co-hosted editor agent answers as.
        #[arg(long, default_value = "cfo")]
        agent_principal: String,
    },
    /// Run a headless file-sync client peer.
    Client {
        #[arg(long, default_value = "127.0.0.1:7878")]
        addr: String,
        /// Document id to subscribe to.
        #[arg(long, default_value = "finops")]
        doc: String,
        /// Principal id presented in HELLO.
        #[arg(long, default_value = "agent:cto/1")]
        principal: String,
    },
    /// Ingest a connector's data and run synthesis.
    Ingest {
        #[arg(long, default_value = "stripe")]
        source: String,
    },
    /// Run the cron scheduler for ETL pipelines.
    Cron,
    /// Run the brain MCP server over stdio.
    Mcp {
        #[arg(long, default_value = "agent:cto/1")]
        principal: String,
    },
    /// Run an agent loop whose only tool surface is the brain MCP.
    Agent {
        #[arg(long, default_value = "agent:cto/1")]
        principal: String,
        /// Ask the brain this question, then exit.
        #[arg(long)]
        ask: Option<String>,
        /// Watch this relay document and answer its `Q:` lines from the brain.
        #[arg(long)]
        watch_doc: Option<String>,
        /// Relay address used with --watch-doc.
        #[arg(long, default_value = "127.0.0.1:7878")]
        addr: String,
    },
    /// Control plane: identity & membership (cannot mint data authority).
    Ctl {
        #[command(subcommand)]
        cmd: CtlCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CtlCommand {
    /// Seed the demo principals, root catalog, envelopes, and initial tokens.
    Seed,
    /// (Re)issue a principal's initial capability token.
    Mint {
        #[arg(long)]
        principal: String,
    },
    /// Revoke a principal's tokens (recorded in the revocation list).
    Revoke {
        #[arg(long)]
        principal: String,
    },
    /// Approve a scoped grant to a principal (CFO models the approval; salary
    /// is always denied). Models Flow A's approve step.
    Grant {
        /// principal receiving the grant, e.g. agent:cto/1
        #[arg(long)]
        to: String,
        /// view id, e.g. stripe/finance_private
        #[arg(long)]
        view: String,
        /// comma-separated fields, e.g. gross,credits,discount_tier
        #[arg(long, value_delimiter = ',')]
        fields: Vec<String>,
        #[arg(long, default_value = "7d")]
        ttl: String,
    },
    /// Show the seeded control-plane state.
    Show,
    /// Show the host-persisted audit trail (grants, denials, routing,
    /// egress blocks; oldest first).
    Audit {
        /// how many of the most recent events to show
        #[arg(long, default_value_t = 20)]
        tail: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli.command {
        Command::Serve {
            addr,
            with_mcp,
            mcp_addr,
            with_cron,
            with_editor_agent,
            agent_doc,
            agent_principal,
        } => {
            if with_mcp {
                tracing::info!("co-hosting the brain MCP over streamable HTTP (spec 06 §4)");
                let mcp_addr = mcp_addr.clone();
                tokio::spawn(async move {
                    if let Err(e) = brain::mcp::serve_http(&mcp_addr).await {
                        tracing::error!(error = %e, "mcp http server failed");
                    }
                });
            }
            if with_cron {
                tracing::info!("co-hosting the cron scheduler (spec 05 §3)");
                tokio::spawn(async {
                    if let Err(e) = cron::scheduler::run().await {
                        tracing::error!(error = %e, "cron scheduler failed");
                    }
                });
            }
            if with_editor_agent {
                tracing::info!(
                    doc = %agent_doc,
                    principal = %agent_principal,
                    "co-hosting the editor agent (spec 04 §1)"
                );
                let dial = agent::editor::dial_addr(&addr);
                tokio::spawn(async move {
                    // the relay binds after this spawn; wait, then keep the
                    // agent alive across relay hiccups / missing brain index
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    loop {
                        match agent::editor::watch(&dial, &agent_doc, &agent_principal).await {
                            Ok(()) => tracing::info!("editor agent exited; reconnecting"),
                            Err(e) => tracing::warn!(error = %e, "editor agent failed; retrying"),
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                });
            }
            rooms::server::run(&addr).await
        }
        Command::Client {
            addr,
            doc,
            principal,
        } => rooms::client::run(&addr, &doc, &principal).await,
        Command::Ingest { source } => cron::ingest_once(&source),
        Command::Cron => cron::scheduler::run().await,
        Command::Mcp { principal } => brain::mcp::run(&principal),
        Command::Agent {
            principal,
            ask,
            watch_doc,
            addr,
        } => match watch_doc {
            Some(doc) => agent::editor::watch(&addr, &doc, &principal).await,
            None => agent::runtime::run(&principal, ask.as_deref()),
        },
        Command::Ctl { cmd } => run_ctl(cmd),
    }
}

fn run_ctl(cmd: CtlCommand) -> Result<()> {
    let config = Config::load();
    match cmd {
        CtlCommand::Seed => controlplane::seed(&config),
        CtlCommand::Mint { principal } => {
            let cap = scenario::initial_capability(&principal)
                .ok_or_else(|| anyhow::anyhow!("unknown principal '{principal}'"))?;
            // sign with the resource root's key — an unsigned mirror would
            // fail verification at load (run `ctl seed` to create the key)
            let keys = controlplane::keys::ensure_root_key(&config, "cfo")?;
            let signed = sync::access::token::sign(&cap, &keys).map_err(anyhow::Error::from)?;
            controlplane::save_capability(&config, &signed)?;
            controlplane::audit::record(
                &config,
                &principal,
                controlplane::audit::MINT,
                serde_json::Value::Null,
            );
            println!("minted + signed initial token for {principal}");
            Ok(())
        }
        CtlCommand::Revoke { principal } => {
            controlplane::revoke(&config, &principal)?;
            println!("revoked tokens for {principal}");
            Ok(())
        }
        CtlCommand::Grant {
            to,
            view,
            fields,
            ttl,
        } => controlplane::grant(&config, &to, &view, &fields, &ttl),
        CtlCommand::Show => controlplane::show(&config),
        CtlCommand::Audit { tail } => {
            for e in controlplane::audit::tail(&config, tail) {
                println!("{}  {:<16} {:<14} {}", e.ts, e.action, e.actor, e.detail);
            }
            Ok(())
        }
    }
}
