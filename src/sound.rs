//! Custom notification sound playback.
//!
//! Sounds ship inside the gresource bundle as `.oga`. External audio CLIs can't
//! read a `resource://` path, so the first time a sound is requested in a
//! session we extract it to `$XDG_RUNTIME_DIR/karere/sounds/<name>.oga` and
//! cache that path. Playback prefers `paplay` (PulseAudio/PipeWire) and falls
//! back to `gst-launch-1.0 playbin`; if neither backend exists we warn once and
//! stay silent rather than surfacing an error (M14 5.2–5.4).

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;

use gtk::gio;
use parking_lot::Mutex;

use crate::application::RESOURCE_BASE_PATH;

/// Per-session cache of extracted sound paths, keyed by sound name.
static EXTRACTED: Mutex<Option<HashMap<String, PathBuf>>> = Mutex::new(None);

/// Set once we have logged that no audio backend is available, so the warning
/// fires at most once per session (5.4).
static BACKEND_WARNED: Mutex<bool> = Mutex::new(false);

/// Play the bundled sound named `name` (without extension), e.g. `"whatsapp"`.
/// Best-effort: any failure is logged, never propagated.
pub fn play_sound(name: &str) {
    let path = match ensure_extracted(name) {
        Some(p) => p,
        None => return,
    };
    spawn_player(&path);
}

/// Resolve `name` to an extracted on-disk path, extracting from the gresource on
/// first use and caching the result for the session (5.2).
fn ensure_extracted(name: &str) -> Option<PathBuf> {
    let mut guard = EXTRACTED.lock();
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(p) = cache.get(name) {
        if p.exists() {
            return Some(p.clone());
        }
        // Extracted file vanished (runtime dir cleared) — re-extract below.
    }

    let resource_path = format!("{RESOURCE_BASE_PATH}/sounds/{name}.oga");
    let bytes = match gio::resources_lookup_data(
        &resource_path,
        gio::ResourceLookupFlags::NONE,
    ) {
        Ok(b) => b,
        Err(err) => {
            log::warn!("sound: no gresource entry {resource_path}: {err}");
            return None;
        }
    };

    let dir = runtime_sounds_dir();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        log::warn!("sound: cannot create {}: {err}", dir.display());
        return None;
    }
    let out = dir.join(format!("{name}.oga"));
    if let Err(err) = std::fs::write(&out, &bytes) {
        log::warn!("sound: cannot write {}: {err}", out.display());
        return None;
    }
    cache.insert(name.to_owned(), out.clone());
    Some(out)
}

/// `$XDG_RUNTIME_DIR/karere/sounds`, falling back to the system temp dir when
/// the runtime dir is unset.
fn runtime_sounds_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("karere").join("sounds")
}

/// Spawn `paplay <path>` detached; on a spawn failure (e.g. `paplay` missing)
/// fall back to GStreamer, then warn once if both are unavailable (5.3, 5.4).
fn spawn_player(path: &PathBuf) {
    if try_spawn(&[OsStr::new("paplay"), path.as_os_str()]) {
        return;
    }
    let uri = format!("file://{}", path.display());
    if try_spawn(&[
        OsStr::new("gst-launch-1.0"),
        OsStr::new("playbin"),
        OsStr::new(&format!("uri={uri}")),
    ]) {
        return;
    }
    let mut warned = BACKEND_WARNED.lock();
    if !*warned {
        *warned = true;
        log::warn!(
            "sound: neither paplay nor gst-launch-1.0 available; notification \
             sounds disabled for this session"
        );
    }
}

/// Spawn `argv` detached via `gio::Subprocess`, inheriting std streams. Returns
/// true on a successful spawn (the process is fire-and-forget; we do not wait).
fn try_spawn(argv: &[&OsStr]) -> bool {
    match gio::Subprocess::newv(argv, gio::SubprocessFlags::NONE) {
        Ok(_proc) => true,
        Err(err) => {
            log::debug!("sound: spawn {:?} failed: {err}", argv.first());
            false
        }
    }
}
