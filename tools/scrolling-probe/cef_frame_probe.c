/* Generated-content-only CEF producer probe. No GTK presenter or account data.
 * Compile against exactly the headers and libcef used by the Flatpak.
 * Callback rates are producer evidence, never presented FPS.
 */
#define _POSIX_C_SOURCE 200809L
#include <inttypes.h>
#include <fcntl.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include "include/cef_api_hash.h"
#include "include/capi/cef_app_capi.h"
#include "include/capi/cef_browser_capi.h"
#include "include/capi/cef_client_capi.h"
#include "include/capi/cef_life_span_handler_capi.h"
#include "include/capi/cef_render_handler_capi.h"

static cef_render_handler_t *render;
static cef_life_span_handler_t *life;
static cef_browser_process_handler_t *process;
static int ready, closed;
static volatile sig_atomic_t interrupted;
static int control_fd = -1;
static uint64_t cpu_frames, gpu_frames, measured_cpu, measured_gpu;
static double sample_start, sample_end, first_frame;
static int probe_width = 960, probe_height = 540;
static void stop(int sig) { (void)sig; interrupted = 1; }

static int option(const char *name, int fallback, int maximum) {
    const char *value = getenv(name);
    if (!value) return fallback;
    char *end = NULL;
    long result = strtol(value, &end, 10);
    if (!*value || *end || result < 1 || result > maximum) {
        fprintf(stderr, "invalid %s\n", name); exit(2);
    }
    return (int)result;
}

static double seconds(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec + ts.tv_nsec / 1e9;
}

static atomic_int *references(cef_base_ref_counted_t *self) {
    return (atomic_int *)((char *)self + self->size);
}
static void CEF_CALLBACK add_ref(cef_base_ref_counted_t *self) {
    atomic_fetch_add(references(self), 1);
}
static int CEF_CALLBACK release(cef_base_ref_counted_t *self) {
    if (atomic_fetch_sub(references(self), 1) == 1) {
        free(self);
        return 1;
    }
    return 0;
}
static int CEF_CALLBACK has_one_ref(cef_base_ref_counted_t *self) {
    return atomic_load(references(self)) == 1;
}
static int CEF_CALLBACK has_ref(cef_base_ref_counted_t *self) {
    return atomic_load(references(self)) > 0;
}
static void *object(size_t size) {
    cef_base_ref_counted_t *base = calloc(1, size + sizeof(atomic_int));
    if (!base) abort();
    base->size = size;
    base->add_ref = add_ref;
    base->release = release;
    base->has_one_ref = has_one_ref;
    base->has_at_least_one_ref = has_ref;
    atomic_init(references(base), 1);
    return base;
}
static void string(const char *text, cef_string_t *out) {
    if (!cef_string_utf8_to_utf16(text, strlen(text), out)) abort();
}
static void CEF_CALLBACK view_rect(cef_render_handler_t *self,
                                 cef_browser_t *browser, cef_rect_t *rect) {
    *rect = (cef_rect_t){0, 0, probe_width, probe_height};
}
static int CEF_CALLBACK screen_info(cef_render_handler_t *self,
                                   cef_browser_t *browser, cef_screen_info_t *info) {
    info->device_scale_factor = 1.0f;
    info->depth = 24;
    info->depth_per_component = 8;
    info->rect = info->available_rect = (cef_rect_t){0, 0, probe_width, probe_height};
    return 1;
}
static void count_frame(int accelerated) {
    const double now = seconds();
    if (!first_frame) first_frame = now;
    if (accelerated) ++gpu_frames; else ++cpu_frames;
    if (now >= sample_start && now < sample_end) {
        if (accelerated) ++measured_gpu; else ++measured_cpu;
    }
}
static void CEF_CALLBACK paint(cef_render_handler_t *self, cef_browser_t *browser,
                              cef_paint_element_type_t type, size_t count,
                              const cef_rect_t *rects, const void *pixels,
                              int width, int height) {
    if (type == PET_VIEW) count_frame(0);
}
static void CEF_CALLBACK accelerated(cef_render_handler_t *self,
                                    cef_browser_t *browser,
                                    cef_paint_element_type_t type, size_t count,
                                    const cef_rect_t *rects,
                                    const cef_accelerated_paint_info_t *info) {
    if (type != PET_VIEW) return;
    if (!gpu_frames) {
        printf("{\"kind\":\"first_accelerated_frame\",\"planes\":%d,"
               "\"modifier\":\"0x%016" PRIx64 "\",\"format\":%d,"
               "\"width\":%d,\"height\":%d}\n",
               info->plane_count, info->modifier, info->format,
               info->extra.coded_size.width, info->extra.coded_size.height);
        fflush(stdout);
    }
    /* Inspect metadata only. Do not retain or access producer resources. */
    count_frame(1);
}
static cef_render_handler_t *CEF_CALLBACK get_render(cef_client_t *self) {
    add_ref(&render->base);
    return render;
}
static cef_life_span_handler_t *CEF_CALLBACK get_life(cef_client_t *self) {
    add_ref(&life->base);
    return life;
}
static void CEF_CALLBACK before_close(cef_life_span_handler_t *self,
                                     cef_browser_t *browser) {
    closed = 1;
}
static void CEF_CALLBACK context_ready(cef_browser_process_handler_t *self) {
    ready = 1;
}
static cef_browser_process_handler_t *CEF_CALLBACK get_process(cef_app_t *self) {
    add_ref(&process->base);
    return process;
}
static void pump_until(double deadline) {
    const struct timespec tick = {0, 1000000};
    while (!interrupted && !closed && seconds() < deadline) {
        char command;
        if (control_fd >= 0 && read(control_fd, &command, 1) == 1) {
            interrupted = 1;
            break;
        }
        cef_do_message_loop_work();
        nanosleep(&tick, NULL);
    }
}

int main(int argc, char **argv) {
    probe_width = option("KARERE_FRAME_PROBE_WIDTH", 960, 8192);
    probe_height = option("KARERE_FRAME_PROBE_HEIGHT", 540, 8192);
    const int duration = option("KARERE_FRAME_PROBE_DURATION", 15, 3600);
    const int rate = option("KARERE_FRAME_PROBE_RATE", 240, 1000);
    const char *hash = cef_api_hash(CEF_API_VERSION, 0);
    if (!hash || strcmp(hash, CEF_API_HASH_PLATFORM)) {
        fprintf(stderr, "CEF header/library API hash mismatch\n");
        return 2;
    }
    cef_main_args_t args = {argc, argv};
    process = object(sizeof(*process));
    process->on_context_initialized = context_ready;
    cef_app_t *app = object(sizeof(*app));
    app->get_browser_process_handler = get_process;
    /* CEF consumes one reference for each interface passed across the C ABI. */
    add_ref(&app->base);
    int child_status = cef_execute_process(&args, app, NULL);
    if (child_status >= 0) return child_status;
    const char *control = getenv("KARERE_FRAME_PROBE_CONTROL");
    if (control) {
        control_fd = open(control, O_RDONLY | O_NONBLOCK | O_CLOEXEC);
        if (control_fd < 0) { perror("probe control"); return 2; }
    }

    char profile[] = "/tmp/karere-cef-frame-probe-XXXXXX";
    if (!mkdtemp(profile)) { perror("mkdtemp"); return 2; }
    cef_settings_t settings = {.size = sizeof(settings)};
    settings.no_sandbox = 1; /* Executed inside the Flatpak sandbox. */
    settings.windowless_rendering_enabled = 1;
    settings.log_severity = LOGSEVERITY_VERBOSE;
    string(profile, &settings.root_cache_path);
    string(profile, &settings.cache_path);
    string("/app/lib/cef", &settings.resources_dir_path);
    string("/app/lib/cef/locales", &settings.locales_dir_path);
    add_ref(&app->base);
    if (!cef_initialize(&args, &settings, app, NULL)) {
        fprintf(stderr, "CEF initialization failed: %d\n", cef_get_exit_code());
        return 2;
    }
    /* Chromium initialization may replace signal handlers. The private pipe
     * also lets the runner request orderly close without stopping Flatpak's
     * D-Bus proxy or relying on CDP Browser.close for a windowless host. */
    signal(SIGTERM, stop);
    signal(SIGINT, stop);
    double deadline = seconds() + 5;
    while (!ready && seconds() < deadline) pump_until(seconds() + .005);
    if (!ready) { fprintf(stderr, "CEF context deadline exceeded\n"); return 2; }

    render = object(sizeof(*render));
    render->get_view_rect = view_rect;
    render->get_screen_info = screen_info;
    render->on_paint = paint;
    render->on_accelerated_paint = accelerated;
    life = object(sizeof(*life));
    life->on_before_close = before_close;
    cef_client_t *client = object(sizeof(*client));
    client->get_render_handler = get_render;
    client->get_life_span_handler = get_life;
    int shared = !getenv("KARERE_FRAME_PROBE_CPU");
    cef_window_info_t window = {.size = sizeof(window)};
    window.windowless_rendering_enabled = 1;
    window.shared_texture_enabled = shared;
    cef_browser_settings_t browser_settings = {.size = sizeof(browser_settings)};
    browser_settings.windowless_frame_rate = rate;
    browser_settings.background_color = 0xff202020;
    cef_string_t url = {0};
    const char *fixture_url = getenv("KARERE_FRAME_PROBE_URL");
    string(fixture_url ? fixture_url : "data:text/html,<style>body{margin:0;background:rgb(24,24,24)}"
           "div{width:300px;height:300px;background:rgb(32,180,220);"
           "animation:a 1s linear infinite alternate}"
           "@keyframes a{to{transform:translateX(600px) rotate(180deg)}}"
           "</style><div></div>", &url);
    add_ref(&client->base);
    cef_browser_t *browser = cef_browser_host_create_browser_sync(
        &window, client, &url, &browser_settings, NULL, NULL);
    cef_string_clear(&url);
    if (!browser) { fprintf(stderr, "CEF browser creation failed\n"); return 2; }
    cef_browser_host_t *host = browser->get_host(browser);
    host->was_hidden(host, 0);
    host->was_resized(host);
    const double start = seconds();
    sample_start = start + 5;
    sample_end = sample_start + duration;
    pump_until(sample_end);
    const double end = seconds();
    const double sampled = end > sample_start ? end - sample_start : 0;
    printf("{\"kind\":\"producer_result\",\"shared_texture\":%d,"
           "\"effective_fps\":%d,\"warmup_seconds\":5,\"requested_sample_seconds\":%d,"
           "\"sample_seconds\":%.6f,\"sample_complete\":%s,"
           "\"cpu_frames\":%" PRIu64 ",\"accelerated_frames\":%" PRIu64 ","
           "\"sample_cpu_frames\":%" PRIu64 ",\"sample_accelerated_frames\":%" PRIu64 ","
           "\"first_frame_ms\":%.3f}\n",
           shared, host->get_windowless_frame_rate(host), duration,
           sampled < duration ? sampled : duration, end >= sample_end ? "true" : "false", cpu_frames, gpu_frames,
           measured_cpu, measured_gpu, first_frame ? (first_frame - start) * 1000 : -1);
    fflush(stdout);
    if (control_fd >= 0) { close(control_fd); control_fd = -1; }
    interrupted = 0;
    if (!closed) host->close_browser(host, 1);
    deadline = seconds() + 5;
    while (!closed && seconds() < deadline) pump_until(seconds() + .005);
    if (!closed) { fprintf(stderr, "CEF close deadline exceeded\n"); return 3; }
    host->base.release(&host->base);
    browser->base.release(&browser->base);
    cef_shutdown();
    release(&client->base);
    release(&render->base);
    release(&life->base);
    release(&app->base);
    release(&process->base);
    return 0;
}
