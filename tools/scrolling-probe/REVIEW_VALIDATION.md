# PR #193 diagnostic review validation

Validated on 28 September 2026 against the review of `cc69b2f`. This revision fixes
the measurement tools: capture clock alignment, large trace buckets, wallpaper
condition control, page selection and interrupted-run cleanup. The production
wallpaper stylesheet and historical aggregates are unchanged.

The earlier eight real-conversation production samples used the explicit
`--production` path, which already disabled/enabled the stylesheet and checked
computed `will-change`. The reviewed default-inline toggle issue did not affect
that procedure. These new generated-page results validate the tools; they are
not another measurement of WhatsApp's raster bottleneck or input-to-screen latency.

[Machine-readable results](measurements/review193_validation.json) retain only
checks, counts and numeric sample summaries. Raw generated-page captures and the
sanitized trace remain ignored under `results/`.

## Offline and static checks

- **30 standard-library Python regression tests passed**, covering capture-time
  offsets after delayed analysis, separate capture clocks, flagged legacy fallback,
  incomplete captures, oversized/unfragmented and fragmented trace messages,
  interleaved pings, completion ordering, disconnects, argument removal, target
  ambiguity, pinned children, termination/reaping, cleanup error preservation,
  manual identities and invalid comparison results.
- **85/85 Python functions** in the revised scripts and new regression harness
  have docstrings (AST check). This exceeds the review's 80% requirement.
- **Ruff 0.16.6 passed** with isolated configuration, Python 3.9 compatibility and
  `E4,E7,E9,F,I`. Isolated configuration avoids inheriting unrelated user-level
  rules; the reported E701 is included. The suggested Flask `jsonify` substitution
  is inapplicable to a local JSON file writer, so `json.dump` remains in use.
- **Node 24.21.0 syntax checks** and **`cargo fmt --all -- --check`** passed using
  the installed GNOME SDK 50 toolchain.

Run the regressions with:

```sh
python3 -m unittest discover -s tools/scrolling-probe -p 'test_*.py' -v
```

The prior release build, Clippy, all-target check and Rust test results are
recorded in [the production validation](CONVERSATION_FINDINGS.md#production-validation-on-current-main).
They were not repeated for this Python/diagnostic-JavaScript revision. That prior
suite includes a GL-startup integration failure reproduced on unchanged upstream
`fb0179f`; it remains a separately documented baseline failure.

## Isolated CEF checks

The existing production executable was run in the installed GNOME Platform 51 /
CEF 152.0.6 Flatpak with a generated page, temporary profile and memory-only
settings. No normal account page was used. The exact executable checksum is in
the JSON record. As in the original generated-fixture check, the production
wallpaper script was evaluated explicitly because the earlier identity hook
cannot access localStorage on the fixture's opaque `data:` origin.

**53 browser checks passed**: the original 16 stylesheet scope checks plus
production/legacy toggling, originally enabled/disabled stylesheets, exact inline
priority restoration, repeated injection, conflicting inline declarations,
replaced elements/stylesheets, owned cancellation before the first frame,
setup failure, duplicate-start rejection, manual completion and scroller replacement.

The default comparison command (without `--production`) detected the compiled
stylesheet and ran the following sequence. Every sample used three seconds of
settling, five seconds warmup and twenty measured seconds on the same target,
with a 3452.5 CSS-pixel bottom anchor and no loaded-history growth.

| Sample | Before | After | Capture clock source | Pump events joined |
| --- | --- | --- | --- | ---: |
| Original 1 | `auto` | `auto` | `capture_start` | 2238 |
| Hint 1 | `transform` | `transform` | `capture_start` | 2146 |
| Hint 2 | `transform` | `transform` | `capture_start` | 2019 |
| Original 2 | `auto` | `auto` | `capture_start` | 2178 |

The original stylesheet state, inline declaration and scroll position matched
exactly after the comparison. Summarizing these fresh captures with a deliberately
incorrect summary-time fallback still joined all pump events using the recorded
capture-time clock pair. Every launcher row contained an absolute monotonic time,
and the capture ended with an exit record and return code 0.

A real Chromium trace completed through `tracingComplete` and wrote **760,812
sanitized events / 106,594,307 bytes**. No stored event retained an `args` field.
The offline oversized-frame regression separately proves that a bucket over
1,000,000 bytes is accepted by the trace client and rejected by the ordinary client;
total trace-file size does not establish individual WebSocket frame sizes.

## Interruption and restoration

A live comparison was sent SIGTERM after its child began sampling. The parent
stopped and reaped the child, cancelled the owned page probe, restored the original
wallpaper/scroll state and exited unsuccessfully, as expected. No successful page
sample was written. Offline tests also cover nonzero child exits and unresponsive
children requiring a bounded terminate/kill sequence.

Separate CLI smoke checks passed for manual start/finish and wallpaper
`off` / `on` / `restore`. Validation refused an already active manual experiment
before modifying the fixture, and that manual experiment subsequently finished
successfully.

`ss -ltnp '( sport = :9333 )'` showed the temporary listener on **127.0.0.1:9333**.
This is not authentication or proof of per-user isolation: local processes able
to reach that port can use CDP. After the diagnostic instance exited, the listener
was closed. The installed Karere was relaunched, and saved GPU, window, zoom and
Reduce Motion settings matched the pre-test snapshot.
