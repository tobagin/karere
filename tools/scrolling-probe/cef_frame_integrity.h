/* Generated OSR image-integrity check, deliberately separate from FPS sampling.
 * CEF retains its buffers: complete the copy into owned storage before returning.
 * Compile cef_frame_probe.c with -DKARERE_FRAME_INTEGRITY -lEGL -lGLESv2.
 */
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <GLES3/gl3.h>
#include <GLES2/gl2ext.h>

static EGLDisplay integrity_display = EGL_NO_DISPLAY;
static EGLContext integrity_context = EGL_NO_CONTEXT;
static EGLSurface integrity_surface = EGL_NO_SURFACE;
static PFNEGLCREATEIMAGEKHRPROC integrity_create_image;
static PFNEGLDESTROYIMAGEKHRPROC integrity_destroy_image;
static PFNGLEGLIMAGETARGETTEXTURE2DOESPROC integrity_bind_image;
static GLuint integrity_import, integrity_owned, integrity_fbos[2];
static unsigned char *integrity_pixels;
static size_t integrity_capacity;
static int integrity_width, integrity_height, integrity_resize, integrity_target;
static int integrity_checks[73], integrity_per_resize;
static uint64_t integrity_checked, integrity_partial, integrity_blank;
static double integrity_started, integrity_next;
static int integrity_enabled;

static void integrity_fatal(const char *reason) {
    fprintf(stderr, "integrity probe failed: %s (EGL=0x%x GL=0x%x)\n",
            reason, eglGetError(), glGetError());
    /* Do not return borrowed resources after unknown GPU completion. */
    fflush(NULL);
    _Exit(8);
}

static int integrity_open_display(EGLDisplay display) {
    EGLint major, minor;
    if (display == EGL_NO_DISPLAY || !eglInitialize(display, &major, &minor))
        return 0;
    const EGLint attributes[] = {
        EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT,
        EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
        EGL_NONE};
    EGLConfig config;
    EGLint count;
    const EGLint context[] = {EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE};
    const EGLint pbuffer[] = {EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE};
    if (!eglBindAPI(EGL_OPENGL_ES_API) ||
        !eglChooseConfig(display, attributes, &config, 1, &count) || !count) {
        eglTerminate(display);
        return 0;
    }
    EGLContext ctx = eglCreateContext(display, config, EGL_NO_CONTEXT, context);
    EGLSurface surface = eglCreatePbufferSurface(display, config, pbuffer);
    if (ctx == EGL_NO_CONTEXT || surface == EGL_NO_SURFACE ||
        !eglMakeCurrent(display, surface, surface, ctx)) {
        if (surface != EGL_NO_SURFACE) eglDestroySurface(display, surface);
        if (ctx != EGL_NO_CONTEXT) eglDestroyContext(display, ctx);
        eglTerminate(display);
        return 0;
    }
    const char *renderer = (const char *)glGetString(GL_RENDERER);
    if (!renderer || strstr(renderer, "llvmpipe") || strstr(renderer, "softpipe")) {
        eglMakeCurrent(display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
        eglDestroySurface(display, surface);
        eglDestroyContext(display, ctx);
        eglTerminate(display);
        return 0;
    }
    integrity_display = display;
    integrity_context = ctx;
    integrity_surface = surface;
    printf("{\"kind\":\"integrity_consumer\",\"renderer\":\"%s\","
           "\"vendor\":\"%s\",\"egl_vendor\":\"%s\"}\n", renderer,
           glGetString(GL_VENDOR), eglQueryString(display, EGL_VENDOR));
    fflush(stdout);
    return 1;
}

static void integrity_gl_init(void) {
    PFNEGLQUERYDEVICESEXTPROC devices = (PFNEGLQUERYDEVICESEXTPROC)
        eglGetProcAddress("eglQueryDevicesEXT");
    PFNEGLGETPLATFORMDISPLAYEXTPROC platform = (PFNEGLGETPLATFORMDISPLAYEXTPROC)
        eglGetProcAddress("eglGetPlatformDisplayEXT");
    EGLDeviceEXT found[16];
    EGLint count = 0;
    if (platform && devices && devices(16, found, &count)) {
        for (int i = 0; i < count; ++i)
            if (integrity_open_display(platform(EGL_PLATFORM_DEVICE_EXT, found[i], NULL)))
                break;
    }
    if (integrity_display == EGL_NO_DISPLAY && platform)
        integrity_open_display(platform(EGL_PLATFORM_SURFACELESS_MESA, NULL, NULL));
    if (integrity_display == EGL_NO_DISPLAY)
        integrity_fatal("no hardware EGL consumer display");
    integrity_create_image = (PFNEGLCREATEIMAGEKHRPROC)eglGetProcAddress("eglCreateImageKHR");
    integrity_destroy_image = (PFNEGLDESTROYIMAGEKHRPROC)eglGetProcAddress("eglDestroyImageKHR");
    integrity_bind_image = (PFNGLEGLIMAGETARGETTEXTURE2DOESPROC)
        eglGetProcAddress("glEGLImageTargetTexture2DOES");
    if (!integrity_create_image || !integrity_destroy_image || !integrity_bind_image)
        integrity_fatal("missing DMA-BUF import functions");
    glGenTextures(1, &integrity_import);
    glGenTextures(1, &integrity_owned);
    glGenFramebuffers(2, integrity_fbos);
}

static int integrity_should_check(int width, int height) {
    return integrity_enabled && integrity_started > 0 &&
           seconds() >= integrity_started + 5 &&
           width == probe_width && height == probe_height &&
           integrity_per_resize < 3;
}

static void integrity_inspect(const unsigned char *pixels, int width, int height,
                              int bgra, size_t stride) {
    int bad = 0, blank = 0;
    const unsigned char *first = pixels + (size_t)(height / 32) * stride +
                                 (size_t)(width / 32) * 4;
    int r = first[bgra ? 2 : 0], g = first[1], b = first[bgra ? 0 : 2];
    int frame = r - 64;
    int palette = frame >= 0 && frame < 128 &&
                  abs(g - (64 + (frame * 37) % 128)) <= 2 &&
                  abs(b - (64 + (frame * 53) % 128)) <= 2;
    for (int y = 0; y < 16; ++y) {
        for (int x = 0; x < 16; ++x) {
            const unsigned char *p = pixels +
                (size_t)((2 * y + 1) * height / 32) * stride +
                (size_t)((2 * x + 1) * width / 32) * 4;
            if (abs((int)p[0] - first[0]) > 2 ||
                abs((int)p[1] - first[1]) > 2 ||
                abs((int)p[2] - first[2]) > 2 || p[3] < 253) ++bad;
            if (p[0] < 3 && p[1] < 3 && p[2] < 3) ++blank;
        }
    }
    ++integrity_checked;
    ++integrity_checks[integrity_resize];
    ++integrity_per_resize;
    if (bad || !palette) {
        if (blank == 256) ++integrity_blank;
        else ++integrity_partial;
        printf("{\"kind\":\"integrity_bad_frame\",\"resize\":%d,"
               "\"width\":%d,\"height\":%d,\"unequal_grid_pixels\":%d,"
               "\"black_grid_pixels\":%d,\"palette_valid\":%s,"
               "\"first_rgb\":[%d,%d,%d]}\n", integrity_resize, width, height,
               bad, blank, palette ? "true" : "false", r, g, b);
        fflush(stdout);
    }
}

static void integrity_gpu(const cef_accelerated_paint_info_t *info) {
    int width = info->extra.coded_size.width, height = info->extra.coded_size.height;
    if (!integrity_should_check(width, height)) return;
    if (integrity_display == EGL_NO_DISPLAY) integrity_gl_init();
    if (!eglMakeCurrent(integrity_display, integrity_surface,
                        integrity_surface, integrity_context))
        integrity_fatal("consumer context activation");
    if (info->plane_count < 1 || info->plane_count > 4)
        integrity_fatal("invalid producer plane count");
    /* CEF RGBA/BGRA formats have DRM byte order AB24/AR24 on little endian. */
    uint32_t format = info->format == CEF_COLOR_TYPE_BGRA_8888 ? 0x34325241 :
                      info->format == CEF_COLOR_TYPE_RGBA_8888 ? 0x34324241 : 0;
    if (!format) integrity_fatal("unsupported producer pixel format");
    const EGLint fds[] = {0x3272, 0x3275, 0x3278, 0x3440};
    const EGLint offsets[] = {0x3273, 0x3276, 0x3279, 0x3441};
    const EGLint pitches[] = {0x3274, 0x3277, 0x327a, 0x3442};
    const EGLint lows[] = {0x3443, 0x3445, 0x3447, 0x3449};
    const EGLint highs[] = {0x3444, 0x3446, 0x3448, 0x344a};
    EGLint attributes[64] = {EGL_WIDTH, width, EGL_HEIGHT, height,
                            EGL_LINUX_DRM_FOURCC_EXT, (EGLint)format};
    int n = 6;
    for (int i = 0; i < info->plane_count; ++i) {
        attributes[n++] = fds[i]; attributes[n++] = info->planes[i].fd;
        attributes[n++] = offsets[i]; attributes[n++] = (EGLint)info->planes[i].offset;
        attributes[n++] = pitches[i]; attributes[n++] = (EGLint)info->planes[i].stride;
        if (info->modifier && info->modifier != UINT64_C(0x00ffffffffffffff)) {
            attributes[n++] = lows[i]; attributes[n++] = (EGLint)info->modifier;
            attributes[n++] = highs[i]; attributes[n++] = (EGLint)(info->modifier >> 32);
        }
    }
    attributes[n] = EGL_NONE;
    EGLImageKHR image = integrity_create_image(integrity_display, EGL_NO_CONTEXT,
        EGL_LINUX_DMA_BUF_EXT, NULL, attributes);
    if (image == EGL_NO_IMAGE_KHR) integrity_fatal("producer DMA-BUF import");
    glBindTexture(GL_TEXTURE_2D, integrity_import);
    integrity_bind_image(GL_TEXTURE_2D, image);
    glBindFramebuffer(GL_READ_FRAMEBUFFER, integrity_fbos[0]);
    glFramebufferTexture2D(GL_READ_FRAMEBUFFER, GL_COLOR_ATTACHMENT0,
                           GL_TEXTURE_2D, integrity_import, 0);
    if (glCheckFramebufferStatus(GL_READ_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE)
        integrity_fatal("producer framebuffer");
    if (width != integrity_width || height != integrity_height) {
        glBindTexture(GL_TEXTURE_2D, integrity_owned);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, width, height, 0,
                     GL_RGBA, GL_UNSIGNED_BYTE, NULL);
        integrity_width = width; integrity_height = height;
    }
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, integrity_fbos[1]);
    glFramebufferTexture2D(GL_DRAW_FRAMEBUFFER, GL_COLOR_ATTACHMENT0,
                           GL_TEXTURE_2D, integrity_owned, 0);
    if (glCheckFramebufferStatus(GL_DRAW_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE)
        integrity_fatal("owned framebuffer");
    glBlitFramebuffer(0, 0, width, height, 0, 0, width, height,
                      GL_COLOR_BUFFER_BIT, GL_NEAREST);
    GLsync fence = glFenceSync(GL_SYNC_GPU_COMMANDS_COMPLETE, 0);
    GLenum waited = glClientWaitSync(fence, GL_SYNC_FLUSH_COMMANDS_BIT, 1000000000);
    if (waited != GL_ALREADY_SIGNALED && waited != GL_CONDITION_SATISFIED)
        integrity_fatal("owned copy completion deadline");
    glDeleteSync(fence);
    glBindFramebuffer(GL_READ_FRAMEBUFFER, integrity_fbos[1]);
    size_t bytes = (size_t)width * (size_t)height * 4;
    if (bytes > integrity_capacity) {
        unsigned char *replacement = realloc(integrity_pixels, bytes);
        if (!replacement) integrity_fatal("owned readback allocation");
        integrity_pixels = replacement; integrity_capacity = bytes;
    }
    glReadPixels(0, 0, width, height, GL_RGBA, GL_UNSIGNED_BYTE, integrity_pixels);
    if (glGetError() != GL_NO_ERROR) integrity_fatal("owned texture readback");
    integrity_inspect(integrity_pixels, width, height, 0, (size_t)width * 4);
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    integrity_destroy_image(integrity_display, image);
}

static void integrity_tick(cef_browser_host_t *host) {
    double now = seconds();
    if (now < integrity_next || integrity_resize >= integrity_target) return;
    ++integrity_resize;
    integrity_per_resize = 0;
    probe_width = integrity_resize % 2 ? 3840 : 1280;
    probe_height = integrity_resize % 2 ? 2160 : 720;
    host->was_resized(host);
    integrity_next = now + .35;
}

static int integrity_report(void) {
    int covered = 0;
    for (int i = 1; i <= integrity_target; ++i) if (integrity_checks[i]) ++covered;
    int passed = integrity_checked > 0 && covered == integrity_target &&
                 !integrity_partial && !integrity_blank;
    printf("{\"kind\":\"integrity_result\",\"generated_only\":true,"
           "\"presentation_verified\":false,\"osr_resize_events\":%d,"
           "\"requested_resize_events\":%d,\"resize_events_with_checks\":%d,"
           "\"checked_frames\":%" PRIu64 ",\"partial_frames\":%" PRIu64 ","
           "\"blank_frames\":%" PRIu64 ",\"passed\":%s}\n",
           integrity_resize, integrity_target, covered, integrity_checked,
           integrity_partial, integrity_blank, passed ? "true" : "false");
    fflush(stdout);
    return passed;
}

static void integrity_cleanup(void) {
    free(integrity_pixels);
    if (integrity_display == EGL_NO_DISPLAY) return;
    glDeleteFramebuffers(2, integrity_fbos);
    glDeleteTextures(1, &integrity_import);
    glDeleteTextures(1, &integrity_owned);
    eglMakeCurrent(integrity_display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
    eglDestroySurface(integrity_display, integrity_surface);
    eglDestroyContext(integrity_display, integrity_context);
    eglTerminate(integrity_display);
}
