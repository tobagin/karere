## 1. Manifest hardening

- [x] 1.1 Flip `appstream-compose: false` to `true` for the karere module in `packaging/io.github.tobagin.karere.yml`.
- [x] 1.2 Add `--talk-name=org.kde.StatusNotifierWatcher` to `finish-args`. (already present from M15)
- [x] 1.3 ~~Add `--filesystem=xdg-config/autostart:create` to `finish-args`.~~ REVISED: not added. `flatpak-builder-lint` flags it (`finish-args-unnecessary-xdg-config-autostart-create-access`) — autostart is handled host-side by the XDG Background portal (`ashpd` `background` feature), so the sandbox never writes the autostart entry. Also dropped the redundant `--talk-name=org.freedesktop.portal.Desktop` (portals auto-granted). See updated flatpak-finalize spec + design.
- [x] 1.4 Remove any leftover `--share=network` build-arg from the karere module. (none existed; the `--share=network` in finish-args is the runtime grant the web client needs — kept)
- [x] 1.5 Add a `cleanup` array dropping `*.la`, `*.a`, and `/app/include` while preserving `/app/lib/cef/include`. (`/lib/cef/include` is not under `/include`, so it survives)
- [x] 1.6 Confirm the `cef-binaries` module is unchanged from M6 (no edits required).

## 2. Debug extension

- [x] 2.1 Add an `add-extensions` block keyed `io.github.tobagin.karere.Debug` with `directory: lib/debug`, `autodelete: 'true'`, `no-autodownload: 'true'`.
- [x] 2.2 Ensure the karere module's build pipeline emits detached debug symbols under `/app/lib/debug/<binary>.debug` (`Cargo.toml [profile.release]` now sets `strip = false` + `debug = 1`; flatpak-builder strips the installed binary and splits symbols into the Debug ref).
- [x] 2.3 Manual verify: Debug extension installs (built + installed by the 8.1 `--install` run; `runtime/io.github.tobagin.karere.Debug` carries `bin/karere.debug`, `not stripped`, with DWARF sections).
- [x] 2.4 Manual verify: symbolicated crash backtrace. `flatpak-coredumpctl io.github.tobagin.karere` (sandbox-aware) on a deliberate SIGSEGV core loaded the Debug ext symbols — `Reading symbols from /usr/lib/debug/app/bin/karere.debug` — and resolved `karere::` frames with `file:line`, no user-side debuginfod. (Plain host `coredumpctl debug` won't resolve; symbols live in the flatpak Debug ext, mounted only inside the sandbox.)

## 3. Metainfo template

- [x] 3.1 Port karere v3's metainfo content into `data/io.github.tobagin.karere.metainfo.xml.in`: `<id>`, `<metadata_license>CC0-1.0</metadata_license>`, `<project_license>GPL-3.0-or-later</project_license>`, `<name>`, `<summary>`. (already ported in M7; kept the Flathub-compliant `<summary>Chat on WhatsApp</summary>` rather than "…for Linux", which the Flathub linter flags)
- [x] 3.2 Write the `<description>` paragraphs from v3 and append a "Now built on CEF/Chromium 148" paragraph.
- [x] 3.3 Add `<launchable type="desktop-id">io.github.tobagin.karere.desktop</launchable>`. (already present)
- [x] 3.4 Copy v3 screenshots into `data/screenshots/` and reference them from `<screenshots>`. (already present)
- [x] 3.5 Add `<release version="4.0.0">` with notes summarizing CEF migration plus M7-M22 features.
- [x] 3.6 Copy `<content_rating type="oars-1.1">` ratings verbatim from v3. (already present)
- [x] 3.7 Manual verify: `appstreamcli validate` passes for the composed metainfo. (validated on substituted source: "✔ Validation was successful", infos only — pre-existing lowercase v3 release descriptions)

## 4. Desktop template

- [x] 4.1 Populate `data/io.github.tobagin.karere.desktop.in` with `[Desktop Entry]`, `Name`, `Comment`, `Categories=Network;InstantMessaging;Chat;`, `Keywords`, `Icon`, `Exec=karere %U`, `MimeType=x-scheme-handler/whatsapp;`. (already populated)
- [x] 4.2 Manual verify: `desktop-file-validate` passes with no warnings. (validated on substituted source: exit 0, no warnings)
- [x] 4.3 Manual verify: `xdg-open whatsapp://send?text=hello` launches Karere when set as default handler. (confirmed; default handler is `io.github.tobagin.karere.desktop`)

## 5. Icons

- [x] 5.1 Confirm M7 has already copied `data/icons/hicolor/**` from karere v3. (present: 48-512px + scalable + symbolic)
- [x] 5.2 Restore `gtk_update_icon_cache: true` in `meson.build` post_install. (already set)
- [x] 5.3 Manual verify: after install, application icon appears in launchers without manual cache refresh. (confirmed; icons exported to `~/.local/share/flatpak/exports/share/icons/hicolor/**/apps/io.github.tobagin.karere.*`)

## 6. cargo-sources.json

- [x] 6.1 Regenerate `packaging/cargo-sources.json` covering every `Cargo.toml` change accumulated through M7-M22. (regenerated from current `Cargo.lock`; 639 entries, already current — no diff)
- [x] 6.2 Manual verify: karere module builds offline (no network) end-to-end. (built with `--disable-download --disable-updates`; exit 0, no "failed to download"/"no matching package" — all crate + CEF sources came from `cargo-sources.json` / cache)

## 7. README & CHANGELOG

- [x] 7.1 Replace `README.md` with karere v4 content based on the v3 README plus a "Switched to CEF/Chromium 148" section. (added v4 section; updated Architecture, Known Limitations/video, acknowledgments)
- [x] 7.2 Document the locked decisions in the README: hard-fork from v3, no automatic migration.
- [x] 7.3 Prepend a `4.0.0` entry to `CHANGELOG.md` summarizing the CEF rewrite plus every M7-M22 feature.

## 8. Quality gate

- [x] 8.1 `flatpak-builder --user --install --force-clean --repo=repo build-dir packaging/io.github.tobagin.karere.yml` succeeds end-to-end (exit 0; appstream-compose ran, base app + Debug + Locale refs built and installed).
- [x] 8.2 `flatpak-builder-lint` (org.flatpak.Builder) on manifest + builddir + repo: clean except (a) `finish-args-home-filesystem-access` — pre-approved Flathub exception, app is live; (b) `appstream-external-screenshot-url` / `…not-mirrored-in-ostree` — screenshots are mirrored by Flathub CI at submission, cannot pass on a local build. All three M23-introduced lint errors fixed (quoted-bool extension props → unquoted; dropped unnecessary autostart-fs + portal talk-name finish-args).
- [x] 8.3 Manual smoke-test on KDE Plasma 6 (Wayland + X11). (user-confirmed)
- [x] 8.4 Manual smoke-test on GNOME 50 (Wayland + X11). (user-confirmed)
- [x] 8.5 Manual smoke-test on XFCE (X11). (user-confirmed)
