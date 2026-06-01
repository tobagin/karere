## ADDED Requirements

### Requirement: First-run migration dialog fires for upgrading v3 users only

On application startup, when `$XDG_DATA_HOME/karere/sessions/` exists AND `$XDG_DATA_HOME/karere/accounts/accounts.json` does NOT exist AND the GSetting `migration-acknowledged-v4` is `false`, the application SHALL present a one-shot `AdwAlertDialog` titled "Welcome to Karere 4.0" before any main window content becomes interactive.

#### Scenario: Upgrading v3 user sees the dialog

- **WHEN** the application launches with a v3 `sessions/` directory present and no v4 `accounts/accounts.json` and `migration-acknowledged-v4` is `false`
- **THEN** an `AdwAlertDialog` titled "Welcome to Karere 4.0" is shown.

#### Scenario: Fresh install never sees the dialog

- **WHEN** the application launches with neither `sessions/` nor `accounts/accounts.json` present
- **THEN** no migration dialog is shown.

#### Scenario: v4 user who already paired does not see the dialog

- **WHEN** the application launches with both `sessions/` and `accounts/accounts.json` present
- **THEN** no migration dialog is shown.

#### Scenario: Dialog never re-fires after acknowledgment

- **WHEN** the application launches with `migration-acknowledged-v4` set to `true`
- **THEN** no migration dialog is shown, regardless of the state of `sessions/` or `accounts/accounts.json`.

### Requirement: Dialog body explains the engine switch and the data position

The migration dialog body SHALL state that Karere 4 uses a new web engine, that the user must re-link their WhatsApp account(s), that existing chat history stays on the phone, and that old session data can be removed safely.

#### Scenario: Body content

- **WHEN** the dialog is displayed
- **THEN** its body text reads, substantively, "Karere 4 uses a new web engine. You'll need to re-link your WhatsApp account(s). Existing chat history stays on your phone; old session data can be removed safely."

### Requirement: Dialog offers two actions and both latch acknowledgment

The dialog SHALL present exactly two response actions: "Open Settings" and "Got it". Whichever action the user picks, the GSetting `migration-acknowledged-v4` SHALL be set to `true` before the dialog closes.

#### Scenario: Open Settings opens add-account flow

- **WHEN** the user selects the "Open Settings" action
- **THEN** the add-account dialog is presented immediately after the migration dialog dismisses.

#### Scenario: Got it dismisses without further UI

- **WHEN** the user selects the "Got it" action
- **THEN** the migration dialog dismisses and no further migration UI is shown.

#### Scenario: Acknowledgment latched on either action

- **WHEN** the user selects either action
- **THEN** `migration-acknowledged-v4` is set to `true` in GSettings.

#### Scenario: Acknowledgment latched even on window-close dismissal

- **WHEN** the user dismisses the dialog via window close or Escape rather than a response button
- **THEN** `migration-acknowledged-v4` is set to `true` in GSettings.

### Requirement: GSetting key is declared in the project schema

The project's GSettings schema SHALL declare a key named `migration-acknowledged-v4` of type boolean with a default value of `false`.

#### Scenario: Key declared with correct type and default

- **WHEN** the compiled GSettings schema is queried for `migration-acknowledged-v4`
- **THEN** the key is present, its type is boolean, and its default value is `false`.

### Requirement: Migration dialog does not touch legacy data

The migration flow SHALL NOT read, modify, move, or delete any file under `$XDG_DATA_HOME/karere/sessions/`.

#### Scenario: Legacy directory untouched after dialog

- **WHEN** the user acknowledges the dialog via any action
- **THEN** the contents of `$XDG_DATA_HOME/karere/sessions/` are byte-identical to their pre-dialog state.
