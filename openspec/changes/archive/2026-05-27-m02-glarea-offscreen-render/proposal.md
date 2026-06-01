## Why

CEF offscreen rendering (OSR) emits CPU BGRA8 pixel buffers each frame but has no native GTK presentation path. M2 bridges those buffers into a `GtkGLArea`-backed widget so the embedded page actually appears on screen, and so resize / DPI / view-rect signals reach the browser host. Without this the M1 scaffold runs a headless browser whose paints go nowhere.

## What Changes

- New `CefGtkArea` widget (`glib::wrapper` over `gtk::GLArea`) that owns a GL texture, a fullscreen-quad VAO/VBO and a GLES 3.0 shader pair.
- New CEF `RenderHandler` implementation that writes paint buffers into `SharedState.frame` and exposes view rect / screen info / screen point.
- New `SharedState` / `SharedRef` (`Arc<Mutex<SharedState>>`) carrying the frame buffer, logical size, scale factor, page title and loading flag.
- Aggregated `Client` wiring `RenderHandler`, `LifeSpanHandler`, `DisplayHandler`, `LoadHandler` via `wrap_client!`.
- `AdwApplicationWindow` shell embedding `CefGtkArea`, plus a 100 ms poll updating title and loading spinner from `SharedState`.
- Browser is created in `realize()` with `windowless_rendering_enabled = 1` and `windowless_frame_rate = 60`; `size_allocate` recomputes physical pixels via `scale_factor()` and calls `notify_screen_info_changed()` + `was_resized()`.
- GLES portability fixes: shaders use `#version 300 es` with `precision highp float;`, fragment swizzles `.bgra` because GLES uploads BGRA bytes as `GL_RGBA`.

## Capabilities

### New Capabilities
- `cef-osr-bridge`: Presents CEF offscreen paint buffers in a GTK widget and forwards size / DPI / view-rect signals back to the browser host.

### Modified Capabilities
<!-- none -->

## Impact

- New source files: `src/cef_gtk_area.rs`, `src/handlers/render.rs`, `src/handlers/client.rs`, `src/handlers/{life_span,display,load}.rs`, `src/handlers/mod.rs`, `src/window.rs`.
- Adds runtime dependency on a GLES 3.0 context being available from `GtkGLArea` (Mesa+Wayland verified).
- No public API changes outside the new widget; downstream milestones (M3 input, M4 shutdown) build on `SharedState` and the widget lifecycle defined here.
