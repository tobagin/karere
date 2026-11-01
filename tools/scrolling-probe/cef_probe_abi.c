/* Compile against the exact headers before loading schedule_probe.so. */
#include <stddef.h>
#include <stdio.h>
#include "include/cef_version.h"
#include "include/capi/cef_app_capi.h"

int main(void) {
    size_t settings = sizeof(cef_settings_t);
    size_t port = offsetof(cef_settings_t, remote_debugging_port);
    size_t app = sizeof(cef_app_t);
    size_t handler = sizeof(cef_browser_process_handler_t);
    size_t get = offsetof(cef_app_t, get_browser_process_handler);
    size_t schedule = offsetof(cef_browser_process_handler_t, on_schedule_message_pump_work);
    int known = (CEF_VERSION_MAJOR == 152 && settings == 448) ||
                (CEF_VERSION_MAJOR == 156 && settings == 496);
    int supported = known && port == 336 && app == 80 && handler == 96 &&
                    get == 64 && schedule == 72;
    printf("{\"cef_major\":%d,\"settings_size\":%zu,\"port_offset\":%zu,"
           "\"app_size\":%zu,\"handler_size\":%zu,\"get_offset\":%zu,"
           "\"schedule_offset\":%zu,\"supported\":%s}\n", CEF_VERSION_MAJOR,
           settings, port, app, handler, get, schedule, supported ? "true" : "false");
    return supported ? 0 : 1;
}
