use anyhow::{Context, Result};
use clap::Subcommand;

use crate::client::IpcClient;

#[derive(Debug, Subcommand)]
pub enum SshCommands {
    /// Connect to a remote host
    Connect {
        /// Target in format user@host[:port]
        target: String,
    },
    /// Disconnect from current remote workspace
    Disconnect,
}

pub async fn handle(_client: &IpcClient, json_mode: bool, cmd: SshCommands) -> Result<bool> {
    match cmd {
        SshCommands::Connect { target } => {
            // Validate target format early before attempting any connection
            let _config = wmux_core::remote::RemoteConfig::parse(&target)
                .context("invalid SSH target format (expected user@host[:port])")?;

            // The actual SSH tunneling will be implemented when the Go daemon is ready
            eprintln!(
                "SSH remote connection to '{}' is not yet fully implemented.",
                target
            );
            eprintln!("Remote workspace model is ready — daemon integration pending.");

            if json_mode {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "target": target,
                        "message": "SSH remote connection pending daemon integration"
                    })
                );
            }

            Ok(true)
        }
        SshCommands::Disconnect => {
            eprintln!("No active SSH connections to disconnect.");
            Ok(true)
        }
    }
}
