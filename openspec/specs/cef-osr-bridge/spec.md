# cef-osr-bridge Specification

## Purpose

Presents CEF offscreen paint buffers in a GTK widget and forwards size,
DPI, and view-rect signals back to the browser host. The bridge bonds a
`GtkGLArea`-backed widget, a GLES 3.0 textured-quad renderer, a CEF
`RenderHandler`, and a `SharedState` aggregate so the embedded browser's
BGRA8 frames appear on screen and so resize / scale-factor changes
propagate to CEF.

## Requirements

### Requirement: GtkGLArea-backed widget hosts the embedded browser

The shell SHALL provide a `CefGtkArea` widget that subclasses `gtk::GLArea` via `glib::wrapper` and owns the CEF `Browser` instance for its lifetime. The widget MUST create the browser inside `realize()` after the GL context is current, with `windowless_rendering_enabled = 1` and `windowless_frame_rate = 60`, and MUST close the browser in `unrealize()`.

#### Scenario: Browser is created when widget realizes

- **WHEN** the `CefGtkArea` widget is realized and `make_current()` succeeds
- **THEN** `init_gl()` runs to create the program, VAO, VBO and texture, and `browser_host_create_browser_sync` is called with `windowless_rendering_enabled = 1` and `windowless_frame_rate = 60`, storing the resulting `Browser` under a `Mutex<Option<Browser>>` in the imp

#### Scenario: Browser is closed when widget unrealizes

- **WHEN** the `CefGtkArea` widget is unrealized
- **THEN** the stored browser host has `close_browser()` invoked and `teardown_gl()` deletes the program, VAO, VBO and texture under the current GL context

#### Scenario: URL set before realize is honored

- **WHEN** a caller invokes `set_url` before the widget has realized
- **THEN** the URL is stored in a `Mutex<Option<String>>` pending slot and applied when `create_browser` runs at realize time

### Requirement: Production GLArea explicitly negotiates GLES 3.0

The production `KarereWebView` GLArea SHALL set `allowed-apis` to GLES and require version 3.0 in `ObjectImpl::constructed`, before widget realization. The main-account and DevTools constructors MUST share this contract. After `make_current()`, realization MUST report GTK's original context error and return before `init_gl()` or browser-pool bootstrap when context creation fails; on success it SHALL log the negotiated API and version before invoking raw GL.

#### Scenario: GLES-only system realizes the production widget

- **WHEN** a production main or DevTools view realizes on a system that provides GLES 3.x but not desktop OpenGL
- **THEN** GTK negotiates a GLES context at least version 3.0 and the existing `#version 300 es` shaders initialize

#### Scenario: Context creation fails before renderer or browser setup

- **WHEN** GTK records an error while creating or making current the GLArea context
- **THEN** the original GTK error remains observable, `init_gl()` is not called, and no browser-pool bootstrap occurs

### Requirement: GLES 3.0 renderer uploads BGRA8 frames as a textured fullscreen quad

The widget SHALL render each frame by sampling the CEF paint buffer from a 2D texture drawn over a fullscreen quad. Shaders MUST target `#version 300 es` with `precision highp float;`. The CPU paint fallback MUST upload CEF's BGRA bytes as `GL_RGBA`, and the fragment shader MUST swizzle `.bgra` because GLES has no portable `GL_BGRA` upload format. The texture MUST use `GL_RGBA8` internal format with `GL_LINEAR` filtering and `GL_CLAMP_TO_EDGE` wrap. Accelerated DMA-BUF OSR SHALL remain optional and capability/setting-gated; a failed import MUST release the pending accelerated frame so an existing or subsequent CPU `on_paint` frame can become visible.

#### Scenario: First paint allocates texture storage

- **WHEN** `GLAreaImpl::render` runs and the cached texture dimensions differ from the incoming frame size
- **THEN** `glTexImage2D(GL_RGBA8, w, h, GL_RGBA, GL_UNSIGNED_BYTE, pixels)` is called and the cached `tex_w` / `tex_h` atomics are updated

#### Scenario: Subsequent paints reuse texture storage

- **WHEN** `GLAreaImpl::render` runs and the incoming frame size equals the cached texture size
- **THEN** `glTexSubImage2D(0, 0, w, h, GL_RGBA, GL_UNSIGNED_BYTE, pixels)` is used instead of reallocating storage

#### Scenario: Fragment shader corrects channel order

- **WHEN** the fragment shader samples a CPU paint texture
- **THEN** the sampled color is swizzled `.bgra` so BGRA bytes appear with red, green, blue, alpha in the correct channels on screen

#### Scenario: Accelerated import rejection preserves CPU fallback

- **WHEN** accelerated OSR is enabled but the current GLES/EGL stack rejects a DMA-BUF import
- **THEN** the rejected accelerated frame is discarded, accelerated OSR is disabled for that view, and its browser pool is recreated without CEF shared textures so subsequent `on_paint` CPU frames can become visible instead of remaining permanently blank

### Requirement: Resize propagates physical pixels and DPI to the browser host

On allocation the widget SHALL compute the physical pixel size by multiplying the logical allocation by `widget.scale_factor()`, write the result into `SharedState.size`, then notify the browser host via `notify_screen_info_changed()` followed by `was_resized()`.

#### Scenario: size_allocate updates SharedState and notifies CEF

- **WHEN** `size_allocate()` fires with logical width W and height H and the current scale factor is S
- **THEN** `SharedState.size` is set to `(W * S, H * S)`, then the browser host receives `notify_screen_info_changed()` followed by `was_resized()`

#### Scenario: view_rect reflects the latest stored size

- **WHEN** CEF re-queries the render handler for `view_rect`
- **THEN** the handler returns the (width, height) currently stored in `SharedState.size`

### Requirement: Render handler ferries paint buffers and screen metadata via SharedState

The shell SHALL provide a CEF `RenderHandler` (constructed by `wrap_render_handler!` as `ShellRenderHandlerBuilder`) that:

- returns the stored size from `view_rect`
- populates `screen_info` with `device_scale_factor = 1.0` (pinned; physical view rect + compensating `host_zoom_level`)
- implements `screen_point(view_x, view_y, &mut sx, &mut sy)` as `sx = view_x + origin_x`, `sy = view_y + origin_y` via saturating `i32` add in physical pixels (no scale re-applied), where `origin` is `SharedState.window_origin` — Wayland `(0,0)` fallback so `screen==view`; on X11 origin is the `gdk4-x11` `X11Surface` root-relative position in physical pixels (xlib results are physical and not re-scaled; logical results would be scaled); returns `1` on success (at least one out-param written) and `0` only when both out-params are null; logs `screen_point view=(..) origin=(..) screen=(..)` at debug level
- in `on_paint` copies the incoming buffer into `SharedState.frame.pixels`, records width / height, sets `dirty = true`, and logs `on_paint <w>x<h>` at debug level

#### Scenario: on_paint stores the buffer and marks the frame dirty

- **WHEN** CEF invokes `on_paint(type, dirty_rects, buffer, width, height)`
- **THEN** the bytes are copied into `SharedState.frame.pixels`, `frame.width` / `frame.height` are updated, `frame.dirty` is set to `true`, and a `debug!` line `on_paint <w>x<h>` is emitted

#### Scenario: screen_info reports pinned device scale factor

- **WHEN** CEF requests `screen_info`
- **THEN** the returned struct carries `device_scale_factor = 1.0` (pinned)

#### Scenario: screen_point maps view to screen with seeded origin (unit-tested helper)

- **WHEN** CEF invokes `screen_point(10, 20, &mut sx, &mut sy)` with `SharedState.window_origin = (50, 80)` (seeded in tests to exercise the `view+origin` math; now also valid on X11 production, not just tests)
- **THEN** `sx == 60`, `sy == 100` and the return value is `1`

#### Scenario: screen_point Wayland fallback and null out-params

- **WHEN** CEF invokes `screen_point` with `window_origin = (0, 0)` (Wayland and post-unrealize production value) and both out-params present
- **THEN** `screen == view` and return is `1`; when both out-params are null return is `0`; when only one is present only that one is written and return is `1` (Wayland `origin=(0,0)` scenario; X11 real-origin scenario is the preceding one)

### Requirement: Tick callback re-renders when SharedState.frame is dirty

Because `GLArea` cannot expose a `Send + Sync` weak reference, the widget MUST NOT call `queue_render` from the CEF UI thread. Instead it SHALL register a tick callback via `add_tick_callback` that, on each GTK frame, inspects `SharedState.frame.dirty`, clears the flag, and calls `queue_render()` when set.

#### Scenario: Dirty flag triggers redraw

- **WHEN** the tick callback fires and `SharedState.frame.dirty` is `true`
- **THEN** the flag is cleared and `self.queue_render()` is invoked

#### Scenario: Clean frame skips redraw

- **WHEN** the tick callback fires and `SharedState.frame.dirty` is `false`
- **THEN** `queue_render()` is not invoked

### Requirement: SharedState aggregates frame buffer, size, scale, title and loading flag

The shell SHALL define `SharedState { frame: FrameBuffer, size: (i32, i32), scale_factor: f32, window_origin: (i32, i32), title: String, is_loading: bool }` and `SharedRef = Arc<Mutex<SharedState>>` with a `new_shared(size, scale)` constructor (`window_origin` defaults to `(0,0)`). All CEF handlers — render, lifespan, display, load — MUST hold a `SharedRef` and update only their respective fields; `KarereWebView::size_allocate` / `refresh_screen_scale` keep `window_origin` in sync (best-effort, `(0,0)` on Wayland).

#### Scenario: new_shared seeds the initial state

- **WHEN** `new_shared((w, h), s)` is called
- **THEN** the returned `SharedRef` wraps a `SharedState` with `size = (w, h)`, `scale_factor = s`, an empty title, `is_loading = false`, and a zero-sized clean frame buffer

#### Scenario: Display handler updates only the title

- **WHEN** the display handler observes a title change
- **THEN** it writes the new value to `SharedState.title` and leaves all other fields untouched

#### Scenario: Load handler updates only the loading flag

- **WHEN** the load handler observes loading start or end
- **THEN** it writes the new value to `SharedState.is_loading` and leaves all other fields untouched

### Requirement: Aggregated Client wires render, lifespan, display and load handlers

The shell SHALL provide a `wrap_client!` aggregation that returns the `RenderHandler`, `LifeSpanHandler`, `DisplayHandler` and `LoadHandler` instances backed by a single `SharedRef`.

#### Scenario: Client exposes all four handlers

- **WHEN** CEF requests handlers from the client during browser creation
- **THEN** the client returns the render, lifespan, display and load handlers constructed against the shared state

### Requirement: Application window embeds the widget and polls SharedState

The shell SHALL provide an `AdwApplicationWindow` composite template containing an `AdwHeaderBar`, an `AdwToolbarView`, a `CefGtkArea` and a `GtkRevealer` hosting a loading spinner. The window MUST install a 100 ms timeout that copies the current title and `is_loading` from `SharedState` into the header bar title and the revealer.

#### Scenario: Window reflects the latest title

- **WHEN** the 100 ms poll fires and `SharedState.title` differs from the displayed title
- **THEN** the header bar title is updated to match `SharedState.title`

#### Scenario: Window reveals the spinner while loading

- **WHEN** the 100 ms poll fires and `SharedState.is_loading` is `true`
- **THEN** the revealer hosting the spinner is set to revealed; otherwise it is hidden
