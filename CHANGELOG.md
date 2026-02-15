# Changelog

All notable changes to Karere will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]
### Added

### Changed
### Fixed

## [2.5.1] - 2026-02-15

### Fixed
- **Stability**: Handle WebKit web process crashes with auto-reload and user notification.
- **Security**: Fix JS injection and path traversal vulnerabilities.
- **Robustness**: Fix panic in notification handling and resolved all clippy warnings.

## [2.5.0] - 2026-02-14

### Added
- **Multi-Account**: Complete multi-account support with sidebar switching, tray menu integration, and unread counting.
- **Zoom**: Added per-account zoom settings and reworked accessibility zoom.

### Fixed
- **Startup**: Resolved race conditions when starting in background and initializing tray icon.
- **Resources**: Fixed file descriptor exhaustion issues.
- **Notifications**: Fixed issue where desktop notifications weren't clearing when window gained focus.
- **Downloads**: Added fallback to XDG Downloads directory when download-directory is unset.


## [2.4.2] - 2026-02-08

### Added
- **Zoom Support**: Added zoom controls (Ctrl +/-/0), persistence, and preference settings.

### Fixed
- **Window Size**: Fixed window size increasing on restart.

## [2.4.1] - 2026-02-06

### Fixed
- **Startup Reload**: Fixed an issue where the webview would double-reload on startup due to improper window size initialization (Mobile/Desktop detection).

## [2.4.0] - 2026-02-06

### Added
- **Call Preparation**: Added underlying WebRTC permission handling for Camera and Microphone to support future WhatsApp Web voice and video calls.
- **Mobile UI**: Improved resizing logic for better responsiveness on mobile devices.

## [2.3.6] - 2026-02-04

### Fixed
- **Notifications**: Respect system "Do Not Disturb" settings on GNOME (suppresses sound and lowers visual priority).

## [2.3.5] - 2026-02-02

### Fixed
- Fixed tray icons rendering black on light/dark themes in KDE
- Restored correct PNG app icons with transparent backgrounds
- Removed complex runtime icon rendering to improve stability
- Fixed icon visibility issues across different desktop environments


## [2.3.4] - 2026-02-02

### Fixed
- **Freezing**: Fixed frequent app freezing on resume by reverting `CacheModel::DocumentViewer` optimization.
- **Background Startup**: Fixed issue where app would show window despite "Start in background" setting being enabled.
- **Styling**: Fixed symbolic icon rendering to correctly use `currentColor` and cleaned up SVG assets.

### Changed
- **Memory**: Optimized `mobile_responsive.js` to prevent excessive DOM observation and reduce memory usage.

## [2.3.3] - 2026-01-24

### Fixed
- **UI**: Fixed window resize behavior to prevent excessive reloading on resize.
- **Notifications**: Fixed background notifications by ensuring window realization and visibility spoofing.
## [2.3.2] - 2026-01-18

### Fixed
- **Build**: Fixed Flatpak build failure by correcting cargo source vendor directory path.

## [2.3.1] - 2026-01-18

### Fixed
- **Crash**: Resolved "Too many open files" crash by correcting file descriptor leaks in notification handling.
- **Stability**: Fixed issue where notification proxy creation was not reused, causing resource exhaustion.
- **Zombie Processes**: Fixed potential zombie process accumulation during custom sound playback.


## [2.3.0] - 2026-01-17

### Added
- **Ctrl+N for New Chat**: Native keyboard shortcut to open new chat sidebar via simulated keypress.
- **Mobile Layout Support**: Responsive layout for mobile Linux phones (Phosh, Plasma Mobile, Lomiri) with auto-detection based on desktop environment and window width.
- **Tray Icon Toggle**: Click the system tray icon to show/hide the application window.
- **Window Size Persistence**: Window dimensions are saved when hiding and restored when showing.

### Changed
- **Mobile Layout Setting**: Changed from boolean toggle to dropdown (auto/enabled/disabled) for better control.
- **User Agent**: Switched WebView to Chrome on Linux user agent for improved WhatsApp Web compatibility.
- **Ko-Fi Badge**: Added support button to README.


## [2.2.2] - 2026-01-12

### Changed
- **Metadata**: Updated application summary and description to better comply with AppStream/Flathub guidelines.

## [2.2.1] - 2026-01-11

### ✨ New Features

- **🔔 Notification Chat Linking**: Clicking a notification now opens the exact chat in the application window.
- **📷 Webcam Support**: Added full support for using the camera in WhatsApp Web (e.g., for Status updates and Video Calls).
- **🔒 Permission Dialogs**: Added new permission dialogs for Camera and Microphone access with persistent "Allow/Deny" settings.
- **📲 Device Access**: Updated sandbox permissions to ensure reliable webcam device detection.
- **👥 Credits**: Updated About dialog with new contributors and GitHub URLs.
- **🙏 Acknowledgements**: Replaced Vala with The Rust Project in special thanks.

## [2.1.0] - 2026-01-10

### ✨ New Features

- **📋 Enhanced Clipboard Support**: Improved image paste functionality with Base64 encoding and JavaScript injection for reliable WhatsApp Web compatibility.
- **🖱️ Middle-Click Paste**: Added primary selection paste support via middle mouse button click.

### 🐛 Bug Fixes

- **Clipboard**: Fixed double-paste issue when using middle-click (now only pastes primary selection, not regular clipboard).


## [2.0.9] - 2026-01-07

### ✨ New Features

- **New Icons**: Updated application icons for a fresh look (Thanks to @oiimrosabel).

### 🐛 Bug Fixes

- **Packaging**: Fixed missing GSettings schema in Flatpak builds, resolving startup crashes.
- **Metadata**: Added missing VCS browser URL to AppStream metadata to satisfy linter warnings.
- **Documentation**: Corrected "Built with Vala" references to "Built with Rust" in metadata.

## [2.0.8] - 2026-01-06

### Changed
- **Notifications**: Switch to **XDG Notification Portal** (`ashpd`) for native notifications in sandboxed environments.
- **Permissions**: Remove `org.freedesktop.Notifications` permission from manifests.

### Fixed
- **Downloads**: Fixed "Open" action for downloaded files in Flatpak (using `ashpd`).
- **Input**: Fixed **Dead Keys** (composition) bug by disabling navigator overrides.


## [2.0.7] - 2026-01-02

### Changed
- Switch to XDG Background Portal for autostart functionality.
- Add PipeWire permission (`xdg-run/pipewire-0`) for improved audio support.

### Fixed
- **Auto-Start**: Implemented robust toggling using a global Tokio runtime to prevent XDG Portal hangs on repeated requests.
- **Quit**: Improved window closing logic to ensure application termination.

## [2.0.6] - 2026-01-02

### Added
- Add Sabri Ünal as a contributor to the About dialog.

## [2.0.5] - 2026-01-02

### Added
- Restore `xdg-config/autostart` permission (linter exception granted).

## [2.0.4] - 2026-01-02

### Fixed
- Remove unnecessary `xdg-config/autostart` permission to satisfy Flathub linter (autostart is handled via portals).

## [2.0.3] - 2026-01-02

### Added
- Add `--filesystem=xdg-config/autostart` permission to Flatpak manifest to support autostart functionality.

## [2.0.2] - 2026-01-02

### Fixed
- **Tray Icon**: Fixed missing tray icon on GNOME by granting `org.freedesktop.StatusNotifierWatcher` permission.

## [2.0.1] - 2026-01-01

### Added
- **Auto-Start**: Added a toggle in preferences to control "Run on Startup" behavior (default: false)
- **Development Icons**: Stripe pattern for development version icons
- **Translations**: Completed translation coverage for English (UK/US), Portuguese (BR/PT), and Spanish.

### Fixed
- **Auto-Start**: Fixed autostart toggle pathing for Flatpak environments
- **Tray Icon**: Fixed quit behavior to properly terminate the application
- **Audio Playback**: Resolved issues with voice messages and video playback audio output.
- **Build**: Fixed installation issues with GSettings schemas and desktop file naming for development builds.
- **Build**: Migrated Flatpak build logic to `meson.build` for better offline build support.
- **Flatpak Build**: Fixed `appstreamcli compose` errors and ensured correct asset installation.

### Changed
- **Build System**: Improved `cargo-sources.json` generation and synchronization.

## [2.0.0] - 2025-12-29

### Added
- **Rewrite in Rust**: Complete rebuild of the application using native Rust, GTK4, and Libadwaita for superior performance and memory safety.
- **System Tray Icon**: Support for background execution with unread message indication.
- **Custom Notification Sounds**: Integrated 5 custom sound options including "WhatsApp" style, with preview capability.
- **Accessibility Suite**: Full accessibility implementation including High Contrast mode, Reduce Motion, Focus Indicators, Screen Reader optimizations, and Webview Zoom.
- **Auto-Correct**: Toggleable spell-checking auto-correction with dictionary management.
- **Keyboard Shortcuts**: Global F12 accelerator for Developer Tools and improved shortcut handling.

### Changed
- **Preferences UI**: Completely redesigned using Blueprint (`.blp`) for a modern, maintainable declarative UI.
- **Download Path Display**: Now shows friendly paths (e.g., `/home/user/Downloads`) instead of Flatpak portal paths.
- **Notification Settings**: Granular controls for messages, sounds, and downloads with a master toggle.

### Fixed
- **Startup Stability**: Resolved critical crashes related to modal properties and invalid settings.
- **Flatpak Integration**: Improved permissions for home directory access to fix file chooser limitations.
- **DevTools**: Fixed unreliable F12 shortcut registration.

## [1.1.1] - 2025-12-25

### Fixed
- Fixed critical 10-second startup freeze/spinner
- Implemented proper D-Bus activation for background execution

## [1.1.0] - 2025-12-12

### Added
- Microphone permission support for voice notes and calls
- "Start in background" preference option
- Manual reload action (Ctrl+R)
- Automatic reload when network connection is restored
- MPRIS workaround documentation in README

### Changed
- Optimized application startup and shutdown performance
- Made dictionary loading asynchronous to prevent UI freeze
- Debounced settings saves to improve responsiveness

### Fixed
- Fixed missing spell check dictionaries on startup
- Resolved 20s freeze during application lifecycle events

## [1.0.3] - 2025-10-21

### Changed
- Updated build configuration and dependencies
- Enhanced packaging for better compatibility

### Fixed
- Minor bug fixes and stability improvements

## [1.0.2] - 2025-10-20

### Changed
- Updated about dialog screenshot
- Updated application screenshots
- Minor README corrections

## [1.0.0] - 2025-10-01

### Added
- **Notifications**: Fully working native notifications with persistent permission state
- **Image Paste Support**: Ctrl+V now works for both text and images
- **Download Manager**: Custom download directory with toast notifications
- **Spell Checking**: 80+ language dictionaries with auto-detection
- **Accessibility**: Enhanced screen reader support, keyboard navigation, and focus indicators
- **Keyboard Shortcuts**: Comprehensive shortcuts dialog (F1)

### Fixed
- Fixed WhatsApp Web notification banner appearing on every launch
- Fixed notification permission not persisting across sessions
- Fixed Ctrl+V paste for text content by allowing native browser behavior
- Fixed WebView zoom being enabled by default (now disabled)

### Changed
- Refactored Window.vala into specialized manager components
- Reorganized codebase with PascalCase filenames
- Removed Do Not Disturb (DND) feature (handled by system)
- Removed custom logging and crash reporting systems
- Improved storage configuration and permission handling

## [0.9.4] - 2025-09-14

### Added
- WebKit cache control setting for improved rendering

### Changed
- Runtime platform update
- Build system improvements

### Fixed
- Various bug fixes and stability improvements

## [0.9.3] - 2025-09-10

### Added
- Enhanced emoji support with Noto Color Emoji font
- External link handling with system browser
- Improved build configuration

### Fixed
- Emoji rendering issues
- Link opening in external applications

## [0.9.2] - 2025-09-08

### Added
- External link handling improvements

### Fixed
- Various bug fixes
- Build and dependency issues

## [0.9.1] - 2025-09-05

### Added
- Initial stable release features
- WebKit optimizations

### Fixed
- Meson and dependency version requirements
- Build configuration issues

## [0.9.0] - 2025-09-01

### Added
- Initial public release
- GTK4 and LibAdwaita integration
- WhatsApp Web wrapper functionality
- Native desktop notifications
- Theme support (Light/Dark/System)
- Basic preferences dialog
- Flatpak packaging

---

## Types of Changes

- `Added` for new features
- `Changed` for changes in existing functionality
- `Deprecated` for soon-to-be removed features
- `Removed` for now removed features
- `Fixed` for any bug fixes
- `Security` for vulnerability fixes

## Release Notes

### Version 1.0.0 Highlights

This major release marks Karere as stable and production-ready with:

- **Fully Working Notifications**: Complete notification system with persistent permissions
- **Enhanced Accessibility**: Comprehensive a11y support including screen readers, keyboard navigation, and high contrast
- **Image & Text Paste**: Seamless clipboard integration for both content types
- **Download Manager**: Custom download paths with notification feedback
- **Spell Checking**: Multi-language support with 80+ dictionaries
- **Code Quality**: Major refactoring for maintainability and performance

### Migration from 0.9.x to 1.0.0

No migration steps required. Settings and data are preserved across versions.

### Known Issues

- Some optional codec libraries (libjxl, libSvtAv1Enc, libtheoraenc) are not included but don't affect core functionality
- GStreamer plugin warnings may appear in logs but don't impact application behavior

---

For more details on each release, see the [commit history](https://github.com/tobagin/karere/commits/main).
