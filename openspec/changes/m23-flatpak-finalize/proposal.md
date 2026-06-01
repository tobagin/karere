## Why

M6 introduced a minimal, functional Flatpak manifest sufficient to build and run karere v4 locally, but it intentionally deferred the work required for a Flathub submission: `appstream-compose` is disabled, the `metainfo` and `desktop` templates are empty placeholders, icons are not registered with `gtk-update-icon-cache`, and there is no `add-extensions` block for a separate Debug runtime. After M7-M22 added IPC, tray, notifications, downloads, paste bridge, mobile responsive, accessibility, multi-account, and preferences, the manifest still references stale build-args and an outdated `cargo-sources.json`. Independently, the locked decision for crash triage is "ship a Debug extension to Flathub" rather than depending on user-side `debuginfod`, so the Debug extension declaration and its mount layout must land before submission. M23 is the final packaging milestone that turns the working manifest into a Flathub-ready bundle plus a separately installable symbol extension.

## What Changes

- Flip `appstream-compose: true` in `packaging/io.github.tobagin.karere.yml` and restore `gtk_update_icon_cache: true` in meson `post_install`.
- Add `--talk-name=org.kde.StatusNotifierWatcher` (M15) and `--filesystem=xdg-config/autostart:create` (autostart portal) to `finish-args`.
- Remove leftover `--share=network` build-arg from the karere module (vendored crates work offline).
- Add a `cleanup` array dropping `*.la`, `*.a`, and `/app/include` (except `/app/lib/cef/include`).
- Add an `add-extensions: io.github.tobagin.karere.Debug` block mounted at `lib/debug`, with `autodelete: 'true'` and `no-autodownload: 'true'`.
- Populate `data/io.github.tobagin.karere.metainfo.xml.in.in` from karere v3 (id, licenses, name/summary, description with a new "Now built on CEF/Chromium 148" paragraph, `<launchable>`, `<screenshots>` referencing copies under `data/screenshots/`, a `<release version="4.0.0">` summarizing M1-M22, and the v3 `<content_rating type="oars-1.1">`).
- Populate `data/io.github.tobagin.karere.desktop.in.in` with Name, Comment, `Categories=Network;InstantMessaging;Chat;`, `Keywords`, `Icon=io.github.tobagin.karere`, `Exec=karere %U`, `MimeType=x-scheme-handler/whatsapp;`.
- Regenerate `packaging/cargo-sources.json` to cover every `Cargo.toml` change from M7-M22.
- Replace README with karere v4 content based on v3 README plus CEF/Chromium 148 note and the locked hard-fork / no-migration decisions; prepend a `4.0.0` CHANGELOG entry.

## Capabilities

### New Capabilities
- `flatpak-finalize`: Flathub-ready manifest with `appstream-compose` enabled, populated metainfo/desktop templates, icon-cache regeneration, hardened `finish-args` (StatusNotifierWatcher talk-name, autostart filesystem), cleanup of dev artifacts, and refreshed `cargo-sources.json`.
- `flatpak-debug-extension`: `add-extensions: io.github.tobagin.karere.Debug` mounted at `lib/debug` with `autodelete: 'true'` and `no-autodownload: 'true'`, enabling symbolicated crash reports via `coredumpctl debug karere` once the user installs the extension.

### Modified Capabilities
<!-- None: M23 only adds new capabilities; M6's flatpak-packaging remains the structural baseline. -->

## Impact

- Modified files: `packaging/io.github.tobagin.karere.yml`, `packaging/cargo-sources.json`, `data/io.github.tobagin.karere.metainfo.xml.in.in`, `data/io.github.tobagin.karere.desktop.in.in`, `data/screenshots/` (new images copied from v3), `meson.build` (post_install icon cache), `README.md`, `CHANGELOG.md`.
- Build: `flatpak-builder --user --install --force-clean build-dir packaging/io.github.tobagin.karere.yml` must succeed end-to-end including appstream-compose.
- Runtime: installing `io.github.tobagin.karere.Debug` enables symbolicated stack traces; absence of the extension does not block normal use.
- Risk: appstream-compose validation can fail on minor metainfo issues; mitigated by porting verbatim from v3 (already shipped on Flathub) and running `flathub-quality-check` before tagging.
- Depends on: M6 (manifest baseline), M7 (icon assets copied), M15 (tray talk-name), and every M7-M22 Cargo change that needs to be reflected in `cargo-sources.json`.
