# Released CEF 156 retest

Checkpoint: 1 November 2026. **In progress; accelerated presentation acceptance
and the final codec-enabled bundle are not verified.** Earlier local-backport
measurements retain their original engine, binary, driver and display identities.

## Frozen inputs

- Karere 4.3.4 stock, release `450311a`, installed Flatpak commit
  `7928a66fca79343fa12a89fc9afacc607a69e8ff9f5173504066331337dca8eb`.
- Independent #195 comparison merged with upstream main `81db8b1`, local merge
  `cc57436`. #193's production wallpaper fix is already upstream (`58e7e8d`);
  its probes and checks are reused. #193 is closed, not merged as a PR.
- Bazzite 44.20261026 / kernel 7.2.11, RTX 5090 / NVIDIA 615.78.08,
  KDE Wayland, 3840×2160 at 240 Hz, **125% scaling**, adaptive sync **Never**.
- GNOME Platform 51 `36d7e84`, SDK 51 `3bb470f`, GTK 4.24.1.
  Chrome control: 156.0.8078.79-1. Installed stable and remote Flatpak commits
  matched when checked; no display or stable-account settings were changed.
- Official CEF **156.0.15+gcf671c0 / Chromium 156.0.8078.80**,
  matching `cef`/`cef-dll-sys` **156.3.0+156.0.15**.
  Official minimal x64 archive SHA-256:
  `6873f975bb047316680ea1839980ed5bd4c33c8cd4ce45761c94211856605388`.
  Official CDN library SHA-256:
  `212a48487066a4de63618d9a1b3ffaa89c95c18af5fe338473a958fea0e8eb35`.

The original NVIDIA native-handle allocation fix, [CEF #4238], is present in the
released engine. [CEF #4314] remains open and is **not applied**. Local successes
do not establish a general resolution of the producer synchronization issue.
The CDN engine lacks H.264/AAC, so its results are engine controls, not proof of
the final codec-enabled application. Production pins in this checkpoint remain
CEF 152; the experimental 156 migration and matching codec build are separate.

The tested migration is supplied as [`released-cef156.patch`](released-cef156.patch).
It updates bindings/lockfile, generated Cargo sources, ABI refusal, engine build
defaults and recovery, and removes the obsolete native-handle backport. It
contains no pending synchronization fix. Apply it in an isolated worktree:

```sh
git worktree add --detach ../karere-cef156 HEAD
git -C ../karere-cef156 apply "$PWD/tools/scrolling-probe/released-cef156.patch"
# For image checks only, also apply frame-integrity.patch and build debug.
```

Set `CEF_PATH` to the matching staged engine for SDK builds. For an installable
Devel candidate, expand its manifest with `flatpak-builder --show-manifest`,
remove non-x64 CEF archive/metadata entries from that **local** expanded manifest,
then use `tools/cef-container/stage-local.py` with the matching codec archive and
build with `--arch=x86_64`. The patch does not replace production archive pins;
both architecture artifacts must be coherent before a production migration.

## Checks completed

- Official 156.0.15 standalone CPU, GL/EGL and Vulkan image controls: 72 resizes
  and 219 checked frames per configuration, zero blank or partial frames.
- Actual GTK consumer: CPU and all four GL/Vulkan producer/presenter combinations,
  72 maximize/restore cycles and 216 checked frames each, zero bad frames.
- CEF settings ABI checked against matching headers: 448 bytes in 152 and 496 in
  156, with the CDP field at offset 336. Unknown settings sizes are rejected by
  the instrumentation. Loaded library hashes and actual GPU/backend are recorded.
- Migrated application: 112 Rust unit tests, both startup integration tests,
  formatting and strict release Clippy passed. Diagnostic Python regressions:
  54 passed with local socket access, including actual interposer fixtures for
  verified layouts, unknown/mismatched versions and a missing version symbol.

Image checks use generated content only and are excluded from FPS acceptance.
They detect partial/blank images; they cannot prove that a coherent image is the
newest page frame. No WhatsApp contents or account screenshots are collected.

## Stock 4.3.4 account baseline

Three 15-second samples per size, each after five seconds of active scrolling
warmup. Compilation was paused. All 18 text/list/media samples completed without
history/layout changes. CPU transfer was selected in the private clone; CEF still
used actual NVIDIA ANGLE OpenGL hardware compositing/rasterization.

| Workload | Size | Page frames/s | Window presentations/s | Window p95 ms | CPU % |
|---|---|---:|---:|---:|---:|
| Long text | Windowed | 60.000 | 60.000–60.067 | 20.825–20.837 | 63.8–68.1 |
| Long text | Maximized | 58.933–60.000 | 52.267–60.000 | 20.835–25.056 | 144.9–168.3 |
| Chat list | Windowed | 60.000 | 59.933–60.000 | 20.851–20.857 | 47.1–52.9 |
| Chat list | Maximized | 60.000 | 58.600–59.933 | 20.883–25.003 | 90.7–127.9 |
| Substantial media | Windowed | 60.000 | 60.000–60.067 | 16.722–20.854 | 49.4–76.6 |
| Substantial media | Maximized | 60.000 | 59.933–60.000 | 16.696–20.832 | 126.1–127.4 |

Ranges cover the three samples, not confidence intervals. CPU is the measured
process tree including instrumentation, with 100% representing one logical CPU.
Windowed viewport: 1859×944 CSS pixels; maximized: 3072×1636; page DPR 2.
Stock lacks content serials: **fresh-content presentation is unverified**.
Window feedback rates are upper bounds. Numeric per-sample cadence, CPU by role,
geometry and unused refresh opportunities are in
[`measurements/released_cef156_retest_20261101.json`](measurements/released_cef156_retest_20261101.json).
Scroll positions and the user's window size were restored before normal exit.

## PR comparison with original CEF 152

The same protocol, viewports and text anchor produced 12 stable text/list samples
on the explicit CPU-transfer recovery path. Content serials and presentation
feedback correlate reliably; every sample has a **lower ceiling measured**.
The existing wallpaper optimization is installed and applied to its one matching
decorative layer. Media testing is still pending.

| Workload | Size | Fresh presentations/s | Median ms | p95 ms | CPU % |
|---|---|---:|---:|---:|---:|
| Long text | Windowed | 125.933–128.600 | 8.331–8.332 | 12.315–12.495 | 213.8–215.2 |
| Long text | Maximized | 42.933–43.400 | 24.986–24.996 | 25.013–25.019 | 231.8–232.6 |
| Chat list | Windowed | 225.200–230.867 | 4.167 | 4.232–8.323 | 166.5–169.5 |
| Chat list | Maximized | 66.667–91.533 | 8.369–16.661 | 16.674–16.684 | 213.4–220.6 |

Windowed text reaches about 239 CPU paints/s while GTK/fresh presentation reaches
126–129/s; maximized delivery is about 85–86 CPU paints/s and 43 presentations/s.
Raising CEF's rate alone does not remove the remaining transfer/presentation cost.
CPU recovery is measured here and has no 235 FPS requirement.

The first maximized attempt was rejected before measurement because its loaded
range was too short. More history was loaded outside timing and only the missing
three samples were retried; completed windowed samples were retained. Full
numeric results, including unused refresh opportunities, are in the same JSON
aggregate. The original scroll anchor and windowed geometry were restored.

## Reproduction and remaining gates

Follow [the probe README](README.md) to prepare the matching ABI instrumentation,
run generated producer/image controls and collect compositor feedback. Cold-copy
only the independent Devel profile while it is stopped, keep the copy private,
mark it explicitly and never reopen a 156-upgraded copy with 152. The account
capture uses memory-only preferences and a separate marked profile:

```sh
KARERE_PROBE_CDP=1 python3 tools/scrolling-probe/karere_probe.py LABEL \
  --app-id io.github.tobagin.karere.Devel \
  --profile-directory /absolute/path/to/marked-private-clone \
  --profile-transfer accelerated --windowed --schedule-probe \
  --wayland-timing --gsk-renderer gl
python3 tools/scrolling-probe/conversation_probe.py SAMPLE \
  --target conversation --duration 15 --amplitude 900 --anchor-bottom 3452.5
python3 tools/scrolling-probe/presentation_report.py \
  tools/scrolling-probe/results/LABEL.jsonl \
  --page tools/scrolling-probe/results/SAMPLE_page.json --single-view
```

Use unique labels, an already selected and fully loaded conversation, fixed
ranges and no unrelated input. Close the diagnostic instance normally afterward;
verify that its listener is gone. The original stable profile is never a target.

Still required: matching codec-enabled x64 Devel bundle and hashes, repeated
generated/list/text/media accelerated presentation samples, recovery checks,
media and fake-device local WebRTC, settled idle, physical rate adaptation and
input confirmation on the final build. ARM artifacts and wider hardware remain
unverified. Do not pair 156 Rust bindings with a 152 ARM engine.

Keep #195 draft and performance incomplete until **each** accepted accelerated
240 Hz sample delivers at least 235 fresh presentations/s, median near 4.17 ms
and p95 at most 8.33 ms. CPU recovery does not satisfy that gate.

[CEF #4238]: https://github.com/chromiumembedded/cef/pull/4238
[CEF #4314]: https://github.com/chromiumembedded/cef/pull/4314
