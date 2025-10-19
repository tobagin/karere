# dictionary-discovery Specification

## Purpose
TBD - created by archiving change fix-spell-checking-system. Update Purpose after archive.
## Requirements
### Requirement: Dictionary Path Scanning

The system MUST scan multiple filesystem paths to discover available hunspell dictionaries, in priority order.

#### Scenario: Flatpak bundled dictionaries are found first

**GIVEN** dictionaries exist in `/app/share/hunspell/`
**AND** dictionaries exist in `/usr/share/hunspell/`
**WHEN** the dictionary discovery system scans for dictionaries
**THEN** dictionaries from `/app/share/hunspell/` MUST be prioritized
**AND** each discovered dictionary MUST be cataloged with its language code and file path

#### Scenario: System dictionaries are discovered as fallback

**GIVEN** no dictionaries exist in `/app/share/hunspell/`
**AND** dictionaries exist in `/usr/share/hunspell/`
**WHEN** the dictionary discovery system scans for dictionaries
**THEN** dictionaries from `/usr/share/hunspell/` MUST be available
**AND** all valid .dic and .aff file pairs MUST be detected

#### Scenario: Custom dictionary path via environment variable

**GIVEN** `WEBKIT_SPELL_CHECKER_DIR` environment variable is set to `/custom/path`
**AND** valid dictionaries exist at `/custom/path`
**WHEN** the dictionary discovery system scans for dictionaries
**THEN** dictionaries from `/custom/path` MUST be included in the available dictionary list

### Requirement: Dictionary Validation

The system MUST validate that both .dic and .aff files exist for each language before marking it as available.

#### Scenario: Complete dictionary files present

**GIVEN** files `en_US.dic` and `en_US.aff` exist in a scanned path
**WHEN** validating the `en_US` dictionary
**THEN** the language `en_US` MUST be marked as available
**AND** the dictionary path MUST be stored for future reference

#### Scenario: Incomplete dictionary files detected

**GIVEN** file `fr_FR.dic` exists but `fr_FR.aff` is missing
**WHEN** validating the `fr_FR` dictionary
**THEN** the language `fr_FR` MUST NOT be marked as available
**AND** a warning MUST be logged about the incomplete dictionary

#### Scenario: Symlinked dictionaries are resolved

**GIVEN** `en_AG.dic` is a symlink to `en_GB.dic`
**AND** both `en_GB.dic` and `en_GB.aff` exist
**WHEN** validating the `en_AG` dictionary
**THEN** the language `en_AG` MUST be marked as available
**AND** the symlink MUST be resolved to the actual dictionary files

### Requirement: Language Code Extraction

The system MUST extract language codes from dictionary filenames according to hunspell naming conventions.

#### Scenario: Standard language_COUNTRY format

**GIVEN** a dictionary file named `pt_BR.dic`
**WHEN** extracting the language code
**THEN** the language code MUST be identified as `pt_BR`
**AND** the code MUST be stored in the format `language_COUNTRY`

#### Scenario: Language-only format without country code

**GIVEN** a dictionary file named `en.dic`
**WHEN** extracting the language code
**THEN** the language code MUST be identified as `en`
**AND** the code MUST be stored without a country suffix

### Requirement: Dictionary Count Reporting

The system MUST provide an accurate count of available dictionaries for user feedback.

#### Scenario: Multiple dictionaries discovered

**GIVEN** 15 valid dictionary pairs are discovered across all search paths
**WHEN** the dictionary count is requested
**THEN** the system MUST report exactly 15 dictionaries
**AND** duplicate language codes from different paths MUST be counted only once

#### Scenario: No dictionaries found

**GIVEN** no valid dictionary pairs exist in any search path
**WHEN** the dictionary count is requested
**THEN** the system MUST report 0 dictionaries
**AND** a status message MUST indicate that no dictionaries are available

### Requirement: Available Languages Enumeration

The system MUST provide a sorted list of all available language codes for UI presentation.

#### Scenario: Language list for UI dropdown

**GIVEN** dictionaries for `en_US`, `fr_FR`, `de_DE`, and `es_ES` are available
**WHEN** the available languages list is requested
**THEN** the system MUST return `["de_DE", "en_US", "es_ES", "fr_FR"]` (alphabetically sorted)
**AND** each language code MUST have a validated dictionary pair

### Requirement: Locale to Dictionary Matching

The system MUST match system locale strings to available dictionary language codes with intelligent fallback.

#### Scenario: Exact locale match found

**GIVEN** the system locale is `en_US.UTF-8`
**AND** dictionary `en_US` is available
**WHEN** matching the locale to a dictionary
**THEN** the system MUST return `en_US` as the matched language
**AND** no fallback attempts MUST be made

#### Scenario: Fallback to language variant

**GIVEN** the system locale is `en_US.UTF-8`
**AND** dictionary `en_US` is NOT available
**AND** dictionary `en_GB` is available
**WHEN** matching the locale to a dictionary
**THEN** the system MUST return `en_GB` as the matched language
**AND** a debug message MUST log the fallback from `en_US` to `en_GB`

#### Scenario: Fallback to base language

**GIVEN** the system locale is `en_CA.UTF-8`
**AND** no `en_CA` dictionary is available
**AND** no other `en_*` variants are available
**AND** dictionary `en` (base language) is available
**WHEN** matching the locale to a dictionary
**THEN** the system MUST return `en` as the matched language

#### Scenario: No match found for locale

**GIVEN** the system locale is `ja_JP.UTF-8`
**AND** no `ja_JP`, `ja_*`, or `ja` dictionaries are available
**WHEN** matching the locale to a dictionary
**THEN** the system MUST return `null`
**AND** a warning MUST be logged that no dictionary matches the locale

### Requirement: Performance Optimization

The system MUST cache discovered dictionaries to avoid repeated filesystem scans.

#### Scenario: Dictionary cache on first scan

**GIVEN** the dictionary discovery system is initialized
**WHEN** the first dictionary scan completes
**THEN** all discovered dictionaries MUST be cached in memory
**AND** subsequent requests for available languages MUST use the cache
**AND** no additional filesystem scanning MUST occur unless explicitly refreshed

#### Scenario: Cache refresh on demand

**GIVEN** dictionaries have been cached from a previous scan
**AND** new dictionaries are installed in a search path
**WHEN** a cache refresh is explicitly requested
**THEN** the system MUST re-scan all search paths
**AND** the cache MUST be updated with newly discovered dictionaries

