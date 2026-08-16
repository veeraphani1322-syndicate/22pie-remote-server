use crate::config::app_data_dir;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use uuid::Uuid;

const SCREEN_VIEW: &str = "SCREEN_VIEW";
const MOUSE_CONTROL: &str = "MOUSE_CONTROL";
const KEYBOARD_CONTROL: &str = "KEYBOARD_CONTROL";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalTrust {
    pub server_url: String,
    pub user_id: String,
    pub viewer_name: String,
    pub permission: String,
    pub created_at: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredTrust {
    device_id: Uuid,
    grants: Vec<LocalTrust>,
    pending_revocations: Vec<LocalTrust>,
}

pub struct TrustedAccess {
    path: PathBuf,
    stored: StoredTrust,
}

impl TrustedAccess {
    pub fn load(device_id: Uuid) -> Result<Self> {
        let path = app_data_dir()?.join("trusted-access.json");
        let stored = if path.exists() {
            let parsed: StoredTrust = serde_json::from_str(&fs::read_to_string(&path)?)
                .with_context(|| format!("invalid trusted access data at {}", path.display()))?;
            if parsed.device_id == device_id {
                parsed
            } else {
                StoredTrust {
                    device_id,
                    ..Default::default()
                }
            }
        } else {
            StoredTrust {
                device_id,
                ..Default::default()
            }
        };
        Ok(Self { path, stored })
    }

    pub fn has_screen_view(&self, server_url: &str, user_id: &str) -> bool {
        self.stored.grants.iter().any(|grant| {
            grant.server_url == server_url
                && grant.user_id == user_id
                && grant.permission == SCREEN_VIEW
        })
    }

    pub fn grant_screen_view(
        &mut self,
        server_url: &str,
        user_id: &str,
        viewer_name: &str,
    ) -> Result<()> {
        if !self.has_screen_view(server_url, user_id) {
            self.stored.grants.push(LocalTrust {
                server_url: server_url.to_owned(),
                user_id: user_id.to_owned(),
                viewer_name: viewer_name.to_owned(),
                permission: SCREEN_VIEW.to_owned(),
                created_at: now_string(),
            });
            self.save()?;
        }
        Ok(())
    }

    pub fn has_mouse_control(&self, server_url: &str, user_id: &str) -> bool {
        self.has_permission(server_url, user_id, MOUSE_CONTROL)
    }

    pub fn grant_mouse_control(
        &mut self,
        server_url: &str,
        user_id: &str,
        viewer_name: &str,
    ) -> Result<()> {
        self.grant_permission(server_url, user_id, viewer_name, MOUSE_CONTROL)
    }

    pub fn revoke_mouse_control(&mut self, server_url: &str, user_id: &str) -> Result<()> {
        self.revoke_permission(server_url, user_id, MOUSE_CONTROL)
    }

    pub fn has_keyboard_control(&self, server_url: &str, user_id: &str) -> bool {
        self.has_permission(server_url, user_id, KEYBOARD_CONTROL)
    }

    pub fn grant_keyboard_control(
        &mut self,
        server_url: &str,
        user_id: &str,
        viewer_name: &str,
    ) -> Result<()> {
        self.grant_permission(server_url, user_id, viewer_name, KEYBOARD_CONTROL)
    }

    pub fn revoke_keyboard_control(&mut self, server_url: &str, user_id: &str) -> Result<()> {
        self.revoke_permission(server_url, user_id, KEYBOARD_CONTROL)
    }

    pub fn has_remote_control(&self, server_url: &str, user_id: &str) -> bool {
        self.has_screen_view(server_url, user_id)
            && self.has_mouse_control(server_url, user_id)
            && self.has_keyboard_control(server_url, user_id)
    }

    pub fn grant_remote_control(
        &mut self,
        server_url: &str,
        user_id: &str,
        viewer_name: &str,
    ) -> Result<()> {
        self.grant_screen_view(server_url, user_id, viewer_name)?;
        self.grant_mouse_control(server_url, user_id, viewer_name)?;
        self.grant_keyboard_control(server_url, user_id, viewer_name)
    }

    fn has_permission(&self, server_url: &str, user_id: &str, permission: &str) -> bool {
        self.stored.grants.iter().any(|grant| {
            grant.server_url == server_url
                && grant.user_id == user_id
                && grant.permission == permission
        })
    }

    fn grant_permission(
        &mut self,
        server_url: &str,
        user_id: &str,
        viewer_name: &str,
        permission: &str,
    ) -> Result<()> {
        if !self.has_permission(server_url, user_id, permission) {
            self.stored.grants.push(LocalTrust {
                server_url: server_url.to_owned(),
                user_id: user_id.to_owned(),
                viewer_name: viewer_name.to_owned(),
                permission: permission.to_owned(),
                created_at: now_string(),
            });
            self.save()?;
        }
        Ok(())
    }

    fn revoke_permission(
        &mut self,
        server_url: &str,
        user_id: &str,
        permission: &str,
    ) -> Result<()> {
        self.stored.grants.retain(|grant| {
            !(grant.server_url == server_url
                && grant.user_id == user_id
                && grant.permission == permission)
        });
        self.save()
    }

    pub fn revoke_screen_view(&mut self, server_url: &str, user_id: &str) -> Result<()> {
        self.stored.grants.retain(|grant| {
            !(grant.server_url == server_url
                && grant.user_id == user_id
                && grant.permission == SCREEN_VIEW)
        });
        self.save()
    }

    pub fn grants(&self) -> &[LocalTrust] {
        &self.stored.grants
    }

    pub fn revoke_all_locally(&mut self) -> Result<()> {
        self.stored
            .pending_revocations
            .append(&mut self.stored.grants);
        self.save()
    }

    pub fn pending_revocations_for(&self, server_url: &str) -> Vec<LocalTrust> {
        self.stored
            .pending_revocations
            .iter()
            .filter(|record| record.server_url == server_url)
            .cloned()
            .collect()
    }

    pub fn clear_pending_revocations_for(&mut self, server_url: &str) -> Result<()> {
        self.stored
            .pending_revocations
            .retain(|record| record.server_url != server_url);
        self.save()
    }

    fn save(&self) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("trusted access path has no parent")?;
        fs::create_dir_all(parent)?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(
            &temporary,
            format!("{}\n", serde_json::to_string_pretty(&self.stored)?),
        )?;
        fs::rename(temporary, &self.path)?;
        Ok(())
    }
}

fn now_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_is_scoped_to_server_user_and_permission() {
        let mut access = TrustedAccess {
            path: PathBuf::from("unused"),
            stored: StoredTrust {
                device_id: Uuid::nil(),
                ..Default::default()
            },
        };
        access.stored.grants.push(LocalTrust {
            server_url: "wss://one".into(),
            user_id: "a".into(),
            viewer_name: "A".into(),
            permission: SCREEN_VIEW.into(),
            created_at: "0".into(),
        });
        assert!(access.has_screen_view("wss://one", "a"));
        assert!(!access.has_screen_view("wss://one", "b"));
        assert!(!access.has_screen_view("wss://two", "a"));
        access.stored.grants[0].permission = "MOUSE_CONTROL".into();
        assert!(!access.has_screen_view("wss://one", "a"));
        assert!(!access.has_remote_control("wss://one", "a"));
        for permission in [SCREEN_VIEW, KEYBOARD_CONTROL] {
            access.stored.grants.push(LocalTrust {
                server_url: "wss://one".into(),
                user_id: "a".into(),
                viewer_name: "A".into(),
                permission: permission.into(),
                created_at: "0".into(),
            });
        }
        assert!(access.has_remote_control("wss://one", "a"));
        assert!(!access.has_remote_control("wss://one", "b"));
    }
}
