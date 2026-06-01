# spellcheck-chromium Specification

## Purpose
Wire Chromium's built-in spellchecker into the CEF runtime from GSettings: seed the initial language at startup, switch languages live on the running browser via request-context preferences (no reload), auto-detect the user's locale mapped to the closest supported dictionary, and let Chromium lazily auto-download `.bdic` dictionaries — without bundling Hunspell.

## Requirements
### Requirement: Command-line wiring for Chromium spellchecker
The CEF runtime SHALL configure Chromium's built-in spellchecker via command-line switches in `on_before_command_line_processing`, sourced from GSettings.

#### Scenario: Spellcheck disabled via GSettings
- **WHEN** `spell-checking-enabled` is `false`
- **THEN** the runtime appends `--disable-spell-checking` to the Chromium command line and no `--spell-check-languages` switch is added

#### Scenario: Explicit language list
- **WHEN** `spell-checking-enabled` is `true` and `spell-checking-languages` contains `['en-US', 'pt-BR']`
- **THEN** the runtime appends `--spell-check-languages=en-US,pt-BR` to the Chromium command line

#### Scenario: Auto-detect from locale
- **WHEN** `spell-check-auto-detect` is `true` and `spell-checking-languages` is empty
- **THEN** the runtime derives a single BCP-47 code from `glib::language_names()[0]` via `spellcheck::parse_locale` and appends it as `--spell-check-languages=<code>`

### Requirement: Locale parsing helpers
The crate SHALL provide a `spellcheck` module ported from karere v3 with public helpers `parse_locale`, `display_name`, `region_name`, `short_code`, and a `KNOWN_LANGUAGES` static slice of `(BCP-47, friendly name)` tuples.

#### Scenario: parse_locale splits language and region
- **WHEN** `parse_locale("en_GB.UTF-8")` is called
- **THEN** it returns `Some(("en".to_string(), Some("GB".to_string())))`

#### Scenario: short_code strips region
- **WHEN** `short_code("en-GB")` is called
- **THEN** it returns `"en"`

#### Scenario: display_name renders friendly form
- **WHEN** `display_name("en-GB")` is called
- **THEN** it returns `"English (United Kingdom)"`

### Requirement: Live language switch via request-context preference
`KarereWebView` SHALL expose `set_spellcheck_languages(langs, enabled)` that switches the active spellcheck language list on the live browser by writing the Chromium `spellcheck.dictionaries` (list) and `browser.enable_spellchecking` (boolean) preferences on the browser host's `RequestContext`, without recreating the browser or reloading the page.

> Note: this supersedes the original "browser recreation" requirement. CEF 148
> exposes `RequestContext::set_preference` (via `ImplPreferenceManager`) and
> `BrowserHost::request_context()`, so a live switch is possible — and it
> preserves URL, scroll, zoom, and ephemeral page state that a recreation would
> lose.

#### Scenario: Language change applies in place
- **WHEN** `spell-checking-languages` is updated from `['en-US']` to `['pt-BR']` and `set_spellcheck_languages(["pt-BR"], true)` is invoked
- **THEN** the browser's `spellcheck.dictionaries` preference is set to `["pt-BR"]`, no browser is closed or recreated, the page is not reloaded, and Chromium re-checks the page against the new dictionary (downloading `pt-BR.bdic` on demand if absent)

### Requirement: Dictionary auto-download to cache_path
The runtime SHALL allow Chromium to auto-download `.bdic` dictionaries into `cache_path/dictionaries/` on first need; no Hunspell module or bundled dictionary files are required.

#### Scenario: First launch downloads dictionaries
- **WHEN** the app launches with `spell-checking-languages=['en-US']` and no cached `en-US.bdic`
- **THEN** Chromium downloads `en-US.bdic` to `cache_path/dictionaries/` and the second launch can spellcheck offline
