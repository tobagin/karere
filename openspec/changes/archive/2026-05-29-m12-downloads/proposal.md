## Why

WhatsApp Web users routinely save photos, videos, and documents. Without a download handler, CEF silently drops these or prompts via its own UI. Karere v3 (`window.rs:1142-1296`) already established the contract: pick a target directory from a GSetting, de-duplicate filenames with `(1)`, `(2)` suffixes, and surface completion via an `AdwToast` with "Open" and "Show in Folder" actions. v4 must match that behaviour so users can save media without overwriting earlier downloads and without leaving the app to find the file.

## What Changes

- Add a new `ShellDownloadHandlerBuilder` in `src/handlers/download.rs` wired through `wrap_download_handler!`.
- `on_before_download`: resolve target directory from the `download-directory` GSetting (falling back to `XDG_DOWNLOAD_DIR` when empty), sanitize the suggested filename, and resolve collisions by walking `name.ext` → `name (1).ext` → `name (2).ext` until a free path exists; then `callback.cont(target_path, /*show_dialog=*/false)`.
- `on_download_updated`: push a `DownloadCompleted { path, name }` event into shared state when `is_complete`; window polling raises an `AdwToast` `"<name> downloaded"` with "Open" (invokes `app.open-download <path>`) and "Show in Folder" (opens parent dir via OpenURI portal). On `is_canceled` or non-zero error code, present an `AdwAlertDialog` with the failure reason.
- Register `download_handler` on `ShellClient` in `src/handlers/client.rs`.
- Fill in the previously-stubbed `app.open-download <path>` action so it uses `ashpd::desktop::open_uri::OpenURI::default().open_file(...)` on the tokio runtime.
- Extend the gschema with `download-directory` (string, default `""`), `notify-downloads-enabled` (bool, default `true`), and `notify-download-type` (enum `toast` | `notification` | `both`, default `toast`).

## Capabilities

### New Capabilities
- `cef-downloads`: Download lifecycle handling — target-directory resolution, duplicate-filename suffixing, completion toasts with Open / Show-in-Folder actions, and failure dialogs.

### Modified Capabilities
<!-- None: prior milestones did not ship a download capability spec. -->

## Impact

- Code: new `src/handlers/download.rs`; edits to `src/handlers/client.rs`, `src/handlers/mod.rs`, the app-action registration site (`app.open-download`), shared state struct, and the window's toast polling loop.
- Schema: additions to `data/<app-id>.gschema.xml` for the three new keys (UI wiring deferred to M22).
- Dependencies: `ashpd` (already present for portal use); no new crates expected.
- Behaviour: previously-discarded downloads now land on disk; no breaking change to existing public APIs.
