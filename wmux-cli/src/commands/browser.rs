use anyhow::Result;
use clap::Subcommand;

use crate::client::IpcClient;
use crate::output::format_response;

#[derive(Debug, Subcommand)]
pub enum BrowserCommands {
    /// Open a browser tab in the focused pane
    Open {
        /// URL to navigate to (default: about:blank)
        #[arg(long, default_value = "about:blank")]
        url: String,
    },
    /// Navigate an existing browser panel to a URL
    Navigate {
        /// Target URL
        url: String,
        /// Surface ID of the browser panel
        #[arg(long)]
        surface_id: String,
    },
    /// Go back in browser history
    Back {
        /// Surface ID of the browser panel
        #[arg(long)]
        surface_id: String,
    },
    /// Go forward in browser history
    Forward {
        /// Surface ID of the browser panel
        #[arg(long)]
        surface_id: String,
    },
    /// Reload the current page
    Reload {
        /// Surface ID of the browser panel
        #[arg(long)]
        surface_id: String,
    },
    /// Get the current URL
    Url {
        /// Surface ID of the browser panel
        #[arg(long)]
        surface_id: String,
    },
    /// Evaluate JavaScript in the browser
    Eval {
        /// JavaScript expression to evaluate
        expression: String,
        /// Surface ID of the browser panel
        #[arg(long)]
        surface_id: String,
    },
}

pub async fn handle(client: &IpcClient, json_mode: bool, cmd: BrowserCommands) -> Result<bool> {
    let (method, params) = match &cmd {
        BrowserCommands::Open { url } => ("browser.open", serde_json::json!({ "url": url })),
        BrowserCommands::Navigate { url, surface_id } => (
            "browser.navigate",
            serde_json::json!({ "url": url, "surface_id": surface_id }),
        ),
        BrowserCommands::Back { surface_id } => (
            "browser.back",
            serde_json::json!({ "surface_id": surface_id }),
        ),
        BrowserCommands::Forward { surface_id } => (
            "browser.forward",
            serde_json::json!({ "surface_id": surface_id }),
        ),
        BrowserCommands::Reload { surface_id } => (
            "browser.reload",
            serde_json::json!({ "surface_id": surface_id }),
        ),
        BrowserCommands::Url { surface_id } => (
            "browser.url",
            serde_json::json!({ "surface_id": surface_id }),
        ),
        BrowserCommands::Eval {
            expression,
            surface_id,
        } => (
            "browser.eval",
            serde_json::json!({ "expression": expression, "surface_id": surface_id }),
        ),
    };

    let response = client.request(method, Some(params)).await?;
    println!("{}", format_response(&response, json_mode));
    Ok(response.error.is_none())
}
