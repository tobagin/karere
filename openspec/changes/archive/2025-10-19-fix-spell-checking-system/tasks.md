# Tasks: Fix Spell Checking System

## Phase 1: Dictionary Discovery Foundation

### 1.1 Create SpellCheckingManager class skeleton

- Create `src/managers/SpellCheckingManager.vala`
- Define class structure with settings manager integration
- Add basic constructor and initialization
- Set up signal handlers for settings changes
- Add debug logging for initialization

**Validation**: Class compiles, can be instantiated in Window.vala

**Dependencies**: None

---

### 1.2 Implement dictionary path scanning

- Add method `scan_dictionary_paths()` to scan multiple filesystem locations:
  - `/app/share/hunspell` (Flatpak bundled)
  - `/usr/share/hunspell` (GNOME runtime)
  - `$WEBKIT_SPELL_CHECKER_DIR` (environment variable)
- Use `Dir.open()` and `Dir.read_name()` to list files
- Store search paths in priority order
- Add error handling for inaccessible directories

**Validation**: Scan logs all discovered .dic and .aff files

**Dependencies**: 1.1

---

### 1.3 Implement dictionary file validation

- Add method `validate_dictionary(string lang_code, string path)`
- Check for both `.dic` and `.aff` files for each language
- Resolve symlinks using `FileUtils.read_link()`
- Handle edge cases (broken symlinks, missing .aff files)
- Return bool indicating if dictionary is complete and valid

**Validation**: Test with system hunspell directory, correctly identifies valid/invalid dictionaries

**Dependencies**: 1.2

---

### 1.4 Build available dictionaries cache

- Add `HashMap<string, string>` to store language_code → dictionary_path
- Extract language codes from .dic filenames (e.g., "en_US.dic" → "en_US")
- Populate cache during scanning with only validated dictionaries
- Implement `get_available_languages()` returning sorted string array
- Implement `is_language_available(string lang)` for validation
- Add `get_dictionary_count()` method

**Validation**: Returns accurate list of available dictionaries from system

**Dependencies**: 1.3

---

### 1.5 Implement locale matching algorithm

- Add `match_locale_to_dictionary(string locale)` method
- Parse locale string (handle formats like "en_US.UTF-8", "en_US", "en")
- Implement fallback logic:
  1. Try exact match (en_US)
  2. Try other variants of same language (en_GB, en_AU)
  3. Try base language (en)
  4. Return null if no match
- Add unit test cases for various locale formats

**Validation**: Correctly matches "en_US.UTF-8" → "en_GB" when en_US unavailable

**Dependencies**: 1.4

---

## Phase 2: WebKit Integration

### 2.1 Move spell checking logic from Window.vala to SpellCheckingManager

- Move `setup_spell_checking()` logic to SpellCheckingManager
- Move `get_spell_checking_languages()` logic to SpellCheckingManager
- Move `update_spell_checking()` logic to SpellCheckingManager
- Add `configure_webkit(WebContext context)` method
- Refactor Window.vala to delegate to SpellCheckingManager

**Validation**: Spell checking still works with existing implementation (no regression)

**Dependencies**: 1.5

---

### 2.2 Add language validation before WebKit configuration

- In `configure_webkit()`, validate all languages before setting
- Filter out unavailable languages from user settings
- Log warnings for languages that are filtered out
- Use only validated languages in `set_spell_checking_languages()`
- Handle empty language list (disable spell checking)

**Validation**: WebKit only receives valid language codes; invalid codes are filtered and logged

**Dependencies**: 2.1

---

### 2.3 Implement auto-detect with dictionary matching

- Update auto-detect logic to use `match_locale_to_dictionary()`
- Get system locale using `Intl.setlocale()`
- Match locale to available dictionary
- Log fallback when exact locale unavailable
- Return empty array if no match found

**Validation**: Auto-detect uses en_GB when system is en_US but only en_GB available

**Dependencies**: 2.2

---

### 2.4 Add status reporting methods

- Implement `get_status_message()` returning human-readable status
- Handle states: enabled with N languages, disabled, unavailable (no dictionaries)
- Add status information for debugging and UI display
- Include active language codes in status

**Validation**: Status messages accurately reflect spell checking state

**Dependencies**: 2.3

---

### 2.5 Add settings change reactivity

- Listen to `spell-checking-enabled` changes
- Listen to `spell-checking-auto-detect` changes
- Listen to `spell-checking-languages` changes
- Reconfigure WebKit immediately when settings change
- Validate languages on every update

**Validation**: Changing settings in preferences updates spell checking in real-time

**Dependencies**: 2.4

---

## Phase 3: Flatpak Dictionary Bundling

### 3.1 Research and select dictionary sources

- Identify hunspell dictionary sources (LibreOffice dictionaries, language packages)
- Select 10-15 most common languages based on WhatsApp usage stats:
  - English (en_US, en_GB)
  - Spanish (es_ES, es_MX)
  - Portuguese (pt_BR, pt_PT)
  - French (fr_FR)
  - German (de_DE)
  - Italian (it_IT)
  - Russian (ru_RU)
  - Arabic (ar)
  - Hindi (hi_IN)
  - Indonesian (id_ID)
- Verify licenses are compatible (most are GPL/LGPL/MPL)
- Download dictionary files for testing

**Validation**: Have source URLs and licenses documented for all selected dictionaries

**Dependencies**: None (parallel track)

---

### 3.2 Add hunspell-dictionaries module to Flatpak manifest

- Edit `packaging/io.github.tobagin.karere.yml`
- Add new module `hunspell-dictionaries` before `karere` module
- Use `buildsystem: simple`
- Add build-commands to copy dictionaries to `/app/share/hunspell/`
- Add sources for dictionary archives or files
- Test Flatpak build completes successfully

**Validation**: Flatpak builds without errors, `/app/share/hunspell` contains dictionaries in build

**Dependencies**: 3.1

---

### 3.3 Configure WEBKIT_SPELL_CHECKER_DIR environment variable

- Add `--env=WEBKIT_SPELL_CHECKER_DIR=/app/share/hunspell:/usr/share/hunspell` to finish-args
- Alternatively set environment in application startup
- Test that WebKitGTK can find bundled dictionaries
- Verify both bundled and runtime dictionaries are accessible

**Validation**: WebKitGTK spell checking works with bundled dictionaries in Flatpak

**Dependencies**: 3.2

---

### 3.4 Add optional host dictionary access

- Add `--filesystem=/usr/share/hunspell:ro` to finish-args (optional)
- Document this allows access to additional host system dictionaries
- Test that host dictionaries are discovered when permission granted
- Ensure app works without this permission (bundled dictionaries sufficient)

**Validation**: App finds both bundled and host dictionaries when permission granted

**Dependencies**: 3.3

---

### 3.5 Test Flatpak build with bundled dictionaries

- Build Flatpak with `flatpak-builder`
- Install and run the Flatpak
- Verify dictionaries are discovered in `/app/share/hunspell`
- Enable spell checking and test with multiple languages
- Verify spell checking actually underlines misspellings in WhatsApp Web

**Validation**: Spell checking fully functional in Flatpak build

**Dependencies**: 3.4

---

## Phase 4: UI Implementation

### 4.1 Add dictionary status display to preferences

- Edit `data/ui/preferences.blp.in`
- Add `Adw.ActionRow` showing dictionary count
- Bind label to SpellCheckingManager.get_dictionary_count()
- Add status indicator (success/warning icon)
- Show "X dictionaries available" or "No dictionaries available"

**Validation**: Preferences shows accurate dictionary count

**Dependencies**: 2.5

---

### 4.2 Replace manual language entry with dropdown

- Remove text entry dialog for adding languages
- Add `Adw.ComboRow` for language selection
- Populate StringList model from `get_available_languages()`
- Show user-friendly language names using locale display names
- Handle selection event to add language to settings

**Validation**: Can only select languages with available dictionaries

**Dependencies**: 4.1

---

### 4.3 Implement current languages list with removal

- Add expandable list showing currently selected languages
- For each language, show user-friendly name and remove button
- Implement remove button handler to remove from settings
- Update list when settings change
- Handle auto-detect mode (show auto-detected language, disable manual list)

**Validation**: Can add and remove languages, list updates correctly

**Dependencies**: 4.2

---

### 4.4 Add language name localization

- Create mapping from locale codes to user-friendly names
- Use GLib locale display name functions if available
- Fallback to hardcoded names for common languages
- Show raw code if language unknown
- Add translations for language names

**Validation**: Languages display with friendly names like "English (United States)"

**Dependencies**: 4.3

---

### 4.5 Implement UI state management

- Disable spell checking switch when no dictionaries available
- Update switch subtitle to indicate status
- Show/hide language controls based on spell checking enabled state
- Disable language selection when auto-detect enabled
- Add help/info messages for various states

**Validation**: UI clearly indicates when spell checking is unavailable and why

**Dependencies**: 4.4

---

### 4.6 Add warning messages and help text

- Add info box when no dictionaries available with installation instructions
- Add warning when spell checking enabled but no languages configured
- Add info text explaining auto-detect behavior
- Consider adding link to documentation
- Test all message scenarios

**Validation**: Users get clear feedback about spell checking status and how to fix issues

**Dependencies**: 4.5

---

## Phase 5: Testing & Polish

### 5.1 Add unit tests for dictionary discovery

- Create `tests/test_spell_checking_manager.vala`
- Test `scan_dictionary_paths()` with mock directories
- Test `validate_dictionary()` with various file scenarios
- Test `match_locale_to_dictionary()` with different locale formats
- Test language list generation and sorting

**Validation**: All unit tests pass

**Dependencies**: 4.6

---

### 5.2 Integration testing

- Test spell checking in development build
- Test spell checking in Flatpak build
- Test with various system locales
- Test adding/removing languages through UI
- Test auto-detect with different locales
- Test with no dictionaries available (delete /usr/share/hunspell temporarily)

**Validation**: All integration scenarios work correctly

**Dependencies**: 5.1

---

### 5.3 Manual testing of spell checking functionality

- Type text with misspellings in WhatsApp Web
- Verify misspellings are underlined in red
- Right-click on misspelling, verify suggestions appear
- Test with multiple languages enabled
- Test language switching mid-typing
- Verify spell checking persists across app restarts

**Validation**: Spell checking works correctly in real WhatsApp Web usage

**Dependencies**: 5.2

---

### 5.4 Performance testing

- Measure dictionary scan time on startup
- Verify no noticeable lag when changing settings
- Check memory usage with 20+ dictionaries cached
- Profile if needed and optimize hot paths

**Validation**: No performance regression, startup time impact < 10ms

**Dependencies**: 5.3

---

### 5.5 Documentation updates

- Update README.md with spell checking features
- Document bundled dictionary languages
- Add troubleshooting section for spell checking
- Update preferences documentation
- Add code comments to SpellCheckingManager

**Validation**: Documentation is clear and complete

**Dependencies**: 5.4

---

### 5.6 Accessibility testing

- Test keyboard navigation of language dropdown
- Test with screen reader (Orca)
- Verify ARIA labels are appropriate
- Test focus management when adding/removing languages
- Verify all interactive elements are keyboard accessible

**Validation**: Spell checking UI is fully accessible

**Dependencies**: 5.5

---

## Phase 6: Migration & Cleanup

### 6.1 Handle migration from old settings

- Check existing `spell-checking-languages` on first run with new system
- Validate saved languages against available dictionaries
- Remove unavailable languages with user notification
- Re-run auto-detect if it was enabled but failed previously
- Log migration actions

**Validation**: Existing users' settings migrate smoothly

**Dependencies**: 5.6

---

### 6.2 Update translations

- Mark new UI strings for translation
- Update .pot template file
- Add context comments for translators
- Test with non-English locale

**Validation**: New strings appear in translation template

**Dependencies**: 6.1

---

### 6.3 Update Flatpak manifest version and metadata

- Update version in meson.build
- Update Flatpak manifest sources to new tag/commit
- Update metainfo.xml with release notes about spell checking fixes
- Update screenshots if UI changed significantly

**Validation**: Manifest points to correct version

**Dependencies**: 6.2

---

### 6.4 Final code review and cleanup

- Review all code changes for consistency with project conventions
- Remove debug logging that's no longer needed
- Check for TODOs and FIXMEs
- Ensure proper error handling everywhere
- Verify all code follows Vala style guide

**Validation**: Code passes review

**Dependencies**: 6.3

---

### 6.5 Update CHANGELOG and prepare release

- Document all changes in CHANGELOG.md
- Prepare release notes highlighting spell checking fixes
- Tag release version
- Test final build one more time

**Validation**: Ready for release

**Dependencies**: 6.4

---

## Notes

**Parallel Work Possible**:
- Phase 3 (Flatpak Dictionary Bundling) can be done in parallel with Phase 1-2
- UI mockups can be created early while backend is being developed

**Critical Path**:
Phase 1 → Phase 2 → Phase 4 → Phase 5 → Phase 6

**Estimated Effort**:
- Phase 1: 4-6 hours
- Phase 2: 3-4 hours
- Phase 3: 2-3 hours
- Phase 4: 4-5 hours
- Phase 5: 3-4 hours
- Phase 6: 2-3 hours
- **Total: ~18-25 hours**

**Testing Environment Requirements**:
- Access to both development and Flatpak builds
- Ability to manipulate /usr/share/hunspell for testing
- Multiple test dictionaries (at least 3-5 languages)
- Screen reader for accessibility testing
