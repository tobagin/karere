/* Run inside the target Flatpak: VA-API capabilities, not proof of playback. */
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <va/va.h>
#include <va/va_drm.h>
#include <va/va_str.h>

static void json_string(const char *value) {
    putchar('"');
    for (const unsigned char *p = (const unsigned char *)value; *p; ++p) {
        if (*p == '"' || *p == '\\') putchar('\\');
        if (*p >= 32) putchar(*p);
    }
    putchar('"');
}

int main(int argc, char **argv) {
    const char *node = argc > 1 ? argv[1] : "/dev/dri/renderD128";
    int fd = open(node, O_RDWR | O_CLOEXEC);
    if (fd < 0) { perror(node); return 1; }
    VADisplay display = vaGetDisplayDRM(fd);
    int major = 0, minor = 0;
    VAStatus status = display ? vaInitialize(display, &major, &minor) : VA_STATUS_ERROR_INVALID_DISPLAY;
    printf("{\"device\":"); json_string(node);
    if (status != VA_STATUS_SUCCESS) {
        printf(",\"error\":"); json_string(vaErrorStr(status)); puts("}");
        close(fd); return 1;
    }
    printf(",\"va_version\":\"%d.%d\",\"vendor\":", major, minor);
    json_string(vaQueryVendorString(display));
    printf(",\"profiles\":[");
    VAProfile *profiles = calloc((size_t)vaMaxNumProfiles(display), sizeof(*profiles));
    VAEntrypoint *entries = calloc((size_t)vaMaxNumEntrypoints(display), sizeof(*entries));
    if (!profiles || !entries) abort();
    int count = 0, first = 1;
    status = vaQueryConfigProfiles(display, profiles, &count);
    if (status != VA_STATUS_SUCCESS) return 2;
    for (int i = 0; i < count; ++i) {
        int n = 0;
        if (vaQueryConfigEntrypoints(display, profiles[i], entries, &n) != VA_STATUS_SUCCESS) continue;
        for (int j = 0; j < n; ++j) {
            if (!first) putchar(',');
            first = 0;
            printf("{\"profile\":"); json_string(vaProfileStr(profiles[i]));
            printf(",\"entrypoint\":"); json_string(vaEntrypointStr(entries[j]));
            printf(",\"decode\":%s,\"encode\":%s}",
                entries[j] == VAEntrypointVLD ? "true" : "false",
                (entries[j] == VAEntrypointEncSlice || entries[j] == VAEntrypointEncSliceLP || entries[j] == VAEntrypointEncPicture) ? "true" : "false");
        }
    }
    puts("]}");
    free(entries); free(profiles);
    vaTerminate(display); close(fd);
    return 0;
}
