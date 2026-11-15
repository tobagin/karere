//! Live reload of the user's GTK4 stylesheet.
//!
//! GTK loads `$XDG_CONFIG_HOME/gtk-4.0/gtk.css` once at startup into an
//! internal provider at `STYLE_PROVIDER_PRIORITY_USER`. That provider is not
//! exposed to applications, so it never sees later edits — Noctalia-style
//! theme switches that rewrite `gtk.css` (or the files it `@import`s) leave
//! the window chrome stale until restart.
//!
//! This module works around that by keeping a *reloadable copy* of the user
//! stylesheet in a provider registered one notch above the built-in user
//! priority. A single GIO directory monitor watches the config directory and,
//! after a short debounce, re-loads `gtk.css` from disk — which also re-runs
//! its `@import` rules, so imported sheets like `noctalia.css` are honored.
//!
//! Known limitation: the built-in user provider cannot be removed or rewritten
//! by applications. While `gtk.css` exists our copy shadows it (same content),
//! but if the file is deleted the built-in provider keeps whatever it parsed
//! at startup, so stale user rules can persist until Karere restarts.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk::{gio, glib, prelude::*};

/// Debounce interval for filesystem notifications.
const DEBOUNCE: Duration = Duration::from_millis(150);

pub(crate) struct UserCss {
    provider: gtk::CssProvider,
    display: gtk::gdk::Display,
    file: gio::File,
    monitor: gio::FileMonitor,
    pending: RefCell<Option<glib::SourceId>>,
    #[cfg(test)]
    reload_count: std::cell::Cell<u64>,
}

/// True when `event` describes a change that can affect the user stylesheet:
/// any CSS file in the monitored directory, including the two names GIO
/// reports for a move/rename (`file` and `other`), so atomic replacements are
/// caught no matter which side is the CSS file.
fn relevant_event(
    file: &gio::File,
    other: Option<&gio::File>,
    event: gio::FileMonitorEvent,
) -> bool {
    matches!(
        event,
        gio::FileMonitorEvent::Changed
            | gio::FileMonitorEvent::ChangesDoneHint
            | gio::FileMonitorEvent::Created
            | gio::FileMonitorEvent::Deleted
            | gio::FileMonitorEvent::AttributeChanged
            | gio::FileMonitorEvent::Moved
            | gio::FileMonitorEvent::Renamed
            | gio::FileMonitorEvent::MovedIn
            | gio::FileMonitorEvent::MovedOut
    ) && std::iter::once(file).chain(other).any(|file| {
        file.basename()
            .is_some_and(|name| Path::new(&name).extension().is_some_and(|ext| ext == "css"))
    })
}

impl UserCss {
    /// Start monitoring the user GTK config directory on the default display.
    /// Returns `None` when there is no display or the directory cannot be
    /// monitored; the app degrades to GTK's startup-only behavior.
    pub(crate) fn start() -> Option<Rc<Self>> {
        let display = gtk::gdk::Display::default()?;
        let directory = glib::user_config_dir().join("gtk-4.0");
        Self::start_in(display, &directory)
    }

    fn start_in(display: gtk::gdk::Display, directory: &Path) -> Option<Rc<Self>> {
        if let Err(error) = std::fs::create_dir_all(directory) {
            log::warn!(
                "cannot create GTK CSS directory {}: {error}",
                directory.display()
            );
            return None;
        }
        let monitor = match gio::File::for_path(directory)
            .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        {
            Ok(monitor) => monitor,
            Err(error) => {
                log::warn!(
                    "cannot monitor GTK CSS directory {}: {error}",
                    directory.display()
                );
                return None;
            }
        };
        let provider = gtk::CssProvider::new();
        provider.connect_parsing_error(|_, _, error| {
            log::warn!("user GTK CSS: {error}");
        });
        // One notch above GTK's own (immutable) user provider, so our
        // reloadable copy wins while the file exists.
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER + 1,
        );
        let state = Rc::new(Self {
            provider,
            display,
            file: gio::File::for_path(directory.join("gtk.css")),
            monitor,
            pending: RefCell::new(None),
            #[cfg(test)]
            reload_count: std::cell::Cell::new(0),
        });
        let weak = Rc::downgrade(&state);
        state.monitor.connect_changed(move |_, file, other, event| {
            if !relevant_event(file, other, event) {
                return;
            }
            let Some(state) = weak.upgrade() else { return };
            // Debounce: cancel any pending reload and schedule a fresh one.
            if let Some(source) = state.pending.borrow_mut().take() {
                source.remove();
            }
            let weak = Rc::downgrade(&state);
            let source = glib::timeout_add_local_once(DEBOUNCE, move || {
                if let Some(state) = weak.upgrade() {
                    state.pending.borrow_mut().take();
                    state.reload();
                }
            });
            *state.pending.borrow_mut() = Some(source);
        });
        state.reload();
        Some(state)
    }

    fn reload(&self) {
        #[cfg(test)]
        self.reload_count.set(self.reload_count.get() + 1);
        if self.file.query_exists(gio::Cancellable::NONE) {
            // Loading via the file preserves the base URI, so relative
            // `@import`s resolve against the gtk.css directory.
            self.provider.load_from_file(&self.file);
        } else {
            self.provider.load_from_string("");
        }
        log::debug!("reloaded user GTK stylesheet");
    }

    #[cfg(test)]
    fn reload_count(&self) -> u64 {
        self.reload_count.get()
    }
}

impl Drop for UserCss {
    fn drop(&mut self) {
        self.monitor.cancel();
        if let Some(source) = self.pending.get_mut().take() {
            source.remove();
        }
        gtk::style_context_remove_provider_for_display(&self.display, &self.provider);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The GTK-backed tests use `#[gtk::test]` (they must run on the single
    /// thread that initializes GTK) and are `#[ignore]`d because they need a
    /// display. Run them under Xvfb like the other widget tests, e.g.:
    /// `xvfb-run -a cargo test --bin karere -- --ignored gtk_css::tests`
    /// (or with `GDK_BACKEND=headless` where that backend is compiled in).
    ///
    /// Unique, self-cleaning directory underneath `std::env::temp_dir()`.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("karere-gtk-css-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    impl std::ops::Deref for TempDir {
        type Target = std::path::Path;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[test]
    fn watches_entrypoint_and_imported_stylesheets() {
        for name in ["gtk.css", "noctalia.css", "noctalia-extensions.css"] {
            let file = gio::File::for_path(format!("/config/gtk-4.0/{name}"));
            for event in [
                gio::FileMonitorEvent::Changed,
                gio::FileMonitorEvent::ChangesDoneHint,
                gio::FileMonitorEvent::Created,
                gio::FileMonitorEvent::Deleted,
                gio::FileMonitorEvent::AttributeChanged,
                gio::FileMonitorEvent::Moved,
                gio::FileMonitorEvent::Renamed,
                gio::FileMonitorEvent::MovedIn,
                gio::FileMonitorEvent::MovedOut,
            ] {
                assert!(relevant_event(&file, None, event), "{name}: {event:?}");
            }
        }
    }

    #[test]
    fn ignores_unrelated_files_and_events() {
        for name in [
            "gtk.css.bak",
            "gtk.css~",
            "settings.ini",
            "noctalia.toml",
            ".replacement",
            "README.txt",
        ] {
            let file = gio::File::for_path(format!("/config/gtk-4.0/{name}"));
            for event in [
                gio::FileMonitorEvent::Changed,
                gio::FileMonitorEvent::Created,
                gio::FileMonitorEvent::Deleted,
                gio::FileMonitorEvent::Renamed,
            ] {
                assert!(!relevant_event(&file, None, event), "{name}: {event:?}");
            }
        }
        let css = gio::File::for_path("/config/gtk-4.0/gtk.css");
        assert!(!relevant_event(
            &css,
            None,
            gio::FileMonitorEvent::Unmounted
        ));
    }

    #[test]
    fn atomic_replacement_reports_both_paths() {
        let temp = gio::File::for_path("/config/gtk-4.0/.tmp123");
        let css = gio::File::for_path("/config/gtk-4.0/noctalia.css");
        assert!(relevant_event(
            &temp,
            Some(&css),
            gio::FileMonitorEvent::Renamed
        ));
        assert!(relevant_event(
            &css,
            Some(&temp),
            gio::FileMonitorEvent::Renamed
        ));
        assert!(relevant_event(
            &temp,
            Some(&css),
            gio::FileMonitorEvent::MovedIn
        ));
        assert!(!relevant_event(&temp, None, gio::FileMonitorEvent::Changed));
        assert!(!relevant_event(
            &css,
            None,
            gio::FileMonitorEvent::Unmounted
        ));
    }

    #[ignore = "requires a GTK display or the headless backend (GDK_BACKEND=headless)"]
    #[gtk::test]
    fn reloads_after_writes_imports_and_atomic_replacement() {
        let dir = TempDir::new("reload");
        let entrypoint = dir.join("gtk.css");
        let imported = dir.join("noctalia.css");
        std::fs::write(&entrypoint, "@import url(\"noctalia.css\");").unwrap();
        std::fs::write(&imported, "@define-color karere_live_test #ff0000;").unwrap();

        let label = gtk::Label::new(None);
        let color = || {
            #[allow(deprecated)]
            label.style_context().lookup_color("karere_live_test")
        };
        let wait_for = |expected: Option<gtk::gdk::RGBA>| {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while color() != expected {
                assert!(
                    std::time::Instant::now() < deadline,
                    "GTK CSS did not reload (color={:?}, expected={expected:?})",
                    color()
                );
                while glib::MainContext::default().iteration(false) {}
                std::thread::sleep(Duration::from_millis(10));
            }
        };

        let state = UserCss::start_in(gtk::gdk::Display::default().unwrap(), &dir).unwrap();
        wait_for(Some(gtk::gdk::RGBA::RED));

        // Change an imported stylesheet: the entrypoint must be reloaded so
        // the @import is re-evaluated.
        std::fs::write(&imported, "@define-color karere_live_test #0000ff;").unwrap();
        wait_for(Some(gtk::gdk::RGBA::BLUE));

        // Atomic replacement of the imported file (rename over it).
        let replacement = dir.join(".noctalia.replacement");
        std::fs::write(&replacement, "@define-color karere_live_test #ff0000;").unwrap();
        std::fs::rename(&replacement, &imported).unwrap();
        wait_for(Some(gtk::gdk::RGBA::RED));

        // Deletion of the entrypoint clears the reloadable provider.
        std::fs::remove_file(&entrypoint).unwrap();
        wait_for(None);

        // Recreating it restores the styles.
        std::fs::write(&entrypoint, "@import url(\"noctalia.css\");").unwrap();
        wait_for(Some(gtk::gdk::RGBA::RED));

        drop(state);
    }

    #[ignore = "requires a GTK display or the headless backend (GDK_BACKEND=headless)"]
    #[gtk::test]
    fn debounces_rapid_change_notifications() {
        let dir = TempDir::new("debounce");
        let entrypoint = dir.join("gtk.css");
        std::fs::write(&entrypoint, "/* start */").unwrap();
        let state = UserCss::start_in(gtk::gdk::Display::default().unwrap(), &dir).unwrap();
        let baseline = state.reload_count();

        // Burst many writes inside the debounce window; they must collapse into
        // a single reload instead of one per notification.
        for i in 0..12 {
            std::fs::write(&entrypoint, format!("/* burst {i} */")).unwrap();
            std::thread::sleep(Duration::from_millis(8));
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while state.reload_count() <= baseline {
            assert!(
                std::time::Instant::now() < deadline,
                "debounced reload never happened"
            );
            while glib::MainContext::default().iteration(false) {}
            std::thread::sleep(Duration::from_millis(10));
        }
        // Let any stragglers settle, then assert the burst collapsed.
        std::thread::sleep(Duration::from_millis(300));
        while glib::MainContext::default().iteration(false) {}
        let reloads = state.reload_count() - baseline;
        assert_eq!(
            reloads, 1,
            "expected a single reload for the burst, got {reloads}"
        );
        drop(state);
    }

    #[ignore = "requires a GTK display or the headless backend (GDK_BACKEND=headless)"]
    #[gtk::test]
    fn cleanup_on_drop_cancels_pending_and_monitoring() {
        let dir = TempDir::new("cleanup");
        let entrypoint = dir.join("gtk.css");
        std::fs::write(&entrypoint, "/* start */").unwrap();
        let state = UserCss::start_in(gtk::gdk::Display::default().unwrap(), &dir).unwrap();

        // Trigger a pending (debounced) reload, then drop before it fires:
        // neither the cancelled timer nor the monitor may panic afterwards.
        std::fs::write(&entrypoint, "/* changed */").unwrap();
        drop(state);
        std::thread::sleep(Duration::from_millis(400));
        while glib::MainContext::default().iteration(false) {}

        // Changes after drop must be inert and must not crash.
        std::fs::write(&entrypoint, "/* after drop */").unwrap();
        std::thread::sleep(Duration::from_millis(400));
        while glib::MainContext::default().iteration(false) {}
    }
}
