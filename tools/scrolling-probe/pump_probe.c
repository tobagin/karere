/* Local diagnostic interposer. No application data or account contents are read.
 * Default: observe CEF pump call timing and identify its 100 ms backstop.
 * KARERE_PROBE_BACKSTOP_MS=8 changes only the first main-thread 100 ms GLib
 * timer registered immediately after a successful cef_initialize(). In 4.3.0
 * src/cef_runtime.rs this is the CEF backstop; it is not a proposed final fix.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>
#include <sys/syscall.h>

typedef int (*source_func)(void *);
typedef void (*destroy_notify)(void *);
static int (*real_initialize)(const void *, const void *, void *, void *);
static void (*real_pump)(void);
static unsigned (*real_timeout)(int, unsigned, source_func, void *, destroy_notify);
static atomic_int awaiting_backstop;
static uint64_t last_start;
static unsigned override_ms;

static uint64_t nanos(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * UINT64_C(1000000000) + ts.tv_nsec;
}

__attribute__((constructor)) static void setup(void) {
    real_initialize = dlsym(RTLD_NEXT, "cef_initialize");
    real_pump = dlsym(RTLD_NEXT, "cef_do_message_loop_work");
    real_timeout = dlsym(RTLD_NEXT, "g_timeout_add_full");
    const char *value = getenv("KARERE_PROBE_BACKSTOP_MS");
    if (value && value[0] == '8' && value[1] == '\0') override_ms = 8;
}

int cef_initialize(const void *args, const void *settings, void *app, void *info) {
    if (!real_initialize) _exit(125);
    int result = real_initialize(args, settings, app, info);
    if (result == 1) {
        atomic_store(&awaiting_backstop, 1);
        fprintf(stderr, "pump-probe initialized pid=%d override_ms=%u\n", getpid(), override_ms);
    }
    return result;
}

unsigned g_timeout_add_full(int priority, unsigned interval, source_func function,
                            void *data, destroy_notify notify) {
    if (!real_timeout) _exit(125);
    if (interval == 100 && syscall(SYS_gettid) == getpid() &&
        atomic_exchange(&awaiting_backstop, 0)) {
        fprintf(stderr, "pump-probe backstop pid=%d original_ms=100 effective_ms=%u\n",
                getpid(), override_ms ? override_ms : interval);
        if (override_ms) interval = override_ms;
    }
    return real_timeout(priority, interval, function, data, notify);
}

void cef_do_message_loop_work(void) {
    if (!real_pump) _exit(125);
    uint64_t start = nanos();
    uint64_t gap = last_start ? start - last_start : 0;
    last_start = start;
    real_pump();
    uint64_t duration = nanos() - start;
    fprintf(stderr, "pump-probe call mono_ns=%llu gap_us=%llu duration_us=%llu\n",
            (unsigned long long)start, (unsigned long long)(gap / 1000),
            (unsigned long long)(duration / 1000));
}
