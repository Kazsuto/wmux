use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod client;
mod commands;
mod output;

#[derive(Parser)]
#[command(name = "wmux", about = "wmux terminal multiplexer CLI")]
struct Cli {
    /// Override Named Pipe path
    #[arg(long)]
    pipe: Option<String>,

    /// Output raw JSON instead of human-readable format
    #[arg(long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// System commands
    System {
        #[command(subcommand)]
        command: SystemCommands,
    },
    /// Workspace management
    Workspace,
    /// Surface (tab) management
    Surface,
    /// Sidebar operations
    Sidebar,
    /// Notification operations
    Notify,
    /// Browser panel operations
    Browser,
}

#[derive(Subcommand)]
enum SystemCommands {
    /// Ping the wmux server
    Ping,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    let pipe_name = cli.pipe.unwrap_or_else(wmux_ipc::pipe_name);
    let client = client::IpcClient::new(pipe_name);

    let exit_code = match cli.command {
        Commands::System { command } => match command {
            SystemCommands::Ping => {
                let ok = commands::system::ping(&client, cli.json)
                    .await
                    .context("ping failed")?;
                if ok {
                    0
                } else {
                    1
                }
            }
        },
        Commands::Workspace => {
            eprintln!("workspace commands not yet implemented");
            1
        }
        Commands::Surface => {
            eprintln!("surface commands not yet implemented");
            1
        }
        Commands::Sidebar => {
            eprintln!("sidebar commands not yet implemented");
            1
        }
        Commands::Notify => {
            eprintln!("notify commands not yet implemented");
            1
        }
        Commands::Browser => {
            eprintln!("browser commands not yet implemented");
            1
        }
    };

    std::process::exit(exit_code);
}
