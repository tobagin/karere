use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cef::{
    self, BeforeDownloadCallback, Browser, CefString, DownloadHandler, DownloadInterruptReason,
    DownloadItem, DownloadItemCallback, ImplBeforeDownloadCallback, ImplDownloadHandler,
    ImplDownloadItem, WrapDownloadHandler, rc::Rc, wrap_download_handler,
};
use gio::prelude::SettingsExt;
use parking_lot::Mutex;

use crate::application::APP_ID;

use super::{DownloadCompleted, DownloadFailed, SharedRef};

/// Cap on the ` (N)` collision walk before falling back to a unique suffix.
const MAX_SUFFIX: u32 = 9999;

#[derive(Clone)]
pub struct ShellDownloadHandler {
    shared: SharedRef,
    /// Download ids already reported complete/failed, so the toast/dialog fires
    /// once even though `on_download_updated` keeps firing after terminal state.
    seen: Arc<Mutex<HashSet<u32>>>,
}

impl ShellDownloadHandler {
    pub fn new(shared: SharedRef) -> Self {
        Self {
            shared,
            seen: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

wrap_download_handler! {
    pub struct ShellDownloadHandlerBuilder {
        handler: ShellDownloadHandler,
    }

    impl DownloadHandler {
        fn can_download(
            &self,
            _browser: Option<&mut Browser>,
            _url: Option<&CefString>,
            _request_method: Option<&CefString>,
        ) -> ::std::os::raw::c_int {
            // Default impl returns 0 (cancel). Allow all; on_before_download
            // picks the target path.
            1
        }

        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> ::std::os::raw::c_int {
            let Some(callback) = callback else { return 0 };
            let suggested = suggested_name
                .map(CefString::to_string)
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    download_item
                        .as_ref()
                        .map(|item| CefString::from(&item.suggested_file_name()).to_string())
                })
                .unwrap_or_default();
            let target = resolve_target_path(&suggested);
            // show_dialog = 0: silent, no native chooser.
            callback.cont(Some(&CefString::from(target.to_string_lossy().as_ref())), 0);
            0
        }

        fn on_download_updated(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            _callback: Option<&mut DownloadItemCallback>,
        ) {
            let Some(item) = download_item else { return };
            let complete = item.is_complete() != 0;
            let canceled = item.is_canceled() != 0;
            let interrupted = item.is_interrupted() != 0;
            // Still streaming: nothing to surface yet.
            if !complete && !canceled && !interrupted {
                return;
            }
            // Fire once per download id.
            let id = item.id();
            {
                let mut seen = self.handler.seen.lock();
                if !seen.insert(id) {
                    return;
                }
            }

            if complete {
                // Honour the master download-notification toggle; the file is on
                // disk regardless.
                let settings = gio::Settings::new(APP_ID);
                if !settings.boolean("notify-downloads-enabled") {
                    return;
                }
                let full = CefString::from(&item.full_path()).to_string();
                let path = PathBuf::from(full);
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.handler
                    .shared
                    .lock()
                    .downloads_completed
                    .push(DownloadCompleted { path, name });
            } else {
                let reason = if canceled {
                    "canceled".to_string()
                } else {
                    interrupt_reason_text(item.interrupt_reason())
                };
                let name = CefString::from(&item.suggested_file_name()).to_string();
                self.handler
                    .shared
                    .lock()
                    .downloads_failed
                    .push(DownloadFailed { name, reason });
            }
        }
    }
}

impl ShellDownloadHandlerBuilder {
    pub fn build(handler: ShellDownloadHandler) -> DownloadHandler {
        Self::new(handler)
    }
}

/// Resolve the absolute, collision-free path for a download given its
/// renderer-suggested filename.
fn resolve_target_path(suggested: &str) -> PathBuf {
    let dir = resolve_dir();
    let mut name = sanitize(suggested);
    if name.is_empty() {
        name = "download".to_string();
    }
    dedupe(&dir, &name)
}

/// Pick the download directory: `download-directory` GSetting when set and
/// usable, else XDG `DOWNLOAD` (falling back to `$HOME`).
fn resolve_dir() -> PathBuf {
    let settings = gio::Settings::new(APP_ID);
    let configured = settings.string("download-directory").to_string();
    if !configured.is_empty() {
        let dir = PathBuf::from(&configured);
        if ensure_writable_dir(&dir) {
            return dir;
        }
        log::warn!("download-directory {dir:?} is missing or not writable; using XDG Downloads");
    }
    let xdg = glib::user_special_dir(glib::UserDirectory::Downloads)
        .unwrap_or_else(glib::home_dir);
    ensure_writable_dir(&xdg);
    xdg
}

/// Create `dir` if needed and confirm writable via a sentinel file. False when
/// the directory cannot be used.
fn ensure_writable_dir(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".karere-write-test");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Strip path separators and NUL from a renderer-supplied filename. Unicode is
/// preserved.
fn sanitize(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '/' | '\\' | '\0'))
        .collect()
}

/// Walk `name.ext` → `name (1).ext` → `name (2).ext` … until a free path is
/// found, capping at `MAX_SUFFIX` before appending a unique numeric suffix.
fn dedupe(dir: &Path, name: &str) -> PathBuf {
    let initial = path_with_suffix(dir, name, None);
    if !initial.exists() {
        return initial;
    }
    let (stem, ext) = split_name(name);
    for n in 1..=MAX_SUFFIX {
        let candidate = path_with_suffix(dir, &format!("{stem} ({n})"), ext.as_deref());
        if !candidate.exists() {
            return candidate;
        }
    }
    // Pathological collision count: append a unique suffix.
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path_with_suffix(dir, &format!("{stem} ({unique})"), ext.as_deref())
}

/// Join `dir` with `stem` plus an optional extension.
fn path_with_suffix(dir: &Path, stem: &str, ext: Option<&str>) -> PathBuf {
    match ext {
        Some(ext) => dir.join(format!("{stem}.{ext}")),
        None => dir.join(stem),
    }
}

/// Split a filename into stem + optional (last) extension. Extension-less and
/// dotfile names yield `None`.
fn split_name(name: &str) -> (String, Option<String>) {
    let path = Path::new(name);
    match (path.file_stem(), path.extension()) {
        (Some(stem), Some(ext)) => (
            stem.to_string_lossy().into_owned(),
            Some(ext.to_string_lossy().into_owned()),
        ),
        (Some(stem), None) => (stem.to_string_lossy().into_owned(), None),
        _ => (name.to_string(), None),
    }
}

/// Map a CEF download interrupt reason to a short human-readable string.
fn interrupt_reason_text(reason: DownloadInterruptReason) -> String {
    use DownloadInterruptReason as R;
    let raw = reason.get_raw();
    let label = if raw == R::FILE_FAILED.get_raw() {
        "file write failed"
    } else if raw == R::FILE_ACCESS_DENIED.get_raw() {
        "access denied"
    } else if raw == R::FILE_NO_SPACE.get_raw() {
        "no space left on device"
    } else if raw == R::FILE_NAME_TOO_LONG.get_raw() {
        "file name too long"
    } else if raw == R::FILE_TOO_LARGE.get_raw() {
        "file too large"
    } else if raw == R::NETWORK_FAILED.get_raw() {
        "network failed"
    } else if raw == R::NETWORK_TIMEOUT.get_raw() {
        "network timed out"
    } else if raw == R::NETWORK_DISCONNECTED.get_raw() {
        "network disconnected"
    } else if raw == R::NETWORK_SERVER_DOWN.get_raw() {
        "server is down"
    } else if raw == R::SERVER_FAILED.get_raw() {
        "server error"
    } else if raw == R::SERVER_FORBIDDEN.get_raw() {
        "forbidden by server"
    } else if raw == R::USER_CANCELED.get_raw() {
        "canceled"
    } else if raw == R::CRASH.get_raw() {
        "browser crashed"
    } else {
        return format!("error {raw}");
    };
    label.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_separators() {
        assert_eq!(sanitize("../etc/passwd"), "..etcpasswd");
        assert_eq!(sanitize("a\\b\0c"), "abc");
    }

    #[test]
    fn sanitize_preserves_unicode() {
        assert_eq!(sanitize("相片.jpg"), "相片.jpg");
    }

    #[test]
    fn split_name_handles_extension_and_none() {
        assert_eq!(
            split_name("photo.jpg"),
            ("photo".to_string(), Some("jpg".to_string()))
        );
        assert_eq!(split_name("notes"), ("notes".to_string(), None));
    }

    #[test]
    fn dedupe_walks_suffixes() {
        let dir = std::env::temp_dir().join(format!(
            "karere-dl-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // No collision.
        assert_eq!(dedupe(&dir, "photo.jpg"), dir.join("photo.jpg"));

        // One collision.
        std::fs::File::create(dir.join("photo.jpg")).unwrap();
        assert_eq!(dedupe(&dir, "photo.jpg"), dir.join("photo (1).jpg"));

        // Multiple collisions.
        std::fs::File::create(dir.join("photo (1).jpg")).unwrap();
        std::fs::File::create(dir.join("photo (2).jpg")).unwrap();
        assert_eq!(dedupe(&dir, "photo.jpg"), dir.join("photo (3).jpg"));

        // Extension-less collision.
        std::fs::File::create(dir.join("notes")).unwrap();
        assert_eq!(dedupe(&dir, "notes"), dir.join("notes (1)"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
