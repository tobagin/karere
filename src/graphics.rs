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
    record_display()
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
    !matches!(
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
