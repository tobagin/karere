//! Directory-based user stylesheet live reload. All callbacks run on the GTK
//! main context, which also drives CEF's UI thread.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use super::imp::KarereWebView;
use super::page_settings::{apply_user_css_script, read_user_css_script, user_css_path};

const DEBOUNCE_DELAY: Duration = Duration::from_millis(100);

/// Both ends of a rename matter: theme generators commonly rename a temporary
/// file onto user.css. Compare full paths so unrelated files never trigger it.
fn relevant_event(
    target: &gio::File,
    file: &gio::File,
    other_file: Option<&gio::File>,
    event: gio::FileMonitorEvent,
) -> bool {
    use gio::FileMonitorEvent::*;
    matches!(
        event,
        Changed
            | ChangesDoneHint
            | Created
            | Deleted
            | AttributeChanged
            | Moved
            | Renamed
            | MovedIn
            | MovedOut
    ) && std::iter::once(file)
        .chain(other_file)
        .any(|f| f.equal(target))
}

/// Owns both the directory watch and its trailing-edge debounce. Dropping it
/// cancels queued work even if the view is disposed before the timer fires.
pub(super) struct UserCssMonitor {
    monitor: gio::FileMonitor,
    pending: Rc<RefCell<Option<glib::SourceId>>>,
}

impl UserCssMonitor {
    fn new(path: &Path, on_change: impl Fn() + 'static) -> anyhow::Result<Self> {
        let directory = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("user.css has no parent directory"))?;
        // Watch even when user.css has never existed, including on first start.
        std::fs::create_dir_all(directory)?;
        let target = gio::File::for_path(path);
        let monitor = gio::File::for_path(directory)
            .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)?;
        let pending = Rc::new(RefCell::new(None::<glib::SourceId>));
        let on_change = Rc::new(on_change);
        monitor.connect_changed({
            let pending = pending.clone();
            move |_, file, other_file, event| {
                if !relevant_event(&target, file, other_file, event) {
                    return;
                }
                if let Some(source) = pending.borrow_mut().take() {
                    source.remove();
                }
                let pending_callback = pending.clone();
                let on_change = on_change.clone();
                let source = glib::timeout_add_local_once(DEBOUNCE_DELAY, move || {
                    // The source has fired; its ID must not be removed again.
                    pending_callback.borrow_mut().take();
                    on_change();
                });
                *pending.borrow_mut() = Some(source);
            }
        });
        Ok(Self { monitor, pending })
    }
}

impl Drop for UserCssMonitor {
    fn drop(&mut self) {
        self.monitor.cancel();
        if let Some(source) = self.pending.borrow_mut().take() {
            source.remove();
        }
    }
}

impl KarereWebView {
    /// Start on the account spawn path (also used by background prewarming),
    /// not construction: new_devtools marks its view only after construction.
    pub(super) fn ensure_user_css_monitor(&self) {
        if self.devtools.load(std::sync::atomic::Ordering::Relaxed)
            || self.user_css_monitor.borrow().is_some()
        {
            return;
        }
        let weak = self.obj().downgrade();
        let path = user_css_path();
        match UserCssMonitor::new(&path, move || {
            if let Some(view) = weak.upgrade() {
                view.imp().reload_user_css();
            }
        }) {
            Ok(monitor) => *self.user_css_monitor.borrow_mut() = Some(monitor),
            Err(error) => log::warn!(
                "Could not monitor user stylesheet {}: {error}",
                path.display()
            ),
        }
    }

    fn reload_user_css(&self) {
        let Some(script) = read_user_css_script(&user_css_path()) else {
            return;
        };
        // The foreground handle is already in this pool. Do not add it again,
        // or fall back to the legacy single browser (the DevTools frontend).
        let browsers: Vec<_> = self.browsers.lock().values().cloned().collect();
        // Never execute JavaScript while holding the browser-pool lock.
        for browser in browsers {
            apply_user_css_script(&browser, &script);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::time::Instant;

    #[test]
    fn filters_events_and_both_sides_of_atomic_replacements() {
        use gio::FileMonitorEvent::*;
        let target = gio::File::for_path("/config/karere/user.css");
        let temporary = gio::File::for_path("/config/karere/user.css.tmp");
        let unrelated = gio::File::for_path("/elsewhere/user.css");
        for event in [
            Changed,
            ChangesDoneHint,
            Created,
            Deleted,
            AttributeChanged,
            Moved,
            Renamed,
            MovedIn,
            MovedOut,
        ] {
            assert!(relevant_event(&target, &target, None, event));
            assert!(!relevant_event(&target, &temporary, None, event));
            assert!(!relevant_event(&target, &unrelated, None, event));
        }
        for event in [Moved, Renamed] {
            assert!(relevant_event(&target, &temporary, Some(&target), event));
            assert!(relevant_event(&target, &target, Some(&temporary), event));
            assert!(!relevant_event(
                &target,
                &temporary,
                Some(&unrelated),
                event
            ));
        }
        for event in [PreUnmount, Unmounted] {
            assert!(!relevant_event(&target, &target, None, event));
        }
    }

    fn dispatch_for(context: &glib::MainContext, duration: Duration) {
        let end = Instant::now() + duration;
        while Instant::now() < end {
            while context.iteration(false) {}
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn directory_watch_debounces_rearms_and_cleans_up() {
        // GTK keeps ownership of the default main context after init, even
        // when its test thread exits. Isolate this GIO-only test from GTK tests.
        const CHILD_ENV: &str = "KARERE_USER_CSS_MONITOR_TEST";
        if std::env::var_os(CHILD_ENV).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "web_view::user_css::tests::directory_watch_debounces_rearms_and_cleans_up",
                    "--nocapture",
                ])
                .env(CHILD_ENV, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let context = glib::MainContext::default();
        let _guard = context.acquire().unwrap();
        let directory =
            std::env::temp_dir().join(format!("karere-css-monitor-{}", uuid::Uuid::new_v4()));
        let path = directory.join("user.css");
        let calls = Rc::new(Cell::new(0));
        let watch = UserCssMonitor::new(&path, {
            let calls = calls.clone();
            move || calls.set(calls.get() + 1)
        })
        .unwrap();
        let target = gio::File::for_path(&path);

        // Synthetic signals make trailing-edge timing deterministic, without
        // depending on which notification sequence the filesystem emits.
        for _ in 0..3 {
            watch.monitor.emit_by_name::<()>(
                "changed",
                &[&target, &None::<gio::File>, &gio::FileMonitorEvent::Changed],
            );
            dispatch_for(&context, Duration::from_millis(40));
            assert_eq!(calls.get(), 0);
        }
        dispatch_for(&context, Duration::from_millis(150));
        assert_eq!(calls.get(), 1);
        assert!(watch.pending.borrow().is_none());

        // Actual GIO notifications: file creation after startup, unrelated
        // files, replacement, deletion, recreation, and repeated writes.
        for (operation, expected) in [
            ("create", 2),
            ("unrelated", 2),
            ("replace", 3),
            ("delete", 4),
            ("recreate", 5),
            ("rewrite", 6),
        ] {
            match operation {
                "delete" => std::fs::remove_file(&path).unwrap(),
                "unrelated" => std::fs::write(directory.join("settings.json"), "{}").unwrap(),
                "replace" => {
                    let temporary = directory.join("theme.tmp");
                    std::fs::write(&temporary, "body { color: blue; }").unwrap();
                    std::fs::rename(temporary, &path).unwrap();
                }
                _ => std::fs::write(&path, "body { color: red; }").unwrap(),
            }
            dispatch_for(&context, Duration::from_millis(350));
            assert_eq!(calls.get(), expected, "{operation}");
        }
        watch.monitor.emit_by_name::<()>(
            "changed",
            &[&target, &None::<gio::File>, &gio::FileMonitorEvent::Changed],
        );
        assert!(watch.pending.borrow().is_some());
        let monitor = watch.monitor.clone();
        drop(watch);
        assert!(monitor.is_cancelled());
        dispatch_for(&context, Duration::from_millis(150));
        assert_eq!(calls.get(), 6, "dropping the owner must cancel the timer");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
