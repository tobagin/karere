# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Karere is a native WhatsApp desktop client for Linux, built with Rust using GTK4/LibAdwaita and WebKitGTK 6.0. It wraps WhatsApp Web in a sandboxed WebKit engine with full GNOME desktop integration (notifications, system tray, spell checking, multi-account support).

## Build Commands

```bash
# Flatpak development build (recommended)
./build.sh --dev
flatpak run io.github.tobagin.karere.Devel

# Flatpak production build
./build.sh

# Local development (requires gtk4-devel, libadwaita-devel, webkit2gtk6.0-devel on host)
cargo run

# Check and lint
cargo check
cargo clippy

# Format
cargo fmt
```

The Flatpak build uses Meson as the build system which invokes Cargo internally. The `build.rs` script handles Blueprint compilation (.blp → .ui), GResource compilation, GSchema compilation, and translation compilation (msgfmt).

## Architecture

### Source Modules (src/)

- **main.rs** — Application entry point. Initializes GTK4/LibAdwaita app, global Tokio runtime, tray icon, application-level actions (quit, about, preferences), multi-account switching, and gettext localization.
- **window.rs** — Main window widget (composite GTK4 template). Manages a **WebView pool** (separate WebKit instances per account to avoid reload on switch), account switcher UI (bottom sheet), notification capture/routing, window state persistence, zoom controls, and keyboard shortcuts.
- **tray.rs** — System tray via `ksni` (KDE Status Notifier Item protocol). Shows unread status, toggles window visibility, provides account switching menu. Auto-detects GNOME vs. other DEs.
- **preferences.rs** — Settings dialog using LibAdwaita. Controls for theme, startup behavior, spell checking, downloads, multi-account, accessibility (zoom, high contrast, reduced motion), notifications, and developer tools.
- **accounts.rs** — `AccountManager` reads/writes per-account JSON data from user config directory. Tracks account state: id, name, profile picture, permissions (notifications, microphone), unread counts.
- **spellcheck.rs** — Hunspell dictionary detection from system paths, locale-to-dictionary matching.

### Key Patterns

- **Composite GTK Objects**: UI defined in Blueprint `.blp` files (in `data/ui/`), compiled to GtkBuilder XML at build time.
- **GSettings**: Persistent preferences via `data/io.github.tobagin.karere.gschema.xml.in`. Window state, theme, notifications, etc.
- **WebView Pool**: Multiple WebKit instances kept alive in memory to avoid reload overhead when switching accounts.
- **Async/Await**: Tokio runtime for non-blocking portal/notification operations via `ashpd` (XDG Desktop Portals).
- **Signal Connections**: Standard GTK4 signal pattern for event handling.

### Key Dependencies

| Purpose | Crate |
|---------|-------|
| UI toolkit | gtk 0.10.3, libadwaita 0.8.1 |
| Web engine | webkit6 0.5.0 |
| System tray | ksni 0.3.3 |
| Desktop portals | ashpd 0.12.1 |
| DBus | zbus 5.13.2 |
| Async runtime | tokio 1.49.0 |
| Localization | gettext-rs 0.7.7 |

### Resource Layout

- `data/ui/` — Blueprint UI files (window.blp, preferences.blp, keyboard-shortcuts.blp)
- `data/icons/` — App icons (SVG, PNG at multiple sizes, symbolic)
- `data/sounds/` — Notification audio
- `data/resources.gresource.xml` — GTK resource manifest
- `packaging/` — Flatpak manifests (Devel and production)
- `po/` — Translation files (gettext .po)

## Coding Standards

- Rust formatting: `cargo fmt`
- Linting: `cargo clippy`
- Blueprint UI filenames: `kebab-case` (e.g., `keyboard-shortcuts.blp`)
- Rust edition: 2024
- License: GPL-3.0-or-later
