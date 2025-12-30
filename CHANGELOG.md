# Changelog

All notable changes to Karere will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
