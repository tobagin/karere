# Default-on acceleration diagnostics

GPU frame transfer now defaults on, including NVIDIA, while explicit saved
opt-outs and `KARERE_GPU_OSR=0` remain effective. A preference or visible chat is
not proof of accelerated delivery. The original pinned NVIDIA CEF delivered
zero accelerated callbacks in the standalone producer fixture; the native-handle
CEF/Chromium backport must be built and verified before claiming the fix works.
The [isolated backend controls](measurements/original_cef_producer_433.json)
confirm this with both hardware ANGLE GL/EGL and ANGLE Vulkan. Vulkan CPU transfer
does produce frames. These generated checks ran during compilation and establish
the failure/recovery behavior, not a presentation rate or performance comparison.

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

To isolate a rebuilt engine before installing it, expand the Devel manifest with
`flatpak-builder --show-manifest`, then run `tools/cef-container/stage-local.py`
with the rebuilt archive, `--manifest` pointing to that JSON, `--source` pointing
to the chosen application checkout, and a new `--output` directory. The helper
records archive/library hashes and stages matching resources under `cef/` plus a
local `manifest.json`. It changes no installed application or production manifest;
the stage is x64 only and does not assert that a patch or acceleration works.

Run the producer with that directory available inside the sandbox and both
`--env=LD_LIBRARY_PATH=/path/to/stage/cef` and
`--env=KARERE_FRAME_PROBE_CEF_DIR=/path/to/stage/cef` before the app ID. The latter
selects the matching CEF resources/locales; the API hash check still applies.
After producer validation, the staged manifest can build the separate Devel
bundle with `flatpak-builder --arch=x86_64`. ARM archive sources remain unchanged
and unvalidated. Use fresh labels for each control and retain original hashes.

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
`--cef-directory /path/to/stage/cef` selects an isolated rebuilt engine and records
its library hash. `--transfer accelerated` requests producer GPU callbacks;
the default remains CPU transfer for comparison with the original media series.
Neither producer path presents a GTK window, so neither establishes display FPS.
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

The pinned Chromium Linux encoder factory selects VA-API or V4L2, with no NVENC
backend. This NVIDIA VA-API driver exposes no encoder entrypoints, so the current
CEF/NVIDIA combination cannot gain hardware encoding from the feature flag alone.
Adding a supported encoder requires engine work beyond enabling a switch; the
working FFmpeg NVENC control is not substituted for CEF/WebRTC evidence. See the
[pinned encoder factory](https://chromium.googlesource.com/chromium/src/+/79460ebecaa5625e57a5fb679a735659e73dc687/media/gpu/gpu_video_encode_accelerator_factory.cc).

PipeWire desktop capture is already enabled in the pinned engine. Its WebRTC
DMA-BUF import path ends in `GlReadPixels` into CPU storage, so DMA-BUF negotiation
alone does not establish capture-to-encoder transport without readback. That
transport has not been replaced or validated in this change; no desktop capture
or real call was initiated by these generated tests. See the
[matching WebRTC implementation](https://webrtc.googlesource.com/src/+/6f37672d358475cd17544121a12494da454d85fb/modules/desktop_capture/linux/wayland/egl_dmabuf.cc).

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

The existing `karere_probe.py` runner exposes these independent controls as
`--gsk-renderer`, `--cef-graphics`, `--cpu-presenter` and `--frame-transfer` and
records their requested values. `--cef-directory` loads a staged matching engine,
including resources/locales, and records its library SHA-256. Combined with
`--binary`, this compares application and engine changes in the same Flatpak
runtime without replacing installed packages. Backend logs and CDP device data
must still confirm the effective path; the recorded options alone do not.

Both GPU transfer paths copy borrowed CEF resources before callback return.
The Vulkan pool is bounded to three images and waits for GTK release plus GPU
completion before reuse. GL bounds retained owned textures to three and uses a
finite completion fence; it does not reuse texture storage still held by GTK.
The old deferred borrowed-DMA-BUF draw path was removed. An uncertain completion
terminates the worker without unwinding in-flight resources; a lightweight parent
restarts once with CPU transfer and retains attempted CEF backends. No saved
preference changes. A five-second operation watchdog sleeps while idle.
The visible-startup watchdog counts only main-view frames; popup-only rendering
cannot make a blank chat view appear healthy. Popup frames still advance the
presentation content serial when their pixels change.
Kernel/driver hangs that prevent process teardown remain a physical validation
limit; fault-injection tests are not proof of recovery from every driver failure.

The tested GTK 4.24 renderer retains imported texture references through its GPU
frame fence, then releases the shader operations holding those references. The
pool therefore waits for GTK's final texture release as well as completing its
own copy before reuse. This is distinct from merely duplicating a borrowed CEF
descriptor. See GTK's [texture retention](https://github.com/GNOME/gtk/blob/4.24.0/gsk/gpu/gskgpuimage.c),
[Vulkan frame cleanup](https://github.com/GNOME/gtk/blob/4.24.0/gsk/gpu/gskvulkanframe.c)
and [GL frame cleanup](https://github.com/GNOME/gtk/blob/4.24.0/gsk/gpu/gskglframe.c).
GTK itself still has indefinite driver waits; the application operation watchdog
does not establish recovery from a hang wholly inside GTK's renderer. That device
loss coverage remains outstanding.

`presentation_report.py` reuses #193's captured epoch/monotonic clock pairs to
select compositor timestamps inside each sample, instead of using delayed log
arrival. Multiple feedback objects for one commit count as one presentation.
Acceptance requires an unambiguous content serial, hardware completion/timing
and vsync feedback, a stable captured monotonic clock, and at least 15 seconds.
Missing clock evidence in legacy captures leaves their rates diagnostic and their
acceptance unverified; saved historical reports are not rewritten. The separate
`accelerated_sample_accepted` field also requires accelerated callbacks without
CPU callbacks during the sample. A fast CPU recovery does not verify the normal
GPU path. A complete matrix still requires all three repeats and every requested
workload/window geometry; a single passing sample does not complete that matrix.
