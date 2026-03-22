use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::IpcClient;
use crate::output::format_response;

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommands {
    /// List all workspaces
    List,
    /// Create a new workspace
    Create {
        /// Workspace name
        #[arg(long, default_value = "workspace")]
        name: String,
    },
    /// Show current workspace
    Current,
    /// Select a workspace by ID or index
    Select {
        /// Workspace UUID
        #[arg(long, group = "target", required_unless_present = "index")]
        id: Option<String>,
        /// Workspace index (1-based)
        #[arg(long, group = "target")]
        index: Option<u64>,
    },
    /// Close a workspace
    Close {
        /// Workspace UUID
        id: String,
    },
    /// Rename a workspace
    Rename {
        /// Workspace UUID
        id: String,
        /// New name
        name: String,
    },
}

pub async fn handle(client: &IpcClient, json_mode: bool, cmd: WorkspaceCommands) -> Result<bool> {
    let response = match cmd {
        WorkspaceCommands::List => client.request("workspace.list", None).await?,
        WorkspaceCommands::Create { name } => {
            client
                .request("workspace.create", Some(json!({ "name": name })))
                .await?
        }
        WorkspaceCommands::Current => client.request("workspace.current", None).await?,
        WorkspaceCommands::Select { id, index } => {
            let params = if let Some(id) = id {
                json!({ "workspace_id": id })
            } else if let Some(index) = index {
                json!({ "index": index })
            } else {
                anyhow::bail!("either --id or --index is required");
            };
            client.request("workspace.select", Some(params)).await?
        }
        WorkspaceCommands::Close { id } => {
            client
                .request("workspace.close", Some(json!({ "workspace_id": id })))
                .await?
        }
        WorkspaceCommands::Rename { id, name } => {
            client
                .request(
                    "workspace.rename",
                    Some(json!({ "workspace_id": id, "name": name })),
                )
                .await?
        }
    };

    let ok = response.ok;
    println!("{}", format_response(&response, json_mode));
    Ok(ok)
}
