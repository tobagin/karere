## Context

M1 produced a CEF subprocess and main process with offscreen rendering enabled but no presentation surface — paints had nowhere to go. GTK 4 ships `GtkGLArea`, a widget that vends a current GL/GLES context per frame, which is the natural CPU-buffer landing pad in the absence of an `accelerated_osr` (shared-texture) path on Linux/Wayland. CEF's `RenderHandler` callback delivers BGRA8 buffers on the CEF UI thread; the GTK render must happen on the GTK main thread with a current GL context. The two are bridged by `Arc<Mutex<SharedState>>`.

## Goals / Non-Goals

**Goals:**
- Display the embedded page in a `GtkGLArea` subclass without flicker, tearing or stale frames on resize.
- Drive CEF's view rectangle, screen info and DPI from the GTK widget's allocation and scale factor.
- Provide a `SharedState` hub that later milestones (input, shutdown, dialogs) hang behaviour off without touching CEF threading rules.
- Run on Mesa+Wayland with the GLES 3.0 context `GtkGLArea` provides by default on that stack.

**Non-Goals:**
- Input event forwarding (mouse, keyboard, IME) — M3.
- Orderly browser teardown / `cef_shutdown` gating — M4.
- Permission / JS dialogs — M5.
- Any accelerated OSR / shared-texture / Vulkan path. CPU upload is sufficient at the target frame rate.

## Decisions

### Subclass `GtkGLArea` via `glib::wrapper`
Alternatives considered: a `GtkDrawingArea` blitting via Cairo, or a `GtkPicture` fed from a `GdkMemoryTexture`. Cairo path is too slow for 60 Hz BGRA uploads and forces an extra copy. `GdkMemoryTexture` works but defers GPU upload to the compositor and gives up control over the GL state (texture parameters, swizzle). `GtkGLArea` subclass lets us own the texture and shader and keeps presentation on the GPU.

### Imp uses `Mutex<Option<...>>` + atomics instead of `RefCell` / `Cell`
glib 0.22's `TypedObjectRef<imp, GLArea>` requires `Sync`. `GLArea` contains a raw pointer, so the imp struct cannot use `RefCell` (not `Sync`). Switched to `Mutex<Option<T>>` for owned handles (`SharedRef`, `Browser`, `ShellLifeSpanHandler`, pending URL) and `AtomicU32`/`AtomicI32` for GL object names (program, vao, vbo, texture, tex_w, tex_h). This is also why cross-thread `queue_render` from CEF's UI thread is not used — we cannot hand out a `Send + Sync` `WeakRef` to a `GLArea`; instead a `add_tick_callback` polls `SharedState.frame.dirty` on the GTK thread and calls `queue_render()` locally.

### GLES 3.0, not 3.30 core
Initial implementation used `#version 330 core` and was rejected by Mesa on Wayland inside the `GtkGLArea` context. Switched to `#version 300 es` plus `precision highp float;`. Vertex shader is plain attribute/varying; fragment shader samples the texture and swizzles `.bgra` so that BGRA bytes uploaded as `GL_RGBA` come out in the correct channel order. GLES has no `GL_BGRA` upload format, so the shader-side swizzle is the portable choice.

### Texture upload strategy
Allocate once with `glTexImage2D(GL_RGBA8, w, h, GL_RGBA, GL_UNSIGNED_BYTE)`. While the (w, h) matches the previous frame, subsequent frames use `glTexSubImage2D` to avoid storage reallocation. When CEF reports a new size, fall back to `glTexImage2D`. Filtering is `GL_LINEAR`, wrap is `GL_CLAMP_TO_EDGE`.

### Resize handling
`size_allocate()` multiplies logical width/height by `widget.scale_factor()`, stores the physical size in `SharedState.size`, then calls `host.notify_screen_info_changed()` followed by `host.was_resized()`. CEF then re-queries `view_rect` / `screen_info` from the `RenderHandler` and reschedules a paint at the new size. Without the explicit `notify_screen_info_changed`, HiDPI changes (monitor swap) are missed.

### Browser lifecycle pinned to widget lifecycle
`create_browser` runs in `realize()` after `make_current()` + `init_gl()`, with `windowless_rendering_enabled = 1` and `windowless_frame_rate = 60`. `unrealize()` calls `close_browser()` and `teardown_gl()`. This guarantees the GL context exists before the browser starts emitting paints and that GL objects are deleted with a current context. A `pending_url` slot lets `set_url` be called before the browser exists; `realize` consumes it.

## Risks / Trade-offs

- **CPU upload cost** → Mitigation: damage-aware partial updates are a future optimisation; current 60 Hz cap and `glTexSubImage2D` keep cost acceptable.
- **Shader swizzle vs. driver-side BGRA** → Mitigation: portable across GLES drivers; cost is one extra ALU per fragment.
- **Polling `dirty` flag at frame rate** → Mitigation: cheap mutex lock; alternative cross-thread `queue_render` is blocked by the `Sync` constraint above.
- **Mutex contention between CEF UI thread (`on_paint`) and GTK render thread** → Mitigation: critical sections hold the lock only for the copy / read; no GL calls happen under the lock.
- **No `accelerated_osr` path** → Accepted; revisit when a shared-texture extension stabilises on Wayland.
