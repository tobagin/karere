# Conversation scrolling after v4.3.2

Measured on 28 September 2026, after [PR #191](https://github.com/tobagin/karere/pull/191). That change improved the chat list. These measurements target the separate conversation pane.

Two costs explain the remaining slow conversation scrolling on this notebook:

1. **The large decorative SVG mask behind the conversation makes Chromium's raster work expensive during scrolling.** Applying only `will-change: transform` to that background element restores page animation from about 24 to 60 callbacks/s. Reverting that one property consistently restores the slowdown, on both frame-transfer paths.
2. **CPU-buffer frame delivery still misses some refreshes at this viewport and scale.** With the background hint applied, CPU delivery presents about 55–56 frames/s. Verified AMD DMA-BUF delivery presents about 60, with no presentation intervals above 25 ms in either repeated 20-second sample. The CPU path copies about 22.6 MB per damaged conversation frame and then uploads it to an OpenGL texture.

This identifies a page/rendering interaction and a separate frame-transfer cost. It does not identify an upstream Chromium commit responsible for the mask behavior or prove the same behavior in another browser. The production change and its validation are described below; the original diagnostic data remains labeled v4.3.2.

## Controlled results

Each row contains two 20-second active samples. Within each frame-transfer path the order was original → hint → hint → original, with three seconds settling and a five-second scrolling warmup before each sample. The trajectory moved through the same 900 CSS-pixel range at 300 CSS px/s, without loading history during the active interval.

| Conversation background | Frame transfer | Page rAF callbacks/s | GTK draw callbacks/s | p95 window presentation interval | Presentation gaps >25 ms per sample |
| --- | --- | ---: | ---: | ---: | ---: |
| Original | CPU buffer | 22.00 / 24.85 | 22.00 / 24.90 | 66.77 / 50.08 ms | 435 / 494 |
| `will-change: transform` | CPU buffer | 59.90 / 60.00 | 55.35 / 55.65 | 33.38 / 33.38 ms | 89 / 85 |
| Original | AMD DMA-BUF | 24.70 / 24.25 | 24.55 / 24.20 | 50.08 / 50.08 ms | 487 / 479 |
| `will-change: transform` | AMD DMA-BUF | 59.95 / 60.00 | 59.90 / 59.95 | 16.69 / 16.69 ms | 0 / 0 |

The original CPU first sample was slower than the repeat. The large improvement and its reversal occur in both samples and both paths. The GPU comparisons also reproduced without the additional Rust stage instrumentation: two hint samples delivered 59.9 draw callbacks/s; reverting delivered 25.75 and 25.45.

Page rAF callbacks measure page update opportunities. Draw callbacks measure application work. Presentation intervals come separately from completed GDK frame timings with nonzero presentation timestamps. Those describe window presentation and can include native UI or repeated content; they are not an optical measurement of distinct conversation frames or input-to-screen latency.

Before these changes, three installed-release CPU samples produced 23.5–24.0 conversation rAF callbacks/s, versus 59.65–59.95 for the chat list. GPU frame transfer alone left the conversation at 25.35–25.70. With the hint applied, the chat-list control remained at 59.9 page callbacks/s and 57.45 draw callbacks/s on the CPU path.

## Evidence for the background cost

The identified element was an empty, absolutely positioned decorative DIV inside `#main`, covering approximately 1351 × 1173 CSS pixels. Its repeating SVG `mask-image` had a mask size of 412.5 × 749.25 CSS pixels and opacity 0.6. No message text, image URL, mask SVG contents or account identifier was extracted.

The experiment changed exactly this inline property:

```js
wallpaper.style.setProperty('will-change', 'transform', 'important');
```

It retained the mask and its appearance, messages, dimensions, zoom, scroll trajectory and frame-transfer setting. The probe saves the original property and priority and restores them on reversal. No scrolling handler or CEF scheduling behavior was changed.

In a separate 25-second original-background Chromium trace, `CrGpuMain` accumulated 24,098 ms in `RendererRasterWorker`, including 12,595 ms in raster-command deserialization and 10,464 ms in raster flush. These are **inclusive elapsed trace durations**, contain nested events, and must not be added as independent CPU times. The GPU process consumed approximately one CPU core even with accelerated frame transfer. After the hint, GPU-process CPU fell from 108.8% to 31.9–32.7% in the instrumented GPU comparison; 100% denotes one core, not total machine utilization.

Together the trace and repeated one-property reversals implicate expensive raster work associated with the large mask. The likely mechanism is changed compositing/caching of the static decoration. The precise layer invalidation mechanism has not been independently traced or bisected. `will-change` is a rendering hint, and keeping layers alive can consume resources; it should be scoped to the identified decoration, as discussed in the [CSS Will Change specification](https://www.w3.org/TR/css-will-change/).

No ≥50 ms page Long Tasks occurred in the matched automatic conversation samples. That alone would not exclude shorter script or layout costs; the raster trace and reversals supply the stronger evidence. The old recurring 100 ms active-scroll pump stall was not the dominant pattern in these runs.

## Evidence for the remaining CPU transfer cost

With the background hint applied:

| Stage | CPU sample 1: median / p95 | CPU sample 2: median / p95 |
| --- | ---: | ---: |
| `on_paint`, including lock/copy/queue work | 6.445 / 8.851 ms | 6.409 / 9.032 ms |
| Texture subimage upload span | 3.347 / 4.564 ms | 3.464 / 4.746 ms |
| Complete GTK GL render callback | 3.404 / 4.618 ms | 3.519 / 4.796 ms |

The upload span is contained in the render callback and must not be added to it. The paint span is not a measurement of `memcpy` alone. Median damaged area represents 22,591,164 bytes per paint; p95 is 22,601,968. That is about 1.35 GB/s of copied pixel payload at 60 paints/s, before texture upload; this is a calculated payload rate, not measured memory-bus bandwidth.

Page production reached 60 callbacks/s while window presentation missed roughly 85–89 refresh opportunities over 20 seconds. With DMA-BUF import, the render callback p95 was 0.556–0.610 ms and presentation p95 was 16.692 ms, with no >25 ms gaps. Main-process CPU fell from 83.9–84.9% to 21.2–22.5%. This supports frame-transfer overhead as the secondary limit. It does not separate memory bandwidth, lock contention and driver synchronization into independently proven causes.

GPU Rendering here changes **frame transfer**. The CPU-buffer path still uses Chromium graphics processing and a GTK OpenGL surface; “CPU buffer” does not mean every graphics operation is software-rendered.

## Workload, input and limits

- The measured real conversation was mixed content, with 32 image elements in the final matched range. No text-only WhatsApp conclusion is claimed. The final CPU/GPU series used the same 8303 px loaded height and a 3452.5 px bottom anchor; none of their active samples changed the scroll height. Earlier exploratory runs had different loaded heights and are not treated as an exact cross-path content match.
- A generated, conversation-sized text page with its own simpler SVG mask reached 60 page callbacks/s both before and after the hint. It reproduced the roughly 55 draw callbacks/s CPU-delivery ceiling, **but did not reproduce the WhatsApp mask slowdown**. It is a control, not an exact standalone regression reproducer.
- Browser-injected wheel input with the hint produced 160 trusted page wheel events in 20 seconds, approximately 60 rAF callbacks/s and no Long Tasks. It bypasses GTK. At DPR 2 the requested 37.5-unit CDP deltas arrived as 18.75 CSS-pixel wheel deltas; measured movement was about 18.5–19 px per event. Do not equate those command deltas with physical-device distance or the programmatic trajectory.
- That wheel run moved on 160 frames, reflecting the release's disabled smooth-scrolling animation. The `disable-smooth-scrolling` switch was retained throughout; removing it is not an established fix or cause from this investigation.
- A separate physical-input sample on the generated page contained 164 surface-unit GTK events. GTK handler-to-CEF forwarding had median 0.025 ms and p95 0.051 ms. This control does not establish physical-input behavior under the original real conversation workload; only the final real-conversation check below supplies that subjective confirmation.
- Two short visible-idle CPU samples were approximately 7.84% with the hint and 6.0% without it. They are not a battery or idle-regression study. Background behavior and layer-memory costs still require validation for a production change.
- The user's regular browser was previously reported smooth. No fresh matched browser/engine comparison was performed. These results do not assign the mask behavior specifically to CEF, Chromium, GNOME 51 or the graphics driver, nor validate NVIDIA or other machines.

In the final check, the real conversation remained open throughout with the hint
and the original GPU preference (off). The user reported **“Smooth and
responsive”** after being asked to scroll and try Preferences. The recording
contains 279 GTK events and 279 CEF forwards over 8.93 seconds of native input,
plus 148 page wheel events. All GTK events used surface units, so separate
notch-wheel coverage is not established. GTK handler-to-CEF forwarding had a
median of 0.023 ms, p95 of 0.060 ms and maximum of 0.133 ms. These are forwarding
costs, not input-to-screen latency. The longer observation also included idle
time and changes in loaded history, so it is excluded from the matched
steady-scroll table.

## Exact environment and diagnostic changes

- Karere Flathub stable **4.3.2**, deployment `534efc4dd04bbba5248e5927fde32b7da09af1e0bc25e2795eee3f47a62b915a`.
- Shipped source **v4.3.2**, commit `11dfc0e62d7279e5dd3b2e97dc096e521845dedb`. The manifest's `952a12f3412cd35afa1606514f7d967a5b5447aa` identifies the annotated tag object, not the source commit.
- CEF **152.0.6+g708dc14**, Chromium **152.0.7977.83**. Bundled archive SHA-256 `3d64f6fac8911a368ffa415e3ee53d002cbd792866f08ed6578dae1e252cbac8`.
- Installed runtime **GNOME Platform 51**. Diagnostic Rust binary compiled offline using the available GNOME SDK 50, then run with the installed application's runtime/resources/CEF. Binary SHA-256 `c80aa59a6d0202188a8d762b0b09e2a7509bd1f700ade65dd51944db860070e5`.
- KDE Wayland, physical display **3840 × 2400 at 60 Hz**, desktop scale **1.9**. Page **2021 × 1173 CSS pixels**, **DPR 2**; CEF backing buffers **4042 × 2346**. Conversation scroller **1350.703 × 1045 CSS pixels**, client width **1346**.
- Verified accelerated renderer **AMD Radeon 680M / radeonsi rembrandt**, Mesa **26.2.2**, ANGLE OpenGL ES 3.2, GPU rasterization/compositing enabled and DMA-BUF import observed. Merely opening both AMD/NVIDIA device nodes was not treated as proof of the active GPU.
- Battery power and balanced platform profile during checked samples. Stored GPU Rendering **off**, Reduce Motion **off**, zoom **1.0**, maximized. GPU trials used only a launch environment override; saved GPU preference was never changed.

The diagnostic binary included opt-in Rust timestamps for paint, upload, render, GDK presentation and GTK/CEF wheel forwarding. [stage-timing.patch](stage-timing.patch) preserves that instrumentation for a disposable diagnostic build; it is not part of the production application. The trace buffers numeric data in memory and flushes at clean exit. It does not record chat text, contacts, cookies or screenshots. CDP is loopback-only and temporary; normal real-page security flags are retained. Chromium trace arguments are discarded before storage. Raw captures and compiled artifacts remain ignored.

The same diagnostic executable was used in both final CPU/GPU series. Installed-executable measurements reproduced the initial slowdown and the GPU wallpaper improvement. This checks that the phenomenon predates the added Rust instrumentation, although no uninstrumented optical presentation baseline was collected.

The diagnostic build completed with `cargo build --release --locked --offline` and `cargo fmt --all`; Python probes passed syntax compilation. Runtime checks exercised both transfer paths, three initial repeats per main baseline, reversible background comparisons, the chat list and generated controls. This is diagnostic validation, not a production-fix acceptance suite.

## Production fix

[80-conversation-wallpaper.js](../../data/js/80-conversation-wallpaper.js) installs
one stylesheet through the existing renderer bundle after DOM readiness. The
selector matches only an empty direct DIV child of `#main` with an inline mask
URL, as used by the measured WhatsApp decoration. It applies
`will-change: transform !important` without changing the mask or other visual
properties. CSS automatically follows chat and background replacement; no
polling or mutation observer is added. The ID guard makes reinjection
idempotent. Missing or changed markup is a no-op. Nested message/media masks,
nonempty elements and unmasked custom backgrounds do not receive the hint.

Keep the CPU transfer optimization separate. On this tested AMD system, supported GPU Rendering plus the background fix removes the measured residual presentation gaps. Accelerated rendering alone does not fix the mask bottleneck. A general release must retain its compatibility checks and CPU fallback. If improving CPU delivery, measure copy, upload and synchronization separately rather than changing CEF pump intervals again on this evidence.

The production validation uses a clean build from upstream `fb0179f` (v4.3.3
packaging head) inside the same installed v4.3.2 Flatpak runtime. The production
Rust code, CEF flags and saved rendering preferences are unchanged. Fresh
compiled-stylesheet comparisons and generated DOM checks are recorded in the
validation section below. The generated page cannot serve as a performance
regression test for the dominant mask issue.

## Evidence files

- [Aggregate measurements](measurements/conversation_summary.json): shareable timing/geometry summary, with no message/account content.
- [Production validation](measurements/wallpaper_fix_validation.json): eight fresh stylesheet comparisons and 16 browser DOM checks.
- [Conversation probe](conversation_probe.js), [controller](conversation_probe.py), [reversible wallpaper probe](wallpaper_probe.js), [comparison driver](wallpaper_comparison.py), [trace collector](trace_conversation.py), [analyzer](summarize_conversation.py).
- Local ignored captures: `results/conversation_cpu_discovery.jsonl`, `conversation_gpu_comparison.jsonl`, `conversation_cpu_stages.jsonl`, `conversation_gpu_stages.jsonl`, corresponding `*_stages.json`, and `cpu_stage_*_page.json` / `gpu_stage_*_page.json`.
- Local stripped Chromium trace: `results/gpu_conversation_trace_trace.json`. Nested trace durations should be inspected with the cautions above.

The initial diagnostic instance exited cleanly and the official installed
application was restored. GPU Rendering remained off, zoom 1.0, Reduce Motion
off and the window maximized. Testing a compiled binary does not replace the
installed Flatpak or install persistent overrides.

## Production validation on current main

The compiled production executable had SHA-256
`c167ed65496eb85bfb1bfe3200502447b6e5dc39253b9cc1f925bd38c582b474`.
Unlike the original diagnostic build, it contains **no Rust timing hooks**.
The CEF pump/log collector and page probe remain external diagnostic tools.
The stylesheet installed automatically on the real page and matched exactly
one element, the identified empty direct child of `#main`.

Fresh original → fixed → fixed → original comparisons toggled only the compiled
stylesheet's `disabled` state, then restored it. Each used five seconds warmup
and twenty seconds active scrolling after three seconds settling. Both paths
retained the same 2021 × 1173 CSS viewport, DPR 2, 8303 px loaded height,
900 px trajectory and 3452.5 px bottom anchor. There was no loaded-history growth
or ≥50 ms page Long Task during any of the eight active intervals. Build/check
processes had finished before the performance measurements.

These eight production measurements used the explicit `--production` comparison
path, which already checked computed off/on conditions. The later review found
that the default inline-hint path could not disable a compiled stylesheet; that
does not describe the path used for this table. The default now detects the
stylesheet automatically and uses the same checked conditions, with additional
target pinning and restoration checks. Historical aggregates remain unchanged.
Fresh diagnostic-tool regression results are recorded separately in
[REVIEW_VALIDATION.md](REVIEW_VALIDATION.md).

| Compiled stylesheet | Frame transfer | Page callbacks/s (two samples) | GTK draw callbacks/s (two samples) |
| --- | --- | ---: | ---: |
| Disabled | CPU buffer | 24.75 / 25.15 | 24.65 / 25.15 |
| Enabled | CPU buffer | 60.00 / 60.00 | 56.10 / 55.90 |
| Disabled | AMD DMA-BUF | 26.10 / 25.80 | 26.00 / 25.65 |
| Enabled | AMD DMA-BUF | 60.00 / 60.00 | 59.90 / 59.85 |

Presentation feedback was not instrumented in this production executable; the
earlier presentation measurements remain separately labeled above. The active
accelerated renderer was again verified as AMD/radeonsi, with successful DMA-BUF
import. No NVIDIA or high-refresh-rate result is inferred from this notebook's
60 Hz measurements.

All 16 [generated browser checks](validate_wallpaper.py) passed: the hint affects
the decoration while leaving the scroller, messages and nested masked media
alone; it follows replacement chats/backgrounds, disappears when the mask is
removed, retains the original mask and geometry, works at narrow widths and
survives repeated injection without adding duplicate stylesheets. These checks
evaluate the actual production script in CEF. The existing identity hook's
synchronous localStorage access throws on opaque `data:` origins before later
bundle scripts execute, so the fixture evaluates the wallpaper component
directly. Automatic bundle installation was checked on the real WhatsApp page.
This fixture does not claim coverage of every actual WhatsApp theme or a
pixel-by-pixel appearance comparison.

The release build, Rust formatting, strict all-target Clippy, all-target
`cargo check`, existing JavaScript copy-bridge checks and Python syntax checks
passed using the available Rust 1.98.1 / Node 24.21.0 SDK. The original evidence
and fresh production samples remain distinct in the aggregate files.

All 96 Rust unit tests and the graphics-policy integration test passed. The
existing `real_binary_starts_with_software_gles_for_visible_and_prewarmed_windows`
test failed waiting for `GLArea realize error`: the current application emits
`no GL context ... presenting frames in software` instead. The same failure was
reproduced on **unmodified upstream `fb0179f`** with the identical fixture. The
first attempt lacked a display; the comparison then used the available X11
display, software-GLES overrides and a private D-Bus session because this SDK
does not contain Xvfb. The wallpaper PR does not alter that Rust startup path or
change the existing test to conceal the failure.
