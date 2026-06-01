# spellcheck-ui Specification

## Purpose
Surface the Chromium spellcheck language picker in the GTK4/libadwaita UI: a headerbar dropdown over the supported languages with star-pin favorites, a Preferences mirror (hosted by M22), and persistence of the active selection and favorites in GSettings.

## Requirements
### Requirement: Headerbar language dropdown
The main window SHALL expose a `gtk::DropDown` in the headerbar populated from a sorted view of `KNOWN_LANGUAGES`, allowing the user to select the active spellcheck language.

#### Scenario: Selection persists to GSettings
- **WHEN** the user selects "Portuguese (Brazil)" in the dropdown
- **THEN** `spell-checking-languages` is updated to `['pt-BR']` and `KarereWebView::set_spellcheck_languages(["pt-BR"], true)` is invoked (live switch, no reload)

#### Scenario: Dropdown reflects current setting on startup
- **WHEN** the app starts with `spell-checking-languages=['en-US']`
- **THEN** the dropdown's active row is "English (United States)"

### Requirement: Star-pin favorites
The dropdown row factory SHALL render a `gtk::ToggleButton` with a star icon on each row; toggling the star pins or unpins the language. Pinned languages SHALL render at the top of the dropdown and persist across launches in the `favorite-spell-check-languages` GSettings key.

#### Scenario: Pinning a language persists
- **WHEN** the user toggles the star on "English (United Kingdom)" and restarts the app
- **THEN** `favorite-spell-check-languages` contains `'en-GB'` and "English (United Kingdom)" appears at the top of the dropdown on next launch

#### Scenario: Unpinning removes from favorites
- **WHEN** the user toggles the star off on a pinned language
- **THEN** that BCP-47 code is removed from `favorite-spell-check-languages` and the row returns to its sorted position

### Requirement: Preferences mirror (deferred to M22)
The Preferences page mirror is DEFERRED: `app.preferences` is currently an M22 stub with no loaded preferences dialog. When M22 builds the dialog it SHALL mirror the headerbar language list. The originally planned "Changing language reloads the page" notice is OBSOLETE — the live-switch design (see spellcheck-chromium) performs no reload — and SHALL be omitted or reworded.

#### Scenario: Mirror lands with M22
- **WHEN** M22 implements the preferences dialog
- **THEN** its spellcheck section shows the same language list as the headerbar dropdown, with no "reloads the page" notice

### Requirement: GSettings schema additions
The gschema SHALL declare keys `enable-spell-checking` (boolean, default `true`), `auto-detect-language` (boolean, default `true`), `spell-checking-languages` (`as`, default `[]`), and `favorite-spell-check-languages` (`as`, default `[]`). (These keys already existed in the schema; M16 reuses them rather than adding renamed duplicates.)

#### Scenario: Defaults on first run
- **WHEN** the app launches with no prior settings
- **THEN** `enable-spell-checking` is `true`, `auto-detect-language` is `true`, `spell-checking-languages` is `[]`, and `favorite-spell-check-languages` is `[]`
