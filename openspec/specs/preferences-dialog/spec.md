# preferences-dialog Specification

## Purpose

Defines the application's preferences experience: an `AdwPreferencesDialog`
subclass built from a blueprint composite template with six pages — General,
Notifications, Downloads, Spellcheck, Privacy, Accessibility — plus an About
footer, presented via the `app.preferences` GAction. Each page binds the
relevant GSettings keys and wires them to the live application behavior.

## Requirements

### Requirement: Preferences dialog presents six pages plus an About footer
The application SHALL provide an `AdwPreferencesDialog` subclass built from a blueprint composite template (`data/ui/preferences.blp`) containing six pages — General, Notifications, Downloads, Spellcheck, Privacy, Accessibility — and an About footer surfacing the build version and a license button. The dialog SHALL be presented when the `app.preferences` GAction is activated, parented to the active application window.

#### Scenario: Activating app.preferences presents the dialog
- **WHEN** the user activates the `app.preferences` action (via menu or accelerator)
- **THEN** the application constructs a `KarerePreferencesDialog` bound to the active window
- **AND** the dialog appears modal over the window with the General page selected

#### Scenario: Dialog exposes all six pages
- **WHEN** the preferences dialog is open
- **THEN** the page-switcher shows exactly six pages titled General, Notifications, Downloads, Spellcheck, Privacy, and Accessibility
- **AND** an About footer is visible from every page

### Requirement: General page binds theme, close-action, startup, and language keys
The General page SHALL expose rows for `theme` (dropdown system/light/dark), `close-button-action` (dropdown background/quit), `start-in-background` (switch), `run-on-startup` (switch), and the language detect-or-pick dropdown. Toggling `run-on-startup` SHALL trigger the `app.sync-autostart` action so the autostart desktop file is regenerated.

#### Scenario: Theme dropdown updates UI immediately
- **WHEN** the user changes the theme dropdown to "Dark"
- **THEN** the `theme` GSetting is written to "dark"
- **AND** the libadwaita style manager applies the dark color scheme without restarting

#### Scenario: Run-on-startup activates sync-autostart
- **WHEN** the user toggles `run-on-startup` on
- **THEN** the `run-on-startup` GSetting becomes `true`
- **AND** the `app.sync-autostart` action is activated, writing the autostart desktop file

### Requirement: Notifications page binds notification keys and explains Chromium-native popup
The Notifications page SHALL expose rows for `notifications-enabled`, `notify-messages`, `notify-sound-enabled`, `notify-sound-file` (dropdown: whatsapp, pop, alert, soft, start), and the `notify-preview-*` group. The page SHALL include an inline notice reading "Some settings only apply when Karere controls the popup" so users understand the preview settings govern future tray peek behavior rather than the Chromium-native popup.

#### Scenario: Sound dropdown writes selected file name
- **WHEN** the user selects "pop" from the sound dropdown
- **THEN** the `notify-sound-file` GSetting is written to "pop"

#### Scenario: Preview notice is visible
- **WHEN** the user views the Notifications page
- **THEN** an inline notice row reads "Some settings only apply when Karere controls the popup"

### Requirement: Downloads page provides folder picker and notification type choice
The Downloads page SHALL provide a row containing a button that opens `gtk::FileDialog::select_folder` and writes the selected path into the `download-directory` GSetting, plus rows for `notify-downloads-enabled` and `notify-download-type` (dropdown: toast/notification/both).

#### Scenario: Folder picker updates download-directory
- **WHEN** the user clicks the download-directory button and selects `/tmp/karere-dl`
- **THEN** the `download-directory` GSetting holds `/tmp/karere-dl`
- **AND** the next download started by the browser lands in that directory

#### Scenario: Notification type dropdown writes choice
- **WHEN** the user selects "notification" in the notify-download-type dropdown
- **THEN** the `notify-download-type` GSetting holds the string `notification`

### Requirement: Spellcheck page mirrors the headerbar language picker
The Spellcheck page SHALL expose (using the GSettings keys M16 actually reuses): an `enable-spell-checking` switch, an `auto-detect-language` switch, an `enable-auto-correct` switch, and a language list mirroring the headerbar dropdown — a row per Chromium-supported language (from `KNOWN_LANGUAGES`) with a star toggle that adds/removes the BCP-47 code from `favorite-spell-check-languages` (favorites sorted to the top), where selecting a language writes `spell-checking-languages` and switches the live browser via `KarereWebView::set_spellcheck_languages` (M16, no reload). The page SHALL NOT show a "reloads the page" notice — language changes are applied live.

> Auto-correct depends on the m16-1-osr-context-menu change implementing the behavior behind `enable-auto-correct`; this page only surfaces the toggle.

#### Scenario: Star-pinning a language updates favorites and reorders
- **WHEN** the user stars Portuguese (Brazil) in the language list
- **THEN** "pt-BR" is appended to `favorite-spell-check-languages`
- **AND** after closing and reopening the dialog (or restarting the application), Portuguese (Brazil) appears at the top of both the Preferences language list and the headerbar dropdown

#### Scenario: Selecting a language switches live without reload
- **WHEN** the user selects a different language on the Spellcheck page
- **THEN** `spell-checking-languages` is updated and the active browser's spellcheck switches in place (no page reload), consistent with the headerbar dropdown

#### Scenario: Auto-correct toggle binds the GSetting
- **WHEN** the user toggles the "Enable Auto-Correct" switch
- **THEN** `enable-auto-correct` is updated and honored without restarting the app

### Requirement: Privacy page lists permission registry with per-row remove and clear-all
The Privacy page SHALL render every entry in the M11 permission registry as a row showing the origin, the requested permission mask, and the stored state (granted/denied). Each row SHALL have a remove button calling the M11 per-key remove API. The page SHALL include a footer Clear-all button that empties the registry via the M11 clear API.

#### Scenario: Removing a row clears that registry entry
- **WHEN** the user clicks the remove button on the row for `https://example.com` with mask "geolocation"
- **THEN** the M11 registry no longer contains a decision for (`https://example.com`, "geolocation")

#### Scenario: Clear-all empties the registry
- **WHEN** the user clicks the Clear-all button and confirms
- **THEN** the M11 permission registry is empty
- **AND** the next site permission request triggers the M5 prompt instead of using a stored decision

### Requirement: Accessibility page binds a11y keys including webview-zoom floor
The Accessibility page SHALL expose switches for `reduce-motion`, `focus-indicators-enhanced`, and `screen-reader-opts` (with a restart-required hint subtitle), a slider bound to `webview-zoom` as an accessibility zoom floor, and a switch for `zoom-controls-headerbar`.

#### Scenario: Reduce-motion toggle propagates to GtkSettings
- **WHEN** the user toggles `reduce-motion` on
- **THEN** the `reduce-motion` GSetting becomes `true`
- **AND** the existing M19 binding sets `GtkSettings::gtk-enable-animations` to `false` without restart

#### Scenario: Screen-reader-opts row surfaces restart hint
- **WHEN** the user views the row for `screen-reader-opts`
- **THEN** the subtitle text indicates that the application must be restarted for the change to take effect

### Requirement: About footer surfaces version and license
The About footer SHALL display the application version obtained from `env!("CARGO_PKG_VERSION")` at build time and SHALL include a button that opens the bundled license document.

#### Scenario: Version string matches Cargo manifest
- **WHEN** the dialog is open
- **THEN** the About footer displays a version string equal to the `CARGO_PKG_VERSION` value used at build time
