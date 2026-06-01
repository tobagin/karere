# spellcheck-autocorrect Specification

## Purpose
TBD - created by archiving change m16-1-osr-context-menu. Update Purpose after archive.
## Requirements
### Requirement: Optional auto-correct of misspelled words
The shell SHALL provide an optional auto-correct feature that, when enabled via the `enable-auto-correct` GSettings key, replaces a just-completed misspelled word in an editable field with its top spellcheck suggestion. When the key is `false`, no automatic replacement SHALL occur.

#### Scenario: Auto-correct enabled replaces on word boundary
- **WHEN** `enable-auto-correct` is `true` and the user finishes a misspelled word (e.g. types a space or punctuation after it) for which a confident suggestion exists
- **THEN** the word is replaced in place by the top suggestion without the user opening a menu

#### Scenario: Auto-correct disabled leaves text untouched
- **WHEN** `enable-auto-correct` is `false`
- **THEN** misspelled words are only underlined (and correctable via the right-click menu), never replaced automatically

#### Scenario: No confident suggestion leaves the word
- **WHEN** auto-correct is enabled but the misspelled word has no confident suggestion
- **THEN** the word is left unchanged (still underlined) rather than replaced with a poor guess

### Requirement: Preference to toggle auto-correct
The Preferences page (hosted by M22) SHALL expose a control bound to `enable-auto-correct` so the user can turn auto-correct on or off, defaulting to the schema's default.

#### Scenario: Toggling the preference updates behavior
- **WHEN** the user toggles the auto-correct control in Preferences
- **THEN** `enable-auto-correct` is updated and subsequent typing honors the new value without restarting the app

