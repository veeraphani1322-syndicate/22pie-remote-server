use anyhow::{bail, Context, Result};
use std::{env, time::Duration};
use url::Url;

const DEFAULT_SERVER_URL: &str = "ws://localhost:4000/agent";
const DEFAULT_HEARTBEAT_SECONDS: u64 = 20;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub heartbeat_interval: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let server_url =
            env::var("REMOTE_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_owned());
        let parsed = Url::parse(&server_url).context("REMOTE_SERVER_URL is not a valid URL")?;
        if !matches!(parsed.scheme(), "ws" | "wss") {
            bail!("REMOTE_SERVER_URL must use ws:// or wss://");
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
