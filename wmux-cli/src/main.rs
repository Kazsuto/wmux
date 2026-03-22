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

    /// Target workspace UUID
    #[arg(long, global = true)]
    workspace: Option<String>,

    /// Target surface UUID
    #[arg(long, global = true)]
    surface: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// System commands
    System {
        #[command(subcommand)]
        command: commands::system::SystemCommands,
    },
    /// Workspace management
    Workspace {
        #[command(subcommand)]
        command: commands::workspace::WorkspaceCommands,
    },
    /// Surface (pane) management
    Surface {
        #[command(subcommand)]
        command: commands::surface::SurfaceCommands,
    },
    /// Sidebar operations
    Sidebar {
        #[command(subcommand)]
        command: commands::sidebar::SidebarCommands,
    },
    /// Notification operations
    Notify {
        #[command(subcommand)]
        command: commands::notify::NotifyCommands,
    },
    /// Browser panel operations
    Browser,
    /// SSH remote workspace management
    Ssh {
        #[command(subcommand)]
        command: commands::ssh::SshCommands,
    },
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
        Commands::System { command } => {
            let ok = commands::system::handle(&client, cli.json, command)
                .await
                .context("system command failed")?;
            i32::from(!ok)
        }
        Commands::Workspace { command } => {
            let ok = commands::workspace::handle(&client, cli.json, command)
                .await
                .context("workspace command failed")?;
            i32::from(!ok)
        }
        Commands::Surface { command } => {
            let ok =
                commands::surface::handle(&client, cli.json, command, cli.workspace, cli.surface)
                    .await
                    .context("surface command failed")?;
            i32::from(!ok)
        }
        Commands::Sidebar { command } => {
            let ok = commands::sidebar::handle(&client, cli.json, command)
                .await
                .context("sidebar command failed")?;
            i32::from(!ok)
        }
        Commands::Notify { command } => {
            let ok = commands::notify::handle(&client, cli.json, command)
                .await
                .context("notify command failed")?;
            i32::from(!ok)
        }
        Commands::Browser => {
            eprintln!("browser commands not yet implemented");
            1
        }
        Commands::Ssh { command } => {
            let ok = commands::ssh::handle(&client, cli.json, command)
                .await
                .context("ssh command failed")?;
            i32::from(!ok)
        }
    };

    std::process::exit(exit_code);
}
