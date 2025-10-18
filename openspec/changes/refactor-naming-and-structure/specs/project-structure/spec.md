# Spec: Project Structure

## ADDED Requirements

### Requirement: Source files use PascalCase naming convention
All Vala source files MUST use PascalCase naming to match class names and follow Vala standard conventions.

#### Scenario: Vala files match class names with PascalCase
**Given** a Vala source file containing a class
**When** naming the file
**Then** the filename must use PascalCase matching the class name
**And** the file must have a `.vala` extension
**And** the pattern must be `ClassName.vala` for class `ClassName`

**Examples**:
- Class `NotificationManager` → file `NotificationManager.vala`
- Class `PreferencesDialog` → file `PreferencesDialog.vala`
- Class `Utils` → file `Utils.vala`

#### Scenario: Template files use PascalCase with .in extension
**Given** a Vala template file requiring configuration substitution
**When** naming the template file
**Then** the filename must use PascalCase with `.vala.in` extension
**And** the generated file must use PascalCase with `.vala` extension

**Example**: `Config.vala.in` generates `Config.vala`

### Requirement: Source files organized in logical subdirectories
Source files MUST be grouped into subdirectories based on their architectural role for better code organization and navigation.

#### Scenario: Dialog classes in dialogs/ subdirectory
**Given** a class that extends a dialog widget (Adw.Dialog, Adw.PreferencesDialog, etc.)
**When** determining the file location
**Then** the file must be placed in `src/dialogs/` directory
**And** user-facing dialog windows must be in this directory

**Files in this directory**:
- `AboutDialog.vala`
- `PreferencesDialog.vala`
- `ShortcutsDialog.vala`

#### Scenario: Manager/service classes in managers/ subdirectory
**Given** a class that provides a service or manages a specific concern
**When** determining the file location
**Then** the file must be placed in `src/managers/` directory
**And** classes ending in "Manager" or providing services must be in this directory

**Files in this directory**:
- `AccessibilityManager.vala`
- `KeyboardShortcuts.vala`
- `NotificationManager.vala`
- `SettingsManager.vala`
- `WebKitManager.vala`
- `WhatsAppIntegration.vala`

#### Scenario: Utility classes in utils/ subdirectory
**Given** a class that provides utility functions, constants, or helper methods
**When** determining the file location
**Then** the file must be placed in `src/utils/` directory
**And** classes for constants, helpers, and utilities must be in this directory

**Files in this directory**:
- `Constants.vala`
- `DependencyContainer.vala`
- `Utils.vala`

#### Scenario: Core application files in root src/ directory
**Given** a core application lifecycle class (Application, Window, Main)
**When** determining the file location
**Then** the file must be placed in root `src/` directory
**And** only core application entry points belong in root

**Files in this directory**:
- `Application.vala`
- `Config.vala.in` / `Config.vala`
- `Main.vala`
- `Window.vala`

## MODIFIED Requirements

### Requirement: Dialog classes named with "Dialog" suffix
Classes that represent user-facing dialogs MUST be named with a "Dialog" suffix for clarity and consistency.

#### Scenario: Preferences class renamed to PreferencesDialog
**Given** a class extending `Adw.PreferencesDialog` for application preferences
**When** naming the class
**Then** the class must be named `PreferencesDialog`
**And** the file must be `src/dialogs/PreferencesDialog.vala`
**And** all references must use `PreferencesDialog`

**Previous Behavior**: Class named `Preferences`
**New Behavior**: Class named `PreferencesDialog`

#### Scenario: ShortcutsWindow class renamed to ShortcutsDialog
**Given** a class that wraps `AdwShortcutsDialog` for keyboard shortcuts help
**When** naming the class
**Then** the class must be named `ShortcutsDialog`
**And** the file must be `src/dialogs/ShortcutsDialog.vala`
**And** all references must use `ShortcutsDialog`

**Previous Behavior**: Class named `ShortcutsWindow`
**New Behavior**: Class named `ShortcutsDialog`
**Rationale**: The class provides a dialog interface, not a window

### Requirement: Class names use correct capitalization for branded terms
Classes referencing third-party technologies MUST use the official capitalization of those brand names.

#### Scenario: WebKit capitalization corrected
**Given** a class managing WebKit functionality
**When** naming the class
**Then** the class must be named `WebKitManager`
**And** "WebKit" must use capital W and capital K
**And** the file must be `src/managers/WebKitManager.vala`

**Previous Behavior**: Class named `WebkitManager` (lowercase k)
**New Behavior**: Class named `WebKitManager` (capital K)
**Rationale**: Matches official WebKit branding and casing

## REMOVED Requirements

None - this change adds structure, does not remove existing requirements.
