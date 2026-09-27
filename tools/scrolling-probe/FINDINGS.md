**Karere 4.3.0: confirmed CPU-rendering frame-delivery stall**

This is the original investigation record. The subsequent application fix and
compiled-build comparisons are documented in [FIX_VALIDATION.md](FIX_VALIDATION.md).

27 September 2026. The controlled tests identify a bottleneck in Karere's CPU off-screen rendering and CEF event-loop integration. The WhatsApp chat list updates at about 60 times per second inside the page, but Karere draws it only about 10 times per second. Enabling the existing GPU Rendering preference on this notebook's AMD GPU raises drawing to about 59 times per second. Reverting the preference reproduces the original bottleneck.

This establishes a cause of slow chat-list display. It does not establish that all perceived wheel/touchpad or native-menu lag has the same cause. The earlier manual comparisons were inconclusive and remain recorded below.

**Controlled test on the actual WhatsApp chat list**

Each run used the same normal profile, maximized geometry, zoom and power condition. After the chat list appeared, the probe allowed 20 seconds to settle and 5 seconds to warm up. It then moved the list through the same 900 CSS-pixel range for 15 seconds and restored the starting scroll offset. It read timing and geometry only; it did not read chat text or contacts, click, type, or send messages.

| Configuration, in test order | Page animation updates/s | CEF paint callbacks/s | GTK draw callbacks/s | CEF pump gap p95 | Pump duration p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Original: GPU Rendering off, 100 ms fallback | 60.0 | 19.87 | **10.00** | 100.46 ms | 18.58 ms |
| GPU Rendering on, same 100 ms fallback | 59.93 | 59.80 | **58.60** | 16.93 ms | 0.65 ms |
| Reverted: GPU Rendering off, same fallback | 60.0 | 21.00 | **10.07** | 100.34 ms | 13.95 ms |

All three runs recorded actual scroll movement, a visible page, a 2021×1173 CSS viewport, device-pixel ratio 2, and zero observed page long tasks. The accelerated run confirmed `GL_VENDOR=AMD`, successful DMA-BUF import, and accelerated paint/draw at 4042×2346 pixels. GPU preference state alone was not used as proof of acceleration.

The CPU-rendering runs each contained 148 pump gaps longer than 50 ms during the measured 15 seconds. **No CEF scheduling callback arrived between the pump executions across any of those gaps.** The four immediate requests observed in each run were handled in about 8 ms. This is fallback-timer dependence, rather than evidence of GTK ignoring a flood of timely CEF scheduling requests.

In the first CPU run, all 149 pump calls lasting at least 10 ms were followed by a gap exceeding 50 ms. No pump call in the GPU run lasted 10 ms. This points toward a continuation/wakeup problem when software-rendering work consumes CEF's work slice. It does not by itself identify the exact function responsible inside CEF.

A fourth CPU run disabled Karere's rendering debug logs. It still recorded 148 pump gaps over 50 ms, a 100.18 ms p95 gap, four immediate scheduling requests, and no requests between the long gaps. The page animation remained near 60 updates/s. Paint/draw counts are unavailable in that run because those logs were disabled; the event-loop stall itself persisted.

**Independent generated-page controls**

The same experiment also reproduced in an isolated profile containing only generated rows. Each active interval was exactly 15 seconds; page animation stayed close to 60 updates/s throughout.

| Configuration | CEF paint callbacks/s | GTK draw callbacks/s | Pump gap p95 | Total process CPU |
| --- | ---: | ---: | ---: | ---: |
| CPU rendering, original 100 ms fallback | 20.00 | 10.13 | 100.20 ms | 70.3% |
| CPU rendering, temporary 8 ms fallback | 59.27 | 38.87 | 15.78 ms | 134.6% |
| CPU rendering, restored 100 ms fallback | 19.60 | 10.00 | 100.33 ms | 67.6% |
| GPU rendering, original 100 ms fallback | 59.93 | 59.73 | 17.19 ms | 56.1% |
| GPU rendering, temporary 8 ms fallback | 59.93 | 59.67 | 8.63 ms | 58.0% |

Here 100% CPU means one core. More frequent pumping restores CPU-path paint delivery but does not restore full drawing cadence, and it substantially increases active CPU use. GPU rendering avoids both problems in this workload. A permanent 8 ms polling override is therefore not the recommended workaround.

**Source-level interpretation**

Karere drives CEF using an on-demand callback plus an independent [100 ms GLib fallback timer](https://github.com/tobagin/karere/blob/v4.3.0/src/cef_runtime.rs#L379-L385). The callback imposes an 8 ms minimum and coalesces pending requests. The measured CPU-path pauses align with that fallback.

The packaged engine is CEF `152.0.6+g708dc14`. Its [external message pump](https://github.com/chromiumembedded/cef/blob/708dc14/libcef/browser/browser_message_loop.cc) uses a 10 ms work slice and breaks out without explicitly reposting the next continuation at that exit. Chromium's [task controller](https://github.com/chromium/chromium/blob/152.0.7977.83/base/task/sequence_manager/thread_controller_with_message_pump_impl.cc#L328-L387) communicates required continuation through the return value of `DoWork()`. Together with the timing correlation, this makes continuation scheduling after a work-slice exit a strong upstream lead. It is an inference from source and measurements, not a tested CEF patch.

Karere also ships a [patch detaching Chromium's GLib WorkSource](https://github.com/tobagin/karere/blob/v4.3.0/tools/cef-patches/karere-151-detach-worksource.patch) to address idle CPU use. Whether that patch is necessary to trigger this failure has not been isolated. Reverting it blindly could restore the previous idle-CPU problem.

The earlier hypothesis that a long pending Rust timer suppresses an urgent request was **not observed**: measured scheduling requests were immediate, and no urgent request was held behind a long pending timer. Fixing the coalescer's deadline semantics may still be appropriate, but these results do not justify naming that as the demonstrated root cause.

**Manual observations and remaining limits**

The user reported jerky/delayed motion with both input devices and lag in native Preferences/menus. GPU rendering alone and halving window height initially felt about the same. Conversation scrolling improved before any timer change. The temporary 8 ms timer made the chat list feel smoother, but restoring 100 ms did not make it feel worse; native-menu responsiveness had no clear difference. Those manual runs did not establish causality. The automatic tests above provide the repeatable comparison that was missing.

Programmatic scrolling bypasses wheel/touchpad input conversion. Native-menu latency and physical compositor presentation were not measured. Callback counts are not screen FPS; paint/draw window boundaries use host log receipt times, while pump timing uses in-process monotonic timestamps. The same instrumentation was used across each controlled comparison. The normal-logging control rules out rendering debug logs as the explanation for the 100 ms pumping pattern, but diagnostic instrumentation still adds some overhead.

**Practical action and upstream fix direction**

The smallest measured workaround on this AMD notebook is **Preferences → General → Experimental → GPU Rendering**, followed by fully quitting and restarting Karere. This restored frame delivery in the real chat-list comparison without changing the fallback interval. It is not a demonstrated cure for the separately reported native-menu symptom.

An upstream fix should preserve prompt CEF continuations when a work slice ends, including the software path, while retaining low idle CPU use. Validate it against both generated and real chat-list workloads, native-menu responsiveness, and idle/background CPU. The unresolved native-menu symptom should be profiled with the accelerated path active so that the confirmed software-path bottleneck does not obscure it.

The original GPU preference (`false`), window settings, display scale and power configuration are restored for normal use after diagnosis. No application binary, driver or runtime package was modified. Diagnostic preload libraries and the loopback port are used only for explicitly instrumented launches.

Environment: Aurora 44/KDE Wayland, 3840×2400 panel at 60 Hz and 190% desktop scale; AMD Radeon 680M plus NVIDIA RTX 3050 Ti; GNOME 50 Flatpak runtime; source commit `c8b3d71833d6b858f58b73f715aefec63ed98d14`; Flatpak commit `32cc1d66019225336b91ab32909407110c5ddc5216a581df26a44b18f3c1f86e`.

Evidence: [real chat-list results](measurements/chat_summary.json), [generated-page results](measurements/automatic_summary.json), and [diagnostic source/reproduction notes](README.md). The aggregate results contain timing and configuration metadata. Raw machine-specific captures remain local. This investigation is tracked in [issue #173](https://github.com/tobagin/karere/issues/173).
