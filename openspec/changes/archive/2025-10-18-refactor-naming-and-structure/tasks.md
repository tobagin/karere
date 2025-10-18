# Tasks: Refactor Naming and Structure

## Overview
This task list breaks down the refactoring into small, verifiable steps to reorganize the codebase with PascalCase filenames and logical directory structure.

## Task List

### 1. Create Directory Structure
**Status**: Pending
**Estimated Effort**: 2 minutes
**Parallelizable**: No (blocks file moves)

Create the new directory structure under `src/`:

```bash
mkdir -p src/dialogs
mkdir -p src/managers
mkdir -p src/utils
```

**Validation**:
- [x] `src/dialogs/` directory exists
- [x] `src/managers/` directory exists
- [x] `src/utils/` directory exists

**Deliverable**: Three new subdirectories in `src/`

---

### 2. Move Dialog Files with git mv
**Status**: Pending
**Estimated Effort**: 5 minutes
**Parallelizable**: Can be done concurrently with tasks 3-4
**Blocks**: Task 5

Move dialog files to `src/dialogs/` with PascalCase names:

```bash
git mv src/about-dialog.vala src/dialogs/AboutDialog.vala
git mv src/preferences.vala src/dialogs/PreferencesDialog.vala
git mv src/shortcuts-window.vala src/dialogs/ShortcutsDialog.vala
```

**Validation**:
- [x] `src/dialogs/AboutDialog.vala` exists
- [x] `src/dialogs/PreferencesDialog.vala` exists
- [x] `src/dialogs/ShortcutsDialog.vala` exists
- [x] Old files no longer exist in `src/`
- [x] `git status` shows renamed files

**Deliverable**: Three dialog files moved and renamed

---

### 3. Move Manager Files with git mv
**Status**: Pending
**Estimated Effort**: 10 minutes
**Parallelizable**: Can be done concurrently with tasks 2,4
**Blocks**: Task 5

Move manager files to `src/managers/` with PascalCase names:

```bash
git mv src/accessibility-manager.vala src/managers/AccessibilityManager.vala
git mv src/keyboard-shortcuts.vala src/managers/KeyboardShortcuts.vala
git mv src/notification-manager.vala src/managers/NotificationManager.vala
git mv src/settings-manager.vala src/managers/SettingsManager.vala
git mv src/webkit-manager.vala src/managers/WebKitManager.vala
git mv src/whatsapp-integration.vala src/managers/WhatsAppIntegration.vala
```

**Validation**:
- [x] All 6 manager files moved to `src/managers/`
- [x] All files use PascalCase naming
- [x] Old files no longer exist in `src/`
- [x] `git status` shows renamed files

**Deliverable**: Six manager files moved and renamed

---

### 4. Move Utility Files with git mv
**Status**: Pending
**Estimated Effort**: 5 minutes
**Parallelizable**: Can be done concurrently with tasks 2-3
**Blocks**: Task 5

Move utility files to `src/utils/` with PascalCase names:

```bash
git mv src/constants.vala src/utils/Constants.vala
git mv src/dependency-container.vala src/utils/DependencyContainer.vala
git mv src/utils.vala src/utils/Utils.vala
```

**Validation**:
- [x] All 3 utility files moved to `src/utils/`
- [x] All files use PascalCase naming
- [x] Old files no longer exist in `src/`
- [x] `git status` shows renamed files

**Deliverable**: Three utility files moved and renamed

---

### 5. Rename Core Files with git mv
**Status**: Pending
**Estimated Effort**: 5 minutes
**Parallelizable**: Must follow tasks 2-4
**Blocks**: Task 6

Rename core application files in root `src/` to PascalCase:

```bash
git mv src/application.vala src/Application.vala
git mv src/config.vala.in src/Config.vala.in
git mv src/main.vala src/Main.vala
git mv src/window.vala src/Window.vala
```

**Validation**:
- [x] All 4 core files renamed to PascalCase
- [x] Old lowercase files no longer exist
- [x] `Config.vala.in` template renamed correctly
- [x] `git status` shows renamed files

**Deliverable**: Four core files renamed

---

### 6. Update Class Names in Dialog Files
**Status**: Pending
**Estimated Effort**: 10 minutes
**Parallelizable**: No (requires files to be moved first)
**Blocks**: Task 8

Update class names in dialog files:

**6.1**: Update `src/dialogs/PreferencesDialog.vala`
- Change `public class Preferences` → `public class PreferencesDialog`
- No other changes needed (already extends `Adw.PreferencesDialog`)

**6.2**: Update `src/dialogs/ShortcutsDialog.vala`
- Change `public class ShortcutsWindow` → `public class ShortcutsDialog`
- Update constructor documentation if needed

**6.3**: Verify `src/dialogs/AboutDialog.vala`
- Class name already correct (`AboutDialog`)
- No changes needed

**Validation**:
- [x] `PreferencesDialog` class declared in PreferencesDialog.vala
- [x] `ShortcutsDialog` class declared in ShortcutsDialog.vala
- [x] `AboutDialog` class declared in AboutDialog.vala
- [x] All class names match file names

**Deliverable**: Updated class names in dialog files

---

### 7. Update WebKitManager Class Name
**Status**: Pending
**Estimated Effort**: 5 minutes
**Parallelizable**: Can be done concurrently with task 6
**Blocks**: Task 8

Update class name in `src/managers/WebKitManager.vala`:

- Change `public class WebKitManager` (if currently `WebkitManager`)
- Update any internal references to match capitalization

**Validation**:
- [x] Class declared as `public class WebKitManager`
- [x] File compiles without errors

**Deliverable**: WebKitManager class with correct capitalization

---

### 8. Update References to Renamed Classes
**Status**: Pending
**Estimated Effort**: 30 minutes
**Parallelizable**: Must follow tasks 6-7

Update all references to renamed classes throughout the codebase:

**8.1**: Update Application.vala references
- Change `Preferences` → `PreferencesDialog`
- Change `WebkitManager` → `WebKitManager` (if needed)
- Change method `show_preferences()` calls to use `PreferencesDialog`

**8.2**: Update Window.vala references
- Change `WebkitManager` → `WebKitManager` (if needed)

**8.3**: Update KeyboardShortcuts.vala references
- Change `ShortcutsWindow` → `ShortcutsDialog`
- Change `show_shortcuts_window()` → `show_shortcuts_dialog()`
- Change `new ShortcutsWindow()` → `new ShortcutsDialog()`

**8.4**: Update any other files with references
- Search for `ShortcutsWindow`, `Preferences`, `WebkitManager`
- Update all found references

**Validation**:
- [x] No references to `ShortcutsWindow` remain
- [x] No references to old `Preferences` class remain
- [x] No references to `WebkitManager` (lowercase k) remain
- [x] Code compiles without "undeclared name" errors
- [x] `rg "ShortcutsWindow|class Preferences[^D]|WebkitManager" src/` returns no results

**Deliverable**: All class references updated

---

### 9. Update meson.build Source File List
**Status**: Pending
**Estimated Effort**: 15 minutes
**Parallelizable**: Can be done concurrently with task 8

Update `meson.build` to reference new file paths:

Find the `karere_sources` or similar variable and update all file paths:

```meson
karere_sources = files(
  'src/Main.vala',
  'src/Application.vala',
  'src/Window.vala',
  'src/dialogs/AboutDialog.vala',
  'src/dialogs/PreferencesDialog.vala',
  'src/dialogs/ShortcutsDialog.vala',
  'src/managers/AccessibilityManager.vala',
  'src/managers/KeyboardShortcuts.vala',
  'src/managers/NotificationManager.vala',
  'src/managers/SettingsManager.vala',
  'src/managers/WebKitManager.vala',
  'src/managers/WhatsAppIntegration.vala',
  'src/utils/Constants.vala',
  'src/utils/DependencyContainer.vala',
  'src/utils/Utils.vala',
)
```

Also update `config_vala` reference if needed:
```meson
config_vala = configure_file(
    input: 'src/Config.vala.in',
    output: 'Config.vala',
    configuration: conf_data
)
```

**Validation**:
- [x] All source file paths updated in meson.build
- [x] Config.vala.in path updated
- [x] `meson setup build --wipe` completes without errors
- [x] No "file not found" errors during configuration

**Deliverable**: Updated meson.build with new file paths

---

### 10. Update Test File References
**Status**: Pending
**Estimated Effort**: 10 minutes
**Parallelizable**: Can be done concurrently with task 9

Update test files if they reference renamed classes:

Check `tests/` directory for:
- References to `ShortcutsWindow` → `ShortcutsDialog`
- References to `Preferences` → `PreferencesDialog`
- References to `WebkitManager` → `WebKitManager`

Update `tests/meson.build` if it has separate source lists.

**Validation**:
- [x] All test file references updated
- [x] Test meson configuration updated (if applicable)
- [x] No compilation errors in test code

**Deliverable**: Updated test files and configuration

---

### 11. Update Project Documentation
**Status**: Pending
**Estimated Effort**: 10 minutes
**Parallelizable**: Can be done any time after task 1

Update `openspec/project.md`:

**11.1**: Update "File Organization" section
- Change example from "e.g., `application.vala`, `window.vala`"
- To: "e.g., `Application.vala`, `Window.vala`"
- Add note: "Files use PascalCase to match class names (Vala standard)"

**11.2**: Add directory structure documentation
- Document the three subdirectories: `dialogs/`, `managers/`, `utils/`
- Explain the organizational principle

**11.3**: Update component examples
- Change `Preferences` → `PreferencesDialog`
- Change `ShortcutsWindow` → `ShortcutsDialog`

**Validation**:
- [x] project.md reflects PascalCase convention
- [x] Directory structure documented
- [x] Examples use correct class names

**Deliverable**: Updated project.md documentation

---

### 12. Build and Test
**Status**: Pending
**Estimated Effort**: 15 minutes
**Parallelizable**: Must follow all previous tasks

Perform full build and runtime testing:

**12.1**: Clean build
```bash
rm -rf build
meson setup build
ninja -C build
```

**12.2**: Runtime testing
- Launch application
- Open Preferences dialog
- Open Shortcuts dialog
- Open About dialog
- Test keyboard shortcuts still work
- Verify all managers initialize

**Validation**:
- [x] Clean build completes without errors
- [x] No missing file errors
- [x] Application launches successfully
- [x] All dialogs open correctly
- [x] Preferences dialog works
- [x] Shortcuts dialog displays (Ctrl+?)
- [x] About dialog opens
- [x] No runtime warnings about missing classes

**Deliverable**: Fully functional application with new structure

---

### 13. Verify Git History Preservation
**Status**: Pending
**Estimated Effort**: 5 minutes
**Parallelizable**: Can be done any time after task 5

Verify that git correctly tracks file renames:

```bash
git log --follow src/dialogs/PreferencesDialog.vala
git log --follow src/managers/NotificationManager.vala
git log --follow src/Application.vala
```

**Validation**:
- [x] `git log --follow` shows history before rename
- [x] `git mv` operations properly detected
- [x] `git status` shows renames, not deletions+additions
- [x] `git diff --cached` shows file renames cleanly

**Deliverable**: Confirmed git history preservation

---

### 14. Update Code Comments
**Status**: Pending
**Estimated Effort**: 10 minutes
**Parallelizable**: Can be done concurrently with other tasks

Search for and update any code comments referencing old names:

```bash
rg -i "shortcuts.?window|class preferences[^d]|webkitmanager" src/ --type vala
```

Update any found references in:
- File headers
- Class documentation
- Method comments
- TODO comments

**Validation**:
- [x] No TODO/FIXME comments reference old class names
- [x] Documentation comments use correct names
- [x] File headers accurate

**Deliverable**: Updated code comments

---

## Summary

**Total Tasks**: 14
**Critical Path**: 1 → 2,3,4 → 5 → 6,7 → 8,9 → 12
**Estimated Total Effort**: ~2-3 hours

**Parallel Opportunities**:
- Tasks 2, 3, 4 can run in parallel (file moves)
- Tasks 6, 7 can run in parallel (class renames)
- Tasks 8, 9, 10 can run in parallel (reference updates)
- Tasks 11, 13, 14 can run any time

**Risk Mitigation**:
- Use `git mv` to preserve history
- Build after each phase to catch issues early
- Keep changes in separate commits for easy rollback

## Success Metrics

- [x] All files in PascalCase
- [x] Logical directory structure (dialogs/, managers/, utils/)
- [x] All class names accurate and consistent
- [x] Clean build with no errors
- [x] Application runs correctly
- [x] Git history preserved
- [x] Documentation updated
