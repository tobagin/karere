# Karere 4.3.3: adaptive refresh and Vulkan experiment

This records the prototype at `29342bc`, before the default-on follow-up.
For the current defaults, producer repair and media diagnostics, see
[ACCELERATION.md](ACCELERATION.md). Historical measurements retain their original
builds, settings and limitations.

**Lower ceiling measured. The implementation is experimental and the performance
work remains incomplete. No workload verified 240 fresh content presentations/s.**
The new Vulkan snapshot path currently presents less frequently than the old GL
bridge. It must not be treated as a release-ready performance fix.

## Implementation and provenance

The feature branch starts at upstream 4.3.3,
`fb0179f94c5bdb25aafca79b7661fec8f169bcaa`, retaining the merged #191 scheduler.
It does not contain or recreate #193's wallpaper stylesheet or conversation
tooling. The separately tested integration checkout includes
[PR #193](https://github.com/tobagin/karere/pull/193) at
`7cc4615cbfe1707c3a7cd4f424d8d4a7239dba3c`, rechecked open and unmerged before
delivery. A three-way Git merge check succeeds against that revision; the merged
probe retains its options and all 33 #193 regression tests pass. Rebase the
independent feature branch after #193 merges.

Implemented in the experimental branch:

- Choose GTK's working display before CEF initialization and match CEF Ozone to
  it. Prefer Wayland, retaining Flatpak `fallback-x11` and one-launch recovery.
- Remove manifest-forced OpenGL. Use GTK snapshot textures on the default
  Vulkan renderer, retaining the legacy GL presenter and software textures.
- Prefer ANGLE Vulkan independently of GTK. Missing initial frames trigger
  bounded CEF backend attempts; accelerated-transfer failure first falls back
  to CPU transfer while retaining working Vulkan presentation and rendering.
- Derive CEF's rate from the containing monitor's configured millihertz, round
  to whole FPS, and use 60 only when unavailable. Update on activation, monitor
  changes, hotplug and refresh notification. There is no 240 FPS ceiling.
- Copy CPU damage into immutable, owned tile storage, preserving sampling at
  fractional scales. Copy accelerated resources into application-owned Vulkan
  storage inside the callback, with queue ownership barriers, completion fences,
  and at most three retained output buffers. Keep a GL copy fallback.
- Restore NVIDIA GPU Rendering opt-in, retaining default-off and
  `KARERE_GPU_OSR=0`. Enable CEF animated wheel scrolling when Reduce Motion is
  off, using the existing restart flow for a change. Precise input paths remain.

GTK 4.16 is required: this includes the
[DMA-BUF builder introduced in 4.14](https://docs.gtk.org/gdk4/class.DmabufTextureBuilder.html)
and memory-texture/color APIs. The
[CEF callback contract](https://github.com/chromiumembedded/cef/blob/708dc14/include/cef_render_handler.h)
requires the resource copy to finish before the accelerated callback returns;
duplicated producer FDs are not used as long-lived frame storage. Owned image
storage and FDs remain alive until GDK releases the exported texture.

## Devel integration build

The independent feature branch is based on 4.3.3. The tested Devel integration
also includes #193 at the pinned revision above. Keep the wallpaper change in
its original PR; rebase this branch when that merges. Merge those two branches
in a separate integration checkout before building:

```sh
flatpak-builder --user --install --force-clean --repo=repo \
  build-dir packaging/io.github.tobagin.karere.Devel.yml
flatpak build-bundle --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
  repo karere-vulkan-adaptive-devel.flatpak io.github.tobagin.karere.Devel master
```

The resulting app ID and profile are separate from stable Karere. Stable 4.3.2
was not replaced during the measurements. The repeated performance samples used combined executable
SHA-256 `d9d844657bd1ffa756d06e9af897701f5232430ad32322535b8ea65eb49862b0`.
The local bundle and raw diagnostics are excluded from this repository.

After quitting Devel, use one-launch recovery for the failing layer:

```sh
flatpak run --env=KARERE_GPU_OSR=0 io.github.tobagin.karere.Devel
flatpak run --env=GSK_RENDERER=gl io.github.tobagin.karere.Devel
flatpak run --env=KARERE_CEF_GRAPHICS=gl io.github.tobagin.karere.Devel
flatpak run --nosocket=wayland --nosocket=fallback-x11 --socket=x11 \
  --env=GDK_BACKEND=x11 io.github.tobagin.karere.Devel
```

These do not install persistent overrides. `KARERE_GPU_OSR=0` disables shared
frame transfer while leaving the working Vulkan backends in place. The X11
command accounts for the sandbox's
[Wayland/fallback-x11 distinction](https://docs.flatpak.org/en/latest/sandbox-permissions.html).

## Backend evidence and GPU opt-in result

KDE Wayland, RTX 5090, NVIDIA 615.71.09, matching Flatpak driver extension,
GNOME 51, CEF 152.0.6+g708dc14 / Chromium 152.0.7977.83. DP-1 remained
3840×2160 at 240 Hz and 145% scale. The user changed adaptive sync from Automatic
to **Never** during testing; subsequent real-page samples are labeled Never.
The agent made no display setting changes.

Actual GDK diagnostics selected RTX 5090 and `GskVulkanRenderer`. CEF's CDP
SystemInfo reported RTX 5090, `ANGLE_VULKAN`, and
`gl=egl-angle,angle=vulkan`; these are stronger evidence than startup switches.
Chromium's separate native-Vulkan feature status must not be confused with its
ANGLE renderer. The deployed manifest has no `GSK_RENDERER` override.

The user reported: **the chat area appeared when launched with GPU Rendering
enabled**, and a long text conversation opened. Instrumentation nevertheless
recorded **zero accelerated paint callbacks**. After ten seconds without a
shared-texture frame, the app recreated its browsers with CPU transfer. GTK and
CEF retained Vulkan; CEF logged requested/effective 240 FPS. The setting remains
enabled, so the visual report does not verify NVIDIA accelerated delivery.

The generated GPU fixture reproduced that recovery and retained its custom URL.
A real Vulkan producer/copy test verified ownership independently, but it does
not establish that CEF can supply frames on this NVIDIA stack.

## Measurements

Each active group contains three 15-second samples after five seconds of
warmup. CPU percentages sum the process tree; 100% is one core. Tables show
the range across repetitions. Each accepted real-page sample had zero history
height changes, and the existing #193 probe restored its starting scroll offset.

Stock and #193-only measurements below used adaptive sync Automatic and the
old GL presenter. Their compositor results describe **window presentations**;
they do not identify distinct web frames. Native UI updates can exceed page FPS.

| Build and workload | Page FPS | Window presentations/s | Median interval ms | p95 ms | CPU % |
|---|---:|---:|---:|---:|---:|
| Stock synthetic, windowed | 60.00 | 59.87–59.93 | 16.66–16.67 | 16.76–20.85 | 38.0–47.7 |
| Stock synthetic, maximized | 59.93 | 59.60–59.67 | 16.67 | 16.72–20.83 | 59.5–61.7 |
| Stock text conversation, windowed | 40.73–41.13 | 40.13–40.47 | 25.00 | 29.10–29.17 | 137.3–148.3 |
| Stock text conversation, maximized | 40.07–40.87 | 39.67–40.53 | 25.00 | 29.16–29.17 | 165.8–169.8 |
| Stock list, windowed / maximized | 60.00 | 60.00 / 60.00–60.07 | 16.67 | 16.76–20.87 | 49.1–51.9 / 70.4–73.3 |
| #193 synthetic, windowed / maximized | 60.00 / 59.87 | 59.93–61.67 / 59.73–61.53 | 16.67 | 20.78–20.85 | 37.6–38.4 / 58.6–59.7 |
| #193 text conversation, windowed | 60.00 | 59.93–60.07 | 16.67 | 20.83–20.86 | 50.4–50.8 |
| #193 text conversation, maximized | 60.00 | 60.00 | 16.67 | 20.84 | 95.2–97.4 |
| #193 list, windowed / maximized | 60.00 | 60.00 | 16.67 | 20.83–20.88 | 40.7–44.8 / 74.0–74.4 |

Combined implementation with #193; **fresh content** below correlates CEF frame
serials captured by GTK snapshots with committed Wayland presentation feedback.
This requires one visible web view, no DevTools window, and an uninterrupted GTK
connection. Repeated serials are deduplicated, discarded feedback is excluded,
and ambiguous commit associations are not counted. Medians average the two
middle intervals when needed; p95 uses the empirical nearest rank. Interrupted,
hidden or history-loading samples cannot receive a verified verdict.

| Workload | VRR | Page FPS | CEF CPU paints/s | Fresh presentations/s | Median ms | p95 ms | CPU % |
|---|---|---:|---:|---:|---:|---:|---:|
| Synthetic windowed, GPU preference off | Automatic | 218.07–220.73 | 115.40–118.27 | **33.13–33.20** | 29.17 | 33.59–37.49 | 180.5–183.1 |
| Synthetic maximized, GPU preference off | Automatic | 173.87–175.13 | 53.07–53.80 | **26.53–26.87** | 37.50 | 45.83 | 169.3–170.4 |
| Text conversation windowed, recovered CPU | Never | 236.13–237.20 | 122.33–124.13 | **31.13–31.33** | 33.30–33.32 | 37.52–37.54 | 211.4–213.8 |
| Text conversation maximized, recovered CPU | Never | 194.47–195.40 | 31.27–31.80 | **15.80–16.00** | 62.50 | 70.84–74.98 | 190.4–193.8 |
| List windowed, recovered CPU | Never | 213.40–216.47 | 174.73–174.93 | **41.00–41.13** | 25.00 | 29.17–29.18 | 236.8–240.8 |
| List maximized, recovered CPU | Never | 168.67–171.20 | 52.40–52.80 | **26.20–26.40** | 37.50 | 45.83 | 199.3–200.5 |
| Media conversation windowed, recovered CPU | Never | 237.40–238.00 | 122.67–125.60 | **31.40–31.80** | 29.20–33.31 | 37.49–37.52 | 207.7–208.6 |
| Media conversation maximized, recovered CPU | Never | 194.13–195.87 | 30.80–31.87 | **15.67–16.13** | 62.50 | 70.83–74.75 | 180.3–182.0 |

The target is at least 235 fresh presentations/s, median near 4.17 ms and p95
no worse than 8.33 ms in every representative sample. **All measured combined
groups fail.** Wayland reports a 4,166,666 ns display interval. Per-sample unused
refresh opportunities and discarded feedback are in the numeric aggregates;
unused refresh opportunities are not automatically dropped frames.

Settled visible/background intervals, 30 seconds each after ten seconds settling,
averaged **1.0% / 2.6% CPU**, with zero paint/draw/presentation activity and
background `document.visibilityState=hidden`. The historical 4.3.2 generated
idle control was 0.3% in both states. Different pages and instrumentation prevent
calling this a matched idle regression comparison.

Important comparison limits: synthetic windowed content was 1859×944, real
windowed content 1670×849, and maximized content 2648×1398, all at CEF DPR 2.
Loaded text scroll heights differed across stock / #193 / combined captures
(3591 / 6759 / 8748 CSS px). The list header geometry also changed. The combined
real-page set used Never after the user changed VRR. These are not controlled
same-range A/B comparisons. The same-session synthetic Automatic comparison
does establish a substantial regression in the new presentation path. #193's
own controlled wallpaper comparisons remain the evidence for its optimization.
`combined_recovered_*` samples around the VRR transition are excluded.

## Startup recovery follow-up

Fault injection exposed two startup-selection gaps: GTK's X11 default chose GL
although Vulkan worked, and CEF silently chose software after Vulkan failed.
The final build requests GTK Vulkan first (preserving explicit overrides) and
checks basic hardware Vulkan device initialization before requesting ANGLE.
An initialization failure selects ANGLE GL. GTK retains its own fallback order;
[GSK documents](https://docs.gtk.org/gsk4/ctor.Renderer.new_for_surface.html) that
an explicit renderer preference falls back to the backend default and Cairo.

Fresh generated-fixture results on the corrected build:

| Condition | Actual GTK | Actual CEF | Outcome |
|---|---|---|---|
| Missing Vulkan ICD | GL, Wayland | NVIDIA ANGLE OpenGL | Generated animation completed |
| Missing Wayland, normal sandbox sockets | No display | Not initialized | One exit, status 1, one-launch X11 command printed |
| One-launch X11 recovery | Vulkan, X11 | NVIDIA ANGLE Vulkan | Generated animation completed |
| Missing Vulkan with explicit CEF GL | GL, Wayland | NVIDIA ANGLE OpenGL | Generated animation completed |
| X11 with explicit GTK Vulkan | Vulkan, X11 | NVIDIA ANGLE Vulkan | Generated animation completed |
| Default Wayland, GPU preference off | Vulkan, Wayland | NVIDIA ANGLE Vulkan | Generated animation completed |
| Default Wayland, GPU preference on | Vulkan, Wayland | NVIDIA ANGLE Vulkan | Zero accelerated frames; CPU recovery after ten seconds, animation completed |

The final default-path timing control measured 32.60 fresh presentations/s,
median 29.18 ms, p95 33.49 ms and 181.6% CPU. It is one confirmation sample,
not a new three-sample acceptance series. A preliminary GPU-labeled windowed
capture actually selected a CPU schema and is excluded; the corrected run
explicitly logged `shared_texture=1` before recovery to `shared_texture=0`.

These runs used isolated generated profiles. The missing-ICD fixture set both
`VK_DRIVER_FILES` and `VK_ICD_FILENAMES` to a nonexistent file; the missing-display
fixture set `WAYLAND_DISPLAY` to a nonexistent socket inside the sandbox. None
installed an override or changed display/account settings. Original failing
observations remain separate from the corrected runs. Actual ANGLE backends
were read through CDP, rather than inferred from per-process request logs.

This basic device probe does not detect every ANGLE-specific initialization
failure or later silent software downgrade. Runtime backend telemetry and
GPU-hang/device-loss recovery remain open work. The follow-up changes startup
selection only; the repeated performance measurements above used the earlier
combined binary, with the same Wayland/Vulkan presentation implementation. The
corrected executable SHA-256 is
`0a68df7964bd2b96260d977af1f095751cafc4cff66c9ac05f546f82e8b54352`.

## Remaining bottleneck and ranked work

1. **Fix the GTK/Vulkan presentation regression before release.** High page rates
   coexist with much lower CEF delivery and still lower fresh presentation.
   An initial, untiled prototype's Sysprof capture measured GTK Widget render
   median 14.35 ms / p95 20.22 ms versus snapshot median 0.046 ms. The tiled
   version preserves dirty regions but did not remove the slowdown. Profile
   GSK upload, synchronization, render passes and frame scheduling independently;
   the evidence does not yet identify the exact expensive operation.
2. **Resolve NVIDIA CEF shared-texture delivery.** Both Vulkan and an earlier GL
   CEF experiment failed to deliver accelerated callbacks. A working application
   Vulkan import/copy/export fixture does not resolve the producer problem.
   Keep default-off, bounded no-frame recovery and the CPU recovery variable.
3. **Complete failure recovery and hardware validation.** Actual device loss and
   GPU hangs remain unverified; Vulkan fence completion currently waits without
   a finite timeout, and device teardown can wait too. Runtime GTK device-loss
   recovery is not implemented. Do not claim a bounded recovery guarantee for
   a hung driver. The missing-Vulkan and unavailable-Wayland startup cases now pass their
   generated checks. Hardware popups, runtime fallback and accelerated
   release/reuse under sustained load still need validation.
4. **Repeat the full matched acceptance matrix after those fixes.** Use unchanged
   VRR, viewport, loaded conversation range and media state, test physical input,
   account switching and physical 60/120/240 Hz monitor transitions. These
   hardware cases were not inferred from rounding unit tests or page callbacks.

CEF's pinned [rate handling](https://github.com/chromiumembedded/cef/blob/708dc14/libcef/browser/osr/osr_util.cc)
accepts positive rates above 60 without a 240 cap; the bundled setter and creation
paths both accepted 240 in runtime experiments. Raising that limit alone is
therefore implemented but demonstrably insufficient.

## Checks and reproducibility

Six samples of a user-selected substantial-media conversation completed without
history loading; scroll base 502 and amplitude 900 CSS px were unchanged. Its
scroll height was 8257 windowed / 8236 maximized. Physical wheel/touchpad behavior
was not reported, and stock/#193-only media comparisons remain unmeasured.

Passed: release Devel Flatpak build/install, 98 Rust unit tests including real
Vulkan ownership and texture tests, strict all-target Clippy, formatting,
JavaScript copy-bridge tests, repository shell checks, #193's 33 Python probe
regressions, 53 reused CEF browser checks, and five presentation-analysis
regressions. Texture checks
compare colors, damage, tile seams and crop at 100%, 145% and 200% scale. Holding
old exported frames across producer reuse/resize verifies owned storage and the
three-buffer bound. Invalid DMA-BUF imports verified one CPU recovery request.

The GL-startup integration fixture remains a failure, separately from the passing
graphics-policy test. Its forced Mesa GL 2.1 setup actually received NVIDIA GL
4.6, so the expected context failure was not induced. #193 documents a related
pre-existing baseline fixture failure; this run is not claimed as a pass.

From the feature or combined integration checkout, prepare probes for Devel,
quit it, then run a newly named capture. Omit `--windowed` for maximized; repeat
each size three times:

```sh
python3 tools/scrolling-probe/prepare.py --app-id io.github.tobagin.karere.Devel
python3 tools/scrolling-probe/karere_probe.py new_synthetic_1 \
  --app-id io.github.tobagin.karere.Devel --synthetic --windowed \
  --schedule-probe --wayland-timing
python3 tools/scrolling-probe/presentation_report.py \
  tools/scrolling-probe/results/new_synthetic_1.jsonl --single-view
```

For real conversations, use #193's existing `conversation_probe.py` in the
combined integration checkout. Select the chat manually; no content is read:

```sh
env KARERE_PROBE_CDP=1 python3 tools/scrolling-probe/karere_probe.py new_real_session \
  --app-id io.github.tobagin.karere.Devel --schedule-probe --wayland-timing
# In another terminal, after the intended conversation is loaded:
python3 tools/scrolling-probe/conversation_probe.py new_text_1 \
  --target conversation --duration 15
python3 tools/scrolling-probe/presentation_report.py \
  tools/scrolling-probe/results/new_real_session.jsonl --single-view \
  --page tools/scrolling-probe/results/new_text_1_page.json
```

Use `--target list` for the chat list. The existing probe supplies the five-second
warmup and restores scrolling. Reject history-height changes. Quit the diagnostic
process and relaunch normally afterward to remove the loopback CDP listener and
instrumentation. Raw timings, account data, and binaries are not committed.

[Sanitized numeric aggregates](measurements/adaptive_refresh_433.json) retain all
individual rates, intervals, CPU usage, geometry and unused refresh opportunities.
Historical measurements in the other findings documents keep their original
versions and conditions. The installed v4.3.2 manifest's annotated tag object
`952a12f3412cd35afa1606514f7d967a5b5447aa` peels to source commit
`11dfc0e62d7279e5dd3b2e97dc096e521845dedb`; it is not an unavailable source commit.

The diagnostic instances were closed and Devel was relaunched normally. Its
external diagnostic port is closed, the user's GPU preference remains enabled,
and temporary maximization was restored. Stable's installed commit is unchanged;
the user's 4K/240 Hz, 145% scale and VRR Never settings were retained.
