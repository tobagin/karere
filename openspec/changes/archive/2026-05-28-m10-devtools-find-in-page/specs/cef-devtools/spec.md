## ADDED Requirements

### Requirement: DevTools Opens On Action

The shell SHALL expose a `win.show-devtools` GAction with keyboard accelerators `F12` and `<Primary><Shift>i`. Invoking the action when DevTools is closed MUST open DevTools for the active page, docked inside the window.

#### Scenario: F12 opens DevTools

- **WHEN** the user presses `F12` with a loaded page
- **THEN** the DevTools pane appears docked in the window inspecting the active page
- **AND** the main WebView continues rendering and accepting input

#### Scenario: Ctrl+Shift+I opens DevTools

- **WHEN** the user presses `Ctrl+Shift+I` while DevTools is closed
- **THEN** the same `win.show-devtools` action runs and DevTools opens

### Requirement: DevTools Action Toggles

The `win.show-devtools` action MUST toggle: invoking it while DevTools is open MUST close the DevTools pane. The shell SHALL also expose a `win.close-devtools` GAction (and a close button in the pane header) that closes DevTools unconditionally.

#### Scenario: F12 again closes DevTools

- **WHEN** DevTools is open and the user presses `F12`
- **THEN** the DevTools view is closed and its pane is removed
- **AND** the main view repaints and is not left blank

#### Scenario: Close button stops DevTools

- **WHEN** DevTools is open and the user clicks the pane's close button (or invokes `win.close-devtools`)
- **THEN** DevTools closes identically to the toggle and the main view stays alive

### Requirement: DevTools Embedded Via CDP Frontend

Because CEF 148's Chrome runtime refuses windowless rendering for the browser created by `ShowDevTools` (it always opens a native top-level window), DevTools MUST instead be embedded by loading the Chrome DevTools Protocol (CDP) frontend page into an ordinary OSR `KarereWebView`. The shell MUST:

- pass `--remote-debugging-port` (loopback) at process start so the CDP HTTP/WebSocket endpoint is available;
- query the endpoint's target list and select the active content page (the WhatsApp page), never `about:blank`, a worker, or a DevTools frontend page;
- load that target's DevTools frontend URL into the OSR DevTools view.

The DevTools view MUST use a request handler that keeps every navigation in-view (it must not route the frontend to the external browser).

#### Scenario: DevTools renders inside the window

- **WHEN** DevTools opens
- **THEN** the CDP DevTools frontend is loaded into an OSR view docked in the window
- **AND** it is connected to the active page's CDP target and shows its DOM/console

#### Scenario: Target selection skips non-content pages

- **WHEN** the CDP target list contains `about:blank`, service workers, and the DevTools frontend page alongside the WhatsApp page
- **THEN** the shell selects the WhatsApp page target

### Requirement: Loopback Network Access Permitted For DevTools

The DevTools frontend is served from a public origin and must open a WebSocket to the loopback CDP endpoint. The shell MUST disable the Private/Local Network Access blocking features so this public→loopback connection succeeds; otherwise the DevTools frontend loads but stays blank.

#### Scenario: Frontend connects to the loopback endpoint

- **WHEN** the DevTools frontend page loads
- **THEN** its WebSocket connection to the loopback CDP endpoint is not blocked
- **AND** DevTools populates with live page data

### Requirement: DevTools Docks In A Resizable Bottom Pane

The embedded DevTools view MUST be placed in the bottom child of a vertical `gtk::Paned`, with the main WebView in the top child, so the divider is user-draggable. The pane MUST carry a header with a title and a close button, and MUST be hidden when DevTools is closed.

#### Scenario: DevTools is resizable

- **WHEN** DevTools is open
- **THEN** the DevTools view occupies the bottom pane of a vertical split with a header and close button
- **AND** the user can drag the divider to resize it
- **AND** closing DevTools hides the bottom pane
