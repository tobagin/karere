## ADDED Requirements

### Requirement: Per-account RequestContext isolation
The system SHALL create a distinct `RequestContext` for every account, rooted at a per-account `cache_path`.

#### Scenario: Each browser uses its own RequestContext
- **WHEN** a CEF `Browser` is created for an account
- **THEN** it is constructed with a `RequestContext` built from `RequestContextSettings { cache_path: $XDG_DATA_HOME/karere/accounts/sessions/<account_id>/data, ..Default::default() }`

#### Scenario: Cookies and storage do not leak between accounts
- **GIVEN** two accounts A and B with active browsers
- **WHEN** account A logs out
- **THEN** account B's session remains intact (cookies, IndexedDB, and Service Worker registrations are unaffected)

### Requirement: Browser pool with foreground pointer
The system SHALL maintain all account browsers in a single in-process map and track the foreground account via a separate pointer.

#### Scenario: Browser map and foreground pointer types
- **WHEN** `KarereWebView` is compiled
- **THEN** it owns a `Mutex<HashMap<AccountId, Browser>>` for the browsers
- **AND** a `Mutex<Option<AccountId>>` for the foreground id

#### Scenario: Switching swaps the foreground pointer
- **WHEN** the user activates account `new_id` while account `prev_id` is foreground
- **THEN** `host(prev_id).was_hidden(true)` is called
- **AND** the foreground pointer is set to `new_id`
- **AND** `host(new_id).was_hidden(false)` is called
- **AND** `host(new_id).was_resized()` is called
- **AND** the OSR widget is invalidated via `queue_render`

### Requirement: Background browsers paused but not closed
The system SHALL pause background browsers via `BrowserHost::was_hidden(true)` and SHALL NOT call `close_browser` to switch accounts.

#### Scenario: Background browser receives was_hidden
- **WHEN** an account is moved out of the foreground
- **THEN** its browser host receives `was_hidden(true)`
- **AND** the browser is NOT closed

#### Scenario: Session survives switching away and back
- **GIVEN** account A is logged in
- **WHEN** the user switches to B and then back to A
- **THEN** account A does not require a fresh QR scan

### Requirement: OSR paint gating
The system SHALL discard paint output and view-rect calls from non-foreground browsers.

#### Scenario: on_paint early-returns for background browsers
- **WHEN** `RenderHandler::on_paint` is invoked with `browser_id != foreground`
- **THEN** the handler returns early without uploading any pixel data to the GL texture

#### Scenario: view_rect early-returns for background browsers
- **WHEN** `RenderHandler::view_rect` is invoked with `browser_id != foreground`
- **THEN** the handler returns the cached foreground viewport rect (or a zero-sized rect when no foreground exists)

### Requirement: Add-account spawns a hidden browser
The system SHALL spawn a hidden CEF browser for each newly added account and rely on the Store hook to populate its identity.

#### Scenario: Adding an account allocates a hidden browser
- **WHEN** a new `Account` is added
- **THEN** a CEF `Browser` is created with that account's `RequestContext`
- **AND** the browser navigates to `https://web.whatsapp.com`
- **AND** the browser starts in the hidden state (`was_hidden(true)`) unless it is the only account

#### Scenario: Identity arrives via IPC populates the row
- **WHEN** `RendererMessage::ProfileIdentity { wid, pushname }` arrives for the new account
- **THEN** `AccountManager::update_identity(id, wid, pushname)` is invoked
