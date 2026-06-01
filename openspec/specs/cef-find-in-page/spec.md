# cef-find-in-page Specification

## Purpose
TBD - created by archiving change m10-devtools-find-in-page. Update Purpose after archive.
## Requirements
### Requirement: Find Action Reveals Search Bar

The shell SHALL expose a `win.find-in-page` GAction with accelerator `<Primary>f`. Invoking the action MUST reveal a `gtk::SearchBar` defined in `data/ui/window.blp` and place keyboard focus in its `gtk::SearchEntry`.

#### Scenario: Ctrl+F reveals search bar

- **WHEN** the user presses `Ctrl+F` with the main window focused
- **THEN** the `gtk::SearchBar` becomes visible above the WebView
- **AND** the search entry has keyboard focus

### Requirement: Search Bar Drives Chromium Find

Typing in the search entry MUST call `BrowserHost::find(text, forward=true, match_case=false, find_next=false)` on each `connect_search_changed` emission, starting a fresh Chromium find for the new query.

#### Scenario: Typing starts a fresh find

- **WHEN** the user types "chat" in the search entry
- **THEN** `BrowserHost::find` is called with text "chat", `forward=true`, `match_case=false`, `find_next=false`
- **AND** Chromium highlights matches on the active page

### Requirement: Next And Previous Buttons Cycle Matches

The search bar MUST provide Next and Previous buttons. Clicking Next MUST call `BrowserHost::find(last_text, true, false, true)`; clicking Previous MUST call `BrowserHost::find(last_text, false, false, true)`. Both reuse the most recent query and pass `find_next=true` so Chromium cycles the existing match set.

#### Scenario: Next cycles forward

- **WHEN** there is an active query with multiple matches and the user clicks Next
- **THEN** `BrowserHost::find` is called with `forward=true` and `find_next=true`
- **AND** the active match advances by one

#### Scenario: Previous cycles backward

- **WHEN** there is an active query with multiple matches and the user clicks Previous
- **THEN** `BrowserHost::find` is called with `forward=false` and `find_next=true`
- **AND** the active match moves back by one

### Requirement: Match Counter Displays Active Of Count

The search bar MUST display a counter label in the form "active of count". Values MUST come from a `FindHandler::on_find_result` callback that writes a `FindResult { count, active }` into `SharedState`; the GTK polling loop MUST update the label from that state.

#### Scenario: Counter updates after find result

- **WHEN** Chromium fires `on_find_result` with `count=12` and `active_match_ordinal=3`
- **THEN** the search bar label reads "3 of 12" after the next GTK polling tick

### Requirement: Escape Closes Bar And Stops Finding

Pressing `Escape` while the search entry has focus MUST hide the search bar AND call `BrowserHost::stop_finding(clear_selection=true)` to drop highlights.

#### Scenario: Escape hides and clears

- **WHEN** the search entry has focus and the user presses `Escape`
- **THEN** the `gtk::SearchBar` is hidden
- **AND** `BrowserHost::stop_finding(true)` is called

### Requirement: Find Handler Registered On Client

`KarereClient` MUST own a `find_handler` field returning a `FindHandler` implementation built via the project's `wrap_find_handler!` macro pattern (mirroring other handlers in `src/handlers/`). `Client::get_find_handler` MUST be overridden to return this handler.

#### Scenario: Client exposes find handler

- **WHEN** CEF requests the find handler from the client
- **THEN** the registered `ShellFindHandlerBuilder` instance is returned
- **AND** `on_find_result` writes count and active ordinal into `SharedState`

