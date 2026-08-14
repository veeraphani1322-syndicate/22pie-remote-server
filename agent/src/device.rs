use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::Serialize;
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
    let project_dirs = ProjectDirs::from("com", "22Pie", "RemoteAgent")
        .context("could not determine the local application data directory")?;
    Ok(project_dirs.config_dir().join("device-id"))
}

fn load_or_create_device_id() -> Result<Uuid> {
    let path = identity_path()?;
    if path.exists() {
        let saved = fs::read_to_string(&path)
            .with_context(|| format!("failed to read device identity at {}", path.display()))?;
        return Uuid::parse_str(saved.trim())
            .with_context(|| format!("invalid device identity at {}", path.display()));
    }

    let id = Uuid::new_v4();
    let parent = path
        .parent()
        .context("device identity path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(&path, format!("{id}\n"))
        .with_context(|| format!("failed to save device identity at {}", path.display()))?;
    Ok(id)
}
