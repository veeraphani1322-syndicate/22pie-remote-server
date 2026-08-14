use crate::config::app_data_dir;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use sysinfo::System;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdentity {
    pub device_id: Uuid,
    pub device_name: String,
    pub operating_system: String,
    #[serde(skip)]
    pub architecture: String,
    pub agent_version: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredIdentity {
    device_id: Uuid,
}

impl DeviceIdentity {
    pub fn load() -> Result<Self> {
        Ok(Self {
            device_id: load_or_create_device_id()?,
            device_name: System::host_name().unwrap_or_else(|| "Unknown device".to_owned()),
            operating_system: os_description(),
            architecture: std::env::consts::ARCH.to_owned(),
            agent_version: env!("CARGO_PKG_VERSION").to_owned(),
        })
    }
}

fn os_description() -> String {
    let name = System::name().unwrap_or_else(|| std::env::consts::OS.to_owned());
    match System::os_version() {
        Some(version) if !version.trim().is_empty() => format!("{name} {version}"),
        _ => name,
    }
}

fn identity_path() -> Result<PathBuf> {
    Ok(app_data_dir()?.join("device.json"))
}

fn legacy_identity_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "22Pie", "RemoteAgent")
        .map(|project_dirs| project_dirs.config_dir().join("device-id"))
}

fn load_or_create_device_id() -> Result<Uuid> {
    let path = identity_path()?;
    if path.exists() {
        let saved = fs::read_to_string(&path)
            .with_context(|| format!("failed to read device identity at {}", path.display()))?;
        let identity: StoredIdentity = serde_json::from_str(&saved)
            .with_context(|| format!("invalid device identity at {}", path.display()))?;
        return Ok(identity.device_id);
    }

    if let Some(legacy_path) = legacy_identity_path().filter(|legacy_path| legacy_path.exists()) {
        let saved = fs::read_to_string(&legacy_path).with_context(|| {
            format!(
                "failed to read legacy device identity at {}",
                legacy_path.display()
            )
        })?;
        let id = Uuid::parse_str(saved.trim()).with_context(|| {
            format!(
                "invalid legacy device identity at {}",
                legacy_path.display()
            )
        })?;
        save_device_id(&path, id)?;
        return Ok(id);
    }

    let id = Uuid::new_v4();
    save_device_id(&path, id)?;
    Ok(id)
}

fn save_device_id(path: &PathBuf, id: Uuid) -> Result<()> {
    let parent = path
        .parent()
        .context("device identity path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let identity = serde_json::to_string_pretty(&StoredIdentity { device_id: id })
        .context("failed to serialize device identity")?;
    fs::write(&path, format!("{identity}\n"))
        .with_context(|| format!("failed to save device identity at {}", path.display()))?;
    Ok(())
}
