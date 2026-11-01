/* Generated-only CEF 156 watchdog fixture. Never touches frame resources.
 * Drop accelerated callbacks so the real ten-second watchdog must recreate
 * the browser for CPU transfer. This is not a performance instrumenter.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "include/cef_version.h"
#include "include/capi/cef_browser_capi.h"
#include "include/capi/cef_client_capi.h"
#include "include/capi/cef_render_handler_capi.h"

_Static_assert(CEF_VERSION_MAJOR == 156, "fixture requires matching CEF 156 headers");
typedef cef_render_handler_t *(*get_render_fn)(cef_client_t *);
static _Atomic(get_render_fn) original_get;
static atomic_uint dropped;

static int generated_argument(void) {
    FILE *stream = fopen("/proc/self/cmdline", "rb");
    if (!stream) return 0;
    char *argument = NULL;
    size_t capacity = 0;
    int previous_url = 0, found = 0;
    while (getdelim(&argument, &capacity, '\0', stream) > 0) {
        if ((previous_url && !strncmp(argument, "data:", 5))
            || !strncmp(argument, "--url=data:", 11)) {
            found = 1;
            break;
        }
        previous_url = !strcmp(argument, "--url");
    }
    free(argument);
    fclose(stream);
    return found;
}

static void drop_accelerated(cef_render_handler_t *self, cef_browser_t *browser,
                            cef_paint_element_type_t type, size_t count,
                            const cef_rect_t *rects,
                            const cef_accelerated_paint_info_t *info) {
    (void)self; (void)browser; (void)type; (void)count; (void)rects; (void)info;
    /* No import, fd duplication, GPU submission, or resource access. CEF may
     * reclaim its borrowed allocation immediately on callback return. */
    if (atomic_fetch_add(&dropped, 1) == 0)
        fprintf(stderr, "graphics: recovery-fault accelerated_callback_dropped generated_only=1\n");
}

static cef_render_handler_t *get_render(cef_client_t *client) {
    cef_render_handler_t *handler = atomic_load(&original_get)(client);
    if (handler && handler->base.size == sizeof(*handler)) {
        handler->on_accelerated_paint = drop_accelerated;
    } else {
        fprintf(stderr, "graphics: recovery-fault handler_abi actual=%zu expected=%zu\n",
                handler ? handler->base.size : 0, sizeof(cef_render_handler_t));
    }
    return handler;
}

static void wrap(cef_client_t *client, const cef_string_t *url) {
    const char *requested = getenv("KARERE_FAULT_DROP_ACCELERATED");
    const char *settings = getenv("GSETTINGS_BACKEND");
    const char *profile = getenv("XDG_DATA_HOME");
    int (*version)(int) = dlsym(RTLD_NEXT, "cef_version_info");
    if (!requested || strcmp(requested, "1") || !settings || strcmp(settings, "memory")
        || !profile || strncmp(profile, "/tmp/karere-perf-", 17)) return;
    const int data_url = url && url->length >= 5 && url->str
        && url->str[0] == 'd' && url->str[1] == 'a' && url->str[2] == 't'
        && url->str[3] == 'a' && url->str[4] == ':';
    const int generated = data_url || generated_argument();
    fprintf(stderr, "graphics: recovery-fault guard generated=%d create_data_url=%d major=%d client_size=%zu expected=%zu\n",
            generated, data_url, version ? version(0) : 0,
            client ? client->base.size : 0, sizeof(cef_client_t));
    if (!generated || !version || version(0) != 156
        || !client || client->base.size != sizeof(*client)
        || !client->get_render_handler || client->get_render_handler == get_render)
        return;
    get_render_fn expected = NULL;
    atomic_compare_exchange_strong(&original_get, &expected, client->get_render_handler);
    if (atomic_load(&original_get) == client->get_render_handler) {
        client->get_render_handler = get_render;
        fprintf(stderr, "graphics: recovery-fault armed engine=156 generated_only=1\n");
    }
}

int cef_browser_host_create_browser(
    const cef_window_info_t *window, cef_client_t *client, const cef_string_t *url,
    const cef_browser_settings_t *settings, cef_dictionary_value_t *extra,
    cef_request_context_t *context) {
    int (*real)(const cef_window_info_t *, cef_client_t *, const cef_string_t *,
                const cef_browser_settings_t *, cef_dictionary_value_t *,
                cef_request_context_t *) = dlsym(RTLD_NEXT, "cef_browser_host_create_browser");
    if (!real) abort();
    wrap(client, url);
    return real(window, client, url, settings, extra, context);
}

cef_browser_t *cef_browser_host_create_browser_sync(
    const cef_window_info_t *window, cef_client_t *client, const cef_string_t *url,
    const cef_browser_settings_t *settings, cef_dictionary_value_t *extra,
    cef_request_context_t *context) {
    cef_browser_t *(*real)(const cef_window_info_t *, cef_client_t *, const cef_string_t *,
                          const cef_browser_settings_t *, cef_dictionary_value_t *,
                          cef_request_context_t *) = dlsym(RTLD_NEXT, "cef_browser_host_create_browser_sync");
    if (!real) abort();
    wrap(client, url);
    return real(window, client, url, settings, extra, context);
}
