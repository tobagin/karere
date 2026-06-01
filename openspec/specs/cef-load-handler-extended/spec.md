# cef-load-handler-extended Specification

## Purpose

Extend the existing `LoadHandler` with `on_load_error` exponential-backoff retries that respect `ERR_ABORTED` and reset on successful load completion. Scopes retries to the main frame, ignores the Chromium error page, and surfaces load failures with a branded offline overlay instead of a blank surface.

## Requirements

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

### Requirement: Surface offline state with a branded overlay
The shell SHALL show an opaque status-page overlay ("No connection — Waiting for the network — retrying…") over the embedded view whenever the host is offline, driven by BOTH signals: the OS network state via `GNetworkMonitor::is_network_available`, AND the `LoadHandler`'s shared `offline` flag (set on a main-frame load failure, cleared on a successful non-error main-frame `on_load_end`). The window's 100 ms poll loop SHALL show the overlay when either signal indicates offline. When `GNetworkMonitor` transitions from unavailable back to available, the shell SHALL reload the browser so a service-worker-cached page reconnects.

The network monitor is required because WhatsApp Web's service worker serves a cached page while offline: that load succeeds, so no `on_load_error` fires and the load-error `offline` flag alone cannot detect the outage.

#### Scenario: Network drops while the page is loaded
- **WHEN** the host network goes down (e.g. `nmcli networking off`) while WhatsApp Web is already loaded from the service-worker cache
- **THEN** `GNetworkMonitor` reports the network unavailable and the window shows the opaque "No connection" overlay even though no load error fired

#### Scenario: Cold load fails with no network
- **WHEN** the initial load of `https://web.whatsapp.com/` fails because the network is unavailable
- **THEN** the `offline` flag is set and the window shows the opaque "No connection" overlay covering the view

#### Scenario: Network returns
- **WHEN** `GNetworkMonitor` transitions back to available
- **THEN** the shell reloads the browser and, once a successful main-frame `on_load_end` runs (or the network is available), the overlay is hidden, revealing the loaded page
