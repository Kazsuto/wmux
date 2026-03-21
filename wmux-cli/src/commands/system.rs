use anyhow::Result;

use crate::client::IpcClient;
use crate::output::format_response;

/// Execute `system ping` — send a ping to the IPC server and print the result.
pub async fn ping(client: &IpcClient, json_mode: bool) -> Result<bool> {
    let response = client.request("system.ping", None).await?;
    let ok = response.ok;
    println!("{}", format_response(&response, json_mode));
    Ok(ok)
}
