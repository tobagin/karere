# Spec: Build System

## MODIFIED Requirements

### Requirement: Build system references source files with PascalCase paths
The Meson build configuration MUST reference all source files using their new PascalCase names and directory structure.

#### Scenario: Source file list uses PascalCase paths
**Given** the meson.build file defining source files for compilation
**When** listing source files in the `files()` function
**Then** all file paths must use PascalCase naming
**And** all file paths must include subdirectory prefixes (`dialogs/`, `managers/`, `utils/`)
**And** core files must be in root `src/` with PascalCase names

**Previous Behavior**:
```meson
files(
  'src/application.vala',
  'src/preferences.vala',
  'src/shortcuts-window.vala',
  // ... etc
)
```

**New Behavior**:
```meson
files(
  'src/Application.vala',
  'src/dialogs/PreferencesDialog.vala',
  'src/dialogs/ShortcutsDialog.vala',
  'src/managers/NotificationManager.vala',
  // ... etc
)
```

#### Scenario: Config template path updated to PascalCase
**Given** the Config.vala.in template file configuration
**When** defining the configure_file() input path
**Then** the input path must be `'src/Config.vala.in'`
**And** the output must be `'Config.vala'`

**Previous Behavior**: `input: 'src/config.vala.in'`
**New Behavior**: `input: 'src/Config.vala.in'`

#### Scenario: All source files found during build
**Given** the updated meson.build with new file paths
**When** running `meson setup build`
**Then** all source files must be found successfully
**And** no "file not found" errors must occur
**And** the build must configure without warnings about missing files

### Requirement: Test configuration references renamed classes
Test files and test build configuration MUST reference classes by their new names.

#### Scenario: Test files reference PreferencesDialog
**Given** test files that instantiate or test the preferences dialog
**When** referencing the preferences class
**Then** tests must use `PreferencesDialog` class name
**And** tests must compile without "undeclared name" errors

**Previous Behavior**: References to `Preferences` class
**New Behavior**: References to `PreferencesDialog` class

#### Scenario: Test files reference ShortcutsDialog
**Given** test files that instantiate or test the shortcuts dialog
**When** referencing the shortcuts class
**Then** tests must use `ShortcutsDialog` class name
**And** tests must compile without "undeclared name" errors

**Previous Behavior**: References to `ShortcutsWindow` class
**New Behavior**: References to `ShortcutsDialog` class

#### Scenario: Test files reference WebKitManager
**Given** test files that use the WebKit manager
**When** referencing the WebKit manager class
**Then** tests must use `WebKitManager` with capital K
**And** tests must compile without "undeclared name" errors

**Previous Behavior**: References to `WebkitManager` (lowercase k)
**New Behavior**: References to `WebKitManager` (capital K)

## ADDED Requirements

None - this change modifies existing build requirements, does not add new ones.

## REMOVED Requirements

None - all existing build requirements remain, just with updated paths.
