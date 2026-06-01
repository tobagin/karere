## ADDED Requirements

### Requirement: Keyboard shortcuts dialog enumerates every bound accelerator
The application SHALL provide an `AdwShortcutsDialog` built from a blueprint composite template (`data/ui/keyboard-shortcuts.blp`) listing every accelerator the shell binds, grouped by category. The listing SHALL include all template-core action accelerators registered in M8 plus Ctrl+F (find-in-page), F12 (devtools), Ctrl+Shift+I (devtools), Ctrl+W (close-or-background), and Ctrl+B (toggle headerbar zoom-controls where the feature is enabled).

#### Scenario: Dialog lists M8 template-core accelerators
- **WHEN** the shortcuts dialog is presented
- **THEN** every accelerator registered by M8's template-core action wiring appears as a row in the dialog, grouped by category

#### Scenario: Dialog lists additional milestone accelerators
- **WHEN** the shortcuts dialog is presented
- **THEN** rows are present for Ctrl+F (find-in-page), F12 (devtools), Ctrl+Shift+I (devtools), Ctrl+W (close-or-background), and Ctrl+B (toggle headerbar zoom-controls)

### Requirement: app.show-help-overlay presents the shortcuts dialog
The application SHALL implement the `app.show-help-overlay` GAction (stubbed in M8) by constructing the `AdwShortcutsDialog` and presenting it on the active window. The Ctrl+? accelerator bound to this action in M8 SHALL therefore open the dialog.

#### Scenario: Ctrl+? opens the dialog
- **WHEN** the user presses Ctrl+?
- **THEN** the `app.show-help-overlay` action is activated
- **AND** the `AdwShortcutsDialog` is presented modal over the active window

#### Scenario: Menu activation also presents the dialog
- **WHEN** the user activates `app.show-help-overlay` from the application menu
- **THEN** the `AdwShortcutsDialog` is presented modal over the active window
