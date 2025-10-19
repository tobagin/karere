# Spell Checking Test Guide for Karere (Fedora 43)

This guide will help you test the newly implemented spell checking feature with bundled hunspell dictionaries in Karere on Fedora 43.

## Prerequisites

Before testing, ensure you have:
- Fedora 43 (or compatible Linux distribution)
- Flatpak installed and configured
- GNOME runtime 49 available

## Installation

### Option 1: Build and Install Development Version

The development version is already built. Install it with:

```bash
flatpak run io.github.tobagin.karere.Devel
```

### Option 2: Rebuild from Source

If you need to rebuild:

```bash
cd /home/tobagin/Projects/karere
./scripts/build.sh --dev
```

This will:
1. Download all 91 language dictionaries from LibreOffice (libreoffice-25.2.7.1)
2. Bundle them in `/app/share/hunspell` inside the Flatpak
3. Build and install the development version

## Verifying Dictionary Installation

### Check bundled dictionaries

Run the app and check the logs:

```bash
flatpak run io.github.tobagin.karere.Devel 2>&1 | grep -i "dictionary\|spell"
```

You should see output like:
```
SpellCheckingManager initialized with 91 dictionaries
Found dictionary: en_US at /app/share/hunspell
Found dictionary: en_GB at /app/share/hunspell
Found dictionary: es_ES at /app/share/hunspell
...
```

### Check available dictionaries list

You can also inspect the Flatpak installation directly:

```bash
flatpak run --command=sh io.github.tobagin.karere.Devel -c "ls -lah /app/share/hunspell/*.dic | wc -l"
```

This should output `91` (number of installed dictionaries).

### View all available languages

```bash
flatpak run --command=sh io.github.tobagin.karere.Devel -c "ls /app/share/hunspell/*.dic | xargs -n1 basename | sed 's/\.dic$//'"
```

This will list all available language codes.

## Testing Spell Checking Functionality

### Test 1: Auto-Detect System Language

1. **Launch Karere**:
   ```bash
   flatpak run io.github.tobagin.karere.Devel
   ```

2. **Check if your system language is detected**:
   - Open Preferences (Ctrl+,)
   - Go to "Spell Checking" section
   - Verify "Auto-detect system language" is enabled
   - The system should automatically select your locale's dictionary

3. **Verify auto-detection in logs**:
   ```bash
   flatpak run io.github.tobagin.karere.Devel 2>&1 | grep "Auto-detect\|Exact match\|Fallback"
   ```

### Test 2: Manual Language Selection

1. **Disable auto-detect**:
   - Open Preferences (Ctrl+,)
   - Toggle off "Auto-detect system language"

2. **Select languages manually**:
   - The UI currently uses the existing text entry system
   - You can enter language codes like: `en_US`, `es_ES`, `fr`, `de_DE`, `pt_BR`, etc.
   - The SpellCheckingManager will validate these codes against available dictionaries

3. **Verify language validation**:
   ```bash
   flatpak run io.github.tobagin.karere.Devel 2>&1 | grep "not available\|Language"
   ```
   - Invalid language codes will be logged and skipped

### Test 3: Actual Spell Checking in WhatsApp Web

1. **Launch Karere and log into WhatsApp Web**

2. **Enable spell checking** (if not already enabled):
   - Open Preferences (Ctrl+,)
   - Enable "Spell checking"
   - Ensure at least one language is configured

3. **Test misspellings**:
   - Open any chat conversation
   - Type intentional misspellings in the message input
   - Examples to test:
     - **English (en_US)**: Type "teh" (should suggest "the"), "recieve" (should suggest "receive")
     - **Spanish (es_ES)**: Type "porqe" (should suggest "porque"), "dond" (should suggest "donde")
     - **French (fr)**: Type "bonjor" (should suggest "bonjour"), "mersi" (should suggest "merci")
     - **German (de_DE)**: Type "danke shon" (should suggest "danke schön"), "guten tak" (should suggest "guten tag")
     - **Portuguese (pt_BR)**: Type "obrigadu" (should suggest "obrigado"), "ola" (should suggest "olá")

4. **Check for red underlines**:
   - Misspelled words should have red wavy underlines
   - Right-clicking on underlined words should show spelling suggestions

### Test 4: Multiple Languages

1. **Configure multiple languages**:
   - Open Preferences
   - Add multiple language codes separated by commas: `en_US,es_ES,fr`

2. **Test multilingual spell checking**:
   - Type sentences mixing languages
   - WebKitGTK should check spelling for all configured languages simultaneously

### Test 5: Language Fallback

1. **Test locale matching**:
   - Configure a specific locale variant that may not be available
   - For example, if `en_AU` (Australian English) dictionary is not bundled, the system should fall back to `en_GB` or `en_US`

2. **Check fallback in logs**:
   ```bash
   flatpak run io.github.tobagin.karere.Devel 2>&1 | grep "Fallback"
   ```

## Testing Different Language Examples

### Available Language Codes

Here are some of the 91 available languages you can test:

| Language | Code | Example Test Word |
|----------|------|-------------------|
| English (US) | en_US | "recieve" → "receive" |
| English (GB) | en_GB | "colour" (correct), "color" (US spelling) |
| Spanish (ES) | es_ES | "porqe" → "porque" |
| Portuguese (BR) | pt_BR | "obrigadu" → "obrigado" |
| Portuguese (PT) | pt_PT | "obrigada" (correct in PT) |
| French | fr | "bonjor" → "bonjour" |
| German | de_DE_frami | "danke shon" → "danke schön" |
| Italian | it_IT | "grazi" → "grazie" |
| Russian | ru_RU | Test Cyrillic misspellings |
| Arabic | ar | Test Arabic misspellings |
| Indonesian | id_ID | "trimakasih" → "terima kasih" |
| Dutch | nl_NL | "danke" → "dank je" |
| Polish | pl_PL | Test Polish words |
| Czech | cs_CZ | Test Czech words |
| Swedish | sv_SE | "tack" (correct) |
| Finnish | fi_FI | "kiitos" (correct) |
| Greek | el_GR | Test Greek words |
| Hebrew | he_IL | Test Hebrew words |
| Hindi | hi_IN | Test Devanagari script |
| Chinese | Various | Test Chinese characters |
| Korean | ko_KR | Test Korean Hangul |
| Japanese | Various | Test Japanese |

And many more! (91 total dictionaries)

## Debugging Issues

### Check WebKit Spell Checker Paths

Verify that WebKitGTK can see the dictionary paths:

```bash
flatpak run --command=sh io.github.tobagin.karere.Devel -c "echo \$WEBKIT_SPELL_CHECKER_DIR"
```

Expected output:
```
/app/share/hunspell:/usr/share/hunspell
```

### View All Logs

To see all spell checking related logs:

```bash
flatpak run io.github.tobagin.karere.Devel 2>&1 | grep -i "spell\|dictionary\|webkit"
```

### Check Dictionary Files Directly

Inspect the dictionary files:

```bash
flatpak run --command=sh io.github.tobagin.karere.Devel -c "ls -lh /app/share/hunspell/ | head -20"
```

Each language should have both `.dic` and `.aff` files.

### Verify Host Dictionaries Access (Optional)

If you want to also use host system dictionaries:

```bash
# Check if host dictionaries are accessible
flatpak run --command=sh io.github.tobagin.karere.Devel -c "ls -lh /usr/share/hunspell/ 2>&1"
```

This is configured with `--filesystem=/usr/share/hunspell:ro` in the Flatpak manifest.

## Common Issues & Solutions

### Issue: Spell checking not working

**Solution**:
1. Verify spell checking is enabled in Preferences
2. Check that at least one language is configured
3. Ensure the language code is valid (check logs for "not available" warnings)
4. Try restarting the app

### Issue: No dictionaries found

**Solution**:
1. Rebuild the Flatpak: `./scripts/build.sh --dev`
2. Verify the build log shows "Installed 91 dictionaries"
3. Check `/app/share/hunspell` in the Flatpak

### Issue: Wrong language detected

**Solution**:
1. Disable auto-detect
2. Manually specify your preferred language codes
3. Check your system locale: `locale`

### Issue: Suggestions not appearing

**Solution**:
1. Ensure you're typing in a WhatsApp message input field
2. Right-click on the underlined word (don't left-click)
3. The WebKitGTK context menu should show spelling suggestions

## Performance Testing

### Startup Time

Measure the impact of dictionary scanning on startup:

```bash
time flatpak run io.github.tobagin.karere.Devel --quit-after-load
```

Expected impact: < 50ms for scanning 91 dictionaries

### Memory Usage

Check memory usage with multiple dictionaries:

```bash
flatpak run io.github.tobagin.karere.Devel &
sleep 5
ps aux | grep karere | grep -v grep
```

Expected: Minimal memory overhead (< 10MB) for dictionary index cache

## Reporting Issues

When reporting spell checking issues, please include:

1. **System Information**:
   ```bash
   uname -a
   flatpak --version
   locale
   ```

2. **Dictionary Count**:
   ```bash
   flatpak run --command=sh io.github.tobagin.karere.Devel -c "ls -1 /app/share/hunspell/*.dic | wc -l"
   ```

3. **Spell Checking Logs**:
   ```bash
   flatpak run io.github.tobagin.karere.Devel 2>&1 | grep -i "spell\|dictionary" > spell-debug.log
   ```

4. **Language Configuration**:
   - Auto-detect enabled/disabled
   - Manually configured languages
   - Expected vs actual behavior

## Success Criteria

The spell checking system is working correctly if:

- ✅ 91 dictionaries are bundled in the Flatpak
- ✅ Auto-detect matches your system locale to an available dictionary
- ✅ Manual language selection works with valid codes
- ✅ Invalid language codes are filtered and logged
- ✅ Misspellings are underlined in WhatsApp Web message input
- ✅ Right-click shows spelling suggestions
- ✅ Multiple languages can be checked simultaneously
- ✅ Startup time impact is minimal (< 50ms)
- ✅ System works without host dictionaries (bundled ones are sufficient)

## Next Steps

After successful testing:

1. **UI Enhancement** (Phase 4 - not yet implemented):
   - Replace manual text entry with dropdown of available languages
   - Show dictionary count in preferences
   - Display friendly language names
   - Add status indicators

2. **Documentation**:
   - Update README.md with spell checking features
   - Document all 91 available languages
   - Add troubleshooting section

3. **Release**:
   - Tag new version
   - Update changelog
   - Publish to Flathub

---

**Last Updated**: 2025-10-19
**Tested On**: Fedora 43 with GNOME 49 runtime
**Dictionaries Version**: LibreOffice 25.2.7.1 (91 languages)
