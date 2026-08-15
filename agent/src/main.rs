mod config;
mod connection;
mod consent;
mod device;
mod logging;
mod media;
mod protocol;
mod trusted_access;

use anyhow::Result;
use config::Config;
use device::DeviceIdentity;
use tokio::sync::watch;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    logging::init();
    let config = Config::from_env()?;
    let device = DeviceIdentity::load()?;
    let trusted_access = trusted_access::TrustedAccess::load(device.device_id)?;
    let (local_command_tx, local_command_rx) = tokio::sync::mpsc::channel(1);
    tokio::task::spawn_blocking(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if line.trim().eq_ignore_ascii_case("revoke-trust") {
                let _ = local_command_tx.blocking_send(());
            }
        }
    });

    println!(
        "\n22Pie Remote Agent v{}\n\nDevice Name:\n{}\n\nOperating System:\n{}\n\nArchitecture:\n{}\n\nDevice ID:\n{}\n\nServer:\n{}\n",
        device.agent_version,
        device.device_name,
        device.operating_system,
        device.architecture,
        device.device_id,
        config.server_url,
    );
    println!("Trusted access: type revoke-trust and press Enter to review/revoke.\n");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let signal_task = tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Shutdown requested");
                let _ = shutdown_tx.send(true);
            }
            Err(error) => error!(%error, "Could not listen for Ctrl+C"),
        }
    });

    connection::run(
        config,
        device,
        trusted_access,
        local_command_rx,
        shutdown_rx,
    )
    .await;
    signal_task.abort();
    Ok(())
}
