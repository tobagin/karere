# Scrolling and CEF pump diagnostics

Current default-on acceleration work and reproducible producer/media controls
are documented in [ACCELERATION.md](ACCELERATION.md). Its 240 Hz target remains
**unverified**. [ADAPTIVE_REFRESH_FINDINGS.md](ADAPTIVE_REFRESH_FINDINGS.md) retains
the earlier prototype's lower-ceiling measurements and original backend policy.

These optional tools reproduce the CPU-buffer off-screen rendering stall described in [issue #173](https://github.com/tobagin/karere/issues/173). The [original findings](FINDINGS.md) document controlled CPU → GPU → CPU comparisons on Karere 4.3.0. The [fix validation](FIX_VALIDATION.md) describes the paint-driven CEF scheduler and compares the compiled application against the original Flatpak.

The probes were tested with the stable `io.github.tobagin.karere` Flatpak, CEF `152.0.6+g708dc14`, on **Linux x86-64**. They rely on that CEF ABI. They are developer diagnostics and are not included in the application build or installed as persistent overrides.

## Prepare

Requirements: the matching Karere Flatpak, Python 3.9 or newer, GCC, `glib-compile-schemas`, and `gapplication`. Run from the repository root:

```sh
python3 tools/scrolling-probe/prepare.py
```

The preparation script builds the two C libraries and derives memory-backend GSettings schemas from the installed Flatpak. Generated libraries, schemas and captures stay under ignored `.build/` and `results/` directories. No Rust/CEF rebuild is needed. The GCC TLS dialect flag keeps the interposer compatible with the tested Flatpak runtime.

The ABI was verified against the checksum-locked `cef-dll-sys 152.3.0+152.0.6` Linux x86-64 bindings (crate SHA-256 `ced5c6a0f46744561b50e283f9134c21bdd95a2e991695b0fc7399ad79765bbd`). App size/handler offset: 80/64 bytes. Browser-handler size/schedule offset: 96/72 bytes. Settings size/debug-port offset: 448/336 bytes.

## Reproduce with generated content

Fully quit Karere first. Each capture refuses an existing instance or output filename. The synthetic test uses an isolated temporary profile inside Flatpak and memory-only settings, leaving the normal profile and GPU preference intact. Each launch closes automatically after a 5-second warmup and a 15-second scrolling interval.

```sh
python3 tools/scrolling-probe/karere_probe.py cpu --synthetic --schedule-probe
python3 tools/scrolling-probe/karere_probe.py cpu_fast --synthetic --schedule-probe --fast-backstop
python3 tools/scrolling-probe/karere_probe.py cpu_reverted --synthetic --schedule-probe
python3 tools/scrolling-probe/karere_probe.py gpu --synthetic --synthetic-gpu --schedule-probe
```

Keep the diagnostic window visible and avoid interacting with it. Use new labels for subsequent runs. CPU → fast CPU → reverted CPU isolates the fallback interval. CPU → GPU compares the presentation paths while retaining the original interval. Confirm the same viewport/DPR and the actual accelerated backend in the result logs; a preference alone is insufficient.

`--fast-backstop` changes only the first main-thread 100 ms GLib timer registered after CEF initializes to 8 ms for that launch. This is an experimental control: the measurements show increased active CPU use and incomplete drawing recovery on the CPU path. It is not a recommended permanent setting.

## Validate a compiled fix

`--binary` launches a locally compiled executable inside the installed Flatpak,
using its CEF library, resources, GTK runtime and graphics permissions. Compile
against a compatible CEF version first. The tool records the executable's SHA-256
and grants its parent directory read access for that launch. It does not replace
the installed executable.

```sh
python3 tools/scrolling-probe/karere_probe.py fixed_cpu --synthetic --schedule-probe --binary target/release/karere --idle-seconds 30
python3 tools/scrolling-probe/karere_probe.py fixed_chat --chat-list --binary target/release/karere --idle-seconds 30
python3 tools/scrolling-probe/karere_probe.py fixed_background --synthetic --schedule-probe --binary target/release/karere --idle-seconds 30 --idle-window background
```

`--idle-seconds` waits five seconds after the active measurement, then captures a
separate settled idle interval before quitting. `--idle-window minimized` requests
minimization through the fresh window's GTK D-Bus action; `--idle-window background`
uses the isolated profile's close-to-background action and verifies the page
becomes hidden. Both additionally require `gdbus`; background testing requires
`--synthetic` so it cannot change the normal profile's close-button preference.
Wayland minimization does not necessarily change CEF page visibility. Run the
same options without `--binary` for the installed-app control.

The fixed scheduler uses `g_timeout_source_new` and can defer source creation
until the current pump returns. The original interposer's `new_timers` and
`urgent_postponed_events` counters only observe `g_timeout_add_full` calls inside
CEF's scheduling callback; they do not describe the new scheduler. Use actual
pump entry times, gap distributions, paint/draw rates and idle CPU to compare it.
The original `--fast-backstop` experiment likewise only applies to the original
timer implementation.

## Optional test of the real chat list

This mode uses the normal profile and its current GPU preference:

```sh
python3 tools/scrolling-probe/karere_probe.py chat_cpu --chat-list
```

The controller waits for a visible WhatsApp chat list, settles for 20 seconds, warms up for 5 seconds, and moves `scrollTop` through a 900 CSS-pixel range for 15 seconds. It restores the starting offset and quits. It does not click, type, read messages/contacts/cookies, or send messages. Keep Karere untouched during the measurement.

For CPU → GPU → CPU comparisons, change GPU Rendering in Preferences and fully quit between runs. Restore the original preference afterwards. The accelerated workaround in the historical results was verified on AMD. Earlier NVIDIA presenter experiments were opt-in and fell back to CPU transfer; current default-on policy and recovery validation are documented in [ACCELERATION.md](ACCELERATION.md).

The real-page probe temporarily enables CEF's loopback CDP port through its initialization field, preserving normal origin/private-network security flags. It does not pass Karere's `--debug` option. The small CDP client only accepts `127.0.0.1:9333`. The generated-page mode uses Karere's debug option in its isolated profile. Both reject an already occupied diagnostic port. Quitting the diagnostic instance removes the probe and listener; relaunch Karere normally afterwards.

To check whether application debug logging contributes to the pump pattern:

```sh
python3 tools/scrolling-probe/karere_probe.py chat_normal_logging --chat-list --normal-logging
```

Pump timing remains available; paint/draw counters are reported as unavailable when their logs are disabled.

## Analyze and interpret

```sh
python3 tools/scrolling-probe/summarize_automatic.py \
  tools/scrolling-probe/results/cpu.jsonl \
  tools/scrolling-probe/results/cpu_fast.jsonl \
  tools/scrolling-probe/results/cpu_reverted.jsonl \
  tools/scrolling-probe/results/gpu.jsonl
```

The analyzer uses the exact active interval reported by the page, counting CEF paint and GTK draw callbacks and calculating CEF pump gaps/durations. It also records scheduling requests and whether requests arrived between executions separated by long gaps. CPU percentages sum the application's process tree; 100% is one core. All child threads' child-process lists are traversed so renderer processes are included.

The committed [generated-page aggregates](measurements/automatic_summary.json) and [real chat-list aggregates](measurements/chat_summary.json) preserve the original investigation results. In synthetic captures, `configuration.settings` records the unchanged normal-profile preferences; `synthetic_gpu` identifies the isolated GPU override. Raw machine-specific captures and compiled binaries are not committed. Fresh captures use their own labels and timestamps.

`pump_probe.c` measures actual pump entry/exit with a monotonic clock. `schedule_probe.c` additionally observes scheduling callbacks, preserving their arguments and reference-count callbacks. Paint/draw timestamps come from receipt of allowlisted application logs, so events near interval boundaries may shift slightly. Callback rates do not establish compositor presentation FPS or input-to-screen latency. Programmatic scrolling bypasses wheel/touchpad input handling; native-menu latency remains a separate measurement. All comparisons retain the same instrumentation, which itself adds some overhead.
