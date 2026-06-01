## ADDED Requirements

### Requirement: Gated window close handshake

The `KarereWindow` SHALL intercept the first `close_request` signal, invoke `close_browser` on its embedded `CefGtkArea`, and keep the window visible until `is_browser_closed` returns true. The handler SHALL return `Propagation::Stop` on the first invocation and `Propagation::Proceed` only on a subsequent invocation observed after the browser has confirmed closure.

#### Scenario: First close request initiates handshake
- **WHEN** the user requests to close the window for the first time
- **THEN** the window sets its `closing` flag to true, calls `CefGtkArea::close_browser()`, schedules a 50 ms poll, and returns `Propagation::Stop`

#### Scenario: Poll observes browser closed and re-fires close
- **WHEN** the scheduled poll sees `is_browser_closed()` return true
- **THEN** the poll source is dropped and `win.close()` is invoked, causing the close handler to return `Propagation::Proceed`

#### Scenario: Window with no browser yet
- **WHEN** the window receives a close request before any browser has been created
- **THEN** `is_browser_closed()` returns true (default) and the window closes immediately on the first invocation

### Requirement: CEF lifecycle reports browser closed state

The `ShellLifeSpanHandler` SHALL maintain a `closed: bool` field on its `LifeSpanState`, set it to `true` inside `on_before_close`, and return `0` from `do_close` to permit the close.

#### Scenario: OnBeforeClose flips state
- **WHEN** CEF invokes `on_before_close` for the tracked browser
- **THEN** `state.closed` becomes true and any holder of the `LifeSpanState` mutex observes the change

#### Scenario: do_close allows close
- **WHEN** CEF invokes `do_close` on the handler
- **THEN** the handler returns `0`, allowing the standard CEF close path to proceed

### Requirement: Widget unrealize triggers close_browser

`CefGtkArea` SHALL call `close_browser()` from its `unrealize` implementation so the CEF handshake runs even when the widget is removed from its parent before the containing window emits `close_request`.

#### Scenario: Widget detached before window close
- **WHEN** the parent container removes the `CefGtkArea` widget while a browser is active
- **THEN** `unrealize` invokes `close_browser()` and the browser host receives the close request

### Requirement: Process exits cleanly on window close

When the last `KarereWindow` closes, the application SHALL exit with code 0 and SHALL NOT emit `Couldn't release X11 display` warnings or leave orphan renderer subprocesses.

#### Scenario: Manual close from window controls
- **WHEN** the user closes the last window via the window manager
- **THEN** logs show `browser created` followed by `browser closed` and the process exits with code 0

#### Scenario: app.quit shutdown
- **WHEN** the application receives `quit` while windows are open
- **THEN** each window completes its CEF handshake before GTK tears down, and no orphan zygote subprocesses remain visible under `strace -f`
