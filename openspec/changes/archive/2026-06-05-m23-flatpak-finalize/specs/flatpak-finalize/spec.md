## ADDED Requirements

### Requirement: Manifest enables appstream-compose with populated metainfo
The Flatpak manifest `packaging/io.github.tobagin.karere.yml` SHALL set `appstream-compose: true` for the karere module, and the project SHALL ship a fully populated `data/io.github.tobagin.karere.metainfo.xml.in.in` containing `<id>io.github.tobagin.karere</id>`, `<metadata_license>CC0-1.0</metadata_license>`, `<project_license>GPL-3.0-or-later</project_license>`, `<name>`, `<summary>`, `<description>`, `<launchable type="desktop-id">io.github.tobagin.karere.desktop</launchable>`, `<screenshots>`, a `<release version="4.0.0">` entry, and `<content_rating type="oars-1.1">` ratings ported from karere v3.

#### Scenario: flatpak-builder runs appstream-compose successfully
- **WHEN** `flatpak-builder --user --install --force-clean build-dir packaging/io.github.tobagin.karere.yml` is invoked
- **THEN** the build reaches the `appstream-compose` step without errors
- **AND** the resulting bundle contains a compiled appstream catalog under `/app/share/app-info/`

#### Scenario: metainfo lists 4.0.0 release notes
- **WHEN** the installed metainfo is queried via `appstreamcli validate /app/share/metainfo/io.github.tobagin.karere.metainfo.xml`
- **THEN** validation passes
- **AND** the file contains a `<release version="4.0.0">` element summarizing the CEF/Chromium 148 migration

### Requirement: Desktop entry registers WhatsApp scheme handler
The application SHALL ship `data/io.github.tobagin.karere.desktop.in.in` containing `Name=Karere`, a `Comment=`, `Categories=Network;InstantMessaging;Chat;`, `Keywords=WhatsApp;Chat;Messenger;`, `Icon=io.github.tobagin.karere`, `Exec=karere %U`, and `MimeType=x-scheme-handler/whatsapp;`.

#### Scenario: Desktop file lints cleanly
- **WHEN** `desktop-file-validate /app/share/applications/io.github.tobagin.karere.desktop` runs
- **THEN** the command exits with status 0 and emits no warnings

#### Scenario: whatsapp:// URLs route to Karere
- **WHEN** the user runs `xdg-open whatsapp://send?text=hello` with Karere installed and selected as the default handler
- **THEN** the Karere binary is launched with the URL passed as `%U`

### Requirement: Manifest finish-args grant StatusNotifierWatcher; autostart via Background portal
The karere module's `finish-args` SHALL include `--talk-name=org.kde.StatusNotifierWatcher` in addition to the existing M6 baseline. Autostart SHALL be handled by the XDG Background portal (the `ashpd` `background` feature), NOT by direct filesystem access: the manifest SHALL NOT add `--filesystem=xdg-config/autostart:create` or `--talk-name=org.freedesktop.portal.Desktop`, because portal busnames are granted to every sandboxed app automatically and `flatpak-builder-lint` flags both as errors (`finish-args-unnecessary-xdg-config-autostart-create-access`, `finish-args-portal-talk-name`).

#### Scenario: Tray client registers on KDE Plasma
- **WHEN** Karere runs under KDE Plasma 6 with M15 tray code active
- **THEN** the `org.kde.StatusNotifierWatcher` DBus name is reachable and the tray icon appears

#### Scenario: Autostart entry created via the Background portal
- **WHEN** the user enables "Run on Startup" and the `ashpd` background portal request runs
- **THEN** the host writes the autostart `.desktop` entry on the app's behalf
- **AND** no `--filesystem=xdg-config/autostart` grant is present in `finish-args`

### Requirement: Karere module builds offline and cleans dev artifacts
The karere module SHALL build without `--share=network`, relying exclusively on `packaging/cargo-sources.json` for crate sources, and the manifest SHALL declare a `cleanup` array removing `*.la`, `*.a`, and `/app/include` while preserving `/app/lib/cef/include`.

#### Scenario: Network-isolated build succeeds
- **WHEN** flatpak-builder is invoked with the network sandbox enabled for the karere module
- **THEN** the build completes without attempting external network access

#### Scenario: Cleanup removes dev artifacts but preserves CEF headers
- **WHEN** the bundle is inspected post-build
- **THEN** no `*.la` or `*.a` files exist under `/app`
- **AND** no headers exist under `/app/include`
- **AND** CEF headers under `/app/lib/cef/include` are still present

### Requirement: Icons cache regenerated at install time
The `meson.build` post_install SHALL set `gtk_update_icon_cache: true` so that the hicolor icon tree under `data/icons/hicolor/**` is registered with `gtk-update-icon-cache` on installation.

#### Scenario: Icon cache updated on install
- **WHEN** the Flatpak is installed
- **THEN** `gtk-update-icon-cache` has been invoked for `/app/share/icons/hicolor`
- **AND** the application icon appears in launchers without manual cache refresh

### Requirement: cargo-sources.json reflects post-M22 dependency closure
The file `packaging/cargo-sources.json` SHALL be regenerated to cover every `Cargo.toml` and `Cargo.lock` change introduced between M7 and M22 inclusive.

#### Scenario: Offline build resolves every crate
- **WHEN** the karere module builds against the vendored sources only
- **THEN** every transitive crate dependency is satisfied from `cargo-sources.json`
- **AND** no "failed to download" or "no matching package" errors occur

### Requirement: README and CHANGELOG reflect 4.0.0 rewrite
The project SHALL replace `README.md` with a karere v4 document based on the v3 README and adding a "Switched to CEF/Chromium 148" section plus the locked decisions (hard-fork, no migration from v3). `CHANGELOG.md` SHALL have a `4.0.0` entry prepended summarizing the CEF migration and every feature added between M1 and M22.

#### Scenario: README documents the v4 rewrite and locked decisions
- **WHEN** the README is opened
- **THEN** it states that v4 is a hard fork built on CEF/Chromium 148
- **AND** it states that there is no automatic migration from v3

#### Scenario: CHANGELOG 4.0.0 entry is the topmost release
- **WHEN** `CHANGELOG.md` is read
- **THEN** the first release heading is `4.0.0`
- **AND** the entry summarizes the CEF migration plus M7-M22 feature work
