# Design: Spell Checking System Overhaul

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     PreferencesDialog                        │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Spell Checking UI                                    │  │
│  │  - Enable/Disable switch                              │  │
│  │  - Auto-detect system language switch                 │  │
│  │  - Available languages ComboBox (from manager)        │  │
│  │  - Status label (dictionaries found/missing)          │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ reads available languages
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   SpellCheckingManager                       │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Dictionary Discovery                                 │  │
│  │  - scan_available_dictionaries()                      │  │
│  │  - get_available_languages() → string[]               │  │
│  │  - get_dictionary_path(lang) → string?                │  │
│  │  - validate_language(lang) → bool                     │  │
│  │  - match_locale_to_dictionary(locale) → string?       │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  WebKit Integration                                   │  │
│  │  - configure_webkit_spell_checking()                  │  │
│  │  - update_spell_languages()                           │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ searches paths
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Hunspell Dictionary Locations                   │
│  - /app/share/hunspell (Flatpak bundled)                    │
│  - /usr/share/hunspell (system, via --filesystem)           │
│  - /run/host/usr/share/hunspell (host passthrough)          │
│  - $WEBKIT_SPELL_CHECKER_DIR (if set)                       │
└─────────────────────────────────────────────────────────────┘
```

## Component Design

### 1. SpellCheckingManager Class

**Purpose**: Central management of spell checking functionality, dictionary discovery, and WebKit integration.

**Responsibilities**:
- Discover available hunspell dictionaries from multiple paths
- Validate language codes against available dictionaries
- Provide list of available languages to UI
- Configure WebKitGTK spell checking with valid languages
- Match system locale to available dictionaries

**Public API**:
```vala
public class SpellCheckingManager : GLib.Object {
    // Discovery
    public string[] get_available_languages()
    public bool is_language_available(string language)
    public string? match_locale_to_dictionary(string locale)

    // Configuration
    public void configure_webkit(WebKit.WebContext context, Settings settings)
    public void update_languages(WebKit.WebContext context, string[] languages)

    // Status
    public string get_status_message()
    public int get_dictionary_count()
}
```

**Implementation Details**:
- Scan dictionary paths on initialization
- Cache discovered dictionaries in HashMap<string, string> (language → path)
- Support both .dic files and symlinks
- Handle locale variants (en_US, en_GB) and base languages (en)

### 2. Dictionary Search Paths

**Priority Order**:
1. `/app/share/hunspell` - Bundled dictionaries (highest priority)
2. `/usr/share/hunspell` - GNOME runtime dictionaries
3. `/run/host/usr/share/hunspell` - Host system dictionaries (if accessible)
4. `$WEBKIT_SPELL_CHECKER_DIR` - Custom override

**Path Resolution**:
```vala
private void scan_dictionary_paths() {
    string[] search_paths = {
        "/app/share/hunspell",
        "/usr/share/hunspell",
        "/run/host/usr/share/hunspell",
        Environment.get_variable("WEBKIT_SPELL_CHECKER_DIR")
    };

    foreach (var path in search_paths) {
        if (path != null && FileUtils.test(path, FileTest.IS_DIR)) {
            scan_directory_for_dictionaries(path);
        }
    }
}
```

### 3. Language Matching Algorithm

**Locale to Dictionary Mapping**:
```
System Locale → Language Code → Dictionary File

Examples:
en_US.UTF-8 → en_US → en_US.dic (exact match)
en_US.UTF-8 → en_US → en_GB.dic (fallback to base language variant)
pt_BR.UTF-8 → pt_BR → pt_BR.dic (exact match)
de_DE.UTF-8 → de_DE → de_DE.dic (exact match)
```

**Fallback Strategy**:
1. Try exact locale match (e.g., `en_US.dic`)
2. Try base language variants (e.g., `en_GB.dic`, `en_AU.dic`)
3. Try language code only (e.g., `en.dic`)
4. Return null if no dictionary found

### 4. Flatpak Dictionary Bundling

**Approach**: Add hunspell-dictionaries module to Flatpak manifest

**Bundled Languages** (based on WhatsApp popularity):
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

**Flatpak Module Addition**:
```yaml
- name: hunspell-dictionaries
  buildsystem: simple
  build-commands:
    - mkdir -p /app/share/hunspell
    - cp -r dictionaries/*.{dic,aff} /app/share/hunspell/
  sources:
    - type: archive
      url: https://github.com/LibreOffice/dictionaries/archive/refs/tags/...
      # OR individual dictionary sources
```

**Environment Setup**:
```yaml
finish-args:
  - --env=WEBKIT_SPELL_CHECKER_DIR=/app/share/hunspell:/usr/share/hunspell
  # Optional: allow access to host dictionaries
  - --filesystem=/usr/share/hunspell:ro
```

### 5. UI Component Updates

**PreferencesDialog Changes**:

**Replace**:
- Manual text entry for adding languages

**With**:
- ComboBox/Dropdown showing available dictionaries
- Status row showing "X dictionaries available"
- Visual indicator when no dictionaries found

**New UI Elements**:
```blueprint
Adw.ComboRow language_selection_row {
  title: _("Add Language")
  subtitle: _("Select from available dictionaries")
  model: StringList {
    // Populated dynamically from SpellCheckingManager
  }
}

Adw.ActionRow dictionary_status_row {
  title: _("Available Dictionaries")

  [suffix]
  Gtk.Label dictionary_count_label {
    label: "0 found"
  }
}
```

### 6. Settings Schema Changes

**New Settings** (optional):
```xml
<key name="spell-checking-dictionary-paths" type="as">
  <default>[]</default>
  <summary>Custom dictionary search paths</summary>
  <description>Additional paths to search for hunspell dictionaries</description>
</key>
```

**Behavior Changes**:
- `spell-checking-languages`: Only accept languages with available dictionaries
- Automatically filter out unavailable languages when loading settings
- Show warning if saved language is no longer available

### 7. WebKit Integration

**WebContext Configuration**:
```vala
public void configure_webkit_spell_checking(WebKit.WebContext context) {
    var enabled = settings.get_boolean("spell-checking-enabled");
    context.set_spell_checking_enabled(enabled);

    if (enabled) {
        string[] languages = get_validated_languages();
        if (languages.length > 0) {
            context.set_spell_checking_languages(languages);
            debug("Spell checking enabled with %d languages", languages.length);
        } else {
            warning("Spell checking enabled but no dictionaries available");
            // Show user notification
        }
    }
}

private string[] get_validated_languages() {
    var auto_detect = settings.get_boolean("spell-checking-auto-detect");

    if (auto_detect) {
        var locale = Intl.setlocale(LocaleCategory.MESSAGES, null);
        var matched = match_locale_to_dictionary(locale);
        if (matched != null) {
            return {matched};
        }
    }

    var user_languages = settings.get_strv("spell-checking-languages");
    var validated = new ArrayList<string>();

    foreach (var lang in user_languages) {
        if (is_language_available(lang)) {
            validated.add(lang);
        } else {
            warning("Language '%s' not available, skipping", lang);
        }
    }

    return validated.to_array();
}
```

## Error Handling

### Scenario 1: No Dictionaries Found
- **Detection**: `get_dictionary_count() == 0`
- **UI Response**: Show warning in preferences, disable language selection
- **Runtime**: Disable spell checking, log warning
- **User Message**: "No spell checking dictionaries found. Install hunspell dictionaries to enable spell checking."

### Scenario 2: Auto-Detect Fails
- **Detection**: `match_locale_to_dictionary()` returns null
- **UI Response**: Show "Auto-detect unavailable for your language"
- **Runtime**: Fall back to manual selection or disable
- **User Message**: "Dictionary not available for system language (XX_XX). Select a language manually."

### Scenario 3: Selected Language Removed
- **Detection**: Previously saved language no longer in available list
- **UI Response**: Show which languages were removed
- **Runtime**: Remove from settings, continue with remaining languages
- **User Message**: "Dictionary for 'XX_XX' is no longer available."

## Testing Strategy

### Unit Tests
- `test_dictionary_discovery()`: Verify dictionary scanning
- `test_locale_matching()`: Test locale to dictionary matching
- `test_fallback_logic()`: Test language fallback (en_US → en_GB)
- `test_validation()`: Verify language validation

### Integration Tests
- `test_webkit_integration()`: Verify WebContext configuration
- `test_settings_persistence()`: Test language settings save/load
- `test_flatpak_paths()`: Verify bundled dictionaries accessible

### Manual Tests
- Enable spell checking and verify misspellings are underlined
- Test with multiple languages
- Test auto-detect with different system locales
- Verify UI shows correct available languages
- Test in both development and Flatpak builds

## Migration Path

### Existing Users
1. On app update, run dictionary discovery
2. Validate existing `spell-checking-languages` setting
3. Remove any languages without dictionaries
4. If auto-detect enabled, re-match to available dictionaries
5. Show notification if changes were made to their language selection

### New Users
1. Auto-detect system language on first launch
2. Enable spell checking if dictionary available
3. Pre-select system language in preferences

## Performance Considerations

- **Dictionary scanning**: Run once on app startup, cache results
- **File I/O**: Minimal - only scanning directory listings, not reading .dic files
- **Memory**: ~1KB per dictionary entry in HashMap (negligible for ~50 dictionaries)
- **Startup impact**: < 10ms for dictionary discovery

## Security Considerations

- Only read dictionary files, never write
- Validate file paths to prevent path traversal
- Use read-only filesystem permissions for dictionary access
- No user-provided dictionary files (security risk)

## Accessibility

- Ensure ComboBox for language selection is keyboard navigable
- Provide screen reader labels for dictionary status
- Clear error messages for missing dictionaries
- Don't break existing functionality if spell checking unavailable
