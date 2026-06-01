## 1. SharedState plumbing

- [x] 1.1 Define `FrameBuffer { pixels: Vec<u8>, width: i32, height: i32, dirty: bool }` in `src/handlers/mod.rs`
- [x] 1.2 Define `SharedState { frame, size, scale_factor, title, is_loading }` and `SharedRef = Arc<Mutex<SharedState>>` in `src/handlers/mod.rs`
- [x] 1.3 Implement `new_shared(size, scale)` constructor

## 2. CEF handlers

- [x] 2.1 Implement `ShellRenderHandlerBuilder` via `wrap_render_handler!` in `src/handlers/render.rs` with `view_rect`, `screen_info`, `screen_point`, `on_paint`
- [x] 2.2 Log `on_paint <w>x<h>` at debug level inside `on_paint`
- [x] 2.3 Implement lifespan stub in `src/handlers/life_span.rs`
- [x] 2.4 Implement display handler stub updating `SharedState.title` in `src/handlers/display.rs`
- [x] 2.5 Implement load handler stub updating `SharedState.is_loading` in `src/handlers/load.rs`
- [x] 2.6 Aggregate handlers via `wrap_client!` in `src/handlers/client.rs`

## 3. CefGtkArea widget

- [x] 3.1 Declare `CefGtkArea` as `glib::wrapper` over `gtk::GLArea` in `src/cef_gtk_area.rs`
- [x] 3.2 Imp uses `Mutex<Option<SharedRef>>`, `Mutex<Option<Browser>>`, `Mutex<Option<ShellLifeSpanHandler>>`, `Mutex<Option<String>>` for pending URL (justified by `TypedObjectRef` `Sync` requirement)
- [x] 3.3 Imp uses `AtomicU32`/`AtomicI32` for `program`, `vao`, `vbo`, `texture`, `tex_w`, `tex_h`
- [x] 3.4 `init_gl()` compiles `#version 300 es` shaders with `precision highp float;` and fragment `.bgra` swizzle
- [x] 3.5 `init_gl()` creates fullscreen-quad VAO/VBO and a 2D texture with `GL_LINEAR` + `GL_CLAMP_TO_EDGE`
- [x] 3.6 `realize()` calls `make_current()`, `init_gl()`, then `create_browser` with `windowless_rendering_enabled = 1` and `windowless_frame_rate = 60`, honoring any pending URL
- [x] 3.7 `unrealize()` calls `close_browser()` and `teardown_gl()`
- [x] 3.8 `size_allocate()` multiplies by `widget.scale_factor()`, stores into `SharedState.size`, calls `notify_screen_info_changed()` + `was_resized()`
- [x] 3.9 `GLAreaImpl::render` binds the texture, uses `glTexImage2D` on size change and `glTexSubImage2D` otherwise, draws the textured quad
- [x] 3.10 `add_tick_callback` polls `SharedState.frame.dirty` and calls `queue_render()` when set

## 4. Application window

- [x] 4.1 Implement composite-template `AdwApplicationWindow` in `src/window.rs` with `AdwHeaderBar`, `AdwToolbarView`, `CefGtkArea`, `GtkRevealer` + spinner
- [x] 4.2 Install 100 ms timeout that updates header title from `SharedState.title` and toggles the revealer from `SharedState.is_loading`

## 5. Verification

- [x] 5.1 `cargo run -- --url=https://example.com` renders the page in the GTK window
- [x] 5.2 `RUST_LOG=debug cargo run -- --url=https://example.com` emits `on_paint <w>x<h>` lines on every frame
- [x] 5.3 Resizing the window changes the painted size without artifacts and triggers fresh `on_paint` callbacks at the new dimensions
