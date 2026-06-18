//! GPU-accelerated OSR: import CEF's `on_accelerated_paint` DMA-BUF into a GL
//! texture via EGL (`EGL_EXT_image_dma_buf_import`), avoiding the per-frame CPU
//! readback + `glTexImage2D` upload of the software `on_paint` path.
//!
//! The CEF crate ships a Vulkan/wgpu importer; Karere renders through a GTK
//! `GLArea` (OpenGL/epoxy), so we do the EGL→GL import ourselves. All calls run
//! on the glib main thread with the GLArea's GL context current (from `draw`).

use std::ffi::{CStr, c_void};
use std::os::raw::{c_char, c_int, c_uint};
use std::sync::OnceLock;

use libloading::Library;

// ---- EGL / GL-OES constants ------------------------------------------------
const EGL_EXTENSIONS: c_int = 0x3055;
const EGL_LINUX_DMA_BUF_EXT: c_int = 0x3270;
const EGL_LINUX_DRM_FOURCC_EXT: c_int = 0x3271;
const EGL_WIDTH: c_int = 0x3057;
const EGL_HEIGHT: c_int = 0x3056;
const EGL_NONE: c_int = 0x3038;
// Plane 0..3 fd / offset / pitch / modifier-lo / modifier-hi.
const EGL_DMA_BUF_PLANE_FD: [c_int; 4] = [0x3272, 0x3275, 0x3278, 0x3440];
const EGL_DMA_BUF_PLANE_OFFSET: [c_int; 4] = [0x3273, 0x3276, 0x3279, 0x3441];
const EGL_DMA_BUF_PLANE_PITCH: [c_int; 4] = [0x3274, 0x3277, 0x327A, 0x3442];
const EGL_DMA_BUF_PLANE_MOD_LO: [c_int; 4] = [0x3443, 0x3445, 0x3447, 0x3449];
const EGL_DMA_BUF_PLANE_MOD_HI: [c_int; 4] = [0x3444, 0x3446, 0x3448, 0x344A];

const GL_TEXTURE_2D: c_uint = 0x0DE1;
const DRM_FORMAT_MOD_INVALID: u64 = 0x00FF_FFFF_FFFF_FFFF;

type EglDisplay = *mut c_void;
type EglImage = *mut c_void;
// eglCreateImageKHR uses EGLint (32-bit) attributes — NOT the 64-bit EGLAttrib
// of core eglCreateImage. Using the wrong width misaligns the list. (gpu-osr)
type EglAttrib = i32;

/// EGL/GL-OES entry points. Core EGL is dlsym'd from libEGL directly (the GL
/// proc loader / `eglGetProcAddress` does NOT return core EGL functions);
/// extensions are resolved via `eglGetProcAddress`. The `Library` is kept alive
/// so the function pointers stay valid. (gpu-osr)
struct Egl {
    get_current_display: unsafe extern "C" fn() -> EglDisplay,
    query_string: unsafe extern "C" fn(EglDisplay, c_int) -> *const i8,
    create_image: unsafe extern "C" fn(
        EglDisplay,
        *mut c_void, // ctx (EGL_NO_CONTEXT)
        c_uint,      // target
        *mut c_void, // buffer (NULL)
        *const EglAttrib,
    ) -> EglImage,
    destroy_image: unsafe extern "C" fn(EglDisplay, EglImage) -> c_uint,
    image_target_texture_2d: unsafe extern "C" fn(c_uint, EglImage),
    _lib: Library,
}

// SAFETY: the function pointers are immutable after load; we only ever call them
// on the main thread (enforced by callers).
unsafe impl Send for Egl {}
unsafe impl Sync for Egl {}

static EGL: OnceLock<Option<Egl>> = OnceLock::new();

/// `fourcc('A','R','2','4')` etc.
const fn fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

fn load_egl() -> Option<Egl> {
    unsafe {
        let lib = Library::new("libEGL.so.1")
            .or_else(|_| Library::new("libEGL.so"))
            .map_err(|e| log::warn!("gl_dmabuf: cannot load libEGL: {e}"))
            .ok()?;

        // Core EGL via dlsym.
        let get_current_display = *lib
            .get::<unsafe extern "C" fn() -> EglDisplay>(b"eglGetCurrentDisplay\0")
            .ok()?;
        let query_string = *lib
            .get::<unsafe extern "C" fn(EglDisplay, c_int) -> *const i8>(b"eglQueryString\0")
            .ok()?;
        let get_proc = *lib
            .get::<unsafe extern "C" fn(*const c_char) -> *const c_void>(b"eglGetProcAddress\0")
            .ok()?;

        // Extension / client-API entry points via eglGetProcAddress.
        let ci = get_proc(c"eglCreateImageKHR".as_ptr());
        let di = get_proc(c"eglDestroyImageKHR".as_ptr());
        let itt = get_proc(c"glEGLImageTargetTexture2DOES".as_ptr());
        if ci.is_null() || di.is_null() || itt.is_null() {
            log::warn!("gl_dmabuf: missing EGL/GL-OES extension entry points; accelerated OSR unavailable");
            return None;
        }

        Some(Egl {
            get_current_display,
            query_string,
            create_image: std::mem::transmute::<*const c_void, _>(ci),
            destroy_image: std::mem::transmute::<*const c_void, _>(di),
            image_target_texture_2d: std::mem::transmute::<*const c_void, _>(itt),
            _lib: lib,
        })
    }
}

fn egl() -> Option<&'static Egl> {
    EGL.get_or_init(load_egl).as_ref()
}

/// True when EGL + the dma-buf import extension are present on the current
/// display. Must be called with the GLArea GL context current (so EGL has a
/// current display). Cached after the first successful query. (gpu-osr)
#[allow(dead_code)]
pub fn is_supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        let Some(e) = egl() else {
            log::warn!("gl_dmabuf: EGL load failed");
            return false;
        };
        unsafe {
            let dpy = (e.get_current_display)();
            if dpy.is_null() {
                log::warn!("gl_dmabuf: no current EGL display (GL context not current at check)");
                return false;
            }
            let exts = (e.query_string)(dpy, EGL_EXTENSIONS);
            if exts.is_null() {
                log::warn!("gl_dmabuf: eglQueryString(EGL_EXTENSIONS) returned null");
                return false;
            }
            let s = CStr::from_ptr(exts).to_string_lossy();
            let ok = s.contains("EGL_EXT_image_dma_buf_import");
            log::warn!("gl_dmabuf: dma-buf import {}", if ok { "supported" } else { "NOT supported" });
            ok
        }
    })
}

/// One imported frame: the EGLImage kept alive while its texture is sampled.
pub struct ImportedImage {
    dpy: EglDisplay,
    image: EglImage,
}

impl Drop for ImportedImage {
    fn drop(&mut self) {
        if let Some(e) = egl()
            && !self.image.is_null()
            && !self.dpy.is_null()
        {
            unsafe {
                (e.destroy_image)(self.dpy, self.image);
            }
        }
    }
}

/// A single DMA-BUF plane (owned, dup'd fd closed on drop).
pub struct Plane {
    pub fd: std::os::fd::OwnedFd,
    pub offset: u64,
    pub stride: u32,
}

/// One pending accelerated frame handed off from `on_accelerated_paint` (CEF UI
/// thread) to `draw` (GL context current). Owns the dup'd plane fds. (gpu-osr)
pub struct AccelFrame {
    pub width: i32,
    pub height: i32,
    pub fourcc: u32,
    pub modifier: u64,
    pub planes: Vec<Plane>,
    pub dirty: bool,
}

/// Import a DMA-BUF described by `planes`/`modifier`/`fourcc` and bind it to
/// `texture` (a `GL_TEXTURE_2D`). Returns the EGLImage wrapper to keep alive
/// until the next frame replaces it. Context must be current. (gpu-osr)
pub fn import_to_texture(
    texture: c_uint,
    width: i32,
    height: i32,
    fourcc_format: u32,
    modifier: u64,
    planes: &[Plane],
) -> Option<ImportedImage> {
    use std::os::fd::AsRawFd;
    let e = egl()?;
    unsafe {
        let dpy = (e.get_current_display)();
        if dpy.is_null() {
            log::warn!("gl_dmabuf: import skipped — no current EGL display in draw()");
            return None;
        }
        if planes.is_empty() {
            return None;
        }

        let mut attrs: Vec<EglAttrib> = Vec::with_capacity(7 + planes.len() * 10);
        attrs.push(EGL_WIDTH as EglAttrib);
        attrs.push(width as EglAttrib);
        attrs.push(EGL_HEIGHT as EglAttrib);
        attrs.push(height as EglAttrib);
        attrs.push(EGL_LINUX_DRM_FOURCC_EXT as EglAttrib);
        attrs.push(fourcc_format as EglAttrib);

        // Only emit modifier attributes for an explicit (non-linear) modifier:
        // they need EGL_EXT_image_dma_buf_import_modifiers, and passing them for
        // LINEAR (0) / INVALID makes base eglCreateImageKHR fail with
        // EGL_BAD_ATTRIBUTE. Linear buffers import fine without them. (gpu-osr)
        let use_modifier = modifier != DRM_FORMAT_MOD_INVALID && modifier != 0;
        for (i, p) in planes.iter().enumerate().take(4) {
            attrs.push(EGL_DMA_BUF_PLANE_FD[i] as EglAttrib);
            attrs.push(p.fd.as_raw_fd() as EglAttrib);
            attrs.push(EGL_DMA_BUF_PLANE_OFFSET[i] as EglAttrib);
            attrs.push(p.offset as EglAttrib);
            attrs.push(EGL_DMA_BUF_PLANE_PITCH[i] as EglAttrib);
            attrs.push(p.stride as EglAttrib);
            if use_modifier {
                attrs.push(EGL_DMA_BUF_PLANE_MOD_LO[i] as EglAttrib);
                attrs.push((modifier & 0xFFFF_FFFF) as EglAttrib);
                attrs.push(EGL_DMA_BUF_PLANE_MOD_HI[i] as EglAttrib);
                attrs.push((modifier >> 32) as EglAttrib);
            }
        }
        attrs.push(EGL_NONE as EglAttrib);

        let image = (e.create_image)(
            dpy,
            std::ptr::null_mut(),
            EGL_LINUX_DMA_BUF_EXT as c_uint,
            std::ptr::null_mut(),
            attrs.as_ptr(),
        );
        if image.is_null() {
            static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                log::warn!("gl_dmabuf: eglCreateImageKHR failed (fourcc={fourcc_format:#x} mod={modifier:#x})");
            }
            return None;
        }

        gl::BindTexture(GL_TEXTURE_2D, texture);
        (e.image_target_texture_2d)(GL_TEXTURE_2D, image);
        gl::BindTexture(GL_TEXTURE_2D, 0);

        static FIRST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
        if FIRST.swap(false, std::sync::atomic::Ordering::Relaxed) {
            log::info!("gl_dmabuf: first DMA-BUF import OK ({width}x{height} fourcc={fourcc_format:#x}) — GPU OSR active");
        }

        Some(ImportedImage { dpy, image })
    }
}

/// Map CEF's [`cef::ColorType`] to a DRM fourcc. CEF's `*8888` byte orders match
/// the DRM little-endian formats. Returns `None` for unsupported formats. (gpu-osr)
pub fn cef_format_to_fourcc(format: cef::ColorType) -> Option<u32> {
    use cef::ColorType;
    // CEF BGRA8888 = bytes B,G,R,A = DRM_FORMAT_ARGB8888 ("AR24").
    // CEF RGBA8888 = bytes R,G,B,A = DRM_FORMAT_ABGR8888 ("AB24").
    if format == ColorType::BGRA_8888 {
        Some(fourcc(b'A', b'R', b'2', b'4'))
    } else if format == ColorType::RGBA_8888 {
        Some(fourcc(b'A', b'B', b'2', b'4'))
    } else {
        None
    }
}
