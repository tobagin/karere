# cef-context-menu Specification

## Purpose

Provide a CEF `ContextMenuHandler` that strips "open in new window/tab/incognito" entries so the shell exposes only navigation actions it can honor, while leaving unrelated commands intact.

## Requirements

### Requirement: Strip new-window/tab/incognito entries from the link context menu
The shell SHALL install a CEF `ContextMenuHandler` whose `on_before_context_menu` walks the supplied `MenuModel` and removes every entry whose command id matches `cef::sys::cef_menu_id_t::MENU_ID_OPEN_LINK_NEW_WINDOW`, `MENU_ID_OPEN_LINK_NEW_TAB`, or `MENU_ID_OPEN_LINK_IN_INCOGNITO_WINDOW`, along with any separators that immediately wrap the removed entries.

#### Scenario: Right-click a link inside the embedded view
- **WHEN** the user right-clicks an anchor element rendered by WhatsApp Web
- **THEN** the resulting context menu does not include "Open Link in New Window", "Open Link in New Tab", or "Open Link in Incognito Window", and the menu contains no leading or trailing separator orphaned by the removals

#### Scenario: Right-click on plain page chrome
- **WHEN** the user right-clicks an area with no link target
- **THEN** the handler leaves the unrelated commands (copy, paste, inspect, etc.) intact in their original order
