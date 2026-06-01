## 1. CefGtkArea accessors

- [x] 1.1 Add `CefGtkArea::close_browser()` that locks the browser mutex and calls `host.close_browser(0)` (force_close = 0)
- [x] 1.2 Add `CefGtkArea::is_browser_closed() -> bool` reading `life_span.lock().as_ref().map(|life| life.state.lock().closed).unwrap_or(true)`
- [x] 1.3 Call `close_browser()` from `CefGtkArea::unrealize` so widget removal also triggers the handshake

## 2. LifeSpan handler

- [x] 2.1 Add `closed: bool` field (default `false`) to `LifeSpanState` in `src/handlers/life_span.rs`
- [x] 2.2 Set `state.closed = true` inside `ShellLifeSpanHandler::on_before_close`
- [x] 2.3 Return `0` from `ShellLifeSpanHandler::do_close` to allow the close

## 3. Window close handshake

- [x] 3.1 Add a `closing: Cell<bool>` to `KarereWindow` (named `ShellWindow` pre-M7)
- [x] 3.2 Refactor `connect_close_request` from the M1 immediate-proceed pattern into the two-phase gated pattern
- [x] 3.3 On first close, set `closing = true`, call `web_area.close_browser()`, spawn a 50 ms `glib::timeout_add_local` poll that re-invokes `win.close()` when `is_browser_closed()` becomes true, and return `Propagation::Stop`
- [x] 3.4 On subsequent close when `closing` is set and `is_browser_closed()` is true, drop the poll source and return `Propagation::Proceed`

## 4. Verification

- [x] 4.1 Manual close produces `browser created` → `browser closed` → exit code 0 in logs
- [x] 4.2 `strace -f` shows no orphan zygote subprocesses after exit
- [x] 4.3 No `Couldn't release X11 display` warnings on shutdown for either manual close or `app.quit`
