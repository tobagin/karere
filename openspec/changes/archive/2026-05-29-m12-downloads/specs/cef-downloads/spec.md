## ADDED Requirements

### Requirement: Resolve download target directory from configuration
The shell SHALL resolve the directory for each new download by reading the `download-directory` GSetting and, when that value is empty, falling back to `glib::user_special_dir(UserDirectory::Downloads)` (the XDG `DOWNLOAD` directory).

#### Scenario: GSetting is empty
- **WHEN** a download begins and `download-directory` is the empty string
- **THEN** the shell selects `XDG_DOWNLOAD_DIR` as the target directory

#### Scenario: GSetting holds a valid path
- **WHEN** a download begins and `download-directory` is `/home/user/Inbox` and that directory exists and is writable
- **THEN** the shell selects `/home/user/Inbox` as the target directory

#### Scenario: GSetting holds an unusable path
- **WHEN** a download begins and `download-directory` points to a non-existent or non-writable path
- **THEN** the shell falls back to `XDG_DOWNLOAD_DIR` and logs a warning

### Requirement: Resolve duplicate filenames with `(N)` suffix
The shell SHALL avoid overwriting existing files by appending ` (N)` (space, open-paren, integer, close-paren) before the extension, starting from `1` and incrementing until a non-existing path is found.

#### Scenario: No collision
- **WHEN** the suggested filename is `photo.jpg` and `<target_dir>/photo.jpg` does not exist
- **THEN** the final path is `<target_dir>/photo.jpg`

#### Scenario: One collision
- **WHEN** the suggested filename is `photo.jpg` and `<target_dir>/photo.jpg` exists
- **THEN** the final path is `<target_dir>/photo (1).jpg`

#### Scenario: Multiple collisions
- **WHEN** `<target_dir>/photo.jpg`, `<target_dir>/photo (1).jpg`, and `<target_dir>/photo (2).jpg` all exist
- **THEN** the final path is `<target_dir>/photo (3).jpg`

#### Scenario: Extension-less filename
- **WHEN** the suggested filename is `notes` and `<target_dir>/notes` exists
- **THEN** the final path is `<target_dir>/notes (1)`

#### Scenario: Pathological collision count
- **WHEN** more than 9999 colliding files exist for the same name
- **THEN** the shell appends a UUID-derived suffix instead of an integer and proceeds

### Requirement: Sanitize suggested filenames
The shell SHALL strip path-separator characters (`/`, `\`) and NUL from any filename suggested by the renderer before using it on disk.

#### Scenario: Filename contains a slash
- **WHEN** the renderer suggests `../etc/passwd`
- **THEN** the resolved basename is `..etcpasswd` (or equivalent with separators removed)

#### Scenario: Filename is purely Unicode
- **WHEN** the renderer suggests `相片.jpg`
- **THEN** the filename is preserved unchanged

### Requirement: Suppress CEF's native save dialog
The shell SHALL invoke the CEF before-download callback with `show_dialog = false` so downloads proceed silently to the resolved path.

#### Scenario: Download starts
- **WHEN** `on_before_download` fires for any download
- **THEN** the shell calls `callback.cont(target_path, false)` and CEF does not raise its own file chooser

### Requirement: Notify on download completion via toast
The shell SHALL emit an `AdwToast` reading `"<name> downloaded"` when a download finishes successfully, with two action buttons: "Open" and "Show in Folder".

#### Scenario: Download completes
- **WHEN** `on_download_updated` reports `is_complete = true` for a download saved to `/home/user/Downloads/photo.jpg`
- **THEN** the active window displays a toast labelled `photo.jpg downloaded`
- **AND** the toast offers an "Open" button and a "Show in Folder" button

#### Scenario: Open action invokes OpenURI portal on the file
- **WHEN** the user activates the "Open" toast button for `/home/user/Downloads/photo.jpg`
- **THEN** the shell invokes the `app.open-download` action with that path
- **AND** the action calls `ashpd::desktop::open_uri::OpenURI::default().open_file(...)` on the tokio runtime targeting `/home/user/Downloads/photo.jpg`

#### Scenario: Show-in-Folder action invokes OpenURI portal on the parent directory
- **WHEN** the user activates the "Show in Folder" toast button for `/home/user/Downloads/photo.jpg`
- **THEN** the shell invokes the OpenURI portal `open_file` call targeting `/home/user/Downloads`

### Requirement: Notify on download failure via dialog
The shell SHALL present an `AdwAlertDialog` reading `"Download failed: <reason>"` when a download is canceled or reports a non-zero error code.

#### Scenario: Download canceled by renderer
- **WHEN** `on_download_updated` reports `is_canceled = true`
- **THEN** the active window shows an `AdwAlertDialog` titled `Download failed: canceled`

#### Scenario: Download fails with an error code
- **WHEN** `on_download_updated` reports a non-zero error code
- **THEN** the active window shows an `AdwAlertDialog` whose body includes the human-readable error reason

### Requirement: Register the download handler on the CEF client
The shell SHALL attach an instance of `ShellDownloadHandlerBuilder` (constructed via `wrap_download_handler!`) to `ShellClient` so CEF routes download events to it.

#### Scenario: Client construction
- **WHEN** `ShellClient` is built
- **THEN** its `download_handler` field is populated with the shell's handler
- **AND** the corresponding `get_download_handler` override returns that handler

### Requirement: Expose `app.open-download` action backed by the OpenURI portal
The shell SHALL provide an `app.open-download` GAction taking a single string parameter (an absolute filesystem path) that opens the file via `ashpd::desktop::open_uri::OpenURI` on the tokio runtime.

#### Scenario: Activation with a valid path
- **WHEN** `app.open-download` is activated with the parameter `/home/user/Downloads/photo.jpg`
- **THEN** the shell spawns an async task on the tokio runtime that calls `OpenURI::default().open_file(...)` for that path

#### Scenario: Portal returns an error
- **WHEN** the OpenURI portal call returns an error
- **THEN** the shell logs the failure and does not crash

### Requirement: Provide gschema keys for download configuration
The shell SHALL declare three new GSettings keys:
- `download-directory`: string, default `""` (empty falls back to `XDG_DOWNLOAD_DIR`).
- `notify-downloads-enabled`: boolean, default `true`.
- `notify-download-type`: enum with values `toast`, `notification`, `both`, default `toast`.

#### Scenario: Defaults after fresh install
- **WHEN** the schema is compiled and read on a fresh profile
- **THEN** `download-directory` is `""`, `notify-downloads-enabled` is `true`, and `notify-download-type` is `toast`

#### Scenario: Disabling completion notifications
- **WHEN** `notify-downloads-enabled` is `false`
- **THEN** the shell still saves the file but suppresses the completion toast / notification
