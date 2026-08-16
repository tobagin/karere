//! Paste/drop bridge: tempfile lifecycle; `file://` URLs scoped to the paste dir only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use parking_lot::Mutex;

use crate::ipc::PasteBlob;

/// High so all realistic payloads inline (https pages can't fetch the file:// fallback).
const B64_INLINE_MAX: usize = 64 * 1024 * 1024;

const TEMPFILE_TTL: Duration = Duration::from_secs(30);

const SWEEP_AGE: Duration = Duration::from_secs(60 * 60);

fn pending() -> &'static Mutex<HashMap<PathBuf, Instant>> {
    static PENDING: OnceLock<Mutex<HashMap<PathBuf, Instant>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `$XDG_RUNTIME_DIR/karere`, else system temp dir.
pub fn paste_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("karere")
}

pub fn b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

/// IPC envelope: inline base64 when small, else a scoped tempfile.
pub fn make_blob(bytes: &[u8]) -> PasteBlob {
    let encoded = B64.encode(bytes);
    if encoded.len() <= B64_INLINE_MAX {
        return PasteBlob::Base64(encoded);
    }
    match write_tempfile(bytes) {
        Ok(path) => PasteBlob::FilePath(path),
        Err(err) => {
            log::warn!("paste: tempfile write failed ({err}); inlining base64");
            PasteBlob::Base64(encoded)
        }
    }
}

fn ensure_dir() -> std::io::Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = paste_dir();
    if !dir.exists() {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(&dir)?;
    }
    Ok(dir)
}

fn write_tempfile(bytes: &[u8]) -> std::io::Result<PathBuf> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let dir = ensure_dir()?;
    let name = format!("{TEMPFILE_PREFIX}{}", glib::uuid_string_random());
    let path = dir.join(name);

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(bytes)?;
    file.flush()?;

    pending().lock().insert(path.clone(), Instant::now());

    let timer_path = path.clone();
    glib::timeout_add_local_once(TEMPFILE_TTL, move || {
        if pending().lock().remove(&timer_path).is_some() {
            remove_quietly(&timer_path);
            log::debug!("paste: tempfile expired unconsumed, removed");
        }
    });

    Ok(path)
}

/// Renderer acked `path`: drop from pending and unlink. Idempotent.
pub fn consume(path: &Path) {
    pending().lock().remove(path);
    remove_quietly(path);
}

/// Prefix every paste tempfile name carries; only such files are exfiltratable.
const TEMPFILE_PREFIX: &str = "paste-";

/// Renderer `file://` URL allowed only if it resolves to one of our paste
/// tempfiles inside the paste dir.
pub fn is_allowed_file_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("file://") else {
        return false;
    };
    let path_part = rest.split(['?', '#']).next().unwrap_or(rest);
    if path_part.is_empty() {
        return false;
    }
    // Defense-in-depth: reject any still-encoded path. CEF/Chromium normalises
    // and percent-decodes file:// URLs before they reach us, so a legitimate
    // tempfile path never contains `%`; a leftover `%2e%2e` traversal therefore
    // never reaches canonicalize.
    if path_part.contains('%') {
        return false;
    }
    let path = Path::new(path_part);
    // Only our own tempfiles (prefixed) may ever be read, so a file an attacker
    // managed to drop into the paste dir under another name stays unreadable.
    let has_prefix = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(TEMPFILE_PREFIX));
    if !has_prefix {
        return false;
    }
    is_within_paste_dir(path)
}

pub(crate) fn is_within_paste_dir(path: &Path) -> bool {
    let Ok(dir) = paste_dir().canonicalize() else {
        return false;
    };
    match path.canonicalize() {
        Ok(resolved) => resolved != dir && resolved.starts_with(&dir),
        Err(_) => false,
    }
}

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
        assert!(matches!(make_blob(b"hello"), PasteBlob::Base64(_)));
    }

    #[test]
    fn rejects_file_urls_outside_paste_dir() {
        assert!(!is_allowed_file_url("file:///etc/passwd"));
        assert!(!is_allowed_file_url("https://example.com/x"));
        assert!(!is_allowed_file_url("file://"));
    }

    #[test]
    fn allows_file_url_inside_paste_dir() {
        // write_tempfile names the file with the `paste-` prefix the guard now
        // requires, so this exercises a real app tempfile.
        let Ok(path) = write_tempfile(b"hello") else {
            return;
        };
        assert!(
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(TEMPFILE_PREFIX))
        );
        let url = format!("file://{}", path.display());
        assert!(is_allowed_file_url(&url));
        consume(&path);
        assert!(!is_allowed_file_url(&url));
    }

    #[test]
    fn prefixed_file_inside_dir_is_allowed() {
        let Ok(dir) = ensure_dir() else {
            return;
        };
        let path = dir.join(format!("{TEMPFILE_PREFIX}allowed-fixture"));
        if std::fs::write(&path, b"x").is_err() {
            return;
        }
        let url = format!("file://{}", path.display());
        assert!(is_allowed_file_url(&url));
        remove_quietly(&path);
    }

    #[test]
    fn non_prefixed_file_inside_dir_is_rejected() {
        // A file legitimately inside the paste dir but NOT created by us must
        // not be exfiltratable.
        let Ok(dir) = ensure_dir() else {
            return;
        };
        let path = dir.join("not-ours-secret");
        if std::fs::write(&path, b"secret").is_err() {
            return;
        }
        let url = format!("file://{}", path.display());
        assert!(!is_allowed_file_url(&url));
        remove_quietly(&path);
    }

    #[test]
    fn parent_traversal_is_rejected() {
        let Ok(dir) = ensure_dir() else {
            return;
        };
        // ../<dir-name>/../passwd-style escape; canonicalize resolves the `..`
        // and the resulting path is not a tempfile so it is denied.
        let url = format!(
            "file://{}/{TEMPFILE_PREFIX}x/../../etc/passwd",
            dir.display()
        );
        assert!(!is_allowed_file_url(&url));
        // Also a non-existent traversal path: canonicalize fails -> deny.
        let url2 = format!("file://{}/../{TEMPFILE_PREFIX}escape", dir.display());
        assert!(!is_allowed_file_url(&url2));
    }

    #[test]
    fn sibling_dir_prefix_is_rejected() {
        // `karere-evil` must never match the `karere` paste dir via a raw
        // string prefix. Build a sibling of the real paste dir.
        let dir = paste_dir();
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            return;
        };
        let Some(parent) = dir.parent() else {
            return;
        };
        let evil = parent.join(format!("{name}-evil"));
        if std::fs::create_dir_all(&evil).is_err() {
            return;
        }
        let path = evil.join(format!("{TEMPFILE_PREFIX}x"));
        if std::fs::write(&path, b"x").is_err() {
            let _ = std::fs::remove_dir_all(&evil);
            return;
        }
        let url = format!("file://{}", path.display());
        assert!(!is_allowed_file_url(&url));
        let _ = std::fs::remove_dir_all(&evil);
    }

    #[test]
    fn percent_encoded_path_is_rejected() {
        // Belt-and-braces: an undecoded percent escape never reaches canonicalize.
        let dir = paste_dir();
        let url = format!("file://{}/%2e%2e/{TEMPFILE_PREFIX}x", dir.display());
        assert!(!is_allowed_file_url(&url));
    }
}
