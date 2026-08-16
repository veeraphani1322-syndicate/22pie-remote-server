#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod config;
mod connection;
mod consent;
mod device;
mod keyboard;
mod logging;
mod media;
mod mouse;
mod protocol;
mod startup;
mod trusted_access;

use anyhow::Result;
use config::Config;
use device::DeviceIdentity;
use tokio::sync::watch;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    logging::init()?;
    startup::reconcile(config.start_with_windows)?;
    let device = DeviceIdentity::load()?;
    let trusted_access = trusted_access::TrustedAccess::load(device.device_id)?;

    println!(
        "\n22Pie Remote Agent v{}\n\nDevice Name:\n{}\n\nOperating System:\n{}\n\nArchitecture:\n{}\n\nDevice ID:\n{}\n\nServer:\n{}\n",
        device.agent_version,
        device.device_name,
        device.operating_system,
        device.architecture,
        device.device_id,
        config.server_url,
    );
    info!(app_display_name=%config.app_display_name, window_title=%config.window_title, tray_display_name=%config.tray_display_name, start_with_windows=config.start_with_windows, "Background agent starting");

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

    connection::run(config, device, trusted_access, shutdown_rx).await;
    signal_task.abort();
    Ok(())
}
