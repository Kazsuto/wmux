use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::IpcClient;
use crate::output::format_response;

#[derive(Debug, Subcommand)]
pub enum SidebarCommands {
    /// Set a status badge in the sidebar
    SetStatus {
        /// Status key (identifier)
        key: String,
        /// Status value (display text)
        value: String,
        /// Icon (emoji or symbol)
        #[arg(long)]
        icon: Option<String>,
        /// Color name
        #[arg(long)]
        color: Option<String>,
        /// Associated process ID
        #[arg(long)]
        pid: Option<u32>,
    },
    /// Clear a status badge
    ClearStatus {
        /// Status key to clear
        key: String,
    },
    /// List all status badges
    ListStatus,
    /// Set sidebar progress bar
    SetProgress {
        /// Progress value (0.0 to 1.0)
        value: f64,
        /// Optional label
        #[arg(long)]
        label: Option<String>,
    },
    /// Clear sidebar progress bar
    ClearProgress,
    /// Add a log entry
    Log {
        /// Log message
        message: String,
        /// Log level (info, warn, error, debug)
        #[arg(long, default_value = "info")]
        level: String,
        /// Source identifier
        #[arg(long, default_value = "unknown")]
        source: String,
    },
    /// Clear all log entries
    ClearLog,
    /// List recent log entries
    ListLog {
        /// Maximum number of entries
        #[arg(long, default_value = "50")]
        limit: u64,
    },
    /// Show full sidebar state
    State,
}

pub async fn handle(client: &IpcClient, json_mode: bool, cmd: SidebarCommands) -> Result<bool> {
    let response = match cmd {
        SidebarCommands::SetStatus {
            key,
            value,
            icon,
            color,
            pid,
        } => {
            let mut params = json!({ "key": key, "value": value });
            if let Some(i) = icon {
                params["icon"] = json!(i);
            }
            if let Some(c) = color {
                params["color"] = json!(c);
            }
            if let Some(p) = pid {
                params["pid"] = json!(p);
            }
            client.request("sidebar.set_status", Some(params)).await?
        }
        SidebarCommands::ClearStatus { key } => {
            client
                .request("sidebar.clear_status", Some(json!({ "key": key })))
                .await?
        }
        SidebarCommands::ListStatus => client.request("sidebar.list_status", None).await?,
        SidebarCommands::SetProgress { value, label } => {
            anyhow::ensure!(
                (0.0..=1.0).contains(&value),
                "progress value must be between 0.0 and 1.0"
            );
            let mut params = json!({ "value": value });
            if let Some(l) = label {
                params["label"] = json!(l);
            }
            client.request("sidebar.set_progress", Some(params)).await?
        }
        SidebarCommands::ClearProgress => client.request("sidebar.clear_progress", None).await?,
        SidebarCommands::Log {
            message,
            level,
            source,
        } => {
            client
                .request(
                    "sidebar.log",
                    Some(json!({ "message": message, "level": level, "source": source })),
                )
                .await?
        }
        SidebarCommands::ClearLog => client.request("sidebar.clear_log", None).await?,
        SidebarCommands::ListLog { limit } => {
            client
                .request("sidebar.list_log", Some(json!({ "limit": limit })))
                .await?
        }
        SidebarCommands::State => client.request("sidebar.state", None).await?,
    };

    let ok = response.ok;
    println!("{}", format_response(&response, json_mode));
    Ok(ok)
}
