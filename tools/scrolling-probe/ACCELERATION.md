# Default-on acceleration diagnostics

GPU frame transfer now defaults on, including NVIDIA, while explicit saved
opt-outs and `KARERE_GPU_OSR=0` remain effective. A preference or visible chat is
not proof of accelerated delivery. The original pinned NVIDIA CEF delivered
zero accelerated callbacks in the standalone producer fixture; the native-handle
CEF/Chromium backport must be built and verified before claiming the fix works.

The normal accelerated path must meet the monitor rate. At 240 Hz every active
sample must deliver at least 235 fresh presentations/s, median near 4.17 ms and
p95 at most 8.33 ms. CPU transfer is measured recovery without the same FPS gate.
Current performance remains incomplete. The older tables in
[ADAPTIVE_REFRESH_FINDINGS.md](ADAPTIVE_REFRESH_FINDINGS.md) retain their original
build and default-off policy; they are not measurements of the new default.

## Producer isolation

`cef_frame_probe.c` uses the C API, checks the exact header/library API hash,
creates a temporary profile, and animates generated content without GTK. It
records CPU/accelerated callbacks and format/modifier metadata only. Compile
with the GNOME SDK and the headers matching the installed Devel library:

```sh
cc -std=c11 -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
  -I /path/to/cef cef_frame_probe.c -L /path/to/cef \
  -Wl,-rpath,/app/lib/cef -lcef -o /path/to/probes/cef-frame-probe
flatpak run --filesystem=/path/to/probes:ro \
  --command=/path/to/probes/cef-frame-probe io.github.tobagin.karere.Devel \
  --no-first-run --no-zygote --no-sandbox --ozone-platform=wayland \
  --use-gl=angle --use-angle=gl-egl --disable-features=PersistentHistograms
```

Use `--env=KARERE_FRAME_PROBE_CPU=1` before the app ID for CPU transfer, or change
ANGLE to `vulkan` for the producer control. Defaults are five seconds warmup and
15 seconds sampling at 960×540 and 240 FPS. Environment variables ending in
`_WIDTH`, `_HEIGHT`, `_RATE`, `_DURATION`, and `_URL`, with the same
`KARERE_FRAME_PROBE` prefix, support generated fixtures. These are probe-only
controls, not application preferences. Producer counts never establish presented
FPS. Interrupted runs record their actual sampled duration and are invalid as
producer acceptance samples. The media runner uses a private FIFO to request
orderly CEF shutdown, then checks the process exit and listener removal before
the next run. It does not terminate the Flatpak D-Bus proxy to close a browser.

## Media capabilities and generated loopback

Compile `media_capabilities.c` against `libva` and `libva-drm` inside the SDK,
then run it inside the Devel sandbox. It reports actual driver profile and
entrypoint queries, not successful playback. On the tested NVIDIA extension,
VA-API advertises decode profiles and no encoding entrypoints. An independent
FFmpeg NVENC control can work without CEF having an NVENC encoder.

`media_probe.py` starts the standalone producer with an isolated generated page,
temporary loopback HTTP/CDP listeners, and Chromium's fake capture device. It
never requests a real camera/microphone, links an account, or calls another user.
It records actual WebRTC encoder/decoder implementation, codec, dimensions,
bitrate, source/output timing, CPU and playback decoder properties. It reuses
the repository CDP transport; it does not replace #193's conversation tools.

```sh
python3 media_probe.py /path/to/new-webrtc.json \
  --probe /path/to/probes/cef-frame-probe --width 1280 --height 720 \
  --fps 30 --codec H264 --bitrate 2000000
python3 media_probe.py /path/to/new-webrtc-hw.json \
  --probe /path/to/probes/cef-frame-probe --width 1280 --height 720 \
  --fps 30 --codec H264 --bitrate 2000000 \
  --features VaapiOnNvidiaGPUs,AcceleratedVideoEncoder
python3 media_probe.py /path/to/new-playback.json \
  --probe /path/to/probes/cef-frame-probe --kind playback \
  --video /path/to/generated-1080p60-h264.mp4 --width 1920 --height 1080 \
  --fps 60 --bitrate 5000000 --features VaapiOnNvidiaGPUs
```

Each run defaults to three 30-second samples with five seconds warmup per sample.
Use identical generated files, codec, dimensions and bitrate for controls.
Stop compilation and other test workloads during measurements. Video callbacks
are judged against source cadence; they are not compositor presentation proof.
The [matched H.264 measurements](measurements/default_on_media_433.json) use the
original pinned CEF, RTX 5090 / NVIDIA 615.71.09 and GNOME 51 runtime. All eight
conditions completed three samples. Hardware runs maintained source cadence,
with no reported drops. CPU percentages below use 100% for one core:

| Generated workload | Software decode CPU | Hardware decode CPU |
| --- | ---: | ---: |
| Playback 720p30 | 12.39% | 8.26% |
| Playback 1080p60 | 28.24% | 11.91% |
| Local WebRTC 720p30 | 14.48% | 14.30% |
| Local WebRTC 1080p60 | 36.54% | 29.80% |

Playback reported `VaapiVideoDecoder`; WebRTC reported
`ExternalDecoder (VaapiVideoDecoder)` and `powerEfficientDecoder=true`.
WebRTC encoding remained **OpenH264 software** in every run, even when
`AcceleratedVideoEncoder` was requested. The candidate enables
`VaapiOnNvidiaGPUs` through central feature composition. It does not default-enable
the unverified encoder feature. H.264 is the measured codec; other codecs, actual
screen-capture transport and other driver/GPU combinations remain unverified.

Separate generated playback checks compared 16×9 RGBA grids at presented media
timestamps 1, 12 and 25 seconds: software and hardware results matched exactly
at both resolutions. These checks wait for frame delivery after seeking and run
outside timing samples. They do not constitute full-frame analysis of every
decoded frame. Earlier seek checks which read a stale texture are excluded.
`--samples 0 --graphics-check` also exercises real WebGL2 drawing and WebGPU
compute without adding readback to measured intervals; both passed on NVIDIA,
with WebGPU reporting a non-fallback adapter. Raster/compositor capability
flags remain distinct from workload-specific timing evidence.

These are standalone producer/media results. The combined application's scrolling
regression and fresh-presentation acceptance gates remain outstanding.

## Presentation and recovery

Native Wayland remains preferred. GSK uses its platform default; explicit
`GSK_RENDERER=gl`/`vulkan` controls isolate rendering. `KARERE_CEF_GRAPHICS` selects
CEF `gl`, `vulkan` or `software` for diagnostics. `KARERE_CPU_PRESENTER` (`gl` or
`snapshot`) and `KARERE_FRAME_TRANSFER` (`gl` or `vulkan`) independently select
presentation/transfer trials. Defaults are candidates pending matched results,
not a claim that the fastest configuration has been established.

Both GPU transfer paths copy borrowed CEF resources before callback return.
The Vulkan pool is bounded to three images and waits for GTK release plus GPU
completion before reuse. GL bounds retained owned textures to three and uses a
finite completion fence; it does not reuse texture storage still held by GTK.
The old deferred borrowed-DMA-BUF draw path was removed. An uncertain completion
terminates the worker without unwinding in-flight resources; a lightweight parent
restarts once with CPU transfer and retains attempted CEF backends. No saved
preference changes. A five-second operation watchdog sleeps while idle.
Kernel/driver hangs that prevent process teardown remain a physical validation
limit; fault-injection tests are not proof of recovery from every driver failure.
