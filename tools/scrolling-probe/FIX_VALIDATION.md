# CEF scheduling fix for CPU-rendered scrolling

The application fix is in `src/cef_pump.rs`, with a continuation requested after
CPU or accelerated frame delivery in `src/handlers/render.rs`. It restores the
confirmed frame pipeline stall without enabling GPU Rendering or shortening the idle backstop.
The original investigation and inconclusive manual comparisons remain in
[FINDINGS.md](FINDINGS.md).

## Cause and change

In the bundled CEF 152.0.6 engine,
[`MessagePumpExternal::Run`](https://github.com/chromiumembedded/cef/blob/708dc14/libcef/browser/browser_message_loop.cc)
processes a bounded work slice. Its exit after exhausting that slice does not
itself issue `OnScheduleMessagePumpWork` for remaining immediate work. The
observed CPU-frame workload repeatedly reached this boundary: pending frame
processing waited for Karere's independent 100 ms fallback, producing about
10 GTK draws/s even while the page animated at 60 updates/s.

Karere now explicitly requests another main-context turn after accepting an
off-screen frame, including accelerated frames that can occasionally hit the same
boundary. That turn runs after GTK input and redraw sources, at
`glib::Priority::DEFAULT_IDLE`, matching the work priority used by
[Chromium's GLib pump](https://chromium.googlesource.com/chromium/src/+/152.0.7977.83/base/message_loop/message_pump_glib.cc).
Immediate CEF work no longer incurs an unconditional 8 ms timer delay.

One process-wide scheduler owns both demand-driven work and the 100 ms idle
backstop. Earlier requests replace later pending deadlines; bursts coalesce.
Requests received during CEF dispatch are deferred until it returns, preventing
reentry through a nested GTK main-context iteration. A generation check rejects
obsolete dispatches, and shutdown cancels the source before `cef::shutdown()`.
When painting and scheduling requests stop, the continuation stops and the slow
backstop remains. The existing CEF work-source detachment patch for #151 stays
in place.

This is a Karere integration fix for the observed missing continuation. It does
not change the bundled CEF library or claim to repair every external-pump edge
case inside CEF.

## Reproduction and measurements

The optimized executable was built from this branch using the locked Cargo
dependencies and GNOME 50 SDK, then launched **inside the existing stable
Flatpak** with `--binary`. The installed CEF/GTK libraries, GPU settings,
permissions, maximized window and account stayed the same. The build used Rust
1.98.1, two build jobs, release optimization, LTO disabled, 16 codegen units and
no debug information. The executable hash is in the result metadata.

Generated content uses an isolated profile, a 5-second warmup and a 15-second
programmatic scroll. The real WhatsApp workload uses a 20-second settle after
its chat list appears, a 5-second warmup and a 15-second scroll over 900 CSS
pixels, then restores the original offset. It collects timing and geometry;
it does not read messages or contacts, click, type or send messages. All active
comparisons use a 4042×2346 backing image, 2021×1173 CSS viewport and DPR 2.

`--idle-seconds 30` adds 5 seconds of settling and a separate 30-second idle
sample after scrolling. The background check invokes the isolated profile's
close-to-background action and verifies `document.visibilityState` becomes `hidden`. A minimize action alone
did not change page visibility on this Wayland session. One hundred percent CPU
denotes one core, summed over the application process tree.

The [seven final aggregates](measurements/fix_summary.json) retain the original
and reverted controls alongside tests of the final executable. Every fixed-build
row identifies the same executable SHA-256.

| Workload / executable | Paint callbacks/s | GTK draws/s | Pump gap p95 | Active CPU | Settled idle CPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| Generated CPU / original | 20.40 | 10.07 | 100.21 ms | 55.3% | 0.7% |
| Generated CPU / fixed | 59.73 | 59.33 | 10.00 ms | 108.2% | 0.7% |
| WhatsApp CPU / original | 26.87 | 10.07 | 100.14 ms | 72.4% | 5.2% |
| WhatsApp CPU / fixed | 59.87 | 59.53 | 9.85 ms | 123.6% | 5.2% |
| WhatsApp CPU / original restored | 26.73 | 10.07 | 100.16 ms | 75.7% | — |
| Generated AMD GPU / fixed | 59.93 | 59.87 | 11.18 ms | 41.1% | 0.8% |

The separate fixed CPU background check confirmed hidden page visibility,
**0.7% idle CPU** and about 12 pump calls/s over 30 seconds. Every final capture
completed and exited normally. AMD acceleration was verified through the GL
vendor, successful DMA-BUF import and accelerated draw callbacks.

The final fixed captures each contain one long pump interval spanning the idle
to scrolling transition: its ending callback was received 10–22 ms into the
measurement. These intervals remain in the aggregates; there were no subsequent
pump gaps over 50 ms in their 15-second active windows. Original CPU chat-list
captures each had 150 such intervals, 149 without scheduling requests between
pumps. All real-page runs maintained about 60 animation updates/s and recorded
zero page long tasks.

Active CPU use increases when the CPU path delivers almost six times as many
draws. This fix does not make CPU-buffer rendering as efficient as DMA-BUF. The
30-second samples show no idle spin regression in these workloads; they are not
a long-duration power or battery-life study. No high-frequency idle polling or
GPU preference override is installed.

## Regression checks and limits

All 96 Rust unit tests and both existing graphics integration tests passed.
The five new scheduler tests exercise urgent-deadline replacement, request
bursts, requests from worker threads, continuation without a CEF notification,
no reentry, native-event priority, idle backoff and cancellation on shutdown.
The existing graphics fixture exercised rejected legacy GL, visible startup,
background prewarming, presentation and clean exit, using an isolated D-Bus
session and XWayland with forced Mesa software GLES. Explicit `RUST_LOG` was
needed for its startup-marker assertions.

`cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
passed. The release build retains an existing release-only dead-code warning
for `RenderBridgeTarget::send_to_browser`. Both C probes compile with
`-Wall -Wextra -Werror`; Python compilation and recomputation of all nine
original aggregates pass.

Paint/draw rates count callbacks, not actual compositor presentation FPS.
Programmatic scrolling bypasses physical wheel/touchpad event handling. The
native-priority regression demonstrates ordering, not a measured cure for every
reported Preferences/menu delay. GPU checks on this machine use AMD Radeon
680M; this does not validate NVIDIA acceleration or close the broader NVIDIA
compatibility issue #173. A maintainer still needs to merge and distribute the
fix before the official Flathub application includes it.
