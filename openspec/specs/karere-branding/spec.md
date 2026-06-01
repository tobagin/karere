# karere-branding Specification

## Purpose

Defines the Karere v4 fork identity: Cargo package metadata, binary name,
GApplication app-id, GSettings schema id, desktop / AppStream basenames,
gresource resource prefix, GPL-3.0-or-later licensing, the verbatim
karere v3 asset inheritance (icons, sounds, blueprints, gschema), the
`Karere*` Rust type prefix, and the flatpak manifest naming.

## Requirements

### Requirement: Karere application identity

The project SHALL identify itself as `karere` / `io.github.tobagin.karere` across all metadata surfaces: Cargo package name, binary name, GApplication app-id, GSettings schema id, desktop file basename, AppStream metainfo basename, and gresource resource path prefix.

#### Scenario: Cargo package metadata

- **WHEN** `cargo metadata --format-version=1` is run at the project root
- **THEN** the package `name` field equals `karere`
- **AND** the package `version` field equals `4.0.0-dev`
- **AND** the package `license` field equals `GPL-3.0-or-later`
- **AND** the package declares a single binary target named `karere`

#### Scenario: GApplication registration uses karere app-id

- **WHEN** the application starts and registers with the session bus
- **THEN** `g_application_get_application_id()` returns `io.github.tobagin.karere`
- **AND** the gresource bundle is registered under the path `/io/github/tobagin/karere/`

#### Scenario: Desktop and metainfo basenames

- **WHEN** the flatpak is installed
- **THEN** `/app/share/applications/io.github.tobagin.karere.desktop` exists
- **AND** `/app/share/metainfo/io.github.tobagin.karere.metainfo.xml` exists
- **AND** `/app/share/glib-2.0/schemas/io.github.tobagin.karere.gschema.xml` is compiled into the schema cache

### Requirement: Karere visual assets shipped verbatim from v3

The project SHALL ship the karere v3 icon set, sound files, and UI blueprint sources copied byte-for-byte from `/home/tobagin/Projects/karere`, with no value edits.

#### Scenario: Hicolor icons installed

- **WHEN** the flatpak is installed
- **THEN** `/app/share/icons/hicolor/scalable/apps/io.github.tobagin.karere.svg` is present
- **AND** every size directory present in karere v3's `data/icons/hicolor/` is also present in the installed flatpak
- **AND** `gtk-update-icon-cache` runs successfully during install

#### Scenario: Notification sounds installed

- **WHEN** the flatpak is installed
- **THEN** all five `.oga` sound files from karere v3's `data/sounds/` are installed under `/app/share/karere/sounds/`

#### Scenario: Blueprint sources present in source tree

- **WHEN** inspecting `data/ui/` in the project worktree
- **THEN** every `.blp` file from karere v3's `data/ui/` exists with identical content (byte-for-byte)

### Requirement: GPL-3.0-or-later license

The project SHALL be licensed under GPL-3.0-or-later, matching the karere assets it incorporates.

#### Scenario: LICENSE file matches karere

- **WHEN** comparing the project's `LICENSE` file with `/home/tobagin/Projects/karere/LICENSE`
- **THEN** the two files are byte-identical

#### Scenario: Cargo manifest declares GPL-3.0-or-later

- **WHEN** parsing `Cargo.toml`
- **THEN** the `license` field equals exactly `GPL-3.0-or-later`

### Requirement: Karere gschema copied verbatim

The project SHALL copy `io.github.tobagin.karere.gschema.xml.in` from karere v3 unchanged. No key may be added, removed, renamed, or have its default value altered in this milestone.

#### Scenario: gschema byte-identical to v3

- **WHEN** diffing `data/io.github.tobagin.karere.gschema.xml.in` against `/home/tobagin/Projects/karere/data/io.github.tobagin.karere.gschema.xml.in`
- **THEN** the diff is empty

### Requirement: Rust types use Karere prefix

The application SHALL expose its primary Rust types under the `Karere` prefix.

#### Scenario: Renamed types compile

- **WHEN** `cargo build` runs
- **THEN** types `KarereApplication`, `KarereWindow`, and `KarereWebView` are defined and the build succeeds
- **AND** no type named `ShellApplication`, `ShellWindow`, or `CefGtkArea` remains in the source tree

### Requirement: Flatpak manifest uses karere identifiers

The flatpak manifest SHALL be named `io.github.tobagin.karere.yml`, declare app-id `io.github.tobagin.karere`, and produce an installable bundle.

#### Scenario: Manifest builds and installs

- **WHEN** running `flatpak-builder --user --install --force-clean build-dir packaging/io.github.tobagin.karere.yml`
- **THEN** the build completes successfully
- **AND** `flatpak list --app` shows `io.github.tobagin.karere`

#### Scenario: Manifest preserves CEF library path

- **WHEN** inspecting the manifest `finish-args`
- **THEN** `LD_LIBRARY_PATH` is set to a value containing `/app/lib/cef`
