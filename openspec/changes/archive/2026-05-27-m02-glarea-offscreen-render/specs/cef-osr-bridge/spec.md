## ADDED Requirements

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

### Requirement: GLES 3.0 renderer uploads BGRA8 frames as a textured fullscreen quad

The widget SHALL render each frame by sampling the CEF paint buffer from a 2D texture drawn over a fullscreen quad. Shaders MUST target `#version 300 es` with `precision highp float;`. The fragment shader MUST swizzle `.bgra` because the buffer bytes are BGRA but are uploaded as `GL_RGBA` (GLES has no `GL_BGRA` upload format). The texture MUST use `GL_RGBA8` internal format with `GL_LINEAR` filtering and `GL_CLAMP_TO_EDGE` wrap.

#### Scenario: First paint allocates texture storage

- **WHEN** `GLAreaImpl::render` runs and the cached texture dimensions differ from the incoming frame size
- **THEN** `glTexImage2D(GL_RGBA8, w, h, GL_RGBA, GL_UNSIGNED_BYTE, pixels)` is called and the cached `tex_w` / `tex_h` atomics are updated

#### Scenario: Subsequent paints reuse texture storage

- **WHEN** `GLAreaImpl::render` runs and the incoming frame size equals the cached texture size
- **THEN** `glTexSubImage2D(0, 0, w, h, GL_RGBA, GL_UNSIGNED_BYTE, pixels)` is used instead of reallocating storage

#### Scenario: Fragment shader corrects channel order

- **WHEN** the fragment shader samples the texture
- **THEN** the sampled color is swizzled `.bgra` so BGRA bytes appear with red, green, blue, alpha in the correct channels on screen

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
- populates `screen_info` with the stored device scale factor
- returns `(0, 0)` from `screen_point`
- in `on_paint` copies the incoming buffer into `SharedState.frame.pixels`, records width / height, sets `dirty = true`, and logs `on_paint <w>x<h>` at debug level

#### Scenario: on_paint stores the buffer and marks the frame dirty

- **WHEN** CEF invokes `on_paint(type, dirty_rects, buffer, width, height)`
- **THEN** the bytes are copied into `SharedState.frame.pixels`, `frame.width` / `frame.height` are updated, `frame.dirty` is set to `true`, and a `debug!` line `on_paint <w>x<h>` is emitted

#### Scenario: screen_info reports the widget scale factor

- **WHEN** CEF requests `screen_info`
- **THEN** the returned struct carries `device_scale_factor = SharedState.scale_factor`

### Requirement: Tick callback re-renders when SharedState.frame is dirty

Because `GLArea` cannot expose a `Send + Sync` weak reference, the widget MUST NOT call `queue_render` from the CEF UI thread. Instead it SHALL register a tick callback via `add_tick_callback` that, on each GTK frame, inspects `SharedState.frame.dirty`, clears the flag, and calls `queue_render()` when set.

#### Scenario: Dirty flag triggers redraw

- **WHEN** the tick callback fires and `SharedState.frame.dirty` is `true`
- **THEN** the flag is cleared and `self.queue_render()` is invoked

#### Scenario: Clean frame skips redraw

- **WHEN** the tick callback fires and `SharedState.frame.dirty` is `false`
- **THEN** `queue_render()` is not invoked

### Requirement: SharedState aggregates frame buffer, size, scale, title and loading flag

The shell SHALL define `SharedState { frame: FrameBuffer, size: (i32, i32), scale_factor: f32, title: String, is_loading: bool }` and `SharedRef = Arc<Mutex<SharedState>>` with a `new_shared(size, scale)` constructor. All CEF handlers — render, lifespan, display, load — MUST hold a `SharedRef` and update only their respective fields.

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
