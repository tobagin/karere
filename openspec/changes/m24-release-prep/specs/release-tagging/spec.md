## ADDED Requirements

### Requirement: Cargo and Meson versions match the tagged release

The crate version in `Cargo.toml` and the `project()` version in `meson.build` SHALL both read `4.0.0` at the tagged release commit, with no `-dev` suffix on either.

#### Scenario: Cargo.toml version at tag

- **WHEN** `Cargo.toml` is read at the commit pointed to by `v4.0.0`
- **THEN** the `[package]` `version` field equals `4.0.0` exactly, with no pre-release or build metadata suffix.

#### Scenario: meson.build version at tag

- **WHEN** `meson.build` is read at the commit pointed to by `v4.0.0`
- **THEN** the `project()` invocation declares `version: '4.0.0'`.

#### Scenario: Versions stay in lockstep

- **WHEN** the version in `Cargo.toml` is changed
- **THEN** the version in `meson.build` is changed in the same commit to the same value.

### Requirement: Metainfo carries a 4.0.0 release entry covering M7-M22 features

`data/io.github.tobagin.karere.metainfo.xml.in.in` SHALL contain a `<release version="4.0.0" date="YYYY-MM-DD">` element whose description enumerates every M7-M22 user-facing feature in plain language: engine switch to CEF/Chromium 148, multi-account auto-discovery, tray, portal notifications, spell-check, downloads, find-in-page, DevTools, zoom-per-account, paste bridge, accessibility, mobile responsive UI, and the preferences dialog.

#### Scenario: Release element present

- **WHEN** the metainfo template is parsed at the tagged release commit
- **THEN** a `<release>` element with `version="4.0.0"` and a non-placeholder ISO-8601 `date` attribute exists.

#### Scenario: Release notes cover the milestones

- **WHEN** the description body of the `4.0.0` release element is read
- **THEN** it mentions, in user-facing language, the engine switch, multi-account, tray, notifications, spell-check, downloads, find-in-page or DevTools, zoom, mobile responsive UI, preferences, and accessibility.

#### Scenario: Date matches the tag

- **WHEN** the release date attribute is compared against the date of the `v4.0.0` tag
- **THEN** the two dates are equal.

### Requirement: CHANGELOG 4.0.0 entry uses the locked headline and includes the migration note

`CHANGELOG.md` SHALL include a finalized `4.0.0` entry whose first headline reads, verbatim, "Switched rendering engine from WebKitGTK to CEF (Chromium 148); account identity now auto-discovered from WhatsApp Web; tray + portal notifications + spell-check via Chromium.", and SHALL include the locked migration note explaining that v3 users must re-link their account.

#### Scenario: Locked headline present verbatim

- **WHEN** the `4.0.0` entry is read
- **THEN** it contains the exact string "Switched rendering engine from WebKitGTK to CEF (Chromium 148); account identity now auto-discovered from WhatsApp Web; tray + portal notifications + spell-check via Chromium."

#### Scenario: Migration note present

- **WHEN** the `4.0.0` entry is read
- **THEN** it includes a paragraph stating that users upgrading from v3 must re-link their WhatsApp account and that chat history remains on the phone.

### Requirement: README documents the v4 engine and locked decisions

`README.md` SHALL be replaced with v4 content consisting of the v3 README's structure, a "Now built on CEF/Chromium 148" section, and a list of the locked architectural decisions.

#### Scenario: CEF/Chromium section present

- **WHEN** `README.md` is read at the tagged release commit
- **THEN** it contains a section whose heading text includes "CEF" and "Chromium 148".

#### Scenario: Locked decisions list present

- **WHEN** `README.md` is read at the tagged release commit
- **THEN** it includes a bulleted or numbered list summarizing the locked architectural decisions taken across M1-M23.

### Requirement: v4.0.0 git tag exists at a green-CI commit

A signed-or-annotated tag named `v4.0.0` SHALL exist on the repository, and the commit it points to SHALL have passed the project's main CI pipeline.

#### Scenario: Tag exists

- **WHEN** `git tag` is listed
- **THEN** `v4.0.0` appears.

#### Scenario: Tag points to a green commit

- **WHEN** the CI status of the commit referenced by `v4.0.0` is queried
- **THEN** the main CI pipeline reports success.
