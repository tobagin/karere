//! Paste/drop bridge payloads. Everything travels inline as base64 inside the
//! IPC message; https pages cannot read `file://`, so there is no tempfile path.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;

use crate::ipc::PasteBlob;

/// Largest payload sent inline (~64 MB once base64'd). Beyond that the IPC
/// message gets too large, so the paste is refused instead.
const MAX_PASTE_BYTES: usize = 48 * 1024 * 1024;

const SWEEP_AGE: Duration = Duration::from_secs(60 * 60);

/// A paste/drop payload over [`MAX_PASTE_BYTES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasteTooLarge(pub usize);

impl std::fmt::Display for PasteTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "payload of {} bytes exceeds the {} byte paste limit",
            self.0, MAX_PASTE_BYTES
        )
    }
}

impl std::error::Error for PasteTooLarge {}

/// `$XDG_RUNTIME_DIR/karere`, else system temp dir. Older versions wrote
/// paste tempfiles here; only [`sweep_old`] still looks at it.
pub fn paste_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("karere")
}

pub fn b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

/// IPC envelope for a binary paste/drop: inline base64, or an error the
/// caller must surface when the payload is too large to send.
pub fn make_blob(bytes: &[u8]) -> Result<PasteBlob, PasteTooLarge> {
    if bytes.len() > MAX_PASTE_BYTES {
        return Err(PasteTooLarge(bytes.len()));
    }
    Ok(PasteBlob::Base64(B64.encode(bytes)))
}

/// Prefix of the tempfiles older versions left behind.
const TEMPFILE_PREFIX: &str = "paste-";

fn remove_quietly(path: &Path) {
    if let Err(err) = std::fs::remove_file(path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("paste: failed to unlink {}: {err}", path.display());
    }
}

/// Remove orphaned `paste-*` files older than [`SWEEP_AGE`] (leaked by a prior crash).
pub fn sweep_old() {
    let dir = paste_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_paste = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(TEMPFILE_PREFIX));
        if !is_paste {
            continue;
        }
        let aged = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|age| age > SWEEP_AGE);
        if aged {
            remove_quietly(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_payload_inlines_base64() {
        assert_eq!(make_blob(b"hello"), Ok(PasteBlob::Base64("aGVsbG8=".into())));
    }

    #[test]
    fn oversized_payload_is_an_error() {
        let big = vec![0u8; MAX_PASTE_BYTES + 1];
        assert_eq!(make_blob(&big), Err(PasteTooLarge(MAX_PASTE_BYTES + 1)));
    }
}
