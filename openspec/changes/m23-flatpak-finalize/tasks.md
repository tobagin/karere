## 1. Manifest hardening

- [ ] 1.1 Flip `appstream-compose: false` to `true` for the karere module in `packaging/io.github.tobagin.karere.yml`.
- [ ] 1.2 Add `--talk-name=org.kde.StatusNotifierWatcher` to `finish-args`.
- [ ] 1.3 Add `--filesystem=xdg-config/autostart:create` to `finish-args`.
- [ ] 1.4 Remove any leftover `--share=network` build-arg from the karere module.
- [ ] 1.5 Add a `cleanup` array dropping `*.la`, `*.a`, and `/app/include` while preserving `/app/lib/cef/include`.
- [ ] 1.6 Confirm the `cef-binaries` module is unchanged from M6 (no edits required).

## 2. Debug extension

- [ ] 2.1 Add an `add-extensions` block keyed `io.github.tobagin.karere.Debug` with `directory: lib/debug`, `autodelete: 'true'`, `no-autodownload: 'true'`.
- [ ] 2.2 Ensure the karere module's build pipeline emits detached debug symbols under `/app/lib/debug/<binary>.debug` (verify Rust `split-debuginfo` or equivalent strip step is in place).
- [ ] 2.3 Manual verify: `flatpak install --user --no-deps repo io.github.tobagin.karere.Debug` succeeds.
- [ ] 2.4 Manual verify: `coredumpctl debug karere` resolves karere frame function names after a deliberate crash.

## 3. Metainfo template

- [ ] 3.1 Port karere v3's metainfo content into `data/io.github.tobagin.karere.metainfo.xml.in.in`: `<id>`, `<metadata_license>CC0-1.0</metadata_license>`, `<project_license>GPL-3.0-or-later</project_license>`, `<name>Karere</name>`, `<summary>Native WhatsApp client for Linux</summary>`.
- [ ] 3.2 Write the `<description>` paragraphs from v3 and append a "Now built on CEF/Chromium 148" paragraph.
- [ ] 3.3 Add `<launchable type="desktop-id">io.github.tobagin.karere.desktop</launchable>`.
- [ ] 3.4 Copy v3 screenshots into `data/screenshots/` and reference them from `<screenshots>`.
- [ ] 3.5 Add `<release version="4.0.0">` with notes summarizing CEF migration plus M7-M22 features.
- [ ] 3.6 Copy `<content_rating type="oars-1.1">` ratings verbatim from v3.
- [ ] 3.7 Manual verify: `appstreamcli validate` passes for the composed metainfo.

## 4. Desktop template

- [ ] 4.1 Populate `data/io.github.tobagin.karere.desktop.in.in` with `[Desktop Entry]`, `Name=Karere`, `Comment=`, `Categories=Network;InstantMessaging;Chat;`, `Keywords=WhatsApp;Chat;Messenger;`, `Icon=io.github.tobagin.karere`, `Exec=karere %U`, `MimeType=x-scheme-handler/whatsapp;`.
- [ ] 4.2 Manual verify: `desktop-file-validate` passes with no warnings.
- [ ] 4.3 Manual verify: `xdg-open whatsapp://send?text=hello` launches Karere when set as default handler.

## 5. Icons

- [ ] 5.1 Confirm M7 has already copied `data/icons/hicolor/**` from karere v3.
- [ ] 5.2 Restore `gtk_update_icon_cache: true` in `meson.build` post_install.
- [ ] 5.3 Manual verify: after install, application icon appears in launchers without manual cache refresh.

## 6. cargo-sources.json

- [ ] 6.1 Regenerate `packaging/cargo-sources.json` covering every `Cargo.toml` change accumulated through M7-M22.
- [ ] 6.2 Manual verify: karere module builds offline (no network) end-to-end.

## 7. README & CHANGELOG

- [ ] 7.1 Replace `README.md` with karere v4 content based on the v3 README plus a "Switched to CEF/Chromium 148" section.
- [ ] 7.2 Document the locked decisions in the README: hard-fork from v3, no automatic migration.
- [ ] 7.3 Prepend a `4.0.0` entry to `CHANGELOG.md` summarizing the CEF rewrite plus every M7-M22 feature.

## 8. Quality gate

- [ ] 8.1 `flatpak-builder --user --install --force-clean build-dir packaging/io.github.tobagin.karere.yml` succeeds end-to-end.
- [ ] 8.2 `flathub-quality-check` passes (icons present, appstream valid, desktop file lints clean).
- [ ] 8.3 Manual smoke-test on KDE Plasma 6 (Wayland + X11).
- [ ] 8.4 Manual smoke-test on GNOME 50 (Wayland + X11).
- [ ] 8.5 Manual smoke-test on XFCE (X11).
