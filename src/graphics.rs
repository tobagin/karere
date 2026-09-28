//! Independent display, CEF and presentation policy. No saved preferences are changed.
use cef::{CommandLine, ImplCommandLine};
use gtk::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static DISPLAY: AtomicU8 = AtomicU8::new(0);
static RESTART: AtomicBool = AtomicBool::new(false);
static CEF_VULKAN_AVAILABLE: AtomicBool = AtomicBool::new(true);

pub fn startup_environment() {
    // Called before GTK/CEF create threads. Explicit recovery overrides win.
    unsafe {
        if std::env::var_os("GDK_BACKEND").is_none() {
            std::env::set_var("GDK_BACKEND", "wayland,x11");
        }
        // GSK tries this renderer before its backend default, then falls back
        // normally if realization fails. The X11 default otherwise prefers GL
        // even when hardware Vulkan works. Respect an explicit recovery choice.
        if std::env::var_os("GSK_RENDERER").is_none() {
            std::env::set_var("GSK_RENDERER", "vulkan");
        }
    }
}

pub fn record_display() -> anyhow::Result<()> {
    let display = gtk::gdk::Display::default()
        .ok_or_else(|| anyhow::anyhow!("No GTK display. For X11 recovery launch with --nosocket=wayland --nosocket=fallback-x11 --socket=x11 --env=GDK_BACKEND=x11."))?;
    let x11 = display.is::<gdk4_x11::X11Display>();
    DISPLAY.store(if x11 { 2 } else { 1 }, Ordering::Release);
    log::info!(
        "graphics: GTK display={} CEF ozone={}",
        display.type_().name(),
        display_backend()
    );
    Ok(())
}

pub fn initialize_display() -> anyhow::Result<()> {
    gtk::init().map_err(|error| anyhow::anyhow!(
        "GTK could not open Wayland or X11: {error}. One-launch X11 recovery: flatpak run --nosocket=wayland --nosocket=fallback-x11 --socket=x11 --env=GDK_BACKEND=x11 {}",
        crate::application::APP_ID
    ))?;
    record_display()?;
    if cef_vulkan()
        && let Err(error) = check_hardware_vulkan()
    {
        // Chromium can silently deliver software frames after ANGLE Vulkan
        // fails. A missing-frame watchdog cannot detect that case. Check basic
        // hardware initialization before requesting ANGLE, so GL gets a chance.
        // This is a capability check, not proof of ANGLE's eventual backend.
        CEF_VULKAN_AVAILABLE.store(false, Ordering::Release);
        log::warn!(
            "graphics: CEF hardware Vulkan initialization unavailable: {error:#}; trying ANGLE GL"
        );
    }
    Ok(())
}

fn check_hardware_vulkan() -> anyhow::Result<()> {
    use ash::vk;
    use std::ffi::CStr;

    // No resources are submitted. Destroy each trial device before its instance
    // and keep the dynamically loaded entry alive throughout the whole probe.
    unsafe {
        let entry = ash::Entry::load()?;
        let instance = entry.create_instance(
            &vk::InstanceCreateInfo::default()
                .application_info(&vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_1)),
            None,
        )?;
        let result = (|| -> anyhow::Result<()> {
            for physical in instance.enumerate_physical_devices()? {
                let properties = instance.get_physical_device_properties(physical);
                if properties.device_type == vk::PhysicalDeviceType::CPU {
                    continue;
                }
                let queues = instance.get_physical_device_queue_family_properties(physical);
                let Some(family) = queues.iter().position(|queue| {
                    queue.queue_count > 0 && queue.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                }) else {
                    continue;
                };
                let priority = [1.0];
                let queues = [vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family as u32)
                    .queue_priorities(&priority)];
                if let Ok(device) = instance.create_device(
                    physical,
                    &vk::DeviceCreateInfo::default().queue_create_infos(&queues),
                    None,
                ) {
                    device.destroy_device(None);
                    log::info!(
                        "graphics: Vulkan capability probe initialized hardware device={}",
                        CStr::from_ptr(properties.device_name.as_ptr()).to_string_lossy()
                    );
                    return Ok(());
                }
            }
            anyhow::bail!("no hardware Vulkan graphics device initialized")
        })();
        instance.destroy_instance(None);
        result
    }
}

pub fn display_backend() -> &'static str {
    match DISPLAY.load(Ordering::Acquire) {
        1 => "wayland",
        2 => "x11",
        _ if std::env::var("GDK_BACKEND").is_ok_and(|v| v == "x11") => "x11",
        _ if std::env::var_os("WAYLAND_DISPLAY").is_some() => "wayland",
        _ => "x11",
    }
}

pub fn cef_vulkan() -> bool {
    CEF_VULKAN_AVAILABLE.load(Ordering::Acquire)
        && !matches!(
            std::env::var("KARERE_CEF_GRAPHICS").as_deref(),
            Ok("gl") | Ok("software")
        )
}

pub fn configure_cef(cmd: &mut CommandLine) {
    if std::env::var("KARERE_CEF_GRAPHICS").as_deref() == Ok("software") {
        cmd.append_switch(Some(&"disable-gpu".into()));
        log::warn!("graphics: CEF software recovery selected");
        return;
    }
    // Child processes already inherit the primary's switches, including Ozone.
    if cmd.has_switch(Some(&"use-angle".into())) == 1 {
        return;
    }
    cmd.append_switch_with_value(Some(&"use-gl".into()), Some(&"angle".into()));
    let angle = if cef_vulkan() { "vulkan" } else { "gl" };
    cmd.append_switch_with_value(Some(&"use-angle".into()), Some(&angle.into()));
    log::info!("graphics: requesting CEF ANGLE {angle}; actual delivery must be verified");
}

/// Only a verified startup frame failure requests a retry; never downgrade merely
/// because the window is hidden, idle, or below its frame-rate target.
pub fn retry_cef_after_frame_failure() {
    let stage = std::env::var("KARERE_CEF_GRAPHICS").unwrap_or_default();
    if stage == "software" || RESTART.swap(true, Ordering::AcqRel) {
        return;
    }
    log::error!(
        "graphics: visible CEF browser delivered no initial frame; retrying {}",
        if cef_vulkan() { "GL" } else { "software" }
    );
    if let Some(app) = gtk::gio::Application::default() {
        app.quit();
    }
}

pub fn restart_if_requested() -> anyhow::Result<()> {
    if RESTART.load(Ordering::Acquire) {
        use std::os::unix::process::CommandExt;
        // Replace this process after CEF shutdown, releasing the old D-Bus
        // connection before registration. Spawning races the old primary.
        let error = std::process::Command::new(std::env::current_exe()?)
            .args(std::env::args_os().skip(1))
            .env(
                "KARERE_CEF_GRAPHICS",
                if cef_vulkan() { "gl" } else { "software" },
            )
            .exec();
        return Err(error.into());
    }
    Ok(())
}
