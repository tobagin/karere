# Add Download Manager

## Why
Currently, Karere relies on WebKit's default download behavior with no user control or feedback. Users cannot choose where files are saved (hardcoded to xdg-download), receive no notification when downloads complete, and have no quick way to open downloaded files. This creates a poor user experience compared to modern desktop applications and fails to leverage Flatpak security features like the File Chooser Portal. WhatsApp Web users frequently download media, documents, and voice messages, making download management a core workflow that deserves first-class support.

## What Changes
- Create new `DownloadManager` class in `src/managers/DownloadManager.vala` to handle download detection, directory resolution, and file opening (~300 lines)
- Enhance `WebViewManager` to detect downloads via WebKit policy decisions and emit download signals
- Add Downloads preferences page with File Chooser Portal integration for custom directory selection
- Add toast notifications with "Open" action button for completed downloads
- Add two GSettings keys: `custom-download-directory` (string) and `download-notifications-enabled` (boolean)
- Use AppInfo.launch_default_for_uri_async() to open files with default applications via Flatpak portal

## Impact
- **Affected specs**: Creates 3 new capabilities (download-manager, download-preferences, download-notifications), modifies 1 existing (webview-manager)
- **Affected code**: New files: `src/managers/DownloadManager.vala`, Modified: `src/managers/WebViewManager.vala`, `src/Window.vala`, `src/Application.vala`, `src/dialogs/PreferencesDialog.vala`, `data/ui/preferences.blp`, `data/io.github.tobagin.karere.gschema.xml.in`, `meson.build`
- **Breaking changes**: None - purely additive feature
- **Migration**: None required - new settings use safe defaults (empty string for custom directory = use default xdg-download, notifications enabled by default)
- **User experience**: Significantly improved download workflow with user control and feedback
