## Context

gtk-cef-shell is a single-account WebView template built on GTK4 +
libadwaita, embedding Chromium through the upstream CEF C API (the `cef`
crate, version 148). CEF needs a multi-process model: the same binary is
re-executed as renderer, GPU, and utility subprocesses, and the browser
process must own the message pump that drives Chromium work. Because we
intend to render offscreen into a GTK widget (M2 onward), we must run CEF in
windowless mode with an external message pump from day one — flipping this
later would invalidate the entire render path. M1 captures the boot
plumbing that every subsequent milestone depends on.

## Goals / Non-Goals

**Goals:**
- Single-binary CEF process model: one `cef::execute_process` call routes
  both browser and subprocess invocations.
- Browser-process boot: App, BrowserProcessHandler, and `cef::initialize`
  with windowless + external pump + no-sandbox settings.
- Reliable external message pump driven from the glib main loop so CEF
  never stalls.
- Wayland-first Chromium flags applied through
  `on_before_command_line_processing`.
- `cargo build` succeeds with `CEF_PATH` pointed at the downloaded CEF
  distribution; `cargo run` reaches `CEF initialized` and exits on Ctrl-C.

**Non-Goals:**
- Rendering pixels into the GTK widget — that is M2.
- Forwarding keyboard, mouse, IME, or focus into CEF — that is M3.
- Graceful shutdown beyond the existing `cef::shutdown` call — M4.
- Permission prompts and modal dialogs — M5.
- Flatpak manifest, runtime, and packaging — M6.
- Production-grade logging or crash reporting.

## Decisions

### Single binary with `--type` routing
We use one binary. `Args::new()` is passed to `cef::execute_process`; if the
parsed `CommandLine` has a `type` switch we treat the invocation as a
subprocess and `execute_process` runs it to completion (returning a
non-negative exit code we propagate). For the browser process,
`execute_process` returns `-1` and we continue into Adw setup. Alternative
considered: a separate `gtk-cef-shell-subprocess` binary like Chromium's
`chrome_crashpad_handler`. Rejected because a separate binary doubles the
build and Flatpak surface for no architectural gain at this size.

### External message pump driven by glib timer
CEF can either own its own thread loop (`multi_threaded_message_loop`) or
expose `do_message_loop_work()` for an embedder to call. With GTK we must
own the main loop, so `multi_threaded_message_loop` is impossible. The
documented path is `external_message_pump=1` plus a
`BrowserProcessHandler::on_schedule_message_pump_work` callback that
schedules a one-shot main-thread call.

In practice that callback fires from arbitrary CEF threads, and the
glib-rs `timeout_add_local` family is `!Send`, so we cannot schedule from
the callback directly without a Send wrapper. We sidestep that by leaving
`on_schedule_message_pump_work` as a no-op and installing one persistent
`glib::timeout_add_local` at 8 ms that calls `cef::do_message_loop_work()`
unconditionally. This trades a small amount of idle CPU for a robust pump
that never deadlocks early in startup. The 8 ms cadence matches a
~125 Hz tick — fast enough that input/network/timers feel responsive,
slow enough that idle cost stays under noise.

Alternative considered: build a `Send`-able trampoline (e.g.
`glib::MainContext::default().invoke`) from the schedule callback. Deferred
to a later milestone if the steady pump shows up as a measurable battery
or CPU regression.

### Windowless rendering enabled at init time
`Settings.windowless_rendering_enabled = 1` is set in M1 even though no OSR
browser is created yet. CEF reads this flag once at `initialize` time and
it cannot be toggled later, so it must be locked in now to keep M2 viable.

### `no_sandbox = 1` and conditional `--no-sandbox`
Chromium's suid sandbox conflicts with Flatpak's user namespace sandbox and
with a typical developer working tree that does not chown the helper. We
set `Settings.no_sandbox = 1` unconditionally, and additionally append
`--no-sandbox` to the command line when `FLATPAK_ID` is present so child
processes inherit the same posture. The eventual hardening story (seccomp
profile, portals) belongs in M6.

### GL loader via libepoxy + libloading
We need a GL function loader available before the first widget is realised
so that the future GtkGLArea can adopt CEF textures. We load `libepoxy.so`
through `libloading`, leak it for `'static`, and wire both `epoxy::load_with`
and `gl::load_with` to it. Doing this in M1 keeps M2 free of subtle init
ordering bugs.

### Adw `HANDLES_COMMAND_LINE` + `--url` flag
Activation routes through `connect_command_line` so secondary launches can
forward a URL into the running instance later (M4). For M1 we just consume
`--url=` from `std::env::args` before `adw_app.run()` and capture it in the
activation closure.

## Risks / Trade-offs

- [Idle CPU from 8 ms pump tick] → Acceptable for M1. Re-evaluate during M4
  with `powertop`; if it shows up, switch to a Send-wrapped one-shot from
  `on_schedule_message_pump_work`.
- [Hard dependency on `CEF_PATH`] → Documented via `download-cef.sh`. The
  meson custom target forwards the env var so packagers can override.
- [`cef::shutdown` after `adw_app.run()` only — no graceful teardown of
  browsers] → Acceptable in M1 because we do not create browsers yet.
  Tracked for M4.
- [`no-sandbox` on the developer host] → A known trade-off for M1; the
  hardening posture is owned by M6.
- [Single binary increases startup parse cost for subprocesses] →
  Negligible vs. Chromium's own startup cost; not worth a second binary.

## Migration Plan

This is greenfield code; no migration. To pick up the change locally:

1. `./download-cef.sh` to populate `cef-binaries/current/`.
2. `CEF_PATH=$(pwd)/cef-binaries/current/Release cargo build`.
3. `CEF_PATH=...Release ./target/debug/gtk-cef-shell --url=https://example.com`.

Rollback is `git revert` of the M1 commit set; there is no persisted
state to migrate.

## Open Questions

- Should the steady 8 ms pump be replaced by an `on_schedule_message_pump_work`
  trampoline before M4? Decide once the M3 input path lands and we have real
  workload measurements.
- Do we want a `--remote-debugging-port` switch behind a debug build flag?
  Out of scope for M1; revisit when DevTools wiring is needed.
