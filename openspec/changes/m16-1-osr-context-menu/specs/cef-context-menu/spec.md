## ADDED Requirements

### Requirement: Render the context menu in windowless mode
Because the embedded view renders windowless (OSR), CEF cannot display its own context menu. The shell SHALL implement `ContextMenuHandler::run_context_menu` to display the supplied `MenuModel` as a host (GTK) menu anchored at the right-click position over the view, return `1` to take ownership of display, and invoke `RunContextMenuCallback::cont(command_id, event_flags)` for the chosen item or `cancel()` when the menu is dismissed without a selection — exactly once.

#### Scenario: Right-click renders a host menu
- **WHEN** the user right-clicks inside the embedded view and a menu model is produced
- **THEN** a GTK menu mirroring the model's items (labels, separators, enabled state, submenus) is shown at the cursor and `run_context_menu` returns `1`

#### Scenario: Selecting an item dispatches the command
- **WHEN** the user activates an item in the host menu
- **THEN** `RunContextMenuCallback::cont` is called once with that item's command id and the menu closes

#### Scenario: Dismissing the menu cancels
- **WHEN** the user closes the menu without choosing an item (click-away or Escape)
- **THEN** `RunContextMenuCallback::cancel` is called once and no command is dispatched

### Requirement: Spellcheck suggestions in the editable-field menu
When the right-click target is a misspelled word in an editable field, the host menu SHALL present Chromium's spellcheck suggestions and an "Add to dictionary" action, sourced from `ContextMenuParams::misspelled_word()` and `dictionary_suggestions()`, alongside the standard editing commands (cut, copy, paste, select all).

#### Scenario: Suggestions appear for a misspelled word
- **WHEN** the user right-clicks a red-underlined word in the WhatsApp composer
- **THEN** the host menu lists the dictionary suggestions for that word and an "Add to dictionary" item, and choosing a suggestion replaces the word in the field

#### Scenario: Add to dictionary
- **WHEN** the user chooses "Add to dictionary" for a flagged word
- **THEN** the corresponding command is dispatched via the callback and the word is no longer underlined

### Requirement: Cursor-accurate placement at display scale
The host menu SHALL be positioned at the `ContextMenuParams` coordinates translated through the view's scale factor, so the menu appears at the pointer on both standard and HiDPI displays.

#### Scenario: HiDPI placement
- **WHEN** the user right-clicks on a display with a scale factor greater than 1
- **THEN** the menu appears at the pointer location, not offset by the scale factor
