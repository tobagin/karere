# PRD: GTK4 + CEF Single-Account WebView Template

**Project codename:** `gtk-cef-shell` (rename freely)
**Status:** Draft v1
**Author:** Thiago Avila-Fernandes
**Date:** 2026-05-26

## 1. Purpose

Prove that a Chromium Embedded Framework (CEF) browser surface can be embedded inside a GTK4 + libadwaita application window such that all chrome (HeaderBar, menus, dialogs, theming) remains native GTK while the web content area is rendered by Chromium. Output is a minimal, reusable Rust template that future projects (notably a Karere successor) can fork.

## 2. Background

GTK4 currently has only one web engine binding: WebKitGTK 6.0. WebKitGTK's GStreamer-based WebRTC backend cannot reliably negotiate WhatsApp Web voice/video calls, and the LibWebRTC backend is build-prohibitive and still partial. Chromium has complete, battle-tested WebRTC. No prior art exists in Rust for embedding CEF as a single GTK4 widget while preserving libadwaita window decoration.

## 3. Goals

**Must:**
1. Single AdwApplicationWindow with native AdwHeaderBar.
2. A `CefGtkArea` GTK4 widget that hosts one CEF offscreen browser, rendered via GL texture into a GtkGLArea.
3. Load arbitrary URL passed via CLI flag, default `https://example.com`.
4. Mouse input (move, click, scroll), keyboard input (printable + modifier keys), and basic IME text input forwarded from GTK to CEF.
5. HiDPI scale factor propagated correctly.
6. Cursor type changes (text, pointer, resize) reflected on the GTK widget.
7. Resize repaints without flicker or stretching.
8. Builds and runs as Flatpak with bundled CEF binaries.
9. Quits cleanly: CEF browser closed, message loop drained, no zombie subprocesses.

**Should:**
10. Right-click context menu (CEF default suppressed; optional GTK PopoverMenu wired to `CefContextMenuHandler`).
11. Page title bound to AdwHeaderBar title.
12. Loading state shown via AdwHeaderBar subtitle or spinner.
13. Permission prompts (camera/microphone/notifications) surface via `AdwAlertDialog`, decision returned to CEF.
14. WebRTC functional end-to-end: navigate to a WebRTC test page (e.g., `webrtc.github.io/samples/`) and confirm `getUserMedia` succeeds, peer connection completes.

**Won't (out of scope for v1):**
- Multi-account / multi-browser pool.
- Accessibility bridge (CEF AX tree → AT-SPI).
- Drag-and-drop in or out of web content.
- File downloads.
- Print, find-in-page, devtools UI.
- System tray.
- Tabs.

## 4. Non-Goals

- Reimplementing Karere features.
- Productionizing CEF distribution for crates.io.
- Supporting GTK3 or X11-only fallbacks. Wayland-first; X11 acceptable via XWayland but not engineered.

## 5. Target Platform

- **OS:** Linux x86_64 and aarch64.
- **Desktop:** Wayland session (GNOME 48+). X11 via XWayland acceptable.
- **Runtime:** GNOME Platform 50 inside Flatpak. Native build also supported on host (Fedora 44+ / Ubuntu 24.10+).
- **CEF version:** Latest stable matching Chromium ≥ 130. Pin exact CEF binary distribution checksum.
- **Rust edition:** 2024. MSRV: stable at time of writing.

## 6. Functional Requirements

### 6.1 Application shell
- `main.rs` initializes `adw::Application`, parses one CLI arg (`--url`), creates one window.
- Window: `AdwApplicationWindow` containing `AdwHeaderBar` + `AdwToolbarView` + `CefGtkArea` as content.
- Window title binds to `cef_browser.title`.
- Closing window initiates CEF browser close, then quits the GTK loop after CEF confirms shutdown.

### 6.2 CefGtkArea widget
- Subclass of `GtkGLArea` (or `GtkWidget` with a `GtkGLArea` child).
- Holds one `CefBrowser` configured in OSR mode.
- Implements `CefRenderHandler::OnPaint`: copy CPU buffer into a GL texture, queue redraw on the widget.
- Optional fast path: if CEF shared-texture / DMABUF available, import as `GL_TEXTURE_2D` zero-copy.
- `realize` allocates GL texture. `unrealize` releases it and tells CEF to close.
- `render` draws full-quad textured. Repaint only on `OnPaint` invalidation or widget resize.

### 6.3 Input forwarding
- `GtkEventControllerMotion` → `CefBrowserHost::SendMouseMoveEvent`.
- `GtkGestureClick` → `SendMouseClickEvent` (button down/up).
- `GtkEventControllerScroll` → `SendMouseWheelEvent`. Respect smooth-scroll delta.
- `GtkEventControllerKey` → `SendKeyEvent`. Translate keyval/keycode to CEF `KeyEventType` and Windows-style VK codes (CEF uses Windows key codes cross-platform).
- `GtkIMContext` (`gtk_im_multicontext_new`) attached to widget. Commit signal → `SendKeyEvent` with char events. Preedit signal → `CefBrowserHost::ImeSetComposition`.

### 6.4 Lifecycle
- CEF initialized once at process start on the main thread via `cef_initialize`.
- Message pump integrated with GLib main loop using `g_idle_add` calling `cef_do_message_loop_work` at a capped rate (~60 Hz), OR run CEF with `external_message_pump = true` and schedule via `CefBrowserProcessHandler::OnScheduleMessagePumpWork`. Prefer external pump for power efficiency.
- On `AdwApplicationWindow::close_request`: call `CefBrowserHost::CloseBrowser(false)`, wait for `CefLifeSpanHandler::OnBeforeClose`, then `cef_shutdown`, then return `glib::Propagation::Proceed`.

### 6.5 Permissions
- Implement `CefPermissionHandler::OnRequestMediaAccessPermission`.
- Show `AdwAlertDialog` with localized prompt. User's choice resolves the CEF callback.
- No persistence in v1 — every request prompts.

### 6.6 HiDPI
- Read `gtk_widget_get_scale_factor` on realize and on `notify::scale-factor`.
- Propagate via `CefRenderHandler::GetScreenInfo` → `device_scale_factor`.
- On change, call `CefBrowserHost::NotifyScreenInfoChanged` and `WasResized`.

## 7. Non-Functional Requirements

- **Startup:** Window visible with first paint under 1.5 s on a mid-range laptop (cold CEF init included).
- **Memory:** Under 350 MB resident for a loaded `example.com` (CEF baseline ~200 MB).
- **Frame rate:** 60 fps scroll on a typical page; no dropped frames during pure widget resize.
- **Crash isolation:** Renderer crash does not kill the GTK process. Show AdwToast on crash, allow reload.
- **No memory leaks** on open-close cycle. Verify with 100x window create/destroy under valgrind or `heaptrack`.

## 8. Architecture

```
main.rs
├── App (adw::Application)
│   └── Window (AdwApplicationWindow)
│       ├── HeaderBar (AdwHeaderBar)
│       └── CefGtkArea (custom widget)
│           ├── GtkGLArea (texture render)
│           ├── GtkIMMulticontext (IME)
│           ├── EventControllers (mouse/key/scroll/motion)
│           └── CefBrowserHandle
│               └── CefClient impls:
│                   - LifeSpanHandler
│                   - RenderHandler (OSR)
│                   - DisplayHandler (title, cursor, status)
│                   - LoadHandler (loading state)
│                   - PermissionHandler
│                   - KeyboardHandler (passthrough)

cef_runtime.rs   — process init, message pump integration, sub-process exec
```

Sub-process model: CEF requires a helper executable for renderer/GPU/utility processes. Either:
- (a) build a separate `cef-subprocess` binary, or
- (b) detect `--type=...` arg in `main.rs` and route to `cef_execute_process` before any GTK init.

Choose (b) for single-binary simplicity.

## 9. Dependencies

- `gtk4` (gtk-rs)
- `libadwaita`
- `glib`, `gio`
- `cef` crate (pick highest-maturity binding; vendor if needed)
- `gl` or `glow` for GL texture upload
- `anyhow`, `log`, `env_logger`

CEF binary distribution: download from `cef-builds.spotifycdn.com` matching pinned Chromium version. Bundled in repo via a `download-cef.sh` script with SHA256 verification. Not vendored in git.

## 10. Build & Packaging

- `cargo build --release` produces single binary linking `libcef.so`.
- A `build.rs` step ensures `libcef.so` and resource bundle (`.pak`, ICU data, `locales/`) are present in `target/release/` next to the binary.
- Flatpak manifest: `org.gnome.Platform//50`. Modules: download CEF tarball, install to `/app/lib/cef/`, ship resources. Set `LD_LIBRARY_PATH=/app/lib/cef`. Disable inner Chromium sandbox (`--no-sandbox` or configure namespace sandbox SUID — namespace path strongly preferred).
- Flatpak finish-args: `--share=network --socket=wayland --socket=fallback-x11 --device=all --socket=pulseaudio --filesystem=xdg-download`.

## 11. Testing

- Manual smoke test checklist documented in `TESTING.md`:
  - Window opens, page renders, scrolls, types.
  - Resize: no stretch artifacts.
  - HiDPI on 2x monitor: crisp text.
  - WebRTC sample: camera/mic prompt → AdwAlertDialog → allow → peer connection succeeds.
  - Open/close 50 times: no leaks, no zombies (`ps` check).
- One headless Rust integration test launching the binary with `--url about:blank` and asserting it exits cleanly on SIGTERM.

## 12. Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| CEF Rust binding immaturity | High | High | Pick most-maintained crate; be willing to fork and patch. |
| IME bridge complex | Medium | Medium | Defer CJK polish to v2; commit-only IME for v1. |
| Chromium sandbox vs Flatpak bwrap conflict | High | High | Use `--no-sandbox` in v1; document. Investigate namespace sandbox for v2. |
| CEF binary size (200 MB+) bloats Flatpak | Certain | Low | Accept. Document. |
| External message pump bugs cause stalls | Medium | Medium | Fall back to multi-threaded message loop if external pump unstable. |
| WebRTC still fails for unknown reason | Low | High | Validate against `webrtc.github.io/samples` before declaring v1 success. |

## 13. Milestones

- **M1 — Hello CEF (week 1):** Process init + sub-process exec working; CEF browser renders into stdout-logged buffer.
- **M2 — Pixels onscreen (week 2):** `CefGtkArea` widget draws OnPaint texture into AdwApplicationWindow.
- **M3 — Input alive (week 3):** Mouse + keyboard work; can navigate, click, type in a search box.
- **M4 — Lifecycle clean (week 4):** Open/close without leaks; shutdown sequence verified.
- **M5 — WebRTC proof (week 5):** Permission dialog wired; `getUserMedia` succeeds on test page; peer connection completes locally.
- **M6 — Flatpak ship (week 6):** Manifest builds; runs from `flatpak run`; tag v0.1.0 release on GitHub.

## 14. Success Criteria

v1 ships if all of the following hold:
- Binary runs from Flatpak on Wayland.
- A real web page (example.com and any modern site) renders, scrolls, accepts input.
- `webrtc.github.io/samples/src/content/peerconnection/pc1/` completes a local peer connection without errors.
- 100 open/close cycles leak no memory and leave no orphan processes.
- Code is small enough (<3,000 lines Rust excluding generated bindings) to be a fork-able starting point for downstream apps.

## 15. Open Questions

1. Which Rust CEF crate? Decision pending after short evaluation spike (rate `cef`, `cef-rs`, `chromiumoxide-cef` — first 2 days of M1).
2. Single-binary subprocess routing vs separate helper binary? Default single-binary; revisit if `cef_execute_process` proves fragile.
3. Should we ship symbols-stripped libcef.so to halve Flatpak size? Recommend yes for release builds.
