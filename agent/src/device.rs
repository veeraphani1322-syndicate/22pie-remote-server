use crate::config::app_data_dir;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use directories::ProjectDirs;
use ed25519_dalek::{Signer, SigningKey};
use rand_core::OsRng;
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
    #[serde(skip)]
    signing_key: SigningKey,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredIdentity {
    device_id: Uuid,
    private_key: Option<String>,
}

impl DeviceIdentity {
    pub fn load() -> Result<Self> {
        let (device_id, signing_key) = load_or_create_identity()?;
        Ok(Self {
            device_id,
            device_name: System::host_name().unwrap_or_else(|| "Unknown device".to_owned()),
            operating_system: os_description(),
            architecture: std::env::consts::ARCH.to_owned(),
            agent_version: env!("CARGO_PKG_VERSION").to_owned(),
            signing_key,
        })
    }

    pub fn public_key(&self) -> String {
        STANDARD.encode(self.signing_key.verifying_key().as_bytes())
    }

    pub fn sign_challenge(&self, nonce: &str) -> String {
        let payload = format!("{}:{nonce}", self.device_id);
        STANDARD.encode(self.signing_key.sign(payload.as_bytes()).to_bytes())
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

fn load_or_create_identity() -> Result<(Uuid, SigningKey)> {
    let path = identity_path()?;
    if path.exists() {
        let saved = fs::read_to_string(&path)
            .with_context(|| format!("failed to read device identity at {}", path.display()))?;
        let identity: StoredIdentity = serde_json::from_str(&saved)
            .with_context(|| format!("invalid device identity at {}", path.display()))?;
        if let Some(private_key) = identity.private_key {
            return Ok((identity.device_id, decode_signing_key(&private_key)?));
        }
        let key = SigningKey::generate(&mut OsRng);
        save_identity(&path, identity.device_id, &key)?;
        return Ok((identity.device_id, key));
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
        let key = SigningKey::generate(&mut OsRng);
        save_identity(&path, id, &key)?;
        return Ok((id, key));
    }

    let id = Uuid::new_v4();
    let key = SigningKey::generate(&mut OsRng);
    save_identity(&path, id, &key)?;
    Ok((id, key))
}

fn decode_signing_key(value: &str) -> Result<SigningKey> {
    let bytes = STANDARD
        .decode(value)
        .context("invalid device private key encoding")?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid device private key length"))?;
    Ok(SigningKey::from_bytes(&bytes))
}

fn save_identity(path: &PathBuf, id: Uuid, key: &SigningKey) -> Result<()> {
    let parent = path
        .parent()
        .context("device identity path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let identity = serde_json::to_string_pretty(&StoredIdentity {
        device_id: id,
        private_key: Some(STANDARD.encode(key.to_bytes())),
    })
    .context("failed to serialize device identity")?;
    fs::write(&path, format!("{identity}\n"))
        .with_context(|| format!("failed to save device identity at {}", path.display()))?;
    Ok(())
}
