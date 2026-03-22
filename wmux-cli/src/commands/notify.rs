use anyhow::Result;
use clap::Subcommand;
use wmux_ipc::protocol::{RpcErrorCode, RpcResponse};

use crate::client::IpcClient;
use crate::output::format_response;

#[derive(Debug, Subcommand)]
pub enum NotifyCommands {
    /// Create a notification
    Create {
        /// Notification title
        title: String,
        /// Notification body
        #[arg(long)]
        body: Option<String>,
    },
    /// List active notifications
    List,
    /// Clear all notifications
    Clear,
}

pub async fn handle(_client: &IpcClient, json_mode: bool, cmd: NotifyCommands) -> Result<bool> {
    let label = match &cmd {
        NotifyCommands::Create { .. } => "notify create",
        NotifyCommands::List => "notify list",
        NotifyCommands::Clear => "notify clear",
    };

    let response = RpcResponse::error(
        "0",
        RpcErrorCode::InternalError,
        format!("{label} is not yet implemented (pending Task L3_08)"),
    );
    println!("{}", format_response(&response, json_mode));
    Ok(false)
}
