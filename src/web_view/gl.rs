use gl::types::{GLenum, GLint, GLuint};
use gtk::glib;
use gtk::subclass::prelude::*;
use std::cell::RefCell;
use std::ffi::{CString, c_void};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use super::imp::KarereWebView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CpuUpload {
    Empty,
    Allocate,
    Update,
    Reuse,
}

/// Decide how the production draw path should handle the latest CPU frame.
/// Keeping this decision independent from raw GL calls makes empty/resize/reuse
/// behavior explicit while the actual upload remains in `KarereWebView::draw`.
pub(super) fn cpu_upload(frame: &crate::handlers::FrameBuffer, texture_size: (i32, i32)) -> CpuUpload {
    if frame.width <= 0 || frame.height <= 0 || frame.pixels.is_empty() {
        CpuUpload::Empty
    } else if texture_size != (frame.width, frame.height) {
        CpuUpload::Allocate
    } else if frame.dirty {
        CpuUpload::Update
    } else {
        CpuUpload::Reuse
    }
}

/// UV transform `[x, y, w, h]` (fractions of the texture) that samples just
/// `visible` out of a `coded`-sized buffer. Fed to the vertex shader's `u_uv`.
pub(super) fn texture_uv(visible: (i32, i32, i32, i32), coded: (i32, i32)) -> [f32; 4] {
    let cw = coded.0.max(1) as f32;
    let ch = coded.1.max(1) as f32;
    let (x, y, w, h) = visible;
    [x as f32 / cw, y as f32 / ch, w as f32 / cw, h as f32 / ch]
}

/// GL viewport `(x, y, w, h)` that puts `rect` — top-left origin, in CEF view
/// (physical) pixels — on a `framebuffer`-sized target while the view is
/// `view` pixels (`SharedState::size`). GL counts y from the bottom. A frame
/// of the current view size fills the framebuffer exactly; a stale
/// previous-size frame during a resize keeps its own size anchored top-left
/// instead of being stretched over the new size. Popups use the same mapping.
pub(super) fn gl_viewport_for(
    framebuffer: (i32, i32),
    view: (i32, i32),
    rect: (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
    let sx = if view.0 > 0 { framebuffer.0 as f64 / view.0 as f64 } else { 1.0 };
    let sy = if view.1 > 0 { framebuffer.1 as f64 / view.1 as f64 } else { 1.0 };
    let (x, y, w, h) = rect;
    let x0 = (x as f64 * sx).round() as i32;
    let y0 = (y as f64 * sy).round() as i32;
    let x1 = ((x + w) as f64 * sx).round() as i32;
    let y1 = ((y + h) as f64 * sy).round() as i32;
    (x0, framebuffer.1 - y1, x1 - x0, y1 - y0)
}

/// WhatsApp Web's page background, the same pair the shell tints itself with
/// (`match-whatsapp-colors` in `application.rs`). Shown around a frame that
/// does not cover the widget yet (resize, first paint) instead of black.
pub(super) const PAGE_BG_DARK: [f32; 3] = [0x1d as f32 / 255.0, 0x1f as f32 / 255.0, 0x1f as f32 / 255.0];
pub(super) const PAGE_BG_LIGHT: [f32; 3] = [0xf7 as f32 / 255.0, 0xf5 as f32 / 255.0, 0xf3 as f32 / 255.0];

/// Page background for the current theme. Read off the widget's CSS
/// foreground colour, which libadwaita drives from Karere's theme setting: a
/// light foreground means a dark theme.
pub(super) fn page_background(widget: &impl gtk::prelude::WidgetExt) -> [f32; 3] {
    let fg = widget.color();
    page_background_for_foreground(fg.red(), fg.green(), fg.blue())
}

pub(super) fn page_background_for_foreground(r: f32, g: f32, b: f32) -> [f32; 3] {
    if 0.299 * r + 0.587 * g + 0.114 * b > 0.5 {
        PAGE_BG_DARK
    } else {
        PAGE_BG_LIGHT
    }
}

impl KarereWebView {
    // ---- GL ------------------------------------------------------------

    pub(super) unsafe fn init_gl(&self) {
        let vsrc = CString::new(VS).unwrap();
        let fsrc = CString::new(FS).unwrap();
        let program = unsafe { compile_program(&vsrc, &fsrc) };
        self.program.store(program, Ordering::Relaxed);

        // fullscreen quad (pos.xy, uv.xy), y-flipped for CEF's BGRA top-left origin.
        let verts: [f32; 16] = [
            -1.0, -1.0, 0.0, 1.0, //
            1.0, -1.0, 1.0, 1.0, //
            -1.0, 1.0, 0.0, 0.0, //
            1.0, 1.0, 1.0, 0.0, //
        ];

        let mut vao = 0;
        let mut vbo = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
            gl::BindVertexArray(vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (verts.len() * std::mem::size_of::<f32>()) as isize,
                verts.as_ptr() as *const c_void,
                gl::STATIC_DRAW,
            );
            let stride = (4 * std::mem::size_of::<f32>()) as i32;
            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, stride, std::ptr::null());
            gl::EnableVertexAttribArray(1);
            gl::VertexAttribPointer(
                1,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (2 * std::mem::size_of::<f32>()) as *const c_void,
            );
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
        }
        self.vao.store(vao, Ordering::Relaxed);
        self.vbo.store(vbo, Ordering::Relaxed);

        let mut tex = 0;
        unsafe {
            gl::GenTextures(1, &mut tex);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE as GLint,
            );
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE as GLint,
            );
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.texture.store(tex, Ordering::Relaxed);
        self.tex_w.store(0, Ordering::Relaxed);
        self.tex_h.store(0, Ordering::Relaxed);

        // GPU-OSR: a second texture the imported DMA-BUF EGLImage targets,
        // plus the popup layer's CPU and DMA-BUF targets.
        unsafe {
            self.accel_tex.store(new_texture(), Ordering::Relaxed);
            self.popup_tex.store(new_texture(), Ordering::Relaxed);
            self.accel_popup_tex.store(new_texture(), Ordering::Relaxed);
        }
        self.popup_tex_w.store(0, Ordering::Relaxed);
        self.popup_tex_h.store(0, Ordering::Relaxed);
    }

    /// Drop every GL name without deleting it, for when our context cannot
    /// be made current (the objects go with the context). EGLImages need
    /// only their display, so they are still released properly. (I4)
    pub(super) fn forget_gl_names(&self) {
        *self.imported.borrow_mut() = None;
        *self.imported_popup.borrow_mut() = None;
        for name in [
            &self.texture,
            &self.accel_tex,
            &self.popup_tex,
            &self.accel_popup_tex,
            &self.vbo,
            &self.vao,
            &self.program,
        ] {
            name.store(0, Ordering::Relaxed);
        }
        for size in [&self.tex_w, &self.tex_h, &self.popup_tex_w, &self.popup_tex_h] {
            size.store(0, Ordering::Relaxed);
        }
    }

    /// Delete our GL objects. The GLArea context must be current.
    pub(super) unsafe fn teardown_gl(&self) {
        // Release the live EGLImages while the GL context is still current.
        *self.imported.borrow_mut() = None;
        *self.imported_popup.borrow_mut() = None;
        unsafe {
            for name in [
                &self.texture,
                &self.accel_tex,
                &self.popup_tex,
                &self.accel_popup_tex,
            ] {
                let tex = name.swap(0, Ordering::Relaxed);
                if tex != 0 {
                    gl::DeleteTextures(1, &tex);
                }
            }
            let vbo = self.vbo.load(Ordering::Relaxed);
            if vbo != 0 {
                gl::DeleteBuffers(1, &vbo);
                self.vbo.store(0, Ordering::Relaxed);
            }
            let vao = self.vao.load(Ordering::Relaxed);
            if vao != 0 {
                gl::DeleteVertexArrays(1, &vao);
                self.vao.store(0, Ordering::Relaxed);
            }
            let p = self.program.load(Ordering::Relaxed);
            if p != 0 {
                gl::DeleteProgram(p);
                self.program.store(0, Ordering::Relaxed);
            }
        }
    }

    pub(super) unsafe fn draw(&self) {
        let shared = match self.shared.lock().as_ref() {
            Some(s) => s.clone(),
            None => return,
        };
        let mut s = shared.lock();

        unsafe {
            // GtkGLArea's default `resize` handler sets the viewport to its
            // framebuffer. Read it back so layers can be placed (and the
            // full viewport restored) without re-deriving GTK's scale.
            let mut viewport: [GLint; 4] = [0; 4];
            gl::GetIntegerv(gl::VIEWPORT, viewport.as_mut_ptr());

            // Theme-matched page background, not black: it shows around a
            // frame that does not cover the widget yet (resize, startup).
            let [r, g, b] = super::page_background(&*self.obj());
            gl::ClearColor(r, g, b, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            // A CPU paint drops the accelerated frame (render.rs on_paint);
            // release its EGLImage here, with the context current, so the
            // CPU frame is what gets drawn rather than the last DMA-BUF. A
            // hidden popup drops its import the same way. (I5)
            if s.accel.is_none() {
                *self.imported.borrow_mut() = None;
            }
            if s.popup.accel.is_none() {
                *self.imported_popup.borrow_mut() = None;
            }

            // GPU path: import the pending DMA-BUF to the accel texture. (gpu-osr)
            let atex = self.accel_tex.load(Ordering::Relaxed);
            let mut accel_failed = false;
            if let Some(af) = s.accel.as_mut()
                && af.dirty
            {
                if let Some(img) = crate::gl_dmabuf::import_to_texture(
                    atex,
                    af.width,
                    af.height,
                    af.fourcc,
                    af.modifier,
                    &af.planes,
                ) {
                    // Keep the EGLImage alive while the texture is sampled;
                    // dropping the previous one releases it.
                    *self.imported.borrow_mut() = Some(img);
                    af.dirty = false;
                } else {
                    accel_failed = true;
                }
            }
            if accel_failed {
                // A rejected DMA-BUF must not pin draw() to a permanently
                // blank/stale accelerated branch. Drop it so an existing or
                // subsequent CEF CPU on_paint frame can become visible.
                log::warn!("accelerated OSR import failed; falling back to CPU paint");
                s.accel = None;
                *self.imported.borrow_mut() = None;
                // CEF's shared-texture choice is fixed at browser creation.
                // Recreate the pool after this render callback so subsequent
                // callbacks are CPU on_paint rather than more unusable DMA-BUFs.
                self.schedule_software_osr_fallback();
            }

            // Popup DMA-BUF. A rejected one only costs the popup; the view
            // import above is what decides the software fallback.
            let aptex = self.accel_popup_tex.load(Ordering::Relaxed);
            let mut popup_failed = false;
            if let Some(pf) = s.popup.accel.as_mut()
                && pf.dirty
            {
                if let Some(img) = crate::gl_dmabuf::import_to_texture(
                    aptex,
                    pf.width,
                    pf.height,
                    pf.fourcc,
                    pf.modifier,
                    &pf.planes,
                ) {
                    *self.imported_popup.borrow_mut() = Some(img);
                    pf.dirty = false;
                } else {
                    popup_failed = true;
                }
            }
            if popup_failed {
                log::warn!("accelerated popup import failed; popup not shown");
                s.popup.accel = None;
                *self.imported_popup.borrow_mut() = None;
            }

            let accel_view = if self.imported.borrow().is_some() {
                s.accel.as_ref().map(|af| {
                    let layer = Layer {
                        tex: atex,
                        bgra: 0,
                        uv: super::texture_uv(af.visible, (af.width, af.height)),
                    };
                    (layer, (af.visible.2, af.visible.3))
                })
            } else {
                None
            };
            let (view, view_size) = match accel_view {
                Some(v) => v,
                None => {
                    // Software path: upload CEF's CPU BGRA buffer through the
                    // valid GLES context (software OSR does not eliminate GLArea).
                    let tex = self.texture.load(Ordering::Relaxed);
                    if !upload_cpu_frame(tex, &self.tex_w, &self.tex_h, &mut s.frame) {
                        return;
                    }
                    let layer = Layer {
                        tex,
                        bgra: 1,
                        uv: FULL_UV,
                    };
                    (layer, (s.frame.width, s.frame.height))
                }
            };

            // Popup layer (`<select>` dropdowns), CPU or DMA-BUF, drawn over
            // the view at CEF's popup rect.
            let mut popup = None;
            if let Some(rect) = s.popup.visible_rect(s.foreground_browser_id) {
                let accel_popup = if self.imported_popup.borrow().is_some() {
                    s.popup.accel.as_ref().map(|pf| Layer {
                        tex: aptex,
                        bgra: 0,
                        uv: super::texture_uv(pf.visible, (pf.width, pf.height)),
                    })
                } else {
                    None
                };
                popup = match accel_popup {
                    Some(layer) => Some((layer, rect)),
                    None => {
                        let ptex = self.popup_tex.load(Ordering::Relaxed);
                        upload_cpu_frame(
                            ptex,
                            &self.popup_tex_w,
                            &self.popup_tex_h,
                            &mut s.popup.frame,
                        )
                        .then_some((
                            Layer {
                                tex: ptex,
                                bgra: 1,
                                uv: FULL_UV,
                            },
                            rect,
                        ))
                    }
                };
            }
            let view_rect_size = s.size;
            drop(s);

            let framebuffer = if viewport[2] > 0 && viewport[3] > 0 {
                (viewport[2], viewport[3])
            } else {
                view_size
            };
            let prog = self.program.load(Ordering::Relaxed);
            gl::UseProgram(prog);
            let locs = (
                gl::GetUniformLocation(prog, c"u_bgra".as_ptr() as *const _),
                gl::GetUniformLocation(prog, c"u_uv".as_ptr() as *const _),
            );
            gl::BindVertexArray(self.vao.load(Ordering::Relaxed));
            gl::ActiveTexture(gl::TEXTURE0);
            let place = |rect| super::gl_viewport_for(framebuffer, view_rect_size, rect);
            draw_layer(&view, place((0, 0, view_size.0, view_size.1)), locs);
            if let Some((layer, rect)) = popup {
                draw_layer(&layer, place(rect), locs);
            }
            gl::BindVertexArray(0);
            gl::UseProgram(0);
            if viewport[2] > 0 && viewport[3] > 0 {
                gl::Viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
            }
        }
    }
}

/// One textured quad of the GL present path.
struct Layer {
    tex: GLuint,
    /// `u_bgra`: 1 = CPU BGRA bytes uploaded as RGBA, 0 = DMA-BUF import.
    bgra: GLint,
    /// `u_uv`: sampled sub-rect of the texture, `[x, y, w, h]` fractions.
    uv: [f32; 4],
}

const FULL_UV: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Draw `layer` into the GL `viewport` rect. Program + VAO bound by the caller.
unsafe fn draw_layer(layer: &Layer, viewport: (i32, i32, i32, i32), locs: (GLint, GLint)) {
    let (x, y, w, h) = viewport;
    unsafe {
        gl::Viewport(x, y, w, h);
        if locs.0 >= 0 {
            gl::Uniform1i(locs.0, layer.bgra);
        }
        if locs.1 >= 0 {
            let [u, v, uw, vh] = layer.uv;
            gl::Uniform4f(locs.1, u, v, uw, vh);
        }
        gl::BindTexture(gl::TEXTURE_2D, layer.tex);
        gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
    }
}

/// Bring `tex` (currently holding `tex_w`×`tex_h`) up to date with the CPU
/// `frame`, uploading only what changed. Returns false when there is no
/// frame to draw. Context must be current.
unsafe fn upload_cpu_frame(
    tex: GLuint,
    tex_w: &AtomicI32,
    tex_h: &AtomicI32,
    frame: &mut crate::handlers::FrameBuffer,
) -> bool {
    let size = (tex_w.load(Ordering::Relaxed), tex_h.load(Ordering::Relaxed));
    unsafe {
        match super::cpu_upload(frame, size) {
            super::CpuUpload::Empty => return false,
            super::CpuUpload::Allocate => {
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    gl::RGBA8 as GLint,
                    frame.width,
                    frame.height,
                    0,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    frame.pixels.as_ptr() as *const c_void,
                );
                tex_w.store(frame.width, Ordering::Relaxed);
                tex_h.store(frame.height, Ordering::Relaxed);
                frame.dirty = false;
            }
            super::CpuUpload::Update => {
                gl::BindTexture(gl::TEXTURE_2D, tex);
                // Upload only what CEF reported as damaged. Typing
                // dirties a caret-sized rect; re-sending the whole
                // window every frame is what makes CPU OSR stutter
                // on integrated GPUs (#179/#180). GLES 3.0 gives us
                // the UNPACK_* row addressing to do it in one call.
                let (x, y, w, h) = frame.damage.unwrap_or((0, 0, frame.width, frame.height));
                if (x, y, w, h) != (0, 0, frame.width, frame.height) {
                    gl::PixelStorei(gl::UNPACK_ROW_LENGTH, frame.width);
                    gl::PixelStorei(gl::UNPACK_SKIP_PIXELS, x);
                    gl::PixelStorei(gl::UNPACK_SKIP_ROWS, y);
                }
                gl::TexSubImage2D(
                    gl::TEXTURE_2D,
                    0,
                    x,
                    y,
                    w,
                    h,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    frame.pixels.as_ptr() as *const c_void,
                );
                // Leave the unpack state as every other caller expects.
                gl::PixelStorei(gl::UNPACK_ROW_LENGTH, 0);
                gl::PixelStorei(gl::UNPACK_SKIP_PIXELS, 0);
                gl::PixelStorei(gl::UNPACK_SKIP_ROWS, 0);
                frame.dirty = false;
            }
            super::CpuUpload::Reuse => {}
        }
    }
    true
}

/// A linear-filtered, edge-clamped `GL_TEXTURE_2D`. Context must be current.
unsafe fn new_texture() -> GLuint {
    let mut tex = 0;
    unsafe {
        gl::GenTextures(1, &mut tex);
        gl::BindTexture(gl::TEXTURE_2D, tex);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
        gl::BindTexture(gl::TEXTURE_2D, 0);
    }
    tex
}

/// Software present path: the cached texture for `frame`, rebuilt only
/// when CEF painted since the last snapshot. CEF hands back premultiplied
/// BGRA, which is a GdkMemoryTexture format verbatim, so a rebuild is the
/// one copy `Bytes` makes. (P6)
pub(super) fn cached_frame_texture(
    cache: &RefCell<Option<gtk::gdk::MemoryTexture>>,
    frame: &mut crate::handlers::FrameBuffer,
) -> Option<gtk::gdk::MemoryTexture> {
    let mut slot = cache.borrow_mut();
    if frame.width <= 0 || frame.height <= 0 || frame.pixels.is_empty() {
        *slot = None;
        return None;
    }
    if frame.dirty || slot.is_none() {
        *slot = Some(gtk::gdk::MemoryTexture::new(
            frame.width,
            frame.height,
            gtk::gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &glib::Bytes::from(&frame.pixels[..]),
            (frame.width * 4) as usize,
        ));
        frame.dirty = false;
    }
    slot.clone()
}

/// Set once `realize` gives up on GL (#177). Process-global to reach
/// `accel_osr_enabled`, whose answer is fixed for the browser's lifetime.
pub(super) static NO_GL: AtomicBool = AtomicBool::new(false);

/// GPU-OSR capability, decided once by [`probe_accel_osr`] with a GLArea
/// context current. Unset until then. (gpu-osr, I6)
static ACCEL_PROBE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Decide GPU-accelerated OSR: env `KARERE_GPU_OSR=1` (or the
/// `gpu-rendering` setting) AND EGL dma-buf import on this context. Call
/// only with the GLArea context current — `realize` after `init_gl`. An
/// answer is cached only when it was actually measured: without a current
/// EGL display the probe leaves the slot empty for the next realize.
/// Restart-required once cached, as the shared-texture flag is fixed per
/// browser anyway. (gpu-osr, I6)
pub(super) fn probe_accel_osr() {
    if ACCEL_PROBE.get().is_some() || NO_GL.load(Ordering::Relaxed) {
        return;
    }
    // Env wins as a dev override / kill-switch; otherwise the experimental
    // `gpu-rendering` GSetting.
    let requested = match std::env::var("KARERE_GPU_OSR").ok().as_deref() {
        Some("1") | Some("true") => true,
        Some("0") | Some("false") => false,
        _ => {
            use gtk::prelude::SettingsExt;
            let want = super::with_app_settings(|s| s.boolean("gpu-rendering"));
            // NVIDIA can't back CEF's exportable shared images — the GPU
            // process fails SkSurface init and no accelerated frame ever
            // arrives, leaving the chat area black (#167). Probe the GL
            // context actually rendering (current here) instead of the PCI
            // driver list, so hybrid setups rendering on another GPU keep
            // the accel path (#173). KARERE_GPU_OSR=1 forces.
            if want && gl_vendor_is_nvidia() {
                log::warn!(
                    "accel_osr: NVIDIA driver detected — GPU rendering is \
                     unsupported (CEF shared-image export fails, #167); \
                     using software rendering. KARERE_GPU_OSR=1 to force."
                );
                false
            } else {
                want
            }
        }
    };
    // Do not ask CEF for shared textures unless this GL/EGL display can
    // import them. Without this pre-browser fence CEF may never call CPU
    // on_paint, leaving a blank first frame.
    let enabled = if requested {
        match crate::gl_dmabuf::dmabuf_import_supported() {
            Some(ok) => ok,
            None => return, // not measurable here; keep CPU OSR, retry later
        }
    } else {
        false
    };
    log::info!("accel_osr: requested={requested} enabled={enabled}");
    let _ = ACCEL_PROBE.set(enabled);
}

/// Whether a browser created now should ask CEF for shared textures.
///
/// Before the first successful [`probe_accel_osr`] — a background (tray)
/// start creates every account's browser before any GLArea is realized —
/// this answers CPU OSR without caching it, so browsers created after the
/// window first shows (new accounts, DevTools, a software restart) still
/// get the probed answer. Pre-probe browsers stay on CPU paint for their
/// lifetime: requesting shared textures blind risks a never-painting view
/// on machines that can't export them (#167) or have no GL at all (#177).
pub(super) fn accel_osr_enabled() -> bool {
    // Nothing can import a shared texture without a GL context, and asking
    // for one stops CEF ever calling CPU on_paint -- which is the only
    // thing the software present path has to draw. (#177)
    if NO_GL.load(Ordering::Relaxed) {
        return false;
    }
    match ACCEL_PROBE.get() {
        Some(&enabled) => enabled,
        None => {
            log::debug!("accel_osr: not probed yet (no GL context) — CPU OSR for this browser");
            false
        }
    }
}

/// True when the current GL context is driven by the NVIDIA proprietary
/// driver (GL_VENDOR contains "NVIDIA"; Mesa reports "Mesa"/"AMD" even on
/// NVIDIA hardware via nouveau/NVK, where shared images are untested but
/// not known-broken). Expects a current GL context; without one it falls
/// back to whether the nvidia kernel driver is bound. (gpu-osr)
fn gl_vendor_is_nvidia() -> bool {
    let p = unsafe { gl::GetString(gl::VENDOR) };
    if p.is_null() {
        // No current GL context (should not happen from the realize
        // probe). Fall back to the coarse driver-bound check so NVIDIA
        // still gets software rendering instead of the broken accel path.
        return std::path::Path::new("/sys/bus/pci/drivers/nvidia").exists();
    }
    let vendor = unsafe { std::ffi::CStr::from_ptr(p.cast()) }.to_string_lossy();
    log::info!("accel_osr: GL_VENDOR={vendor}");
    vendor.contains("NVIDIA")
}

pub(super) const VS: &str = r#"#version 300 es
precision highp float;
layout(location=0) in vec2 a_pos;
layout(location=1) in vec2 a_uv;
out vec2 v_uv;
// Sampled sub-rect of the texture: xy = offset, zw = size (fractions).
// (0,0,1,1) for CPU frames; a DMA-BUF's visible rect inside its coded size.
uniform vec4 u_uv;
void main() {
    v_uv = u_uv.xy + a_uv * u_uv.zw;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
"#;

pub(super) const FS: &str = r#"#version 300 es
precision highp float;
in vec2 v_uv;
out vec4 frag;
uniform sampler2D u_tex;
// 1 = software path (CEF BGRA bytes uploaded as RGBA → swizzle back).
// 0 = GPU dma-buf path (EGL imports the real format → sample as RGBA). (gpu-osr)
uniform int u_bgra;
void main() {
    vec4 c = texture(u_tex, v_uv);
    frag = (u_bgra == 1) ? c.bgra : c.rgba;
}
"#;

unsafe fn compile_program(vsrc: &CString, fsrc: &CString) -> GLuint {
    unsafe {
        let vs = compile_shader(gl::VERTEX_SHADER, vsrc);
        let fs = compile_shader(gl::FRAGMENT_SHADER, fsrc);
        let p = gl::CreateProgram();
        gl::AttachShader(p, vs);
        gl::AttachShader(p, fs);
        gl::LinkProgram(p);
        let mut ok = 0;
        gl::GetProgramiv(p, gl::LINK_STATUS, &mut ok);
        if ok == 0 {
            let mut log_len = 0;
            gl::GetProgramiv(p, gl::INFO_LOG_LENGTH, &mut log_len);
            let mut buf = vec![0u8; log_len as usize];
            gl::GetProgramInfoLog(
                p,
                log_len,
                std::ptr::null_mut(),
                // c_char differs per arch (i8 x86_64 / u8 aarch64); cast per-arch
                // (hardcoding i8 broke the aarch64 Flatpak build).
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
            );
            log::error!("program link: {}", String::from_utf8_lossy(&buf));
        }
        gl::DeleteShader(vs);
        gl::DeleteShader(fs);
        p
    }
}

unsafe fn compile_shader(kind: GLenum, src: &CString) -> GLuint {
    unsafe {
        let s = gl::CreateShader(kind);
        let p = src.as_ptr();
        gl::ShaderSource(s, 1, &p, std::ptr::null());
        gl::CompileShader(s);
        let mut ok = 0;
        gl::GetShaderiv(s, gl::COMPILE_STATUS, &mut ok);
        if ok == 0 {
            let mut log_len = 0;
            gl::GetShaderiv(s, gl::INFO_LOG_LENGTH, &mut log_len);
            let mut buf = vec![0u8; log_len as usize];
            gl::GetShaderInfoLog(
                s,
                log_len,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
            );
            log::error!("shader compile: {}", String::from_utf8_lossy(&buf));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::{CpuUpload, cpu_upload};

    #[test]
    fn cpu_upload_covers_empty_first_reuse_update_and_resize() {
        let mut frame = crate::handlers::FrameBuffer::default();
        assert_eq!(cpu_upload(&frame, (0, 0)), CpuUpload::Empty);

        // CEF supplies BGRA bytes; the fragment shader performs the matching
        // BGRA swizzle on the software path.
        frame.width = 2;
        frame.height = 1;
        frame.pixels = vec![1, 2, 3, 255, 4, 5, 6, 255];
        frame.dirty = true;
        assert_eq!(cpu_upload(&frame, (0, 0)), CpuUpload::Allocate);
        assert!(super::FS.contains("c.bgra"));

        frame.dirty = false;
        assert_eq!(cpu_upload(&frame, (2, 1)), CpuUpload::Reuse);
        frame.dirty = true;
        assert_eq!(cpu_upload(&frame, (2, 1)), CpuUpload::Update);

        frame.width = 1;
        frame.height = 2;
        assert_eq!(cpu_upload(&frame, (2, 1)), CpuUpload::Allocate);
    }

    #[test]
    fn layers_are_placed_top_left_without_stretching() {
        // Current-size frame fills the framebuffer exactly.
        assert_eq!(
            super::gl_viewport_for((100, 50), (100, 50), (0, 0, 100, 50)),
            (0, 0, 100, 50)
        );
        // Stale smaller frame mid-resize: own size, top-left (GL y is bottom-up).
        assert_eq!(
            super::gl_viewport_for((100, 50), (100, 50), (0, 0, 80, 40)),
            (0, 10, 80, 40)
        );
        // Popup at view (10, 5).
        assert_eq!(
            super::gl_viewport_for((100, 50), (100, 50), (10, 5, 20, 10)),
            (10, 35, 20, 10)
        );
        // A framebuffer at another scale than the view still maps a full
        // frame onto all of it.
        assert_eq!(
            super::gl_viewport_for((150, 75), (200, 100), (0, 0, 200, 100)),
            (0, 0, 150, 75)
        );
    }

    #[test]
    fn accelerated_frames_sample_only_their_visible_rect() {
        assert_eq!(super::texture_uv((0, 0, 4, 2), (4, 2)), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(super::texture_uv((0, 0, 2, 1), (4, 2)), [0.0, 0.0, 0.5, 0.5]);
        assert!(super::VS.contains("u_uv"));
    }

    #[test]
    fn background_follows_the_theme_foreground() {
        // Light text = dark theme.
        assert_eq!(
            super::page_background_for_foreground(0.95, 0.95, 0.95),
            super::PAGE_BG_DARK
        );
        assert_eq!(
            super::page_background_for_foreground(0.1, 0.1, 0.1),
            super::PAGE_BG_LIGHT
        );
    }
}
