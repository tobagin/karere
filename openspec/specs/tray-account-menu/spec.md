# tray-account-menu Specification

## Purpose

Defines the SNI tray context-menu structure (dynamic Show/Hide-window item, app-level actions, Quit) and the `app.refresh-tray-accounts` / `app.switch-account` stub actions that M20's multi-account UI fills in.

## Requirements

### Requirement: Tray context menu structure
The shell SHALL build the SNI context menu in this order: the dynamic Show/Hide-window item, a separator, `Preferences`, `Keyboard Shortcuts`, `About Karere`, a separator, then `Quit`. No per-account entry is rendered in M15.

#### Scenario: Default menu
- **WHEN** the tray menu is rendered
- **THEN** the menu contains the Show/Hide-window item, a separator, `Preferences`, `Keyboard Shortcuts`, `About Karere`, a separator, then `Quit`

### Requirement: Show/Hide-window menu item is dynamic and invokes present-window
The shell SHALL label the first menu item `Hide Window` when the primary chrome window is visible and `Show Window` when it is not, keeping the label in sync with the window's `notify::visible`, and SHALL wire its `activate` callback to invoke the `app.present-window` GAction.

#### Scenario: Window visible
- **WHEN** the primary window is visible and the tray menu is opened
- **THEN** the first item is labelled `Hide Window`

#### Scenario: Window hidden
- **WHEN** the primary window is hidden and the tray menu is opened
- **THEN** the first item is labelled `Show Window`

#### Scenario: User selects Show/Hide
- **WHEN** the user selects the Show/Hide-window item from the tray menu
- **THEN** the shell activates `app.present-window`

### Requirement: App-level menu items invoke their GActions
The shell SHALL wire the `Preferences`, `Keyboard Shortcuts`, and `About Karere` menu items to activate `app.preferences`, `app.show-help-overlay`, and `app.about` respectively.

#### Scenario: User selects an app-level item
- **WHEN** the user selects `About Karere` from the tray menu
- **THEN** the shell activates `app.about`

### Requirement: `Quit` menu item invokes app.quit
The shell SHALL wire the `Quit` menu item's `activate` callback to invoke the `app.quit` GAction.

#### Scenario: User selects Quit
- **WHEN** the user selects `Quit` from the tray menu
- **THEN** the shell activates `app.quit`
- **AND** the application exits cleanly (the M04 shutdown path runs)

### Requirement: `app.refresh-tray-accounts` requests a tray refresh
The shell SHALL provide an `app.refresh-tray-accounts` GAction that calls `ksni::Handle::update` so the next `menu()` invocation reflects any change to `TrayState`. The M15 menu is app-level only (no per-account entry); M20 will populate `TrayState.accounts` and add the per-account UI.

#### Scenario: Action runs before M20 lands
- **WHEN** `app.refresh-tray-accounts` is activated and no `AccountManager` is available
- **THEN** `TrayState.accounts` remains empty and the tray is asked to re-render without error

### Requirement: `app.switch-account` is a registered no-op stub
The shell SHALL register an `app.switch-account` GAction taking a single string parameter (the account id) that, in M15, performs no operation but is callable so M20's per-account menu items can target it without runtime errors.

#### Scenario: Activation with an account id
- **WHEN** `app.switch-account` is activated with the parameter `account-1`
- **THEN** the shell logs the activation at DEBUG and returns without error
