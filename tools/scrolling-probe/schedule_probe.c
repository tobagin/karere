/* Observation-only CEF callback tracing. ABI verified against checksum-locked
 * cef-dll-sys 152.3.0+152.0.6, x86_64_unknown_linux_gnu.rs.
 * Expected sizes: base 40, app 80, browser-process handler 96 bytes.
 * This wrapper preserves callbacks and reference counts; it records no page data.
 */
#define cef_initialize timing_initialize
#define g_timeout_add_full timing_timeout_add_full
#include "pump_probe.c"
#undef cef_initialize
#undef g_timeout_add_full
#include <stddef.h>
#include <string.h>

struct app_prefix;
struct handler_prefix;
typedef void (*schedule_fn)(struct handler_prefix *, int64_t);
typedef struct handler_prefix *(*get_handler_fn)(struct app_prefix *);
struct app_prefix {
    size_t size;
    void *base_callbacks[4];
    void *other_callbacks[3];
    get_handler_fn get_handler;
    void *render_handler;
};
struct handler_prefix {
    size_t size;
    void *base_callbacks[4];
    void *other_callbacks[4];
    schedule_fn schedule;
    void *tail[2];
};
_Static_assert(sizeof(struct app_prefix) == 80, "CEF app ABI size");
_Static_assert(offsetof(struct app_prefix, get_handler) == 64, "CEF app ABI offset");
_Static_assert(sizeof(struct handler_prefix) == 96, "CEF handler ABI size");
_Static_assert(offsetof(struct handler_prefix, schedule) == 72, "CEF schedule ABI offset");
static get_handler_fn original_get_handler;
static _Atomic(schedule_fn) original_schedule;
static atomic_uint_fast64_t pending_deadline;
static _Thread_local int inside_schedule;
static _Thread_local int added_timer;
static atomic_uint request_count;
static atomic_uint handler_count;

static void observe_schedule(struct handler_prefix *handler, int64_t delay_ms) {
    uint64_t start = nanos();
    uint64_t pending = atomic_load(&pending_deadline);
    int previous_inside = inside_schedule;
    int previous_added = added_timer;
    inside_schedule = 1;
    added_timer = 0;
    atomic_load(&original_schedule)(handler, delay_ms);
    unsigned request_number = atomic_fetch_add(&request_count, 1);
    fprintf(stderr, "pump-probe request mono_ns=%llu n=%u delay_ms=%lld timer_added=%d pending_us=%llu\n",
            (unsigned long long)start, request_number, (long long)delay_ms, added_timer,
            (unsigned long long)(pending > start ? (pending - start) / 1000 : 0));
    if (delay_ms <= 0 && !added_timer && pending > start + UINT64_C(16000000)) {
        fprintf(stderr, "pump-probe urgent_postponed mono_ns=%llu requested_ms=%lld pending_us=%llu\n",
                (unsigned long long)start, (long long)delay_ms,
                (unsigned long long)((pending - start) / 1000));
    }
    inside_schedule = previous_inside;
    added_timer = previous_added;
}

static struct handler_prefix *observe_get_handler(struct app_prefix *app) {
    struct handler_prefix *handler = original_get_handler(app);
    if (atomic_fetch_add(&handler_count, 1) < 4) {
        fprintf(stderr, "pump-probe handler_seen size=%zu\n", handler ? handler->size : 0);
    }
    if (!handler || handler->size != sizeof(*handler) || !handler->schedule) return handler;
    schedule_fn expected = NULL;
    atomic_compare_exchange_strong(&original_schedule, &expected, handler->schedule);
    if (atomic_load(&original_schedule) == handler->schedule) handler->schedule = observe_schedule;
    return handler;
}

int cef_initialize(const void *args, const void *settings, void *opaque, void *info) {
    /* cef_settings_t: size 448, remote_debugging_port offset 336, verified in
     * the same checksum-locked binding. This enables loopback CDP without
     * Karere's --debug switches that relax origin/private-network checks. */
    union { max_align_t alignment; unsigned char bytes[448]; } local_settings;
    if (getenv("KARERE_PROBE_CDP") && settings && *(const size_t *)settings == 448) {
        const int port = 9333;
        memcpy(local_settings.bytes, settings, sizeof(local_settings.bytes));
        memcpy(local_settings.bytes + 336, &port, sizeof(port));
        settings = local_settings.bytes;
        fprintf(stderr, "pump-probe local_cdp_enabled port=9333 standard_security_flags=preserved\n");
    }
    struct app_prefix *app = opaque;
    if (app && app->size == sizeof(*app) && app->get_handler) {
        original_get_handler = app->get_handler;
        app->get_handler = observe_get_handler;
        fprintf(stderr, "pump-probe schedule_callback_wrapped ABI=cef152-linux-x86_64\n");
    }
    return timing_initialize(args, settings, opaque, info);
}

unsigned g_timeout_add_full(int priority, unsigned interval, source_func function,
                            void *data, destroy_notify notify) {
    if (inside_schedule) {
        added_timer = 1;
        atomic_store(&pending_deadline, nanos() + (uint64_t)interval * UINT64_C(1000000));
        if (interval > 16) {
            fprintf(stderr, "pump-probe scheduled mono_ns=%llu delay_ms=%u\n",
                    (unsigned long long)nanos(), interval);
        }
    }
    return timing_timeout_add_full(priority, interval, function, data, notify);
}
