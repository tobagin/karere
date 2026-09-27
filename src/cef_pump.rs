//! Drive CEF on GTK's main context, yielding to input and redraw sources.
//!
//! An OSR paint can exhaust CEF's external pump time slice while work remains
//! queued, without another OnScheduleMessagePumpWork callback. Request a
//! continuation after delivering that frame instead of waiting for the idle
//! backstop. Keep one source, including the backstop, so callbacks cannot pile up.

use parking_lot::Mutex;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

const BACKSTOP: Duration = Duration::from_millis(100);

static PUMP: LazyLock<Arc<MessagePump>> =
    LazyLock::new(|| MessagePump::new(glib::MainContext::default(), cef::do_message_loop_work));

pub fn schedule(delay_ms: i64) {
    PUMP.schedule(Duration::from_millis(delay_ms.max(0) as u64));
}

pub fn stop() {
    PUMP.stop();
}

struct Pending {
    source: glib::Source,
    deadline: Instant,
    generation: u64,
}

#[derive(Default)]
struct State {
    pending: Option<Pending>,
    generation: u64,
    running: bool,
    deferred: Option<Instant>,
    stopped: bool,
}

struct MessagePump {
    context: glib::MainContext,
    state: Mutex<State>,
    work: Box<dyn Fn() + Send + Sync>,
}

impl MessagePump {
    fn new(context: glib::MainContext, work: impl Fn() + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            context,
            state: Mutex::new(State::default()),
            work: Box::new(work),
        })
    }

    fn schedule(self: &Arc<Self>, delay: Duration) {
        self.schedule_at(Instant::now() + delay.min(BACKSTOP));
    }

    fn schedule_at(self: &Arc<Self>, deadline: Instant) {
        let mut state = self.state.lock();
        if state.stopped {
            return;
        }
        // CEF can request work from any thread, or while dispatching a paint.
        // Do not attach another source while CEF runs: a nested GTK iteration
        // must not re-enter do_message_loop_work(). Dispatch it after returning.
        if state.running {
            state.deferred = Some(state.deferred.map_or(deadline, |old| old.min(deadline)));
            return;
        }
        if let Some(pending) = &state.pending
            && pending.deadline <= deadline
        {
            return;
        }
        if let Some(pending) = state.pending.take() {
            pending.source.destroy();
        }
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        let pump = self.clone();
        // GLib timeouts use whole milliseconds. Round up so a positive CEF
        // deadline never becomes an early zero-delay source through truncation.
        let delay_ms = deadline
            .saturating_duration_since(Instant::now())
            .as_nanos()
            .div_ceil(1_000_000) as u64;
        let source = glib::timeout_source_new(
            Duration::from_millis(delay_ms),
            Some("karere-cef-pump"),
            // GTK input and frame-clock redraws run before Chromium work, as
            // they do in Chromium's native GLib pump. No fixed 8 ms delay is
            // needed for immediate work; coalescing prevents a timer backlog.
            glib::Priority::DEFAULT_IDLE,
            move || {
                pump.dispatch(generation);
                glib::ControlFlow::Break
            },
        );
        source.attach(Some(&self.context));
        state.pending = Some(Pending {
            source,
            deadline,
            generation,
        });
    }

    fn dispatch(self: &Arc<Self>, generation: u64) {
        {
            let mut state = self.state.lock();
            // A source already selected for dispatch can race an earlier
            // request from another thread. Its replacement owns the next turn.
            if state.stopped || state.pending.as_ref().map(|p| p.generation) != Some(generation) {
                return;
            }
            state.pending = None;
            state.running = true;
        }
        (self.work)();
        let deadline = {
            let mut state = self.state.lock();
            state.running = false;
            state
                .deferred
                .take()
                .unwrap_or_else(|| Instant::now() + BACKSTOP)
        };
        self.schedule_at(deadline);
    }

    fn stop(&self) {
        let mut state = self.state.lock();
        state.stopped = true;
        state.deferred = None;
        if let Some(pending) = state.pending.take() {
            pending.source.destroy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn counter() -> (glib::MainContext, Arc<MessagePump>, Arc<AtomicUsize>) {
        let context = glib::MainContext::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let pump = MessagePump::new(context.clone(), move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
        (context, pump, calls)
    }

    #[test]
    fn urgent_request_replaces_delayed_work_and_coalesces_a_burst() {
        let (context, pump, calls) = counter();
        pump.schedule(BACKSTOP);
        assert!(!context.pending());
        pump.schedule(Duration::ZERO);
        for _ in 0..1_000 {
            pump.schedule(Duration::ZERO);
            pump.schedule(BACKSTOP);
        }
        context.iteration(false);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            !context.pending(),
            "idle work must wait for the slow backstop"
        );
        pump.stop();
    }

    #[test]
    fn worker_thread_requests_share_one_source() {
        let (context, pump, calls) = counter();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let pump = pump.clone();
                scope.spawn(move || {
                    for _ in 0..100 {
                        pump.schedule(Duration::ZERO);
                    }
                });
            }
        });
        context.iteration(false);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(!context.pending());
        pump.stop();
    }

    #[test]
    fn frame_continuation_survives_a_missing_cef_request_without_reentry() {
        let context = glib::MainContext::new();
        let owner = Arc::new(OnceLock::<std::sync::Weak<MessagePump>>::new());
        let callback_owner = owner.clone();
        let callback_context = context.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let pump = MessagePump::new(context.clone(), move || {
            if count.fetch_add(1, Ordering::SeqCst) == 0 {
                // Simulate a CPU paint inside a bounded CEF call, with no
                // subsequent OnScheduleMessagePumpWork notification.
                let pump = callback_owner.get().unwrap().upgrade().unwrap();
                pump.schedule(Duration::ZERO);
                pump.schedule(BACKSTOP);
                assert!(!callback_context.iteration(false));
                assert_eq!(count.load(Ordering::SeqCst), 1);
            }
        });
        owner.set(Arc::downgrade(&pump)).unwrap();
        pump.schedule(Duration::ZERO);
        context.iteration(false);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        context.iteration(false);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(
            !context.pending(),
            "no paint means no further immediate pumping"
        );
        pump.stop();
    }

    #[test]
    fn native_events_run_before_cef_work() {
        let (context, pump, calls) = counter();
        pump.schedule(Duration::ZERO);
        let count = calls.clone();
        let native = glib::idle_source_new(None, glib::Priority::DEFAULT, move || {
            assert_eq!(count.load(Ordering::SeqCst), 0);
            glib::ControlFlow::Break
        });
        native.attach(Some(&context));
        context.iteration(false);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        context.iteration(false);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        pump.stop();
    }

    #[test]
    fn shutdown_cancels_pending_and_future_work() {
        let (context, pump, calls) = counter();
        pump.schedule(Duration::ZERO);
        pump.stop();
        pump.schedule(Duration::ZERO);
        assert!(!context.iteration(false));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
