## Context

`po/` carries v3's 9 locales (`ar en_GB en_US es ga it_IT kk pt_BR pt_PT`). v4 added source files with new `gettext`/blueprint strings; `po/POTFILES.in` currently lists only `main.rs window.rs application.rs web_view.rs` + UI + desktop/metainfo, so strings in account/preferences/tray/CEF modules may not be extracted. The Chromium spellchecker (`src/spellcheck.rs::KNOWN_LANGUAGES`) advertises 50 BCP-47 dictionary locales. Goal: UI translation set == dictionary set.

`KNOWN_LANGUAGES` (50 BCP-47 entries, source of truth):
`af bg ca cs cy da de de-DE el en-AU en-CA en-GB en-US es es-419 es-AR es-ES es-MX es-US et fa fo fr he hi hr hu id it ko lt lv nb nl pl pt-BR pt-PT ro ru sh sk sl sq sr sv ta tg tr uk vi`

## Goals / Non-Goals

**Goals:**
- One `.po` per `KNOWN_LANGUAGES` entry (50 locales), gettext-named (`-`→`_`).
- Complete `POTFILES.in` so every v4 translatable string is extracted.
- Fresh `karere.pot`; all `.po` merged + machine-translated (incl. existing 9).
- Meson builds 50 `.mo`.

**Non-Goals:**
- Human translation review (machine output ships as-is; translators refine later).
- Adding/removing dictionary languages (driven by `KNOWN_LANGUAGES`, untouched).
- Plural-form correctness audit beyond what `msginit` seeds per locale.

## Decisions

**D1 — Locale set = all 50 BCP-47 variants, not 40 base langs.** Maps 1:1 to the dictionary list so coverage is provably equal. Cost: regional pseudo-locales (`es_419`) that glibc won't auto-select. Alternative (base-only) rejected: leaves `en_AU`/`es_MX` spellable but not selectable as a distinct UI catalog, breaking the 1:1 invariant the user asked for.

**D2 — Naming: BCP-47 `-` → gettext `_`, regions upper-cased.** `en-AU`→`en_AU`, `pt-BR`→`pt_BR`, `es-419`→`es_419`. Matches existing `pt_BR`/`en_GB` files and glibc locale convention. `es_419` kept verbatim (documented non-glibc caveat).

**D3 — Generate stubs via `msginit`/`msgmerge`, then fill `msgstr` by LLM.** `msginit` seeds correct `Plural-Forms` headers per locale; `msgmerge` keeps existing translations. LLM fills empties. Alternative (hand-author `.po`) rejected — wrong/absent plural headers break compilation.

**D4 — Re-audit POTFILES.in before regenerating POT.** grep all `src/**/*.rs` for `gettext(`/`tr!(`/`ngettext(` and all `data/ui/*.blp` for `_(`; add every hit. Prevents shipping locales that can't translate strings the extractor never saw.

**D5 — Source of truth is `KNOWN_LANGUAGES`.** A test/script derives the expected LINGUAS from it, so future dictionary additions surface as a LINGUAS gap rather than silent drift.

## Risks / Trade-offs

- [Machine translation quality is unreviewed] → msgstr fallback is the English msgid; flag catalogs `fuzzy`-free but note in commit they need human review. Worst case a string reads awkwardly, never blank.
- [`es_419` / regional locales never load on normal systems] → documented; they exist for explicit `LANGUAGE=es_419` and completeness, no runtime breakage.
- [POT regen churn — line moves create huge diff] → run `msgmerge --no-fuzzy-matching`-aware pass; keep `.pot` header date stable to limit noise.
- [Plural-Forms mismatch breaks `msgfmt`] → rely on `msginit` per-locale seeding; CI/`meson compile` will fail loudly if any `.mo` won't build.
- [50 `.mo` files in Flatpak] → each catalog is small (~few KB); negligible size impact.

## Migration Plan

1. Fix `POTFILES.in`, regenerate `po/karere.pot`.
2. `msgmerge` the 9 existing `.po`; `msginit` the 41 new locales.
3. LLM-fill all `msgstr`.
4. Update `po/LINGUAS` to the 50-locale list.
5. `meson compile` → verify 50 `.mo` build clean.
6. Smoke-test one new locale (`LANGUAGE=pl` shows Polish UI).

Rollback: revert `po/` tree; no code depends on the new locales.
