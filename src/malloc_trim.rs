//! Periodic `malloc_trim(0)` to hand glibc's freed heap pages back to the OS.
//!
//! CEF on Linux is built without Chromium's allocator shim, so everything that
//! calls `malloc` in our processes (Skia, SQLite, image decoders, ICU, GTK, our
//! own Rust code) lands on glibc's allocator rather than PartitionAlloc. glibc
//! keeps one arena per contending thread and only returns memory from the top
//! of the main heap, so WhatsApp's startup sync and media bursts leave the
//! arenas full of free-but-resident pages that never shrink. Measured on a
//! freshly started renderer: 283 MB of arena, 70 MB in use; one `malloc_trim(0)`
//! (12 ms) dropped the renderer by ~140 MB and the browser process by ~30 MB.
//!
//! `malloc_trim(0)` madvises the free pages of every arena away; it does not
//! move live allocations, so it is safe to run from any thread at any time.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// How often each process trims. The freed pages are re-requested slowly
/// (single-digit MB in five minutes on an idle renderer), so there is no point
/// in trimming more often; the call itself takes a few ms on a large renderer.
const INTERVAL: Duration = Duration::from_secs(120);

static STARTED: AtomicBool = AtomicBool::new(false);

/// Start the trim thread for this process. Idempotent, so it is safe to call
/// from both `main` and the renderer hooks. Must not be called in the zygote,
/// which Chromium requires to stay single-threaded until it forks.
pub fn start() {
    if STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("karere:trim".into())
        .spawn(|| {
            loop {
                std::thread::sleep(INTERVAL);
                trim();
            }
        });
    if let Err(e) = spawned {
        log::warn!("malloc_trim: could not start thread: {e}");
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn trim() {
    // SAFETY: plain glibc call with no pointer arguments; it takes each arena's
    // lock itself, so it is thread-safe.
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn trim() {}
