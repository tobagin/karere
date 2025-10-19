# Fix Spell Checking System

## Problem Statement

The current spell checking implementation in Karere is broken and non-functional:

1. **No Dictionary Access in Flatpak**: WebKitGTK relies on hunspell/aspell dictionaries from `/usr/share/hunspell`, but the Flatpak sandbox doesn't have access to system dictionaries
2. **No Dictionary Validation**: The app doesn't verify if dictionaries are actually available before setting languages
3. **No Auto-Detection**: While the app auto-detects the system locale, it doesn't check if corresponding hunspell dictionaries exist
4. **Poor User Experience**: Users enable spell checking but get no feedback when it doesn't work due to missing dictionaries
5. **Manual Language Entry**: Users must manually type language codes without knowing which dictionaries are available

## Goals

1. Make spell checking actually work in the Flatpak sandbox
2. Auto-detect and use system-available hunspell dictionaries
3. Only show/enable languages that have corresponding dictionaries installed
4. Provide clear feedback when dictionaries are missing
5. Bundle essential dictionaries with the Flatpak for common languages

## Proposed Solution

### 1. Dictionary Discovery System
Create a SpellCheckingManager that:
- Scans for available hunspell dictionaries in both system and Flatpak paths
- Maps locale codes to available dictionary files
- Validates dictionary availability before enabling languages
- Provides a list of available languages to the UI

### 2. Flatpak Dictionary Bundling
- Bundle hunspell dictionaries for common languages in the Flatpak
- Configure proper dictionary search paths for WebKitGTK in sandbox
- Add hunspell extension or bundle dictionaries as Flatpak module

### 3. Improved UI/UX
- Replace manual language code entry with a selection from available dictionaries
- Show only languages that have dictionaries installed
- Display clear status about which dictionaries are available
- Warn users if spell checking is enabled but no dictionaries are available

### 4. Smart Auto-Detection
- Match system locale to available dictionaries
- Fall back to base language if specific locale variant unavailable (e.g., en_US → en_GB)
- Automatically enable spell checking if user's system language has a dictionary

## Non-Goals

- Creating or maintaining hunspell dictionaries (we use upstream dictionaries)
- Supporting custom dictionary uploads
- Grammar checking (only spell checking via hunspell)

## Success Criteria

- [ ] Spell checking works out-of-the-box for users with common system languages
- [ ] Users can see which dictionaries are available before selecting them
- [ ] Clear error messages when spell checking can't work due to missing dictionaries
- [ ] At least English, Spanish, French, German, Portuguese dictionaries bundled
- [ ] Auto-detection matches system locale to available dictionary
- [ ] All spell checking settings respect dictionary availability

## Dependencies

- hunspell dictionary files (external, from upstream packages)
- Flatpak bundling system (already in use)
- WebKitGTK spell checking API (already in use)

## Risks & Mitigation

**Risk**: Flatpak size increase from bundling dictionaries
**Mitigation**: Bundle only most common languages (~10-15 languages), each dictionary is typically 500KB-2MB

**Risk**: WebKitGTK path configuration may not work in sandbox
**Mitigation**: Test with WEBKIT_SPELL_CHECKER_DIR environment variable and fallback paths

**Risk**: Dictionary file format changes in hunspell
**Mitigation**: Use stable hunspell 1.7+ format, test with GNOME runtime hunspell version
