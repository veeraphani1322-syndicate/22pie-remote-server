use crate::stream_quality::StreamQuality;
use anyhow::{bail, Context, Result};
use directories::BaseDirs;
use serde::Deserialize;
use std::{env, fs, path::PathBuf, time::Duration};
use url::Url;

const DEFAULT_SERVER_URL: &str = "ws://8.234.114.242:21116/agent";
const DEFAULT_HEARTBEAT_SECONDS: u64 = 20;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileConfig {
    remote_server_url: Option<String>,
    app_display_name: Option<String>,
    window_title: Option<String>,
    tray_display_name: Option<String>,
    start_with_windows: Option<bool>,
    stream_quality: Option<StreamQuality>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub heartbeat_interval: Duration,
    pub app_display_name: String,
    pub window_title: String,
    pub tray_display_name: String,
    pub start_with_windows: bool,
    pub stream_quality: StreamQuality,
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

        let file = read_file_config()?;
        Ok(Self {
            stream_quality: env::var("STREAM_QUALITY")
                .ok()
                .map(|v| StreamQuality::parse(&v))
                .transpose()?
                .or(file.stream_quality)
                .unwrap_or_default(),
            server_url,
            heartbeat_interval: Duration::from_secs(heartbeat_seconds),
            app_display_name: value_or_default(
                env::var("APP_DISPLAY_NAME").ok().or(file.app_display_name),
                "Graphics Services",
            ),
            window_title: value_or_default(
                env::var("WINDOW_TITLE").ok().or(file.window_title),
                "Graphics Services",
            ),
            tray_display_name: value_or_default(
                env::var("TRAY_DISPLAY_NAME")
                    .ok()
                    .or(file.tray_display_name),
                "Graphics Services",
            ),
            start_with_windows: env::var("START_WITH_WINDOWS")
                .ok()
                .map(|value| parse_bool(&value))
                .transpose()?
                .or(file.start_with_windows)
                .unwrap_or(false),
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
            app_display_name: None,
            window_title: None,
            tray_display_name: None,
            start_with_windows: None,
            stream_quality: None,
        });
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read agent configuration at {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("invalid agent configuration at {}", path.display()))
}

fn value_or_default(value: Option<String>, default: &str) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("START_WITH_WINDOWS must be true or false"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_names_have_legitimate_defaults_and_allow_overrides() {
        assert_eq!(
            value_or_default(None, "Graphics Services"),
            "Graphics Services"
        );
        assert_eq!(
            value_or_default(Some("Owner Remote".into()), "Graphics Services"),
            "Owner Remote"
        );
        assert_eq!(
            value_or_default(Some("  ".into()), "Graphics Services"),
            "Graphics Services"
        );
    }
    #[test]
    fn startup_boolean_is_explicit() {
        assert_eq!(parse_bool("true").unwrap(), true);
        assert_eq!(parse_bool("OFF").unwrap(), false);
        assert!(parse_bool("sometimes").is_err());
    }
}
