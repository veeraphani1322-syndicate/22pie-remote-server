use anyhow::{bail, Context, Result};
use directories::BaseDirs;
use serde::Deserialize;
use std::{env, fs, path::PathBuf, time::Duration};
use url::Url;

const DEFAULT_SERVER_URL: &str = "ws://8.234.114.242:4000/agent";
const DEFAULT_HEARTBEAT_SECONDS: u64 = 20;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileConfig {
    remote_server_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub heartbeat_interval: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let environment_url = env::var("REMOTE_SERVER_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let server_url = match environment_url {
            Some(url) => url,
            None => read_file_config()?
                .remote_server_url
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_SERVER_URL.to_owned()),
        };
        let parsed = Url::parse(&server_url).context("server URL is not a valid URL")?;
        if !matches!(parsed.scheme(), "ws" | "wss") {
            bail!("server URL must use ws:// or wss://");
        }

        let heartbeat_seconds = env::var("HEARTBEAT_INTERVAL_SECONDS")
            .unwrap_or_else(|_| DEFAULT_HEARTBEAT_SECONDS.to_string())
            .parse::<u64>()
            .context("HEARTBEAT_INTERVAL_SECONDS must be a whole number")?;
        if !(5..=3_600).contains(&heartbeat_seconds) {
            bail!("HEARTBEAT_INTERVAL_SECONDS must be between 5 and 3600");
        }

        Ok(Self {
            server_url,
            heartbeat_interval: Duration::from_secs(heartbeat_seconds),
        })
    }
}

pub fn app_data_dir() -> Result<PathBuf> {
    let base_dirs =
        BaseDirs::new().context("could not determine the local application data directory")?;
    Ok(base_dirs.data_local_dir().join("22Pie").join("RemoteAgent"))
}

fn read_file_config() -> Result<FileConfig> {
    let path = app_data_dir()?.join("config.json");
    if !path.exists() {
        return Ok(FileConfig {
            remote_server_url: None,
        });
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read agent configuration at {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("invalid agent configuration at {}", path.display()))
}
