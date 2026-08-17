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
