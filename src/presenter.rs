//! GTK textures own their storage and work with Vulkan, GL and software GSK.
use anyhow::{Context, Result, bail};
use gtk::{gdk, prelude::*};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

type CpuTile = (crate::cpu_frame::Rect, crate::cpu_frame::Rect, gdk::Texture);

#[derive(Default)]
pub struct Presenter {
    pub texture: Option<gdk::Texture>,
    gpu_crop: Option<crate::cpu_frame::Rect>,
    cpu_tiles: Vec<CpuTile>,
    cpu_size: (i32, i32),
    copier: Option<crate::vulkan_frame::Copier>,
    vulkan_failed: bool,
    gl_context: Option<gdk::GLContext>,
    gl_failed: bool,
    gl_frames: Arc<AtomicUsize>,
}

impl Presenter {
    fn order(widget: &impl IsA<gtk::Widget>) -> [bool; 2] {
        let gl_first = match std::env::var("KARERE_FRAME_TRANSFER").as_deref() {
            Ok("gl") => true,
            Ok("vulkan") => false,
            _ => widget
                .native()
                .and_then(|native| native.renderer())
                .is_some_and(|renderer| renderer.type_().name().contains("GLRenderer")),
        };
        // Match the actual GTK renderer to avoid an API-crossing readback. The
        // diagnostic override isolates transfer performance from GSK selection.
        [gl_first, !gl_first]
    }

    pub fn supports_acceleration(&mut self, widget: &impl IsA<gtk::Widget>) -> bool {
        let _deadline = crate::gpu_recovery::Deadline::arm();
        for gl in Self::order(widget) {
            if gl && !self.gl_failed {
                match self.ensure_gl(widget) {
                    Ok(()) => return true,
                    Err(error) => {
                        self.gl_failed = true;
                        log::warn!("graphics: GL transfer unavailable: {error:#}");
                    }
                }
            } else if !gl && !self.vulkan_failed {
                if self.copier.is_some() {
                    return true;
                }
                match crate::vulkan_frame::Copier::new() {
                    Ok(copier) => {
                        self.copier = Some(copier);
                        return true;
                    }
                    Err(error) => {
                        self.vulkan_failed = true;
                        log::warn!("graphics: Vulkan transfer unavailable: {error:#}");
                    }
                }
            }
        }
        false
    }

    pub fn cpu(&mut self, frame: &mut crate::handlers::FrameBuffer) {
        if frame.width <= 0 || frame.height <= 0 || (!frame.dirty && !self.cpu_tiles.is_empty()) {
            return;
        }
        self.texture = None;
        self.gpu_crop = None;
        if self.cpu_size != (frame.width, frame.height)
            || self.cpu_tiles.len() != frame.tiles.tiles.len()
        {
            self.cpu_tiles.clear();
            self.cpu_size = (frame.width, frame.height);
        }
        for (index, tile) in frame.tiles.tiles.iter_mut().enumerate() {
            if !tile.dirty && index < self.cpu_tiles.len() {
                continue;
            }
            let texture = gdk::MemoryTextureBuilder::new()
                .set_width(tile.sample.2)
                .set_height(tile.sample.3)
                .set_format(gdk::MemoryFormat::B8g8r8a8Premultiplied)
                .set_stride(tile.sample.2 as usize * 4)
                .set_bytes(Some(&tile.bytes))
                .build();
            let item = (tile.rect, tile.sample, texture);
            if index == self.cpu_tiles.len() {
                self.cpu_tiles.push(item);
            } else {
                self.cpu_tiles[index] = item;
            }
            tile.dirty = false;
        }
        frame.dirty = false;
    }

    pub fn snapshot(&self, snapshot: &gtk::Snapshot, bounds: &gtk::graphene::Rect) {
        if let Some(texture) = self.texture.as_ref() {
            let (x, y, w, h) = self
                .gpu_crop
                .unwrap_or((0, 0, texture.width(), texture.height()));
            let (sx, sy) = (bounds.width() / w as f32, bounds.height() / h as f32);
            snapshot.push_clip(bounds);
            snapshot.append_texture(
                texture,
                &gtk::graphene::Rect::new(
                    bounds.x() - x as f32 * sx,
                    bounds.y() - y as f32 * sy,
                    texture.width() as f32 * sx,
                    texture.height() as f32 * sy,
                ),
            );
            snapshot.pop();
        } else if self.cpu_size.0 > 0 && self.cpu_size.1 > 0 {
            let sx = bounds.width() / self.cpu_size.0 as f32;
            let sy = bounds.height() / self.cpu_size.1 as f32;
            let scaled = |(x, y, w, h): crate::cpu_frame::Rect| {
                gtk::graphene::Rect::new(
                    bounds.x() + x as f32 * sx,
                    bounds.y() + y as f32 * sy,
                    w as f32 * sx,
                    h as f32 * sy,
                )
            };
            for (rect, sample, texture) in &self.cpu_tiles {
                snapshot.push_clip(&scaled(*rect));
                snapshot.append_texture(texture, &scaled(*sample));
                snapshot.pop();
            }
        }
    }

    pub fn accelerated(
        &mut self,
        widget: &impl IsA<gtk::Widget>,
        info: &cef::AcceleratedPaintInfo,
    ) -> Result<bool> {
        let _deadline = crate::gpu_recovery::Deadline::arm();
        let crop = visible_crop(info)?;
        for gl in Self::order(widget) {
            if gl && !self.gl_failed {
                match self.gl(widget, info) {
                    Ok(Some(texture)) => {
                        self.texture = Some(texture);
                        self.gpu_crop = crop;
                        return Ok(true);
                    }
                    Ok(None) => return Ok(false),
                    Err(error) => {
                        self.gl_failed = true;
                        log::warn!(
                            "graphics: GL frame transfer failed: {error:#}; trying remaining transfer backend"
                        );
                    }
                }
            } else if !gl && !self.vulkan_failed {
                match self.vulkan(&widget.display(), info) {
                    Ok(result) => {
                        if result {
                            self.gpu_crop = crop;
                        }
                        return Ok(result);
                    }
                    Err(error) => {
                        self.vulkan_failed = true;
                        self.copier = None;
                        log::warn!(
                            "graphics: Vulkan frame transfer failed: {error:#}; trying remaining transfer backend"
                        );
                    }
                }
            }
        }
        bail!("all eligible accelerated transfer backends failed; CPU frames required")
    }

    fn vulkan(&mut self, display: &gdk::Display, info: &cef::AcceleratedPaintInfo) -> Result<bool> {
        if self.copier.is_none() {
            self.copier = Some(crate::vulkan_frame::Copier::new()?);
        }
        let Some(frame) = self.copier.as_mut().unwrap().copy(info)? else {
            return Ok(false);
        };
        self.texture = Some(texture_from_vulkan(display, frame)?);
        Ok(true)
    }

    fn ensure_gl(&mut self, widget: &impl IsA<gtk::Widget>) -> Result<()> {
        if self.gl_context.is_none() {
            let surface = widget
                .native()
                .and_then(|n| n.surface())
                .context("view has no surface")?;
            let context = surface.create_gl_context()?;
            context.set_allowed_apis(gdk::GLAPI::GLES);
            context.set_required_version(3, 0);
            context.realize()?;
            let previous = gdk::GLContext::current();
            context.make_current();
            let supported = crate::gl_dmabuf::is_supported();
            let renderer = unsafe {
                let value = gl::GetString(gl::RENDERER);
                if value.is_null() {
                    "unavailable".to_owned()
                } else {
                    std::ffi::CStr::from_ptr(value.cast())
                        .to_string_lossy()
                        .into_owned()
                }
            };
            if let Some(previous) = previous {
                previous.make_current();
            } else {
                gdk::GLContext::clear_current();
            }
            anyhow::ensure!(supported, "GL display lacks DMA-BUF import support");
            self.gl_context = Some(context);
            log::info!("graphics: using GL copy for accelerated frames; renderer={renderer}");
        }
        Ok(())
    }

    fn gl(
        &mut self,
        widget: &impl IsA<gtk::Widget>,
        info: &cef::AcceleratedPaintInfo,
    ) -> Result<Option<gdk::Texture>> {
        if self.gl_frames.load(Ordering::Acquire) >= 3 {
            return Ok(None);
        }
        self.ensure_gl(widget)?;
        let context = self.gl_context.as_ref().unwrap();
        let previous = gdk::GLContext::current();
        context.make_current();
        self.gl_frames.fetch_add(1, Ordering::AcqRel);
        let result = unsafe { copy_gl_frame(context, info, self.gl_frames.clone()) };
        if result.is_err() {
            self.gl_frames.fetch_sub(1, Ordering::AcqRel);
        }
        if let Some(previous) = previous {
            previous.make_current();
        } else {
            gdk::GLContext::clear_current();
        }
        result.map(Some)
    }

    #[cfg(test)]
    pub(crate) fn gl_fixture(
        &mut self,
        widget: &impl IsA<gtk::Widget>,
        info: &cef::AcceleratedPaintInfo,
    ) -> Result<Option<gdk::Texture>> {
        self.gl(widget, info)
    }
}

fn visible_crop(info: &cef::AcceleratedPaintInfo) -> Result<Option<crate::cpu_frame::Rect>> {
    let r = &info.extra.visible_rect;
    // Older producers and the ownership fixture omit optional metadata.
    if r.width == 0 && r.height == 0 {
        return Ok(None);
    }
    anyhow::ensure!(
        r.x >= 0
            && r.y >= 0
            && r.width > 0
            && r.height > 0
            && i64::from(r.x) + i64::from(r.width) <= i64::from(info.extra.coded_size.width)
            && i64::from(r.y) + i64::from(r.height) <= i64::from(info.extra.coded_size.height),
        "invalid accelerated visible rectangle"
    );
    Ok(Some((r.x, r.y, r.width, r.height)))
}

pub(crate) fn texture_from_vulkan(
    display: &gdk::Display,
    frame: std::sync::Arc<crate::vulkan_frame::Frame>,
) -> Result<gdk::Texture> {
    let builder = gdk::DmabufTextureBuilder::new()
        .set_display(display)
        .set_width(frame.width)
        .set_height(frame.height)
        .set_fourcc(frame.fourcc)
        .set_modifier(frame.modifier)
        .set_n_planes(1)
        .set_stride(0, frame.stride)
        .set_offset(0, frame.offset)
        .set_premultiplied(true);
    // Copy is complete and the owned allocation outlives GTK's last reference.
    let texture = unsafe { builder.set_fd(0, frame.fd.as_raw_fd()).build()? };
    unsafe {
        texture.set_data("karere-owned-vulkan-frame", frame);
    }
    Ok(texture)
}

/// The borrowed CEF allocation is sampled only before this function returns.
unsafe fn copy_gl_frame(
    context: &gdk::GLContext,
    info: &cef::AcceleratedPaintInfo,
    live_frames: Arc<AtomicUsize>,
) -> Result<gdk::Texture> {
    let _deadline = crate::gpu_recovery::Deadline::arm();
    unsafe {
        let (width, height) = (info.extra.coded_size.width, info.extra.coded_size.height);
        anyhow::ensure!(
            width > 0
                && height > 0
                && info.plane_count > 0
                && info.plane_count as usize <= info.planes.len(),
            "invalid shared frame"
        );
        let fourcc = crate::gl_dmabuf::cef_format_to_fourcc(info.format)
            .context("unsupported pixel format")?;
        let mut planes = Vec::new();
        for p in info.planes.iter().take(info.plane_count as usize) {
            anyhow::ensure!(p.fd >= 0, "invalid DMA-BUF descriptor");
            crate::vulkan_frame::wait_dmabuf(p.fd, false)?;
            planes.push(crate::gl_dmabuf::Plane {
                fd: BorrowedFd::borrow_raw(p.fd).try_clone_to_owned()?,
                offset: p.offset,
                stride: p.stride,
            });
        }
        let (mut imported_texture, mut framebuffer, mut owned_texture) = (0, 0, 0);
        gl::GenTextures(1, &mut imported_texture);
        let result = (|| -> Result<gdk::Texture> {
            let _image = crate::gl_dmabuf::import_to_texture(
                imported_texture,
                width,
                height,
                fourcc,
                info.modifier,
                &planes,
            )
            .context("EGL import rejected")?;
            gl::GenFramebuffers(1, &mut framebuffer);
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                imported_texture,
                0,
            );
            anyhow::ensure!(
                gl::CheckFramebufferStatus(gl::FRAMEBUFFER) == gl::FRAMEBUFFER_COMPLETE,
                "imported frame is not readable"
            );
            gl::GenTextures(1, &mut owned_texture);
            gl::BindTexture(gl::TEXTURE_2D, owned_texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            gl::CopyTexImage2D(gl::TEXTURE_2D, 0, gl::RGBA, 0, 0, width, height, 0);
            // CEF may immediately reuse its image after this callback. A finite
            // fence timeout requires process recovery, never an unsafe return.
            let fence = gl::FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0);
            if fence.is_null() {
                crate::gpu_recovery::completion_unknown("GL copy fence creation failed");
            }
            let status = gl::ClientWaitSync(
                fence,
                gl::SYNC_FLUSH_COMMANDS_BIT,
                crate::gpu_recovery::FENCE_TIMEOUT_NS,
            );
            if status != gl::ALREADY_SIGNALED && status != gl::CONDITION_SATISFIED {
                crate::gpu_recovery::completion_unknown("GL copy fence failed or timed out");
            }
            gl::DeleteSync(fence);
            anyhow::ensure!(gl::GetError() == gl::NO_ERROR, "GL frame copy failed");
            let id = owned_texture;
            let owner = context.clone();
            let texture =
                gdk::GLTexture::with_release_func(context, id, width, height, move || {
                    let _deadline = crate::gpu_recovery::Deadline::arm();
                    let previous = gdk::GLContext::current();
                    owner.make_current();
                    gl::DeleteTextures(1, &id);
                    if let Some(previous) = previous {
                        previous.make_current();
                    } else {
                        gdk::GLContext::clear_current();
                    }
                    live_frames.fetch_sub(1, Ordering::AcqRel);
                });
            owned_texture = 0;
            Ok(texture.upcast())
        })();
        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        gl::DeleteFramebuffers(1, &framebuffer);
        gl::DeleteTextures(1, &imported_texture);
        if owned_texture != 0 {
            gl::DeleteTextures(1, &owned_texture);
        }
        result
    }
}

#[cfg(test)]
pub fn verify_cpu_texture_ownership() {
    let shared = crate::handlers::new_shared((2, 1), 1.0);
    shared.lock().snapshot_present = true;
    let mut pixels = [0_u8, 0, 255, 255, 255, 0, 0, 255];
    crate::handlers::render::dispatch_cpu_paint_for_test(&shared, &pixels, 2, 1);
    pixels.fill(0); // The CEF callback no longer owns valid input.
    let mut presenter = Presenter::default();
    presenter.cpu(&mut shared.lock().frame);
    let previous = presenter.cpu_tiles[0].2.clone();
    crate::handlers::render::dispatch_cpu_paint_for_test(
        &shared,
        &[0, 255, 0, 255, 0, 255, 0, 255],
        2,
        1,
    );
    presenter.cpu(&mut shared.lock().frame);
    let mut download = gdk::TextureDownloader::new(&previous);
    download.set_format(gdk::MemoryFormat::B8g8r8a8Premultiplied);
    assert_eq!(
        &download.download_bytes().0[..8],
        &[0, 0, 255, 255, 255, 0, 0, 255]
    );
    download.set_texture(&presenter.cpu_tiles[0].2);
    assert_eq!(
        &download.download_bytes().0[..8],
        &[0, 255, 0, 255, 0, 255, 0, 255]
    );
    assert!(!shared.lock().frame.dirty);
}

/// Compare tiled sampling with an untiled reference through the real Vulkan
/// renderer. The 145% case matches this monitor; other scales catch tile seams.
#[cfg(test)]
pub fn verify_fractional_tiles() {
    let window = gtk::Window::new();
    gtk::prelude::WidgetExt::realize(&window);
    let Some(renderer) = window.renderer() else {
        eprintln!("SKIP fractional texture fixture: no GSK renderer");
        return;
    };
    let (width, height) = (1000, 80);
    let mut pixels = vec![0_u8; width * height * 4];
    for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (index % width, index / width);
        pixel.copy_from_slice(&[
            (x % 191) as u8,
            (y * 7 % 191) as u8,
            ((x + y) % 191) as u8,
            191,
        ]);
    }
    let mut frame = crate::handlers::FrameBuffer::default();
    frame
        .tiles
        .update(&pixels, width as i32, height as i32, 2.0, None);
    frame.width = width as i32;
    frame.height = height as i32;
    frame.dirty = true;
    let mut tiled = Presenter::default();
    tiled.cpu(&mut frame);
    let reference = gdk::MemoryTexture::new(
        width as i32,
        height as i32,
        gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &glib::Bytes::from_owned(pixels),
        width * 4,
    );
    let mut errors = Vec::new();
    for scale in [1.0_f32, 1.45, 2.0] {
        let bounds = gtk::graphene::Rect::new(0.0, 0.0, width as f32 / 2.0, height as f32 / 2.0);
        let viewport =
            gtk::graphene::Rect::new(0.0, 0.0, bounds.width() * scale, bounds.height() * scale);
        let capture = |tiles: bool| {
            let snapshot = gtk::Snapshot::new();
            snapshot.scale(scale, scale);
            if tiles {
                tiled.snapshot(&snapshot, &bounds);
            } else {
                snapshot.append_texture(&reference, &bounds);
            }
            let texture = renderer.render_texture(snapshot.to_node().unwrap(), Some(&viewport));
            let mut download = gdk::TextureDownloader::new(&texture);
            download.set_format(gdk::MemoryFormat::B8g8r8a8Premultiplied);
            download.download_bytes().0
        };
        let expected = capture(false);
        let actual = capture(true);
        assert_eq!(actual.len(), expected.len());
        let difference = actual
            .iter()
            .zip(expected.iter())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        if difference > 1 {
            let stride = (viewport.width().ceil() as usize) * 4;
            let bad: Vec<_> = actual
                .iter()
                .zip(expected.iter())
                .enumerate()
                .filter(|(_, (a, b))| a.abs_diff(**b) > 1)
                .map(|(i, (a, b))| (i % stride / 4, i / stride, i % 4, *a, *b))
                .collect();
            eprintln!(
                "scale {scale}, errors={} first={:?}",
                bad.len(),
                &bad[..bad.len().min(20)]
            );
            errors.push((scale, difference));
        }
    }
    let texture = gdk::MemoryTexture::new(
        4,
        1,
        gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &glib::Bytes::from_static(&[
            0, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255, 255, 255, 255, 255,
        ]),
        16,
    );
    let cropped = Presenter {
        texture: Some(texture.upcast()),
        gpu_crop: Some((1, 0, 2, 1)),
        ..Default::default()
    };
    let snapshot = gtk::Snapshot::new();
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, 2.0, 1.0);
    cropped.snapshot(&snapshot, &bounds);
    let texture = renderer.render_texture(snapshot.to_node().unwrap(), Some(&bounds));
    let mut download = gdk::TextureDownloader::new(&texture);
    download.set_format(gdk::MemoryFormat::B8g8r8a8Premultiplied);
    let actual = download.download_bytes().0;
    window.destroy();
    assert_eq!(
        &actual[..8],
        &[0, 0, 255, 255, 0, 255, 0, 255],
        "visible rectangle must exclude allocation padding"
    );
    assert!(errors.is_empty(), "tiled sampling differences: {errors:?}");
    eprintln!(
        "PASS {} tiled colors and seams at 100%, 145%, 200% scale",
        renderer.type_().name()
    );
}
