//! Own the pixels before CEF returns its borrowed shared image to its pool.
//!
//! The queue completes its copy before OnAcceleratedPaint returns. GTK receives
//! only our exported image, whose allocation stays alive until its texture dies.
use anyhow::{Context, Result, bail, ensure};
use ash::{Entry, vk};
use std::{
    ffi::CStr,
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd},
    sync::Arc,
};

const DMA: vk::ExternalMemoryHandleTypeFlags = vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT;
const REQUIRED: [&CStr; 4] = [
    ash::khr::external_memory_fd::NAME,
    ash::ext::external_memory_dma_buf::NAME,
    ash::ext::image_drm_format_modifier::NAME,
    ash::ext::queue_family_foreign::NAME,
];

struct Instance {
    _entry: Entry,
    instance: ash::Instance,
}
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { self.instance.destroy_instance(None) }
    }
}

struct Device {
    instance: Arc<Instance>,
    device: ash::Device,
    physical: vk::PhysicalDevice,
    family: u32,
    queue: vk::Queue,
    pool: vk::CommandPool,
}
impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_device(None);
        }
    }
}

struct Image {
    device: Arc<Device>,
    image: vk::Image,
    memory: vk::DeviceMemory,
}
impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_image(self.image, None);
            self.device.device.free_memory(self.memory, None);
        }
    }
}

pub struct Frame {
    allocation: Image,
    initialized: std::sync::atomic::AtomicBool,
    pub fd: OwnedFd,
    pub width: u32,
    pub height: u32,
    pub fourcc: u32,
    pub modifier: u64,
    pub stride: u32,
    pub offset: u32,
}

pub struct Copier {
    device: Arc<Device>,
    // GTK keeps an Arc until it releases the texture. Never overwrite that image.
    frames: Vec<Arc<Frame>>,
}

impl Copier {
    pub fn new() -> Result<Self> {
        unsafe {
            let entry = Entry::load().context("load Vulkan loader")?;
            let app = vk::ApplicationInfo::default()
                .application_name(c"Karere frame transfer")
                .api_version(vk::API_VERSION_1_1);
            let instance = entry.create_instance(
                &vk::InstanceCreateInfo::default().application_info(&app),
                None,
            )?;
            let owner = Arc::new(Instance {
                _entry: entry,
                instance,
            });
            let mut devices = owner.instance.enumerate_physical_devices()?;
            devices.sort_by_key(|p| {
                match owner
                    .instance
                    .get_physical_device_properties(*p)
                    .device_type
                {
                    vk::PhysicalDeviceType::DISCRETE_GPU => 0,
                    vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
                    _ => 2,
                }
            });
            for physical in devices {
                let props = owner.instance.get_physical_device_properties(physical);
                if props.device_type == vk::PhysicalDeviceType::CPU {
                    continue;
                }
                let extensions = owner
                    .instance
                    .enumerate_device_extension_properties(physical)?;
                if !REQUIRED.iter().all(|required| {
                    extensions
                        .iter()
                        .any(|e| CStr::from_ptr(e.extension_name.as_ptr()) == *required)
                }) {
                    continue;
                }
                let Some((family, _)) = owner
                    .instance
                    .get_physical_device_queue_family_properties(physical)
                    .into_iter()
                    .enumerate()
                    .find(|(_, q)| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                else {
                    continue;
                };
                let queues = [vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family as u32)
                    .queue_priorities(&[1.0])];
                let names: Vec<_> = REQUIRED.iter().map(|s| s.as_ptr()).collect();
                let device = owner.instance.create_device(
                    physical,
                    &vk::DeviceCreateInfo::default()
                        .queue_create_infos(&queues)
                        .enabled_extension_names(&names),
                    None,
                )?;
                let pool = match device.create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(family as u32)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                ) {
                    Ok(pool) => pool,
                    Err(e) => {
                        device.destroy_device(None);
                        return Err(e.into());
                    }
                };
                let queue = device.get_device_queue(family as u32, 0);
                log::info!(
                    "graphics: Vulkan frame copier device={}",
                    CStr::from_ptr(props.device_name.as_ptr()).to_string_lossy()
                );
                return Ok(Self {
                    device: Arc::new(Device {
                        instance: owner,
                        device,
                        physical,
                        family: family as u32,
                        queue,
                        pool,
                    }),
                    frames: Vec::new(),
                });
            }
            bail!("no hardware Vulkan device supports DMA-BUF import/export and DRM modifiers")
        }
    }

    /// None means all three owned buffers are still in use; dropping a superseded
    /// frame is safe, allocating an unbounded queue or overwriting one is not.
    pub fn copy(&mut self, info: &cef::AcceleratedPaintInfo) -> Result<Option<Arc<Frame>>> {
        let width = u32::try_from(info.extra.coded_size.width)?;
        let height = u32::try_from(info.extra.coded_size.height)?;
        ensure!(
            width > 0 && height > 0 && info.plane_count == 1,
            "unsupported accelerated image dimensions/planes"
        );
        let fourcc = crate::gl_dmabuf::cef_format_to_fourcc(info.format)
            .context("unsupported CEF pixel format")?;
        let format = match fourcc {
            0x34325241 => vk::Format::B8G8R8A8_UNORM,
            0x34324241 => vk::Format::R8G8B8A8_UNORM,
            _ => bail!("unsupported DMA-BUF fourcc {fourcc:#x}"),
        };
        // Retain old-size buffers while GTK uses them, too. Repeated resizes
        // must not bypass the three-buffer bound by starting a fresh pool.
        self.frames.retain(|f| {
            (f.width, f.height, f.fourcc) == (width, height, fourcc) || Arc::strong_count(f) > 1
        });
        let slot = self
            .frames
            .iter()
            .find(|f| {
                Arc::strong_count(f) == 1
                    && (f.width, f.height, f.fourcc) == (width, height, fourcc)
            })
            .cloned();
        let frame = if let Some(slot) = slot {
            slot
        } else {
            if self.frames.len() == 3 {
                return Ok(None);
            }
            let frame = Arc::new(self.allocate(width, height, fourcc, format)?);
            self.frames.push(frame.clone());
            frame
        };
        wait_dmabuf(info.planes[0].fd, false)?;
        wait_dmabuf(frame.fd.as_raw_fd(), true)?;
        let source = self.import(info, format, width, height)?;
        self.copy_image(&source, &frame, width, height)?;
        frame
            .initialized
            .store(true, std::sync::atomic::Ordering::Release);
        Ok(Some(frame))
    }

    fn memory_index(&self, bits: u32) -> Result<u32> {
        let props = unsafe {
            self.device
                .instance
                .instance
                .get_physical_device_memory_properties(self.device.physical)
        };
        (0..props.memory_type_count)
            .find(|i| bits & (1 << i) != 0)
            .context("no compatible Vulkan image memory type")
    }

    fn import(
        &self,
        info: &cef::AcceleratedPaintInfo,
        format: vk::Format,
        width: u32,
        height: u32,
    ) -> Result<Image> {
        unsafe {
            ensure!(info.planes[0].fd >= 0, "invalid CEF DMA-BUF fd");
            let plane = &info.planes[0];
            let layouts = [vk::SubresourceLayout::default()
                .offset(plane.offset)
                .row_pitch(u64::from(plane.stride))];
            let mut modifier = vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
                .drm_format_modifier(info.modifier)
                .plane_layouts(&layouts);
            let mut external = vk::ExternalMemoryImageCreateInfo::default().handle_types(DMA);
            let create = image_info(format, width, height, vk::ImageUsageFlags::TRANSFER_SRC)
                .push_next(&mut modifier)
                .push_next(&mut external);
            let image = self.device.device.create_image(&create, None)?;
            let result = (|| -> Result<_> {
                let requirements = self.device.device.get_image_memory_requirements(image);
                let fd = BorrowedFd::borrow_raw(plane.fd).try_clone_to_owned()?;
                let loader = ash::khr::external_memory_fd::Device::new(
                    &self.device.instance.instance,
                    &self.device.device,
                );
                let mut properties = vk::MemoryFdPropertiesKHR::default();
                loader.get_memory_fd_properties(DMA, fd.as_raw_fd(), &mut properties)?;
                let index =
                    self.memory_index(requirements.memory_type_bits & properties.memory_type_bits)?;
                let mut imported = vk::ImportMemoryFdInfoKHR::default()
                    .handle_type(DMA)
                    .fd(fd.as_raw_fd());
                let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
                let allocation = vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(index)
                    .push_next(&mut imported)
                    .push_next(&mut dedicated);
                let memory = self.device.device.allocate_memory(&allocation, None)?;
                let _ = fd.into_raw_fd(); // Vulkan owns the duplicated descriptor after success.
                if let Err(e) = self.device.device.bind_image_memory(image, memory, 0) {
                    self.device.device.free_memory(memory, None);
                    return Err(e.into());
                }
                Ok(memory)
            })();
            match result {
                Ok(memory) => Ok(Image {
                    device: self.device.clone(),
                    image,
                    memory,
                }),
                Err(e) => {
                    self.device.device.destroy_image(image, None);
                    Err(e)
                }
            }
        }
    }

    fn allocate(&self, width: u32, height: u32, fourcc: u32, format: vk::Format) -> Result<Frame> {
        unsafe {
            let mut modifiers = vk::DrmFormatModifierPropertiesListEXT::default();
            let mut props = vk::FormatProperties2::default().push_next(&mut modifiers);
            self.device
                .instance
                .instance
                .get_physical_device_format_properties2(self.device.physical, format, &mut props);
            let mut list = vec![
                vk::DrmFormatModifierPropertiesEXT::default();
                modifiers.drm_format_modifier_count as usize
            ];
            modifiers.p_drm_format_modifier_properties = list.as_mut_ptr();
            let mut props = vk::FormatProperties2::default().push_next(&mut modifiers);
            self.device
                .instance
                .instance
                .get_physical_device_format_properties2(self.device.physical, format, &mut props);
            let required =
                vk::FormatFeatureFlags::SAMPLED_IMAGE | vk::FormatFeatureFlags::TRANSFER_DST;
            list.sort_by_key(|m| m.drm_format_modifier != 0);
            let supported: Vec<_> = list
                .iter()
                .filter(|m| {
                    m.drm_format_modifier_plane_count == 1
                        && m.drm_format_modifier_tiling_features.contains(required)
                })
                .map(|m| m.drm_format_modifier)
                .collect();
            ensure!(!supported.is_empty(), "no sampled DMA-BUF export modifier");
            let mut modifiers = vk::ImageDrmFormatModifierListCreateInfoEXT::default()
                .drm_format_modifiers(&supported);
            let mut external = vk::ExternalMemoryImageCreateInfo::default().handle_types(DMA);
            let create = image_info(
                format,
                width,
                height,
                vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            )
            .push_next(&mut modifiers)
            .push_next(&mut external);
            let image = self.device.device.create_image(&create, None)?;
            let result = (|| -> Result<_> {
                let requirements = self.device.device.get_image_memory_requirements(image);
                let mut export = vk::ExportMemoryAllocateInfo::default().handle_types(DMA);
                let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
                let allocation = vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(self.memory_index(requirements.memory_type_bits)?)
                    .push_next(&mut export)
                    .push_next(&mut dedicated);
                self.device
                    .device
                    .allocate_memory(&allocation, None)
                    .map_err(Into::into)
            })();
            let memory = match result {
                Ok(m) => m,
                Err(e) => {
                    self.device.device.destroy_image(image, None);
                    return Err(e);
                }
            };
            let owned = Image {
                device: self.device.clone(),
                image,
                memory,
            };
            self.device.device.bind_image_memory(image, memory, 0)?;
            let loader = ash::khr::external_memory_fd::Device::new(
                &self.device.instance.instance,
                &self.device.device,
            );
            let fd = OwnedFd::from_raw_fd(
                loader.get_memory_fd(
                    &vk::MemoryGetFdInfoKHR::default()
                        .memory(memory)
                        .handle_type(DMA),
                )?,
            );
            let modifiers = ash::ext::image_drm_format_modifier::Device::new(
                &self.device.instance.instance,
                &self.device.device,
            );
            let mut properties = vk::ImageDrmFormatModifierPropertiesEXT::default();
            modifiers.get_image_drm_format_modifier_properties(image, &mut properties)?;
            let layout = self.device.device.get_image_subresource_layout(
                image,
                vk::ImageSubresource::default()
                    .aspect_mask(vk::ImageAspectFlags::MEMORY_PLANE_0_EXT),
            );
            Ok(Frame {
                allocation: owned,
                initialized: std::sync::atomic::AtomicBool::new(false),
                fd,
                width,
                height,
                fourcc,
                modifier: properties.drm_format_modifier,
                stride: u32::try_from(layout.row_pitch)?,
                offset: u32::try_from(layout.offset)?,
            })
        }
    }

    fn copy_image(&self, source: &Image, target: &Frame, width: u32, height: u32) -> Result<()> {
        unsafe {
            let device = &self.device.device;
            let command = device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.device.pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )?[0];
            let result = (|| -> Result<()> {
                device.begin_command_buffer(
                    command,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )?;
                let range = vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1);
                let barriers = [
                    vk::ImageMemoryBarrier::default()
                        .image(source.image)
                        .subresource_range(range)
                        .old_layout(vk::ImageLayout::GENERAL)
                        .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                        .dst_queue_family_index(self.device.family)
                        .dst_access_mask(vk::AccessFlags::TRANSFER_READ),
                    vk::ImageMemoryBarrier::default()
                        .image(target.allocation.image)
                        .subresource_range(range)
                        .old_layout(
                            if target
                                .initialized
                                .load(std::sync::atomic::Ordering::Acquire)
                            {
                                vk::ImageLayout::GENERAL
                            } else {
                                vk::ImageLayout::UNDEFINED
                            },
                        )
                        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .src_queue_family_index(
                            if target
                                .initialized
                                .load(std::sync::atomic::Ordering::Acquire)
                            {
                                vk::QUEUE_FAMILY_FOREIGN_EXT
                            } else {
                                vk::QUEUE_FAMILY_IGNORED
                            },
                        )
                        .dst_queue_family_index(
                            if target
                                .initialized
                                .load(std::sync::atomic::Ordering::Acquire)
                            {
                                self.device.family
                            } else {
                                vk::QUEUE_FAMILY_IGNORED
                            },
                        )
                        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE),
                ];
                device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &barriers,
                );
                let layers = vk::ImageSubresourceLayers::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1);
                device.cmd_copy_image(
                    command,
                    source.image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    target.allocation.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[vk::ImageCopy::default()
                        .src_subresource(layers)
                        .dst_subresource(layers)
                        .extent(vk::Extent3D {
                            width,
                            height,
                            depth: 1,
                        })],
                );
                let released = [
                    vk::ImageMemoryBarrier::default()
                        .image(source.image)
                        .subresource_range(range)
                        .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                        .new_layout(vk::ImageLayout::GENERAL)
                        .src_queue_family_index(self.device.family)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                        .src_access_mask(vk::AccessFlags::TRANSFER_READ),
                    vk::ImageMemoryBarrier::default()
                        .image(target.allocation.image)
                        .subresource_range(range)
                        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .new_layout(vk::ImageLayout::GENERAL)
                        .src_queue_family_index(self.device.family)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE),
                ];
                device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &released,
                );
                device.end_command_buffer(command)?;
                let fence = device.create_fence(&vk::FenceCreateInfo::default(), None)?;
                let submission = device.queue_submit(
                    self.device.queue,
                    &[vk::SubmitInfo::default().command_buffers(&[command])],
                    fence,
                );
                let waited =
                    submission.and_then(|_| device.wait_for_fences(&[fence], true, u64::MAX));
                device.destroy_fence(fence, None);
                waited?;
                Ok(())
            })();
            device.free_command_buffers(self.device.pool, &[command]);
            result
        }
    }
}

/// CEF exposes DMA-BUFs without an explicit sync-file. Honor their reservation
/// fences before submitting work to Vulkan (which does not add implicit waits).
/// GTK has released our texture before reuse; also wait for any exported readers.
pub(crate) fn wait_dmabuf(fd: std::os::fd::RawFd, write: bool) -> Result<()> {
    ensure!(fd >= 0, "invalid DMA-BUF descriptor");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let mut poll = libc::pollfd {
        fd,
        events: if write { libc::POLLOUT } else { libc::POLLIN },
        revents: 0,
    };
    loop {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        ensure!(!timeout.is_zero(), "DMA-BUF synchronization timed out");
        let result = unsafe { libc::poll(&mut poll, 1, timeout.as_millis().max(1) as i32) };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        ensure!(result > 0, "DMA-BUF synchronization timed out");
        ensure!(
            poll.revents & poll.events != 0,
            "DMA-BUF synchronization failed"
        );
        return Ok(());
    }
}

fn image_info(
    format: vk::Format,
    width: u32,
    height: u32,
    usage: vk::ImageUsageFlags,
) -> vk::ImageCreateInfo<'static> {
    vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
}

#[cfg(test)]
impl Copier {
    fn clear_fixture(&self, frame: &Frame, color: [f32; 4]) -> Result<()> {
        unsafe {
            let device = &self.device.device;
            let command = device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.device.pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )?[0];
            device.begin_command_buffer(command, &vk::CommandBufferBeginInfo::default())?;
            let initialized = frame.initialized.load(std::sync::atomic::Ordering::Acquire);
            let range = vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .level_count(1)
                .layer_count(1);
            let image = frame.allocation.image;
            let acquire = vk::ImageMemoryBarrier::default()
                .image(image)
                .subresource_range(range)
                .old_layout(if initialized {
                    vk::ImageLayout::GENERAL
                } else {
                    vk::ImageLayout::UNDEFINED
                })
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_queue_family_index(if initialized {
                    vk::QUEUE_FAMILY_FOREIGN_EXT
                } else {
                    vk::QUEUE_FAMILY_IGNORED
                })
                .dst_queue_family_index(if initialized {
                    self.device.family
                } else {
                    vk::QUEUE_FAMILY_IGNORED
                })
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
            device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[acquire],
            );
            device.cmd_clear_color_image(
                command,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &vk::ClearColorValue { float32: color },
                &[range],
            );
            let release = vk::ImageMemoryBarrier::default()
                .image(image)
                .subresource_range(range)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::GENERAL)
                .src_queue_family_index(self.device.family)
                .dst_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE);
            device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[release],
            );
            device.end_command_buffer(command)?;
            let fence = device.create_fence(&vk::FenceCreateInfo::default(), None)?;
            device.queue_submit(
                self.device.queue,
                &[vk::SubmitInfo::default().command_buffers(&[command])],
                fence,
            )?;
            device.wait_for_fences(&[fence], true, u64::MAX)?;
            device.destroy_fence(fence, None);
            device.free_command_buffers(self.device.pool, &[command]);
            frame
                .initialized
                .store(true, std::sync::atomic::Ordering::Release);
        }
        Ok(())
    }
}

/// Called from the serialized real-widget test so GTK stays on its owning thread.
#[cfg(test)]
pub fn verify_hardware_ownership(display: &gtk::gdk::Display) {
    let producer = match Copier::new() {
        Ok(copier) => copier,
        Err(error) => {
            eprintln!("SKIP Vulkan ownership fixture: {error:#}");
            return;
        }
    };
    let source = producer
        .allocate(32, 32, 0x34325241, vk::Format::B8G8R8A8_UNORM)
        .unwrap();
    producer
        .clear_fixture(&source, [1.0, 0.0, 0.0, 1.0])
        .unwrap();
    let mut info = cef::AcceleratedPaintInfo {
        plane_count: 1,
        format: cef::ColorType::BGRA_8888,
        modifier: source.modifier,
        ..Default::default()
    };
    info.extra.coded_size.width = 32;
    info.extra.coded_size.height = 32;
    info.planes[0].fd = source.fd.as_raw_fd();
    info.planes[0].offset = u64::from(source.offset);
    info.planes[0].stride = source.stride;
    let mut consumer = Copier::new().unwrap();
    let first = consumer.copy(&info).unwrap().unwrap();
    let texture = crate::presenter::texture_from_vulkan(display, first.clone()).unwrap();
    // Simulate immediate producer reuse after the callback; its copied image must
    // remain red even after the producer overwrites the original allocation blue.
    producer
        .clear_fixture(&source, [0.0, 0.0, 1.0, 1.0])
        .unwrap();
    let mut downloader = gtk::gdk::TextureDownloader::new(&texture);
    downloader.set_format(gtk::gdk::MemoryFormat::B8g8r8a8Premultiplied);
    assert_eq!(&downloader.download_bytes().0[..4], &[0, 0, 255, 255]);
    let second = consumer.copy(&info).unwrap().unwrap();
    let third = consumer.copy(&info).unwrap().unwrap();
    assert!(
        consumer.copy(&info).unwrap().is_none(),
        "three live buffers must bound allocation"
    );
    drop(second);
    let reused = consumer.copy(&info).unwrap().unwrap();
    let blue = crate::presenter::texture_from_vulkan(display, reused).unwrap();
    downloader.set_texture(&blue);
    assert_eq!(&downloader.download_bytes().0[..4], &[255, 0, 0, 255]);

    let resized_source = producer
        .allocate(48, 24, 0x34325241, vk::Format::B8G8R8A8_UNORM)
        .unwrap();
    producer
        .clear_fixture(&resized_source, [0.0, 1.0, 0.0, 1.0])
        .unwrap();
    info.extra.coded_size.width = 48;
    info.extra.coded_size.height = 24;
    info.modifier = resized_source.modifier;
    info.planes[0].fd = resized_source.fd.as_raw_fd();
    info.planes[0].offset = u64::from(resized_source.offset);
    info.planes[0].stride = resized_source.stride;
    assert!(
        consumer.copy(&info).unwrap().is_none(),
        "resize must honor retained old-size buffers"
    );
    drop(third);
    let resized = consumer.copy(&info).unwrap().unwrap();
    assert_eq!((resized.width, resized.height), (48, 24));
    assert_eq!(consumer.frames.len(), 3);
    let resized_texture = crate::presenter::texture_from_vulkan(display, resized).unwrap();
    downloader.set_texture(&resized_texture);
    assert_eq!(&downloader.download_bytes().0[..4], &[0, 255, 0, 255]);
    downloader.set_texture(&texture);
    assert_eq!(&downloader.download_bytes().0[..4], &[0, 0, 255, 255]);
    eprintln!(
        "PASS Vulkan owned-copy, producer-reuse, GTK import, resize and bounded-buffer fixture"
    );
}
