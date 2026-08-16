use crate::config::app_data_dir;
use anyhow::{Context, Result};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tracing_subscriber::{fmt::MakeWriter, EnvFilter};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone)]
struct RotatingLog {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl Write for RotatingLog {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| std::io::Error::other("log lock poisoned"))?;
        if fs::metadata(&self.path).map(|meta| meta.len()).unwrap_or(0) >= MAX_LOG_BYTES {
            let previous = self.path.with_extension("log.previous");
            let _ = fs::remove_file(&previous);
            let _ = fs::rename(&self.path, &previous);
        }
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?
            .write(buffer)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for RotatingLog {
    type Writer = RotatingLog;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

pub fn init() -> Result<()> {
    let directory = app_data_dir()?.join("logs");
    fs::create_dir_all(&directory)
        .with_context(|| format!("failed to create log directory at {}", directory.display()))?;
    let writer = RotatingLog {
        path: directory.join("GraphicService.log"),
        lock: Arc::new(Mutex::new(())),
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false)
        .compact()
        .with_writer(writer)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize file logging: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn log_limit_is_reasonable() {
        assert_eq!(MAX_LOG_BYTES, 5 * 1024 * 1024);
    }
}
