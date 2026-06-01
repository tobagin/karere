//! Browser-process side of the M17 paste/drop bridge: tempfile lifecycle and
//! `file://` scoping.
//!
//! Large clipboard/drop payloads (whose base64 would exceed [`B64_INLINE_MAX`])
//! are written to `$XDG_RUNTIME_DIR/karere/paste-<uuid>` (mode `0600`) and
//! handed to the renderer as a [`PasteBlob::FilePath`] which it `fetch()`es over
//! `file://`. To keep that capability from widening the renderer's `file://`
//! reach to arbitrary user files, [`is_allowed_file_url`] scopes every
//! `file://` resource load to the paste directory (enforced by the resource
//! request handler), and tempfiles are unlinked on renderer acknowledgement or
//! a 30 s fallback timer.
//!
//! All functions here run on the glib main thread (the CEF UI thread under the
//! external message pump), so the pending-tempfile map uses a plain mutex.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use parking_lot::Mutex;

use crate::ipc::PasteBlob;

/// Base64 payloads up to this size are inlined into the IPC envelope; larger
/// ones fall back to a tempfile.
///
/// NOTE: the tempfile path serves bytes to the renderer over `file://`, which an
/// `https` page (web.whatsapp.com) cannot fetch — so in practice the fallback is
/// non-functional and this cutoff is set high enough that all realistic
/// clipboard/drop payloads (images, PDFs, documents) inline instead. Truly huge
/// payloads (large videos) still need the custom-scheme path tracked as a
/// follow-up; see `openspec/changes/m17-paste-bridge`.
const B64_INLINE_MAX: usize = 64 * 1024 * 1024;

/// Unlink a leaked tempfile this long after creation if the renderer never
/// acknowledged consuming it.
const TEMPFILE_TTL: Duration = Duration::from_secs(30);

/// Remove orphaned `paste-*` files older than this on startup.
const SWEEP_AGE: Duration = Duration::from_secs(60 * 60);

/// Browser-process registry of tempfiles awaiting a renderer `PasteConsumed`
/// ack, keyed by path with their creation instant.
fn pending() -> &'static Mutex<HashMap<PathBuf, Instant>> {
    static PENDING: OnceLock<Mutex<HashMap<PathBuf, Instant>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `$XDG_RUNTIME_DIR/karere` (falling back to the system temp dir when
/// `XDG_RUNTIME_DIR` is unset, e.g. in a bare test environment).
pub fn paste_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("karere")
}

/// Base64-encode `bytes` (used by the text/middle-click path).
pub fn b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

/// Choose the IPC envelope for `bytes`: inline base64 when small, otherwise a
/// scoped tempfile. Falls back to inline base64 if the tempfile write fails.
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

/// Ensure the paste directory exists with mode `0700`.
fn ensure_dir() -> std::io::Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = paste_dir();
    if !dir.exists() {
        std::fs::DirBuilder::new().mode(0o700).recursive(true).create(&dir)?;
    }
    Ok(dir)
}

/// Write `bytes` to `<paste_dir>/paste-<uuid>` with mode `0600`, register it in
/// the pending map, and schedule a fallback unlink after [`TEMPFILE_TTL`].
fn write_tempfile(bytes: &[u8]) -> std::io::Result<PathBuf> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let dir = ensure_dir()?;
    let name = format!("paste-{}", glib::uuid_string_random());
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
        // Only unlink if still pending (the renderer never acked).
        if pending().lock().remove(&timer_path).is_some() {
            remove_quietly(&timer_path);
            log::debug!("paste: tempfile expired unconsumed, removed");
        }
    });

    Ok(path)
}

/// Renderer acknowledged consuming `path`: drop it from the pending map and
/// unlink it. Idempotent — a missing file is not an error.
pub fn consume(path: &Path) {
    pending().lock().remove(path);
    remove_quietly(path);
}

/// Whether a renderer `file://` URL is permitted: it must resolve to a file
/// directly inside the paste directory. Anything else (e.g. `/etc/passwd`) is
/// rejected.
pub fn is_allowed_file_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("file://") else {
        return false;
    };
    // Drop any query/fragment and a possible empty authority ("file:///path").
    let path_part = rest.split(['?', '#']).next().unwrap_or(rest);
    if path_part.is_empty() {
        return false;
    }
    is_within_paste_dir(Path::new(path_part))
}

/// True when `path` canonicalizes to a file strictly inside the paste dir.
fn is_within_paste_dir(path: &Path) -> bool {
    let Ok(dir) = paste_dir().canonicalize() else {
        return false;
    };
    match path.canonicalize() {
        Ok(resolved) => resolved != dir && resolved.starts_with(&dir),
        Err(_) => false,
    }
}

/// Remove `path`, ignoring a not-found result.
fn remove_quietly(path: &Path) {
    if let Err(err) = std::fs::remove_file(path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("paste: failed to unlink {}: {err}", path.display());
    }
}

/// Remove orphaned `paste-*` files older than [`SWEEP_AGE`]. Called once on
/// browser-process startup to reclaim tempfiles leaked by a prior crash.
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
            .is_some_and(|n| n.starts_with("paste-"));
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
        // Write a real tempfile, then confirm its file:// URL is permitted while
        // it exists and rejected once removed.
        let Ok(path) = write_tempfile(b"hello") else {
            // No XDG_RUNTIME_DIR / unwritable in CI: skip the positive check.
            return;
        };
        let url = format!("file://{}", path.display());
        assert!(is_allowed_file_url(&url));
        consume(&path);
        // Once removed it can no longer be canonicalized, so it is rejected.
        assert!(!is_allowed_file_url(&url));
    }
}
