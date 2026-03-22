use anyhow::Result;
use clap::Subcommand;

use crate::client::IpcClient;
use crate::output::format_response;

#[derive(Debug, Subcommand)]
pub enum SystemCommands {
    /// Ping the wmux server
    Ping,
    /// List server capabilities
    Capabilities,
    /// Identify the wmux server
    Identify,
}

pub async fn handle(client: &IpcClient, json_mode: bool, cmd: SystemCommands) -> Result<bool> {
    let method = match cmd {
        SystemCommands::Ping => "system.ping",
        SystemCommands::Capabilities => "system.capabilities",
        SystemCommands::Identify => "system.identify",
    };

    let response = client.request(method, None).await?;
    let ok = response.ok;
    println!("{}", format_response(&response, json_mode));
    Ok(ok)
}
