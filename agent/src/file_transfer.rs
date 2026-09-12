use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const CHUNK_BYTES: usize = 16 * 1024;
pub const MAX_MESSAGE_BYTES: usize = 24 * 1024;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    FileOffer {
        session_id: String,
        transfer_id: Uuid,
        name: String,
        size: u64,
        sha256: String,
    },
    FileChunk {
        session_id: String,
        transfer_id: Uuid,
        offset: u64,
        data: String,
    },
    FileFinish {
        session_id: String,
        transfer_id: Uuid,
    },
    FileCancel {
        session_id: String,
        transfer_id: Uuid,
    },
}
impl Message {
    pub fn parse(data: &[u8], session: &str) -> Result<Self> {
        if data.len() > MAX_MESSAGE_BYTES {
            bail!("File message is too large");
        }
        let message: Self = serde_json::from_slice(data)?;
        let id = match &message {
            Self::FileOffer { session_id, .. }
            | Self::FileChunk { session_id, .. }
            | Self::FileFinish { session_id, .. }
            | Self::FileCancel { session_id, .. } => session_id,
        };
        if id != session {
            bail!("File message belongs to another session");
        }
        Ok(message)
    }
    pub fn id(&self) -> Uuid {
        match self {
            Self::FileOffer { transfer_id, .. }
            | Self::FileChunk { transfer_id, .. }
            | Self::FileFinish { transfer_id, .. }
            | Self::FileCancel { transfer_id, .. } => *transfer_id,
        }
    }
}

pub fn validate_offer(name: &str, size: u64, hash: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 120
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || name == "."
        || name == ".."
    {
        bail!("Unsupported file name");
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|n| {
                matches!(
                    n,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        })
    {
        bail!("Reserved Windows file name");
    }
    if size > MAX_FILE_BYTES {
        bail!("Files are limited to 32 MiB");
    }
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        bail!("Invalid SHA-256 digest");
    }
    Ok(())
}

pub struct Receiver {
    pub id: Uuid,
    pub received: u64,
    size: u64,
    expected_hash: String,
    digest: Sha256,
    directory: PathBuf,
    partial: PathBuf,
    destination: PathBuf,
    file: Option<File>,
    completed: bool,
}
impl Receiver {
    // Call only after local approval. Remote input never selects a directory.
    pub fn start(root: &Path, id: Uuid, name: &str, size: u64, hash: &str) -> Result<Self> {
        validate_offer(name, size, hash)?;
        fs::create_dir_all(root)?;
        if fs::symlink_metadata(root)?.file_type().is_symlink() {
            bail!("Transfer folder cannot be a symlink");
        }
        let directory = root.join(id.to_string());
        fs::create_dir(&directory)
            .context("Transfer directory already exists or cannot be created")?;
        let partial = directory.join(".22pie-partial");
        let file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
        {
            Ok(file) => file,
            Err(error) => {
                let _ = fs::remove_dir(&directory);
                return Err(error.into());
            }
        };
        Ok(Self {
            id,
            received: 0,
            size,
            expected_hash: hash.into(),
            digest: Sha256::new(),
            destination: directory.join(name),
            directory,
            partial,
            file: Some(file),
            completed: false,
        })
    }
    pub fn chunk(&mut self, id: Uuid, offset: u64, encoded: &str) -> Result<u64> {
        if id != self.id || offset != self.received {
            bail!("Unexpected transfer or offset");
        }
        if encoded.len() > CHUNK_BYTES.div_ceil(3) * 4 {
            bail!("Chunk exceeds limit");
        }
        let bytes = STANDARD.decode(encoded)?;
        if bytes.is_empty()
            || bytes.len() > CHUNK_BYTES
            || self.received + bytes.len() as u64 > self.size
        {
            bail!("Chunk exceeds declared file size");
        }
        self.file
            .as_mut()
            .context("Transfer already closed")?
            .write_all(&bytes)?;
        self.digest.update(&bytes);
        self.received += bytes.len() as u64;
        Ok(self.received)
    }
    pub fn finish(&mut self, id: Uuid) -> Result<PathBuf> {
        if id != self.id || self.received != self.size {
            bail!("Incomplete file");
        }
        let digest = format!("{:x}", self.digest.clone().finalize());
        if digest != self.expected_hash {
            bail!("File integrity check failed");
        }
        self.file
            .take()
            .context("Transfer already closed")?
            .sync_all()?;
        // Preserve Windows' downloaded-file security prompts. Never launch a received file.
        #[cfg(windows)]
        fs::write(
            format!("{}:Zone.Identifier", self.partial.display()),
            b"[ZoneTransfer]\r\nZoneId=3\r\n",
        )?;
        // Hard-link creation fails if a destination exists, unlike rename on some platforms.
        fs::hard_link(&self.partial, &self.destination)?;
        if let Err(error) = fs::remove_file(&self.partial) {
            let _ = fs::remove_file(&self.destination);
            return Err(error.into());
        }
        self.completed = true;
        Ok(self.destination.clone())
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.file.take();
        if !self.completed {
            let _ = fs::remove_file(&self.partial);
            let _ = fs::remove_dir(&self.directory);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        std::env::temp_dir().join(format!("22pie-transfer-test-{}", Uuid::new_v4()))
    }
    fn hash(data: &[u8]) -> String {
        format!("{:x}", Sha256::digest(data))
    }
    #[test]
    fn rejects_paths_streams_devices_and_large_files() {
        for name in [
            "../x",
            "a/b",
            "a\\b",
            "x:stream",
            "CON.txt",
            "COM1",
            "LPT².txt",
            "a.",
            "a\n.txt",
            "",
        ] {
            assert!(validate_offer(name, 0, &hash(b"")).is_err(), "{name}");
        }
        assert!(validate_offer("ok.txt", MAX_FILE_BYTES + 1, &hash(b"")).is_err());
    }
    #[test]
    fn writes_only_complete_verified_files_and_never_overwrites() {
        let root = root();
        let id = Uuid::new_v4();
        let mut receiver = Receiver::start(&root, id, "hello.txt", 5, &hash(b"hello")).unwrap();
        assert!(!root.join(id.to_string()).join("hello.txt").exists());
        assert!(receiver.chunk(id, 1, &STANDARD.encode(b"hello")).is_err());
        receiver.chunk(id, 0, &STANDARD.encode(b"hello")).unwrap();
        let path = receiver.finish(id).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"hello");
        assert!(Receiver::start(&root, id, "hello.txt", 5, &hash(b"hello")).is_err());
        drop(receiver);
        assert!(path.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancellation_and_corruption_remove_partial_files() {
        let root = root();
        let id = Uuid::new_v4();
        {
            let mut receiver = Receiver::start(&root, id, "test.txt", 3, &hash(b"abc")).unwrap();
            receiver.chunk(id, 0, &STANDARD.encode(b"bad")).unwrap();
            assert!(receiver.finish(id).is_err());
        }
        assert!(!root.join(id.to_string()).exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn binds_messages_to_session_and_rejects_extra_fields() {
        let id = Uuid::new_v4();
        let data = format!(r#"{{"type":"file_cancel","session_id":"s","transfer_id":"{id}"}}"#);
        assert!(Message::parse(data.as_bytes(), "s").is_ok());
        assert!(Message::parse(data.as_bytes(), "different").is_err());
        let extra = data.replace("\"type\":", "\"path\":\"/tmp/x\",\"type\":");
        assert!(Message::parse(extra.as_bytes(), "s").is_err());
    }
}
