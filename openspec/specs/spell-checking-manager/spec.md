# spell-checking-manager Specification

## Purpose
TBD - created by archiving change fix-spell-checking-system. Update Purpose after archive.
## Requirements
### Requirement: WebKit Spell Checking Configuration

The manager MUST configure WebKitGTK spell checking with only validated, available dictionaries.

#### Scenario: Enable spell checking with available dictionaries

**GIVEN** the user has enabled spell checking in settings
**AND** dictionaries for `en_US` and `fr_FR` are available
**AND** the user has selected both `en_US` and `fr_FR`
**WHEN** configuring WebKit spell checking
**THEN** `WebContext.set_spell_checking_enabled(true)` MUST be called
**AND** `WebContext.set_spell_checking_languages(["en_US", "fr_FR"])` MUST be called
**AND** both languages MUST be validated as available before passing to WebKit

#### Scenario: Disable spell checking

**GIVEN** the user has disabled spell checking in settings
**WHEN** configuring WebKit spell checking
**THEN** `WebContext.set_spell_checking_enabled(false)` MUST be called
**AND** `WebContext.set_spell_checking_languages([])` MUST be called with an empty array

#### Scenario: Enable spell checking with no dictionaries available

**GIVEN** the user has enabled spell checking in settings
**AND** no dictionaries are available in the system
**WHEN** configuring WebKit spell checking
**THEN** `WebContext.set_spell_checking_enabled(false)` MUST be called
**AND** a warning MUST be logged that spell checking cannot be enabled without dictionaries
**AND** a user notification SHOULD be shown explaining why spell checking is unavailable

### Requirement: Language Validation

The manager MUST validate all language codes against available dictionaries before use.

#### Scenario: Validate available language

**GIVEN** dictionary `de_DE` is available
**WHEN** validating language code `de_DE`
**THEN** the validation MUST return `true`

#### Scenario: Reject unavailable language

**GIVEN** dictionary `zh_CN` is NOT available
**WHEN** validating language code `zh_CN`
**THEN** the validation MUST return `false`
**AND** a debug message MUST log that `zh_CN` is not available

#### Scenario: Filter user settings for unavailable languages

**GIVEN** user settings contain languages `["en_US", "es_ES", "zh_CN"]`
**AND** only `en_US` and `es_ES` dictionaries are available
**WHEN** loading languages from settings
**THEN** the manager MUST filter to `["en_US", "es_ES"]`
**AND** `zh_CN` MUST be logged as unavailable but not applied
**AND** the user settings MUST NOT be modified (preserve user choice for when dictionary becomes available)

### Requirement: Auto-Detection Integration

The manager MUST integrate system locale auto-detection with dictionary availability.

#### Scenario: Auto-detect system language with available dictionary

**GIVEN** auto-detect is enabled in settings
**AND** the system locale is `pt_BR.UTF-8`
**AND** dictionary `pt_BR` is available
**WHEN** determining spell checking languages
**THEN** the manager MUST return `["pt_BR"]`
**AND** the auto-detected language MUST override any manually selected languages

#### Scenario: Auto-detect with fallback to available variant

**GIVEN** auto-detect is enabled in settings
**AND** the system locale is `pt_BR.UTF-8`
**AND** dictionary `pt_BR` is NOT available
**AND** dictionary `pt_PT` is available
**WHEN** determining spell checking languages
**THEN** the manager MUST use locale matching to find `pt_PT`
**AND** the manager MUST return `["pt_PT"]`
**AND** a debug message MUST log the fallback from `pt_BR` to `pt_PT`

#### Scenario: Auto-detect fails with no matching dictionary

**GIVEN** auto-detect is enabled in settings
**AND** the system locale is `ja_JP.UTF-8`
**AND** no Japanese dictionaries are available
**WHEN** determining spell checking languages
**THEN** the manager MUST return an empty array `[]`
**AND** a warning MUST be logged that auto-detect failed
**AND** spell checking MUST be disabled

### Requirement: Settings Integration

The manager MUST respect all spell checking related settings and react to changes.

#### Scenario: Settings change for spell-checking-enabled

**GIVEN** spell checking is currently enabled
**AND** WebKit has been configured with languages
**WHEN** the user disables spell checking via settings
**THEN** the manager MUST call `WebContext.set_spell_checking_enabled(false)`
**AND** the change MUST take effect immediately without restart

#### Scenario: Settings change for spell-checking-languages

**GIVEN** spell checking is enabled with `["en_US"]`
**WHEN** the user adds `de_DE` to the language list
**AND** `de_DE` dictionary is available
**THEN** the manager MUST update WebKit to use `["en_US", "de_DE"]`
**AND** the change MUST take effect immediately in the WebView

#### Scenario: Settings change for auto-detect toggle

**GIVEN** spell checking is enabled with manual languages `["en_US", "fr_FR"]`
**WHEN** the user enables auto-detect
**THEN** the manager MUST switch to auto-detected language
**AND** manual language selections MUST be ignored while auto-detect is enabled
**AND** the previous manual selections MUST be preserved in settings for if auto-detect is disabled again

### Requirement: Status Reporting

The manager MUST provide clear status information about spell checking availability and configuration.

#### Scenario: Report active spell checking status

**GIVEN** spell checking is enabled
**AND** 2 languages are active
**WHEN** requesting spell checking status
**THEN** the manager MUST return a message like "Spell checking active with 2 languages (en_US, de_DE)"

#### Scenario: Report disabled status

**GIVEN** spell checking is disabled in settings
**WHEN** requesting spell checking status
**THEN** the manager MUST return "Spell checking disabled"

#### Scenario: Report unavailable status

**GIVEN** spell checking is enabled in settings
**AND** no dictionaries are available
**WHEN** requesting spell checking status
**THEN** the manager MUST return "Spell checking unavailable - no dictionaries found"

#### Scenario: Report dictionary count

**GIVEN** 12 dictionaries are available
**WHEN** requesting dictionary count
**THEN** the manager MUST return exactly 12
**AND** the count MUST reflect the number of unique language codes with valid dictionaries

### Requirement: Error Handling

The manager MUST handle errors gracefully and provide informative feedback.

#### Scenario: WebContext unavailable

**GIVEN** WebContext is not yet initialized
**WHEN** attempting to configure spell checking
**THEN** the manager MUST defer configuration until WebContext is available
**AND** a debug message MUST log the deferred configuration
**AND** the manager MUST NOT throw an error

#### Scenario: Invalid language code format

**GIVEN** user settings contain language code `invalid-code-123`
**WHEN** validating the language
**THEN** the manager MUST reject the language code
**AND** a warning MUST be logged about the invalid format
**AND** the invalid code MUST NOT be passed to WebKit

### Requirement: Initialization

The manager MUST initialize dictionary discovery on creation and configure WebKit on first use.

#### Scenario: Manager initialization

**GIVEN** the SpellCheckingManager is created
**WHEN** the constructor completes
**THEN** dictionary discovery MUST be performed
**AND** the available languages MUST be cached
**AND** settings listeners MUST be registered for spell checking related settings

#### Scenario: First WebKit configuration

**GIVEN** the manager has been initialized
**AND** WebContext is now available
**WHEN** `configure_webkit()` is called for the first time
**THEN** the current settings MUST be read
**AND** languages MUST be validated against available dictionaries
**AND** WebContext MUST be configured with validated languages
**AND** the configuration MUST be logged for debugging

