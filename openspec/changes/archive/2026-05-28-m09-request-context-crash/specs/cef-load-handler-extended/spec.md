## ADDED Requirements

### Requirement: Exponential-backoff retry on load failure
The existing `LoadHandler` SHALL implement `on_load_error` to ignore `ERR_ABORTED`, increment `SharedState.load_error_count` for every other error, and schedule `browser.reload_ignore_cache()` through `glib::timeout_add_local` with `delay_ms = min(60_000, 500 * 2.pow(count))`.

#### Scenario: Transient network failure during autostart
- **WHEN** the initial load of `https://web.whatsapp.com/` fails with a non-`ERR_ABORTED` error code
- **THEN** the handler schedules a reload after 500 ms, then 1000 ms, then 2000 ms on subsequent consecutive failures, capping at 60 000 ms

#### Scenario: User cancels an in-flight navigation
- **WHEN** `on_load_error` fires with error code `ERR_ABORTED`
- **THEN** the handler returns without bumping `load_error_count` or scheduling a reload

### Requirement: Reset retry counter on successful load
The `LoadHandler` SHALL reset `SharedState.load_error_count` to `0` and cancel any pending retry `SourceId` inside `on_load_end` so a successful navigation clears prior backoff state.

#### Scenario: Reload after backoff succeeds
- **WHEN** a previously failing page finally finishes loading and `on_load_end` runs
- **THEN** `SharedState.load_error_count` is `0` and no future load failure inherits the previous backoff

### Requirement: Scope retries to the main frame and ignore the Chromium error page
The `LoadHandler` SHALL only drive retries and offline state from main-frame loads: `on_load_error` for a sub-frame SHALL return without scheduling a reload, and `on_load_end` SHALL treat a `chrome-error://` URL as a failed load (returning without clearing `load_error_count`, cancelling the pending reload, or clearing the offline state) so the backoff and overlay survive the error page that Chromium loads after a failure.

#### Scenario: Sub-frame load fails
- **WHEN** `on_load_error` fires for a non-main frame (e.g. an embedded analytics iframe)
- **THEN** the handler returns without incrementing `load_error_count` or scheduling a reload

#### Scenario: Chromium error page commits after a failure
- **WHEN** a main-frame load fails and Chromium navigates the main frame to its `chrome-error://` page, firing `on_load_end`
- **THEN** the handler does not reset `load_error_count`, does not cancel the pending retry, and leaves the offline state set

### Requirement: Surface load failures with an offline overlay
On a main-frame load failure the `LoadHandler` SHALL set a shared `offline` flag (cleared on a successful, non-error main-frame `on_load_end`). The window SHALL poll this flag and show an opaque status-page overlay ("No connection — Waiting for the network — retrying…") over the embedded view while it is set, so a load failure shows a branded message instead of a blank surface or the Chromium error page.

#### Scenario: Initial load fails with no network
- **WHEN** the initial load of `https://web.whatsapp.com/` fails because the network is unavailable
- **THEN** the `offline` flag is set and the window shows the opaque "No connection" overlay covering the view

#### Scenario: Network returns and the page loads
- **WHEN** a retry finally completes a successful main-frame load and `on_load_end` runs
- **THEN** the `offline` flag is cleared and the overlay is hidden, revealing the loaded page
