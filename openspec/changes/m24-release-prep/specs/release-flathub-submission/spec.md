## ADDED Requirements

### Requirement: Flathub repo receives copies of the in-tree packaging files

The `flathub/io.github.tobagin.karere` repository SHALL contain, at the root of its submission branch, files that are byte-identical to `packaging/io.github.tobagin.karere.yml` and `packaging/cargo-sources.json` from this repository at the `v4.0.0` tag.

#### Scenario: Manifest copied verbatim

- **WHEN** `io.github.tobagin.karere.yml` in the Flathub submission branch is compared to `packaging/io.github.tobagin.karere.yml` at the `v4.0.0` tag
- **THEN** the two files are byte-identical.

#### Scenario: Cargo sources copied verbatim

- **WHEN** `cargo-sources.json` in the Flathub submission branch is compared to `packaging/cargo-sources.json` at the `v4.0.0` tag
- **THEN** the two files are byte-identical.

### Requirement: Flathub submission PR is opened with the locked title and licensing note

A pull request SHALL be opened against `flathub/flathub` (or the appropriate Flathub new-application repo) with the title "Add Karere 4.0.0" and a body that links the upstream `v4.0.0` tag and explicitly states that CEF/Chromium is BSD-licensed with LGPL libraries and that `libcef.so` is dynamically loaded.

#### Scenario: PR title

- **WHEN** the submission PR is created
- **THEN** its title equals the string "Add Karere 4.0.0".

#### Scenario: PR body links the upstream tag

- **WHEN** the submission PR body is read
- **THEN** it contains a URL pointing to the `v4.0.0` tag in this repository.

#### Scenario: PR body states CEF licensing

- **WHEN** the submission PR body is read
- **THEN** it mentions Chromium's BSD license, the presence of LGPL libraries, and that `libcef.so` is dynamically loaded.

### Requirement: Smoke-test matrix passes before Flathub merge

Before the Flathub submission PR is merged, the v4.0.0 build SHALL be smoke-tested on each of the following targets: KDE Plasma 6 on Wayland x86_64, KDE Plasma 6 on X11 x86_64, GNOME 50 on Wayland x86_64, GNOME 50 on X11 x86_64, XFCE on X11 x86_64, and an aarch64 KDE target (bare-metal, CI, or QEMU).

#### Scenario: All targets exercised

- **WHEN** the publish gate is evaluated
- **THEN** there is a recorded smoke-test pass for each of the six matrix targets.

#### Scenario: Each smoke test exercises the core flows

- **WHEN** a smoke test is run on any target
- **THEN** the tester records pass/fail for: pairing an account, sending and receiving a text message, sending and receiving media, accepting a call, close-to-tray, and clean quit.

#### Scenario: Matrix gate enforced

- **WHEN** any target's smoke test fails or is missing
- **THEN** the Flathub submission PR is not merged until the failure is resolved or the matrix is re-run successfully.

### Requirement: Upgrade path produces the migration dialog exactly once

The published Flathub build SHALL, when installed over an existing v3.1.1 installation, present the migration dialog from `release-migration-notice` exactly once across all subsequent launches.

#### Scenario: First launch after upgrade

- **WHEN** v3.1.1 is upgraded to v4.0.0 via Flathub and the application is launched for the first time
- **THEN** the migration dialog is shown.

#### Scenario: Second launch after upgrade

- **WHEN** the application is launched a second time after the migration dialog has been acknowledged
- **THEN** no migration dialog is shown.
