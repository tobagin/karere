# Default-on acceleration diagnostics

GPU frame transfer now defaults on, including NVIDIA, while explicit saved
opt-outs and `KARERE_GPU_OSR=0` remain effective. A preference or visible chat is
not proof of accelerated delivery. The original pinned NVIDIA CEF delivered
zero accelerated callbacks in the standalone producer fixture. The rebuilt
native-handle CEF/Chromium backport now delivers **3,600 accelerated callbacks in
15 seconds** with both hardware ANGLE GL/EGL and ANGLE Vulkan on native Wayland.
The [verified producer controls](measurements/native_cef_producer_433.json) check
the loaded library, matching API, backend and orderly shutdown. Neither GPU run
delivered CPU callbacks; the CPU control delivered 3,601 callbacks. These small
generated producer fixtures establish frame production, not display FPS.
The [original-engine backend controls](measurements/original_cef_producer_433.json)
reproduce its failure with both hardware ANGLE GL/EGL and ANGLE Vulkan. Vulkan CPU transfer
does produce frames. These generated checks ran during compilation and establish
the failure/recovery behavior, not a presentation rate or performance comparison.

The normal accelerated path must meet the monitor rate. At 240 Hz every active
sample must deliver at least 235 fresh presentations/s, median near 4.17 ms and
p95 at most 8.33 ms. CPU transfer is measured recovery without the same FPS gate.
The installed combined bundle independently passes all six generated repeats at
237.067–237.800 fresh presentations/s with p95 4.185–4.221 ms and actual GPU
transfer. No graphics backend overrides were supplied; the isolated schema
requests the same true value as the installed default. Fresh-default and explicit
opt-out semantics have separate regression coverage. All 53 #193 browser checks
pass on this bundle. [Installed samples and hashes](measurements/native_installed_generated_433.json).
All eighteen real chat-list/text/media samples also pass; the installed recovery
successor passes six further normal-path generated repeats. Matched generated
stock/#193 controls remain near 60 page updates/s. Public engine release/pins,
matched real-conversation controls and wider hardware coverage remain pending.
The older tables in
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
  -Wl,--enable-new-dtags,-rpath,/app/lib/cef -lcef -o /path/to/probes/cef-frame-probe
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

The optional `--port` selects an unused loopback CDP port (default 9333) so the
generated fixture can leave an already-open diagnostic Devel window intact.
Every connection and cleanup check uses that exact port; the transport still
rejects remote hosts and other ports. Run performance workloads sequentially,
even when listeners coexist. `KARERE_PROBE_CDP=1` on the Karere capture runner
now automatically loads its matching schedule interposer; it cannot silently
request CDP without enabling the port. The 47 existing combined diagnostic
regressions still pass, with a separate added check rejecting wheel input during
programmatic acceptance samples, and a live two-listener isolation check passed.


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

Native Wayland remains preferred. GSK now tries GL first unless explicitly
overridden. The repaired-engine generated comparison measured 237.0–237.9 fresh
presentations/s across all six windowed/maximized GL repeats, with p95 at most
4.23 ms ([all path comparisons](measurements/native_presenter_screen_433.json)).
This selects a measured default on the RTX 5090, not a universal GPU
ranking or completed conversation acceptance. GTK retains initialization
fallback and the supervisor bounds stalled-frame recovery. Explicit
`GSK_RENDERER=gl`/`vulkan` controls isolate rendering. `KARERE_CEF_GRAPHICS` selects
CEF `gl`, `vulkan` or `software` for diagnostics. `KARERE_CPU_PRESENTER` (`gl` or
`snapshot`) and `KARERE_FRAME_TRANSFER` (`gl` or `vulkan`) independently select
presentation/transfer trials. Full conversation acceptance remains incomplete.

The existing `karere_probe.py` runner exposes these independent controls as
`--gsk-renderer`, `--cef-graphics`, `--cpu-presenter` and `--frame-transfer` and
records their requested values. `--cef-directory` loads a staged matching engine,
including resources/locales, and records its library SHA-256. Combined with
`--binary`, this compares application and engine changes in the same Flatpak
runtime without replacing installed packages. Backend logs and CDP device data
must still confirm the effective path; the recorded options alone do not.
The first post-build engine controls were excluded because the standalone
probe's `DT_RPATH` still loaded installed CEF. Corrected controls use `DT_RUNPATH`
and require observed process mappings to match the requested engine. Historical
zero-frame results from original CEF remain valid; mislabeled engine trials are
not evidence against the native-handle repair.

The runners also check `/proc` mappings for the engine actually loaded. Compile
the standalone producer with `--enable-new-dtags`: a legacy ELF `DT_RPATH` can
override `LD_LIBRARY_PATH` and silently load the installed engine. A requested
library hash alone is not evidence that the process used that library.

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
GTK itself still has indefinite driver waits. A separate five-second deadline
now brackets its actual before/after-paint phases, so a synchronous stall during
drawing terminates the worker without unwinding GPU resources. The supervisor
tries the other hardware GTK renderer once, then Cairo, preserving CEF's working
backend and accelerated-transfer preference. A renderer which ignores the
recovery override cannot create a restart loop. No deadline runs between frames;
unrealizing a view disconnects its clock handlers. Tests cover bounded restart,
layer isolation and real GL/Vulkan frame-phase cleanup. Physical device loss,
hangs during initial GTK display/renderer creation, and a kernel refusing process
teardown remain unverified; these fixtures do not simulate a failing driver.

The hardware fixture also circulates generated GPU textures through a visible
GTK window for two seconds per transfer path. Both GL and Vulkan copies passed
with GSK GL and GSK Vulkan, without exhausting the three-buffer bound. This tests
actual GTK release/reuse beyond manually dropping references. Run the serialized
`gles_contract_is_shared_by_main_and_devtools_views` test separately with
`GSK_RENDERER=gl` and `GSK_RENDERER=vulkan` on the hardware display. Its small
generated texture and concurrent-build timing are not performance evidence.

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

## Combined conversation diagnostics

The integration checkout applies the optional
[amplitude argument patch](diagnostic-patches/pr193-scroll-amplitude.patch) to
#193's existing Python runner. Its JavaScript already supports the parameter;
no sampler or wallpaper code is duplicated. The default remains 900 CSS pixels.
The tested maximized chat list has 1,265 pixels of loaded scroll range, less than
that probe's 900 + 400 guard. `--amplitude 800` fits without loading data or
relaxing frame-rate acceptance. Geometry, amplitude and range are recorded, and
each sample restores its original position. Apply the patch after #193 until
that branch merges; the production feature branch remains independent.

The repaired installed bundle now passes all 24 requested scrolling samples:
three per workload and window size for generated animation, chat list, long text
and substantial-media conversations. Fresh presentations range from 237.067 to
239.467/s, with p95 below 4.23 ms. See the numeric
[generated](measurements/native_installed_generated_433.json) and
[conversation](measurements/native_conversations_433.json) evidence. This does
not complete the separate matched-control, recovery and media validation work.

`--x11-recovery` exposes X11 only for an isolated generated launch; it installs
no persistent overrides. Background diagnostics derive the D-Bus window path
from `--app-id`, including Devel's suffix, instead of addressing the stable
application's window path. The local Devel idle check exposed that path error.

Repaired-engine X11 controls on the tested NVIDIA driver distinguish the CEF
choices: GL/EGL crashes, native GL produces CPU frames but no accelerated frames,
and Vulkan produces accelerated frames. A complete X11 generated window renders
with CEF Vulkan, GSK GL and owned GL textures. The default CEF selection follows
GTK's actual working display: GL/EGL on Wayland, Vulkan on X11, then native GL
for X11 recovery. GTK's backend remains independent. The supervisor records
failed CEF choices reported by the worker, so a display-dependent default cannot
cause it to skip an untried backend or loop. X11 compositor presentation FPS and
other GPU vendors remain unverified.

The [repaired-engine media series](measurements/native_media_433.json) completes
three 30-second H.264 samples per condition at 720p30 and 1080p60. At 1080p60,
verified VA-API decoding reduces playback process CPU from 26.77% to 11.59% and
local loopback CPU from 36.00% to 29.34%, sustaining source cadence. Sampled
software/hardware output grids match at both resolutions. Encoding remains
software OpenH264, including `AcceleratedVideoEncoder` trials; no NVIDIA CEF
hardware-encoding claim or default is introduced. The 720p resource comparison
is provisional because a separate linked Devel window was open. Other codecs
and real capture/calls remain unverified.

## Final recovery and baseline controls

The [recovery successor](measurements/native_final_generated_433.json), source
`5e26584`, passes six default-path repeats at 237.400–237.733 fresh presentations/s,
with p95 at most 4.224 ms. Its normal Wayland rendering path is unchanged from
the complete 24-sample bundle. Matched generated idle is 0.33% visible and 0.30%
hidden, versus stock/#193's 0.37% and 0.27% (100% represents one CPU core).

The [twelve stock/#193 controls](measurements/native_controls_433.json) retain
their original binaries and CEF. Page/CEF rates remain near 60/s; their missing
content serials leave fresh presentation unverified. This fixture has no masked
wallpaper, so it does not measure #193's conversation-specific optimization.

[Automatic recovery](measurements/native_recovery_433.json) passes X11 Vulkan,
X11 native-GL/CPU after missing Vulkan and absent frames, Wayland acceleration
without Vulkan, and `KARERE_GPU_OSR=0`. The missing-frame retry occurs after ten
seconds without changing the saved preference. Normal `flatpak run` activation
restores the same browser after twelve seconds hidden. Missing Wayland produces
the documented one-launch X11 command without installing persistent overrides.
The measured CPU recovery is 175.4 fresh presentations/s; it has no 235 FPS gate.

The public manifests still require the repaired CEF release artifact and new
pins before merge. The local Devel bundle contains that repair. Hardware CEF
encoding on NVIDIA, physical device loss, other GPUs/codecs, physical 60/120 Hz
modes and matched current real-conversation baseline controls are not claimed
validated. Historical records keep their original artifact/display provenance.

The repaired engine also passes actual [WebGL2 output and WebGPU compute](measurements/native_graphics_433.json)
on the non-fallback NVIDIA adapter. Normal Devel launch and original window
geometry were restored; temporary CDP listeners were closed and the display
remained 4K/240 Hz, 145%, adaptive sync Never. Stable account data was untouched.
