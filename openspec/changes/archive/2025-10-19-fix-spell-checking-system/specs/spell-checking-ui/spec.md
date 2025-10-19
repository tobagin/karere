# Spec: Spell Checking UI

## Overview

The Spell Checking UI provides user interface for configuring spell checking, selecting languages from available dictionaries, and viewing spell checking status.

## ADDED Requirements

### Requirement: Enable/Disable Spell Checking

The UI MUST provide a switch to enable or disable spell checking.

The switch MUST be automatically disabled if no dictionaries are available.

#### Scenario: Enable spell checking with dictionaries available

**GIVEN** spell checking is currently disabled
**AND** at least one dictionary is available
**WHEN** the user toggles the "Enable Spell Checking" switch
**THEN** the `spell-checking-enabled` setting MUST be set to `true`
**AND** the spell checking language controls MUST become visible

#### Scenario: Disable spell checking

**GIVEN** spell checking is currently enabled
**WHEN** the user toggles the "Enable Spell Checking" switch off
**THEN** the `spell-checking-enabled` setting MUST be set to `false`
**AND** the spell checking language controls SHOULD become hidden or grayed out

#### Scenario: Switch disabled when no dictionaries available

**GIVEN** no dictionaries are available in the system
**WHEN** the preferences dialog spell checking page is displayed
**THEN** the "Enable Spell Checking" switch MUST be disabled (grayed out)
**AND** the switch subtitle MUST indicate "No dictionaries available"
**AND** a help message MUST be displayed explaining how to install dictionaries

### Requirement: Auto-Detect Language Toggle

The UI MUST provide a switch to enable automatic language detection from system locale.

The switch MUST show status of whether a dictionary is available for the system locale.

#### Scenario: Enable auto-detect with dictionary available

**GIVEN** auto-detect is currently disabled
**AND** a dictionary is available for the system locale
**WHEN** the user toggles the "Auto-detect Language" switch
**THEN** the `spell-checking-auto-detect` setting MUST be set to `true`
**AND** the manual language selection controls MUST become disabled
**AND** a label MUST show which language was auto-detected (e.g., "Auto: en_US")

#### Scenario: Auto-detect with no matching dictionary

**GIVEN** the system locale is `ja_JP.UTF-8`
**AND** no Japanese dictionaries are available
**WHEN** the spell checking page is displayed with auto-detect enabled
**THEN** a warning message MUST be shown: "Auto-detect unavailable for system language (ja_JP)"
**AND** the auto-detect switch MUST remain functional
**AND** the user MUST be prompted to select languages manually

### Requirement: Dictionary Status Display

The UI MUST display the number and status of available dictionaries.

#### Scenario: Show dictionary count

**GIVEN** 15 dictionaries are available
**WHEN** the spell checking preferences page is displayed
**THEN** a status row MUST display "15 dictionaries available"
**AND** the status MUST use a success/positive visual indicator

#### Scenario: No dictionaries available warning

**GIVEN** 0 dictionaries are available
**WHEN** the spell checking preferences page is displayed
**THEN** a status row MUST display "No dictionaries available"
**AND** the status MUST use a warning/attention visual indicator
**AND** an info message MUST be displayed with installation instructions

#### Scenario: Limited dictionaries available info

**GIVEN** 1 dictionary is available
**WHEN** the spell checking preferences page is displayed
**THEN** a status row MUST display "1 dictionary available"
**AND** an info message SHOULD suggest installing additional dictionaries

### Requirement: Language Selection from Available Dictionaries

The UI MUST allow selecting languages only from available dictionaries, replacing manual text entry.

#### Scenario: Select language from dropdown

**GIVEN** dictionaries for `en_US`, `de_DE`, and `fr_FR` are available
**AND** spell checking is enabled
**AND** auto-detect is disabled
**WHEN** the user opens the "Add Language" dropdown
**THEN** the dropdown MUST show only `["de_DE", "en_US", "fr_FR"]` (alphabetically sorted)
**AND** each option MUST display a user-friendly name (e.g., "English (United States)")
**AND** languages already selected MUST be disabled or hidden in the dropdown

#### Scenario: Add language from selection

**GIVEN** the language dropdown shows available languages
**AND** no languages are currently selected
**WHEN** the user selects `de_DE` from the dropdown
**THEN** `de_DE` MUST be added to the `spell-checking-languages` setting
**AND** the selected language MUST appear in the "Current Languages" list
**AND** the dropdown MUST no longer show `de_DE` as an option

#### Scenario: No languages available to add

**GIVEN** all available dictionaries are already selected
**WHEN** the "Add Language" dropdown is displayed
**THEN** the dropdown MUST be disabled or show "No languages available to add"

### Requirement: Current Languages Display

The UI MUST display currently enabled spell checking languages with the ability to remove them.

#### Scenario: Display multiple selected languages

**GIVEN** the user has selected `en_US` and `fr_FR`
**AND** spell checking is enabled
**WHEN** the spell checking preferences page is displayed
**THEN** the "Current Languages" list MUST show:
- "English (United States)" with a remove button
- "French (France)" with a remove button

#### Scenario: Remove a language

**GIVEN** the "Current Languages" list shows `en_US` and `de_DE`
**WHEN** the user clicks the remove button next to `de_DE`
**THEN** `de_DE` MUST be removed from the `spell-checking-languages` setting
**AND** `de_DE` MUST be removed from the current languages display
**AND** `de_DE` MUST reappear in the "Add Language" dropdown

#### Scenario: Display auto-detected language

**GIVEN** auto-detect is enabled
**AND** the system locale matches `pt_BR`
**WHEN** the spell checking preferences page is displayed
**THEN** the "Current Languages" display MUST show "Auto: Portuguese (Brazil)"
**AND** no remove button MUST be shown (auto-detected language cannot be manually removed)
**AND** the manual language list MUST be disabled/hidden

#### Scenario: Display when auto-detect failed

**GIVEN** auto-detect is enabled
**AND** no dictionary matches the system locale
**WHEN** the spell checking preferences page is displayed
**THEN** the "Current Languages" display MUST show "Auto-detect: No dictionary found for system language"
**AND** a warning indicator MUST be displayed
**AND** the user SHOULD be prompted to disable auto-detect and select manually

### Requirement: User-Friendly Language Names

The UI MUST display human-readable language names instead of locale codes.

#### Scenario: Map locale code to language name

**GIVEN** the dictionary `en_US` is available
**WHEN** displaying the language in the UI
**THEN** the UI MUST show "English (United States)" not "en_US"
**AND** the locale code `en_US` MAY be shown as secondary text for advanced users

#### Scenario: Handle unknown locale codes gracefully

**GIVEN** a dictionary with code `xyz_ZZ` is available (unknown language)
**WHEN** displaying the language in the UI
**THEN** the UI MUST show the raw code "xyz_ZZ"
**AND** a note SHOULD indicate "(Unknown language)"

### Requirement: Help and Information

The UI MUST provide helpful information about spell checking configuration.

#### Scenario: Display help when no dictionaries available

**GIVEN** 0 dictionaries are available
**WHEN** the spell checking preferences page is displayed
**THEN** an information box MUST be shown with:
- "Spell checking requires hunspell dictionaries"
- Installation instructions for the user's distribution
- Link to documentation or support

#### Scenario: Display help for Flatpak users

**GIVEN** the app is running as a Flatpak
**AND** fewer than 5 dictionaries are available
**WHEN** the spell checking preferences page is displayed
**THEN** an info message MAY be shown suggesting:
- "Additional dictionaries can be installed from your system package manager"
- Instructions on granting Flatpak access to system dictionaries

### Requirement: Visual Feedback

The UI MUST provide clear visual feedback about spell checking status and configuration.

#### Scenario: Active spell checking indicator

**GIVEN** spell checking is enabled
**AND** 2 languages are configured
**WHEN** the preferences dialog is displayed
**THEN** the spell checking row MUST show a positive status indicator
**AND** the subtitle MAY show "Active with 2 languages"

#### Scenario: Inactive spell checking indicator

**GIVEN** spell checking is disabled
**WHEN** the preferences dialog is displayed
**THEN** the spell checking row MUST show a neutral/disabled status indicator
**AND** related controls MUST be visually de-emphasized (grayed out)

#### Scenario: Warning when enabled but not functional

**GIVEN** spell checking is enabled in settings
**AND** no languages are selected or available
**WHEN** the preferences dialog is displayed
**THEN** a warning indicator MUST be shown
**AND** a message MUST explain "Spell checking enabled but no languages configured"

### Requirement: Accessibility

The UI MUST be fully accessible for keyboard navigation and screen readers.

#### Scenario: Keyboard navigation of language dropdown

**GIVEN** the "Add Language" dropdown is focused
**WHEN** the user presses Enter or Space
**THEN** the dropdown MUST open
**AND** arrow keys MUST navigate through available languages
**AND** Enter MUST select the focused language

#### Scenario: Screen reader announcements

**GIVEN** a screen reader is active
**WHEN** the user enables spell checking
**THEN** the screen reader MUST announce "Spell checking enabled"
**AND** when a language is added, the screen reader MUST announce "Language [name] added"

#### Scenario: Focus management

**GIVEN** the user adds a language via the dropdown
**WHEN** the language is added to the list
**THEN** focus MUST move to the newly added language item or remain on the dropdown
**AND** focus MUST NOT be lost or jump unexpectedly

### Requirement: Settings Persistence

The UI MUST accurately reflect and persist spell checking settings.

#### Scenario: Load existing settings on dialog open

**GIVEN** the user has previously configured spell checking with `en_US` and `de_DE`
**WHEN** the preferences dialog is opened
**THEN** the spell checking enabled switch MUST be on
**AND** the current languages list MUST show `en_US` and `de_DE`
**AND** the displayed state MUST match the saved settings exactly

#### Scenario: Save changes immediately

**GIVEN** the user adds language `fr_FR`
**WHEN** the language is selected from the dropdown
**THEN** the `spell-checking-languages` setting MUST be updated immediately
**AND** the change MUST be persisted to GSettings
**AND** the change MUST take effect in the WebView without requiring restart

#### Scenario: Handle settings changes from external sources

**GIVEN** the preferences dialog is open
**WHEN** spell checking settings are changed by another part of the application or externally
**THEN** the UI MUST update to reflect the new settings
**AND** the displayed state MUST remain synchronized with the settings

## REMOVED Requirements

None (no existing requirements are removed).
