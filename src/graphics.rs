//! Independent display, CEF and presentation policy. No saved preferences are changed.
use cef::{CommandLine, ImplCommandLine};
use gtk::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static DISPLAY: AtomicU8 = AtomicU8::new(0);
static RESTART: AtomicBool = AtomicBool::new(false);

pub fn startup_environment() {
    // Called before GTK/CEF create threads. Explicit recovery overrides win.
    unsafe {
        if std::env::var_os("GDK_BACKEND").is_none() {
            std::env::set_var("GDK_BACKEND", "wayland,x11");
        }
        // Leave GSK's platform selection intact. Performance comparisons can
        // explicitly select GL or Vulkan; neither is a mandatory preference.
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
        log::warn!(
            "graphics: CEF hardware Vulkan initialization unavailable: {error:#}; trying ANGLE GL"
        );
        crate::gpu_recovery::restart_cef("gl");
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
    std::env::var("KARERE_CEF_GRAPHICS").as_deref() == Ok("vulkan")
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
    let angle = if cef_vulkan() { "vulkan" } else { "gl-egl" };
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
        next_cef_backend()
    );
    if let Some(app) = gtk::gio::Application::default() {
        app.quit();
    }
}

pub fn restart_if_requested() -> anyhow::Result<()> {
    if RESTART.load(Ordering::Acquire) {
        // The supervisor waits for exit before restarting, so the old D-Bus
        // connection and CEF children are gone. It retains the tried backends.
        crate::gpu_recovery::restart_cef(next_cef_backend());
    }
    Ok(())
}

fn next_cef_backend() -> &'static str {
    // An explicitly selected Vulkan trial can fall back to GL. The supervisor
    // skips any backend already attempted in this launch, preventing loops.
    if cef_vulkan() { "gl" } else { "vulkan" }
}
