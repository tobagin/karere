## Why

Karere v3.1.1 is a WebKitGTK WhatsApp client. We are hard-forking `gtk-cef-shell` into Karere v4 (CEF edition) with feature parity. M7 is the first concrete fork step: rename the project, copy karere assets verbatim, and wire up the i18n + blueprint tooling that downstream milestones (M8 actions, M14 notifications, M15 tray, M20 multi-account, M22 prefs) depend on.

## What Changes

- **BREAKING**: rename Cargo package, binary, app-id, gschema, desktop file, metainfo, and gresource paths from `gtk-cef-shell` / `io.github.tobagin.GtkCefShell` to `karere` / `io.github.tobagin.karere`. Directory name stays `gtk-cef-shell`.
- **BREAKING**: relicense from current license to `GPL-3.0-or-later` to match karere assets copied verbatim.
- Rename Rust types: `ShellApplication` → `KarereApplication`, `ShellWindow` → `KarereWindow`, `CefGtkArea` → `KarereWebView`.
- Add deps: `gettext-rs` (with `gettext-system` feature), `toml`, `serde` (derive).
- Copy karere assets verbatim from `/home/tobagin/Projects/karere`: icons (`data/icons/hicolor/**`), sounds (5 `.oga` files), UI blueprints (`data/ui/*.blp`), gschema (`data/io.github.tobagin.karere.gschema.xml.in`), `po/` tree (LINGUAS, POTFILES.in, *.po, meson.build), LICENSE, README.
- `meson.build`: project name `karere` `4.0.0-dev`, add `i18n.gettext('karere', preset: 'glib')`, add `subdir('po')`, re-enable `gtk_update_icon_cache: true`.
- `build.rs`: detect `blueprint-compiler` via `which`; compile each `data/ui/*.blp` to `$OUT_DIR/ui/*.ui`; gresource pulls from `$OUT_DIR/ui/` when present, else falls back to `data/ui/*.ui`. Panic with install instructions if compiler missing.
- `src/main.rs`: gettext init via `setlocale` + `bindtextdomain("karere", "/app/share/locale")` + `textdomain("karere")`, matching karere `src/main.rs:23-80`.
- `packaging/`: rename manifest to `io.github.tobagin.karere.yml`; flip app-id, module name; keep cef-binaries module from M6 unchanged; finish-args `LD_LIBRARY_PATH=/app/lib/cef` preserved.
- Hard-fork model: no `app-config.toml`, no feature gating. All karere features ship default.

## Capabilities

### New Capabilities
- `karere-branding`: app-id, package metadata, gschema id, desktop/metainfo identity, icon set, license posture for the Karere v4 fork.
- `i18n-gettext`: gettext runtime initialization, `po/` tree integration, blueprint+rust string extraction via meson POT regeneration.
- `blueprint-build`: build-time compilation of `.blp` files to `.ui`, gresource source selection, host tool detection.

### Modified Capabilities
<!-- None: this is the first milestone introducing branding/i18n/blueprint specs. -->

## Impact

- Affected code: `Cargo.toml`, `meson.build`, `build.rs`, `src/main.rs`, `src/application.rs`, `src/window.rs`, `src/web_view.rs` (rename), `data/**`, `po/**`, `packaging/**`, `LICENSE`, `README.md`.
- Affected tooling: host must have `blueprint-compiler`; flatpak sdk extension already includes it.
- Downstream: unblocks M8 (window persistence / actions reading karere gschema keys), M14/M15 (notifications/tray using karere sounds/icons), M20 (multi-account using karere gschema schemas), M22 (preferences UI from copied blueprints).
- Non-goals (deferred): window persistence (M8), actions wiring (M8), preferences UI (M22), tray (M15), notifications (M14), multi-account (M20).
