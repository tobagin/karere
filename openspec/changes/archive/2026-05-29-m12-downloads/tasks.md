## 1. Download handler

- [x] 1.1 Create `src/handlers/download.rs` with `wrap_download_handler!` `ShellDownloadHandlerBuilder`
- [x] 1.2 Implement `on_before_download(_browser, _item, suggested_name, callback)`: read `download-directory` GSetting; fall back to `glib::user_special_dir(UserDirectory::Downloads)`
- [x] 1.3 Sanitize `suggested_name` (strip path separators, NUL bytes)
- [x] 1.4 Implement dupe suffix: `name.ext` → `name (1).ext` → `name (2).ext`… until free; helper `path_with_suffix(dir, stem, ext) -> PathBuf`
- [x] 1.5 `callback.cont(target_path, show_dialog=false)`
- [x] 1.6 Implement `on_download_updated`: on `is_complete` push `DownloadCompleted { path, name }` into `SharedState`; on `is_canceled` or non-zero error push `DownloadFailed { name, reason }`

## 2. GSettings

- [x] 2.1 Add `download-directory` (string, default `""` → falls back to XDG Downloads) in gschema
- [x] 2.2 Add `notify-downloads-enabled` (bool, default true)
- [x] 2.3 Add `notify-download-type` enum (`toast`, `notification`, `both`; default `toast`)

## 3. Window-side UX

- [x] 3.1 Extend `KarereWindow` 100 ms polling: drain `DownloadCompleted` → AdwToast `"<name> downloaded"` with two action buttons
- [x] 3.2 Toast "Open" button invokes action `app.open-download <path>`
- [x] 3.3 Toast "Show in Folder" button invokes `OpenURI.open_file(parent_dir)` via portal
- [x] 3.4 Drain `DownloadFailed` → AdwAlertDialog with "Download failed: <reason>"

## 4. Open-download action

- [x] 4.1 Wire `app.open-download <path>` (M8 stub) to `ashpd::desktop::open_uri::OpenURI::default().open_file(...)` on the tokio runtime
- [x] 4.2 Handle portal failure (no handler registered) with a fallback `gio::AppInfo::launch_default_for_uri`

## 5. Client wiring

- [x] 5.1 In `src/handlers/client.rs`, add `download_handler` field to `wrap_client!`
- [x] 5.2 Override `download_handler(&self) -> Option<DownloadHandler>` returning `Some(self.download_handler.clone())`

## 6. Verify

- [x] 6.1 Download a photo from WhatsApp media → lands at `~/Downloads/<name>.jpg`
- [x] 6.2 Download same file again → `<name> (1).jpg`
- [x] 6.3 Set `download-directory=/tmp/karere-dl` → next download lands there
- [x] 6.4 Toast "Open" opens file in default app via OpenURI portal
- [x] 6.5 Toast "Show in Folder" opens parent directory in Files
- [x] 6.6 Simulate failed download (kill renderer mid-transfer) → AlertDialog shows
