use cef::{self, ImplBrowser, ImplBrowserHost};

use super::imp::KarereWebView;
use super::page_settings::with_app_settings;

/// Min/max linear zoom factor the CEF boundary will apply (clamp guards against
/// pathological levels). (M18)
pub(crate) const ZOOM_MIN: f64 = 0.5;
pub(crate) const ZOOM_MAX: f64 = 3.0;

/// Linear zoom factor (1.0 = 100 %) → CEF's logarithmic `BrowserHost` level
/// (each unit = factor 1.2, 0 = 100 %). Clamped to `[ZOOM_MIN, ZOOM_MAX]`. (M18)
pub(crate) fn linear_to_cef(linear: f64) -> f64 {
    linear.clamp(ZOOM_MIN, ZOOM_MAX).ln() / 1.2_f64.ln()
}

/// Inverse of [`linear_to_cef`]: CEF logarithmic level → linear factor. (M18)
pub(crate) fn cef_to_linear(cef: f64) -> f64 {
    (cef * 1.2_f64.ln()).exp()
}

/// CEF `BrowserHost` zoom level to apply for a user zoom factor at a given
/// integer display scale. Because this CEF build ignores device_scale_factor for
/// OSR, the paint buffer is sized to the physical (logical × scale) view rect for
/// crispness; an equal page zoom then restores content to its logical size. The
/// display-scale term is added in CEF's log domain (each unit = ×1.2) so it is
/// NOT subject to the user-zoom `[ZOOM_MIN, ZOOM_MAX]` clamp. (#158)
pub(crate) fn host_zoom_level(user_linear: f64, display_scale: f64) -> f64 {
    let scale_term = display_scale.max(1.0).ln() / 1.2_f64.ln();
    linear_to_cef(user_linear) + scale_term
}

/// Effective accessibility zoom floor (linear): `zoom-level` setting when the
/// `webview-zoom` toggle is on, else the hard CEF minimum. Shared by the window's
/// zoom actions and the load handler's first-paint apply. (M18 5.x)
pub(crate) fn zoom_floor() -> f64 {
    use gtk::prelude::SettingsExt;
    with_app_settings(|s| {
        if s.boolean("webview-zoom") {
            s.double("zoom-level").clamp(ZOOM_MIN, ZOOM_MAX)
        } else {
            ZOOM_MIN
        }
    })
}

/// Apply the per-account persisted zoom (lifted to the accessibility floor) to
/// `browser`, rewriting the persisted value if the floor lifted it. Called from
/// `on_load_end` so each account restores its zoom on first paint and after
/// navigation. (M18 4.1 / 5.2)
pub(crate) fn apply_zoom_from_account(browser: &cef::Browser, display_scale: f64) {
    use cef::{ImplBrowser, ImplBrowserHost};
    let Some(id) = crate::accounts::account_for_browser(browser.identifier()) else {
        log::trace!(
            "coord: J7 apply_zoom account=none browser_id={} display_scale={:.3} (uncompensated)",
            browser.identifier(),
            display_scale
        );
        return;
    };
    let persisted = crate::accounts::manager()
        .get(&id)
        .map(|a| a.zoom_level)
        .unwrap_or(1.0);
    let effective = persisted.max(zoom_floor());
    let cef_level = host_zoom_level(effective, display_scale);
    log::trace!(
        "coord: J7 apply_zoom account={} user_linear={:.3} display_scale={:.3} cef_level={:.3}",
        id,
        effective,
        display_scale,
        cef_level
    );
    if let Some(host) = browser.host() {
        host.set_zoom_level(cef_level);
    }
    if (effective - persisted).abs() >= f64::EPSILON {
        crate::accounts::manager().set_zoom(&id, effective);
    }
}

impl KarereWebView {
    /// Current integer display scale from the shared state (≥ 1). Folded into
    /// the applied zoom to emulate the HiDPI paint buffer (#158).
    pub(super) fn display_scale(&self) -> f64 {
        self.shared
            .lock()
            .as_ref()
            .map(|s| s.lock().scale_factor as f64)
            .unwrap_or(1.0)
            .max(1.0)
    }

    /// Set the foreground browser's zoom from a linear factor. The display
    /// scale is added on top (page-zoom HiDPI emulation, #158). CEF UI thread
    /// only. (M18)
    pub fn set_zoom_linear(&self, linear: f64) {
        let display_scale = self.display_scale();
        let cef_level = super::host_zoom_level(linear, display_scale);
        if let Some(browser) = self.browser.lock().as_ref().cloned() {
            let acct = crate::accounts::account_for_browser(browser.identifier())
                .unwrap_or_else(|| "none".to_string());
            log::trace!(
                "coord: J7 set_zoom_linear account={} user_linear={:.3} display_scale={:.3} cef_level={:.3}",
                acct,
                linear,
                display_scale,
                cef_level
            );
            if let Some(host) = browser.host() {
                host.set_zoom_level(cef_level);
            }
        } else {
            log::trace!(
                "coord: J7 set_zoom_linear account=none user_linear={:.3} display_scale={:.3} cef_level={:.3} (no browser)",
                linear,
                display_scale,
                cef_level
            );
        }
    }

    /// Foreground browser's USER zoom as a linear factor (the display-scale
    /// term removed), or 1.0 if none. (M18)
    pub fn get_zoom_linear(&self) -> f64 {
        if let Some(browser) = self.browser.lock().as_ref().cloned()
            && let Some(host) = browser.host()
        {
            let scale_term = self.display_scale().ln() / 1.2_f64.ln();
            super::cef_to_linear(host.zoom_level() - scale_term)
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::{ZOOM_MAX, cef_to_linear, linear_to_cef};

    /// linear → CEF → linear round-trips within 1e-9 across the range.
    #[test]
    fn round_trip_linear() {
        for x in [0.5_f64, 1.0, 1.1, 1.331, 3.0] {
            let back = cef_to_linear(linear_to_cef(x));
            assert!((back - x).abs() < 1e-9, "round-trip {x} -> {back}");
        }
    }

    /// out-of-range input clamps to the max linear factor (3.0).
    #[test]
    fn clamp_above_max() {
        let back = cef_to_linear(linear_to_cef(5.0));
        assert!((back - ZOOM_MAX).abs() < 1e-9, "clamp 5.0 -> {back}");
    }
}
