# GPU compatibility follow-up — 9 October 2026

The feature checkout now contains upstream 4.3.4 at `10bba03` and the reviewed
#193 diagnostic branch at `7cc4615`. The wallpaper stylesheet is already in
upstream as `58e7e8d`; integration adds the reviewed tools without duplicating
that production change. The original v4.3.2 investigation was separately
preserved in local commit `2d7dd40`.

## Resulting behavior

- The CEF backport retains mappable images by default. Its alternate Linux
  allocation path requires `--karere-osr-native-handle`.
- After ten visible seconds without an initial accelerated frame, Karere
  retries that path once through the existing worker supervisor. GTK/CEF
  backends and the saved GPU preference survive the retry. A second missing
  frame window uses the existing CPU fallback. Explicit
  `KARERE_CEF_NATIVE_HANDLE=0` skips the alternate path; `=1` starts with it.
- Vulkan devices must support BGRA exports GTK accepts. Allocated output
  modifiers must satisfy both Vulkan's sampled/transfer requirements and
  GTK's fourcc/modifier list. An incompatible import still uses GL/CPU recovery.
- Integrated CDP transport retains the custom local media port and the trace
  frame-size option. The combined conversation runner exposes `--amplitude`.
- The upstream merge's GL-test helper call now matches the feature's stronger
  context-failure check. The debug-only renderer reply method has the same
  compile-time guard as its only caller, keeping release Clippy clean.

## Fresh validation

| Check | Result |
| --- | --- |
| Release `cargo build --locked --offline` | Passed |
| `cargo fmt --all -- --check` | Passed |
| Release all-target Clippy with `-D warnings` | Passed |
| All-target Rust tests in a private D-Bus/X11 session | 113 unit tests and 2 integration tests passed |
| GPU/GTK worker fixtures | Both ignored-by-default worker entries were exercised by the passing supervisor tests |
| Generated Vulkan/GL textures | Owned copies, producer reuse, GTK imports, resize and three-buffer bounds passed |
| Generated live buffer reuse | GL and Vulkan each accepted 120 updates with no errors or full-pool stalls |
| GTK colors/scaling/watchdog | Tiled colors and seams at 100/145/200 percent; paint deadline and cleanup checks passed |
| Combined Python diagnostic suite | 49 tests passed, including the merged custom-port/trace-limit regression |
| Python syntax | All diagnostic scripts parsed successfully |
| JavaScript | Copy-bridge regressions and all production/probe syntax checks passed |
| Build wrapper | 27 checks passed |
| Stable/beta graphics policy | 11 checks passed |
| Revised CEF patch | All three patched files passed dry-run application against pinned CEF/Chromium source |
| Git whitespace and shell syntax | Passed |

Build environment: installed GNOME SDK 50, Rust 1.98.1, Node 24.21.0, GTK 4.22.5,
and the installed stable CEF 152.0.6 headers/library mounted read-only. Rust
dependencies were fetched with `--locked` into the workspace cache; builds then
ran offline. The release build used LTO off, 16 codegen units and debug info off
for local validation. It was not installed.

The full Rust suite used `dbus-run-session`, `GDK_BACKEND=x11`,
`GSK_RENDERER=gl`, and explicit SDK `--device=dri --socket=x11 --share=ipc` access.
The initial SDK run without `--device=dri` could not authenticate the AMD device,
skipped the Vulkan fixture, and crashed in the widget fixture. The isolated
fixture and complete suite both passed with the required graphics access.
The GL startup tests used their own about:blank pages and temporary keyfile
profiles; unit widgets used memory-only settings. No normal account was used.

Host Python's media fixture required permission to open its temporary loopback
HTTP server. After that environmental restriction was removed, the complete
suite passed. No saved rendering preferences or Flatpak overrides were changed.

Local logs and the compiled executable are under the workspace's
`diagnostics/compatibility-2026-10-09/`. Executable SHA-256:
`b08bee694a80f31aa24e67f4b7a1e8e4a7c2db89954fecfc2255dfb3a53cb404`.
Shared Cargo build outputs remain under `upstream/target`; the copied executable
preserves this validation artifact independently of later builds.

## Engine and hardware gates

Patch application checks used CEF `708dc140cbc3286826a8abef89dc23a44ff9ea72`
and Chromium `152.0.7977.83`. They verify source compatibility, not an engine
build or native-handle delivery. The engine containing the new opt-in switch
has not been rebuilt. The previous local archive lacks this switch, and the
checked-in download pins still select the stock engine.

Keep #195 in draft while waiting for an official compatible CEF release or an
agreed rebuilt-backport distribution. Once available, validate default and
opt-in allocation on the maintainer's AMD/Quadro hybrid and RTX 5090, then update
download pins and repeat the producer/presentation/media/recovery matrix.
Physical 60/120 Hz modes, other vendors and actual device loss remain separate
coverage gaps. No new performance or hybrid-machine acceptance claim is made.

GitHub PRs were not changed, no messages were posted, and no local commit was
pushed. Stable Karere remains deployment
`7928a66fca79343fa12a89fc9afacc607a69e8ff9f5173504066331337dca8eb`.
