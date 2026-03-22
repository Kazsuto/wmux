use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::IpcClient;
use crate::output::format_response;

#[derive(Debug, Subcommand)]
pub enum SurfaceCommands {
    /// Split the current pane
    Split {
        /// Split direction: "right" or "bottom"
        #[arg(long, default_value = "right")]
        direction: String,
    },
    /// List surfaces in a workspace
    List {
        /// Filter by workspace UUID
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Focus a surface
    Focus {
        /// Surface UUID
        id: String,
    },
    /// Close a surface
    Close {
        /// Surface UUID
        id: String,
    },
    /// Read terminal text from a surface
    ReadText {
        /// Surface UUID (defaults to focused)
        #[arg(long)]
        surface: Option<String>,
        /// Start line offset (negative = from bottom)
        #[arg(long)]
        start: Option<i64>,
        /// End line offset
        #[arg(long)]
        end: Option<i64>,
    },
    /// Send text to a surface
    SendText {
        /// Text to send
        text: String,
        /// Surface UUID (defaults to focused)
        #[arg(long)]
        surface: Option<String>,
    },
    /// Send a key press to a surface
    SendKey {
        /// Key name (e.g. Enter, Tab, Ctrl+C, Up, F1)
        key: String,
        /// Surface UUID (defaults to focused)
        #[arg(long)]
        surface: Option<String>,
    },
}

pub async fn handle(
    client: &IpcClient,
    json_mode: bool,
    cmd: SurfaceCommands,
    global_workspace: Option<String>,
    global_surface: Option<String>,
) -> Result<bool> {
    let response = match cmd {
        SurfaceCommands::Split { direction } => {
            client
                .request("surface.split", Some(json!({ "direction": direction })))
                .await?
        }
        SurfaceCommands::List { workspace } => {
            let ws = workspace.or(global_workspace);
            let params = ws.map(|w| json!({ "workspace_id": w }));
            client.request("surface.list", params).await?
        }
        SurfaceCommands::Focus { id } => {
            client
                .request("surface.focus", Some(json!({ "surface_id": id })))
                .await?
        }
        SurfaceCommands::Close { id } => {
            client
                .request("surface.close", Some(json!({ "surface_id": id })))
                .await?
        }
        SurfaceCommands::ReadText {
            surface,
            start,
            end,
        } => {
            let mut params = json!({});
            if let Some(s) = surface.or(global_surface) {
                params["surface_id"] = json!(s);
            }
            if let Some(s) = start {
                params["start"] = json!(s);
            }
            if let Some(e) = end {
                params["end"] = json!(e);
            }
            client.request("surface.read_text", Some(params)).await?
        }
        SurfaceCommands::SendText { text, surface } => {
            let mut params = json!({ "text": text });
            if let Some(s) = surface.or(global_surface) {
                params["surface_id"] = json!(s);
            }
            client.request("surface.send_text", Some(params)).await?
        }
        SurfaceCommands::SendKey { key, surface } => {
            let mut params = json!({ "key": key });
            if let Some(s) = surface.or(global_surface) {
                params["surface_id"] = json!(s);
            }
            client.request("surface.send_key", Some(params)).await?
        }
    };

    let ok = response.ok;
    println!("{}", format_response(&response, json_mode));
    Ok(ok)
}
