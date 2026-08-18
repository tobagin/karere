# Manual smoke tests

Per PRD §11.

- [ ] Window opens, page renders.
- [ ] Scroll wheel works.
- [ ] Click + type in search box.
- [ ] Resize window: no stretch or tearing.
- [ ] HiDPI on 2x monitor: crisp text.
- [ ] `webrtc.github.io/samples/src/content/peerconnection/pc1/` completes a peer connection.
  - [ ] `getUserMedia` triggers AdwAlertDialog prompt.
  - [ ] Allow → camera + mic active.
- [ ] Open/close 50 times: no leaked memory (`heaptrack ./karere`).
- [ ] After quit: `ps -ef | grep karere` shows no orphan processes.

### Text selection and clipboard (#178)

Use only synthetic/non-sensitive text when recording evidence.

- [ ] On Wayland, drag across several words and multiple lines in a received and sent message; the highlight follows the pointer and remains after release.
- [ ] Immediately press Ctrl+C (before pausing after the drag), then paste into an external GTK text editor; the exact Unicode/multi-line selection appears.
- [ ] Repeat using right-click → CEF **Copy**; dismissing the menu without choosing an item must not change the clipboard.
- [ ] Click without selecting and press Ctrl+C; pre-existing regular clipboard content must remain unchanged.
- [ ] Select text normally, then middle-click in an editable field; PRIMARY selection paste still works once and ordinary selection alone does not overwrite regular CLIPBOARD.
- [ ] Switch between two accounts and repeat; input and copied text must always come from the visible account.
- [ ] Repeat at desktop and narrow/mobile-responsive widths, and at 1× and 2× scale factors.
- [ ] Repeat the drag, immediate Ctrl+C, and context-menu Copy checks on X11.
- [ ] On touch hardware, verify touch scrolling/taps remain independent and do not produce duplicate mouse clicks.

## Automated

Use Rust 1.97.1 with its matching rustfmt and Clippy components. Run the repository-wide acceptance gates from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

```sh
tools/verify-po.sh  # translation catalog health gate (sentinels, msgfmt, LINGUAS parity, version)
meson compile -C build-pot karere-pot && tools/verify-po.sh  # must be Rust-complete: 513-msgid pot, ALL CHECKS PASSED
```

The startup integration test launches the application by its fixed application ID. If another Karere instance is active on the session bus, run the test suite in an isolated session with `dbus-run-session -- cargo test --workspace --all-targets`.

### Software GLES startup (issue #177)

The startup regression launches the real `karere --url about:blank` binary three times. First, a debug-only legacy desktop-GL contract must reproduce the pre-fix context error and prove browser creation remains fenced. It then verifies the fixed GLES contract as a visible window and with `start-in-background` prewarm followed by presentation. Xvfb provides deterministic X11; Mesa is pinned to llvmpipe with desktop GL capped below the required 3.0 API while GLES 3.2 remains available. Accelerated OSR is disabled so CEF uses its CPU `on_paint` fallback, and the production `GSK_RENDERER=gl` policy remains active. The fixed launches require a GLES 3.x context and browser spawn and fail on a GLArea/context-creation error, timeout, premature exit, or unclean app-action shutdown.

```sh
dbus-run-session -- xvfb-run -a -s '-screen 0 1280x800x24' \
  cargo test --test gl_context_startup -- --nocapture --test-threads=1
```

Requirements: `Xvfb`, `dbus-run-session`, `gapplication`, `gsettings`, `glib-compile-schemas`, and a Mesa llvmpipe installation. This committed fixture covers fallback X11; PinePhone Wayland/Phosh remains a supplemental hardware smoke test. The reporter supplied no full startup log beyond an error about creating a GL context, so the regression rejects both Karere's `GLArea realize error` diagnostic and common GL-context creation-error wording without claiming an unobserved exact message.

Production-widget contract and CPU-frame state tests run with:

```sh
GSETTINGS_SCHEMA_DIR="$PWD/target/test-schemas" GSETTINGS_BACKEND=memory \
  xvfb-run -a cargo test web_view::tests:: -- --nocapture
```

Generate `target/test-schemas/gschemas.compiled` from `data/io.github.tobagin.karere.gschema.xml.in` (substituting the stable app ID/path) before that command; the integration test performs this setup automatically.

### PinePhone / Mobian Flatpak smoke

On a PinePhone running Mobian 13 under Phosh:

1. Install the current aarch64 Flatpak from Flathub (or the locally built Devel manifest).
2. Run `flatpak run io.github.tobagin.karere --url about:blank` from a terminal and confirm the log reports `GLArea context ready: api=...GLES... version=3.x`, with no `GLArea realize error`.
3. Launch normally and confirm WhatsApp renders, accepts touch input, and remains visible after portrait/landscape resize.
4. Enable **Start in Background**, restart, then present Karere from the app launcher/tray and confirm the prewarmed browser appears rather than a blank GLArea.
5. Quit and confirm no Karere/CEF process remains. Capture a screenshot and terminal log as release evidence.

### Coordinate probe — pointer alignment (#158 / KARE-016)

Synthetic fixture `tests/fixtures/coord-probe.html` is an 8×5 labeled grid (each cell 100×100, total 800×500) with full-document click capture. Each click writes compact JSON into `document.title` (`{x:clientX,y:clientY,innerW,innerH,dpr,clientW,clientH}`, last event wins, ≤ ~100 chars) and mirrors it on `window.__karereProbe.last` for CDP `Runtime.evaluate`. The page is loaded via `--url 'data:text/html,…'` (URI-encoded, no server, no sandbox filesystem needed) so both native and Flatpak runs work.

Harness `tests/coordinate_probe.sh` drives the real input path under `xvfb-run` for the matrix `{GDK_SCALE=1@1280x800, GDK_SCALE=2@2560x1600} × {cpu-osr (KARERE_GPU_OSR=0), gpu-osr (KARERE_GPU_OSR=1)}`. For each config it: launches the target with `--debug` (CDP on `127.0.0.1:9333` only in this disposable run — release gating in `src/cef_runtime.rs` is unchanged), deterministically positions the window (`xdotool search/getwindowgeometry/windowmove` to 0,0), calibrates the content origin with one probe click (derives header/border offset from observed vs screen coords), synthesizes clicks at 4 known cell centers via `xdotool mousemove --sync` + `click 1`, and polls the recorded coords via `xdotool getwindowname` (title) with a CDP `/json/list` fallback. One JSON row per config is printed to stdout with `expected` vs `observed` `clientX/Y`, per-axis error, viewport metrics, and a capped `joint_logs_tail` from stderr (scale/zoom/paint joints). Inline and standalone negative controls verify detection (intentionally wrong expectation must produce `error >1px` and be flagged). `GSK_RENDERER=gl` and Mesa llvmpipe fixture (`MESA_GL_VERSION_OVERRIDE=2.1`, `MESA_GLES_VERSION_OVERRIDE=3.2`) are preserved.

```sh
# Full matrix (native binary default — build first)
cargo build --bin karere
# Isolated session avoids host Karere instance stealing the new window (else
# secondary-instance forwarding → "window not found"; use dbus-run-session)
dbus-run-session -- bash tests/coordinate_probe.sh
# Single negative control (proves harness can detect misalignment)
dbus-run-session -- bash tests/coordinate_probe.sh --negative
# Flatpak Devel build — always use the Devel app-id (io.github.tobagin.karere.Devel),
# never the prod id (io.github.tobagin.karere); prod is the shipped release, Devel
# is the local build under test (build with: FLATPAK_BUILDER_EXTRA_ARGS=--disable-rofiles-fuse ./build.sh --dev)
dbus-run-session -- bash tests/coordinate_probe.sh --bin "flatpak run io.github.tobagin.karere.Devel"
```

Requirements: `Xvfb` (`xvfb-run`), `xdotool`, `python3`, `gsettings`/`glib-compile-schemas`, `curl` (optional for CDP fallback). Run under `dbus-run-session --` to isolate from any host Karere instance (host instance causes secondary-instance forwarding and `window not found` failures). Flatpak tests **must** target `io.github.tobagin.karere.Devel` (the local `--dev` build); `io.github.tobagin.karere` is the prod/stable Flatpak and is not rebuilt from this tree. The harness auto-resolves the effective `APP_ID`/`GSettings` schema from `KARERE_BIN` so Devel uses its own `io.github.tobagin.karere.Devel` schema. The matrix exits 0 and `SUMMARY` lines are consumed by CI; each row's `passed`/`worst_error_px` indicates whether the config reproduced the reporter's sustained `>1px` symptom. No real account or chat content is ever loaded (synthetic probe only, CEO privacy constraint).

### Pointer-alignment H7 probe (chrome + fractional) — KARE-018

Extends the synthetic probe with a chrome-mimicking fixture and fractional Wayland matrix.

- **Chrome-mimic fixture:** `tests/fixtures/coord-probe.html` gains `?chrome=1` / `?h7=1` / `window.__forceH7` mode that wraps the 8×5 grid in a fixed header (60 px) + side panel (360 px) and a scrolled/transformed container (`transform: translate(10px,20px)` + `scrollTop 80`). The harness injects `window.__forceH7=1` into the data URL so `location.search` detection works without a server. The title JSON is extended with `originX/originY/gridLeft/gridTop/scrollX/scrollY/visualViewport` diagnostics so calibration can compute `contentOrigin`.
- **Origin-calibrated harness:** `tests/coordinate_probe.sh --chrome` loads the fixture in chrome mode, performs a one-probe calibration click to derive `contentOrigin` via `getBoundingClientRect`/`visualViewport`/`scrollX/Y`, then runs remaining cell-center clicks asserting calibrated `|clientX−expectedCalibrated| ≤1px` per axis. Default (no `--chrome`) preserves KARE-016 synthetic behavior bit-identical.
- **Fractional Wayland:** `tests/coordinate_probe.sh --fractional` queries `gsettings get org.gnome.mutter experimental-features`; if neither `scale-monitor-framebuffer` nor `xwayland-native-scaling` is enabled, it prints `SKIP: fractional unverified on this host — missing Mutter experimental-features` and emits `SKIP` JSON rows. When the features are enabled but the host is Xvfb/non-Wayland, the harness still emits explicit `SKIP` rows (`transient scale set not implemented … needs operator Wayland session`) rather than a fake PASS — a true 125/150 % matrix requires an operator Wayland session that can drive `org.gnome.Mutter.DisplayConfig ApplyMonitorsConfig` with trap-restore; integer `GDK_SCALE=1/2` matrix still runs deterministically under Xvfb and findings record host session type and whether fractional was actually exercised.
- **Real-page opt-in (privacy gate):** `KARERE_H7_REAL_PAGE=1 tests/coordinate_probe.sh --chrome` (or `KARERE_H7_REAL_PAGE_URL=<snapshot-file-url>` for an offline mirror) loads the live WhatsApp snapshot with a CDP-injected `window.__karereProbe` recorder and the same origin-calibrated protocol; when unset the harness prints `SKIP: real-page gated — set KARERE_H7_REAL_PAGE=1 for operator-authorized run`, does not attempt login, and runs the offline chrome-mimic fixture instead. No real account is used by default in CI.
- **Flags / env:** `--chrome`, `--fractional`, `--negative` (composes as `--chrome --negative` which must FAIL), `KARERE_H7_REAL_PAGE=1`, `KARERE_GPU_OSR`, `KARERE_BIN`/`KARERE_APP_ID`, `GDK_SCALE` matrix.
- **JSON results:** synthetic rows → `coord-probe-results.json`, H7 chrome/fractional rows → `coord-probe-h7-results.json` (both git-ignored, kept for diffing; also appended to disk as each row is printed). Each row includes `expected` vs `observed` vs `originCorrected`/`expectedCalibrated` vs `error`/`calibrated_error` plus viewport metrics and joint `coord:` logs.

```sh
# Synthetic only (default)
dbus-run-session -- tests/coordinate_probe.sh
# Chrome-mimicked H7 probe (header+panel+transform+scroll, calibrated)
dbus-run-session -- tests/coordinate_probe.sh --chrome
# Fractional Wayland (SKIP on Xvfb; requires operator Mutter Wayland for 125/150% PASS)
dbus-run-session -- tests/coordinate_probe.sh --chrome --fractional
# Negative control must FAIL (proves detection)
dbus-run-session -- tests/coordinate_probe.sh --chrome --negative
# Real-page opt-in (operator-authorized only, never in CI)
KARERE_H7_REAL_PAGE=1 dbus-run-session -- tests/coordinate_probe.sh --chrome
```

### Text selection automation

- [ ] `cargo test` — unit and headless integration coverage, including ordered raw mouse drag lifecycle, click counts, touch-emulation suppression, HiDPI coordinates, CEF Copy command classification, IPC, clipboard sanitization/caps, and clean SIGTERM exit.
- [ ] `node tests/copy_bridge.test.js` — executes the production injected copy bridge with populated, duplicate, Unicode/multi-line, immediate-copy, PRIMARY debounce, and empty/collapsed selection cases.
- [ ] `bash tests/flathub_beta_cef_policy.sh` — offline; asserts the flathub-beta CEF/engine policy (beta manifest mirrors stable: CEF tag, archive.json names, karere tag, GSK_RENDERER=gl, zero chromium-148).

## Beta testing (flathub-beta)

Karere 4.0 betas ship on the **`flathub-beta`** channel — separate from stable, opt-in.
Stable users are unaffected.

### Install

```sh
flatpak remote-add --if-not-exists flathub-beta https://flathub.org/beta-repo/flathub-beta.flatpakrepo
flatpak install flathub-beta io.github.tobagin.karere
flatpak run io.github.tobagin.karere   # add --branch=beta if stable is also installed
```

> **Note:** beta and stable share the app-id, so they share the data dir
> `~/.var/app/io.github.tobagin.karere`. Coming from v3, you'll re-link by QR scan
> (v4 is a hard fork — no session migration).

### What to check

- [ ] First launch shows the v3→v4 migration dialog; QR pairing links an account.
- [ ] Send/receive text and media; **video attachments play in-app** (new in v4).
- [ ] Voice/video call connects (mic/cam permission prompt appears, then audio+video).
- [ ] Notifications fire with preview; clicking one opens the chat.
- [ ] Multi-account: add a second account, switch, confirm isolated sessions.
- [ ] Tray icon shows unread; close-to-tray + background run work.
- [ ] Downloads land in `~/Downloads`; spell-check, zoom (Ctrl +/-/0), paste (Ctrl+V) work.

### Report bugs — open an issue

File anything that breaks at **<https://github.com/tobagin/karere/issues>**.

Include:

- **Version**: `flatpak info io.github.tobagin.karere` (confirm it says `4.0.0`)
- **Desktop**: GNOME/KDE/other, Wayland or X11, distro
- **Steps to reproduce** + what you expected vs. saw
- **Logs**: run `flatpak run io.github.tobagin.karere` from a terminal, paste relevant output
- For crashes: install the Debug extension and attach a backtrace —
  `flatpak install flathub-beta io.github.tobagin.karere.Debug` then
  `flatpak-coredumpctl io.github.tobagin.karere`

Prefix the issue title with `[beta]` so it's easy to triage.
