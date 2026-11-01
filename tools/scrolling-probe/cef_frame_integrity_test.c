/* Calibration: the checker must reject torn/blank frames, not just count them. */
#define KARERE_FRAME_INTEGRITY
#define main cef_probe_main
#include "cef_frame_probe.c"
#undef main
#include <assert.h>

static void fill_fixture(unsigned char *pixels, int frame, int bgra) {
    for (int i = 0; i < 64 * 64; ++i) {
        unsigned char *p = pixels + i * 4;
        p[bgra ? 2 : 0] = 64 + frame;
        p[1] = 64 + frame * 37 % 128;
        p[bgra ? 0 : 2] = 64 + frame * 53 % 128;
        p[3] = 255;
    }
}

int main(void) {
    unsigned char pixels[64 * 64 * 4], other[64 * 64 * 4];
    fill_fixture(pixels, 1, 0);
    integrity_inspect(pixels, 64, 64, 0, 64 * 4);
    fill_fixture(pixels, 7, 1);
    integrity_inspect(pixels, 64, 64, 1, 64 * 4);
    assert(integrity_checked == 2 && !integrity_partial && !integrity_blank);
    fill_fixture(pixels, 1, 0);
    fill_fixture(other, 2, 0);
    memcpy(pixels + sizeof(pixels) / 2, other, sizeof(pixels) / 2);
    integrity_inspect(pixels, 64, 64, 0, 64 * 4);
    assert(integrity_partial == 1 && !integrity_blank);
    memset(pixels, 0, sizeof(pixels));
    integrity_inspect(pixels, 64, 64, 0, 64 * 4);
    assert(integrity_partial == 1 && integrity_blank == 1);
    integrity_target = 72;
    assert(!integrity_report());
    puts("checker calibration passed: RGBA, BGRA, partial, blank, missing coverage");
    return 0;
}
