mod config;
mod connection;
mod device;
mod logging;
mod protocol;

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

    println!(
        "\n22Pie Remote Agent v{}\n\nDevice:\n{}\n\nDevice ID:\n{}\n\nOperating System:\n{}\n\nArchitecture:\n{}\n\nConnecting to:\n{}\n",
        device.agent_version,
        device.device_name,
        device.device_id,
        device.operating_system,
        device.architecture,
        config.server_url,
    );

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

    connection::run(config, device, shutdown_rx).await;
    signal_task.abort();
    Ok(())
}
