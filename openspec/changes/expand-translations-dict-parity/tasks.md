## 1. Audit extraction sources

- [x] 1.1 grep `src/**/*.rs` for `gettext(`, `tr!(`, `ngettext(`; list every file with a hit
- [x] 1.2 grep `data/ui/*.blp` (and compiled `.ui`) for `_(`; list every file with a hit
- [x] 1.3 Update `po/POTFILES.in` to include all hits + desktop/metainfo inputs (add account/preferences/tray/CEF modules missing vs v3)
- [x] 1.4 Confirm meson POTFILES path uses `.blp` vs `.ui` consistently with how `karere.pot` is generated

## 2. Regenerate POT

- [x] 2.1 Run the meson `karere-pot` (or `xgettext` equivalent) target to regenerate `po/karere.pot`
- [x] 2.2 Verify new v4 strings (account manager, switcher, preferences, tray, CEF) appear as `msgid` entries
- [x] 2.3 Keep POT header churn minimal (stable date / sorted output)

## 3. Build the 50-locale list

- [x] 3.1 Derive locale names from `src/spellcheck.rs` `KNOWN_LANGUAGES`: replace `-` with `_`, upper-case region (`en-AU`→`en_AU`, `es-419`→`es_419`)
- [x] 3.2 Rewrite `po/LINGUAS` with all 50 locales, sorted
- [x] 3.3 Cross-check: LINGUAS count == KNOWN_LANGUAGES count, no extras (50 dict locales + 4 inherited extras `ar ga kk it_IT` kept per user decision; see spec update)

## 4. Create / merge catalogs

- [x] 4.1 `msgmerge` the 9 existing `.po` (`ar en_GB en_US es ga it_IT kk pt_BR pt_PT`) against the new POT, preserving existing msgstr
- [x] 4.2 `msginit` the 41 new locales from the POT (seeds correct per-locale `Plural-Forms`) — 45 new (superset)
- [x] 4.3 Verify each new `.po` has a valid header (Language, Plural-Forms, charset UTF-8) — patched de_DE/es_ES Language + sh Plural-Forms

## 5. Machine-translate

- [x] 5.1 LLM-fill all empty `msgstr` for every locale (new 41 + refresh untranslated in existing 9) — 50-agent workflow, applied via compendium (empties-only, preserved existing)
- [x] 5.2 Respect `printf`/brace placeholders and `&`/`_` mnemonics — do not translate or reorder them destructively — verified 0 placeholder mismatches
- [x] 5.3 Fill plural forms (`msgstr[0]`, `msgstr[1]`, …) per each locale's Plural-Forms — N/A: POT has 0 `msgid_plural` entries
- [x] 5.4 Leave English-base locales (`en_*`) as English where translation is identity

## 6. Build & verify

- [x] 6.1 `meson compile` — confirm all 50 `.mo` build with no `msgfmt` errors — 54 `.mo` build clean
- [x] 6.2 Confirm installed `karere.mo` count under `/app/share/locale` == 50 — 54 catalogs (50 dict + 4 extras) == LINGUAS
- [x] 6.3 Smoke-test a new locale (e.g. `LANGUAGE=pl` or `LANG=pl_PL.UTF-8`) shows translated UI — pl/ko/ru/de/uk/vi/ar/… verified via built `.mo`
- [x] 6.4 Run `cargo test -p ... spellcheck` (KNOWN_LANGUAGES tests still pass) and `openspec validate expand-translations-dict-parity` — 6/6 pass, change valid

## 7. Document caveat

- [x] 7.1 Note in commit / po README that `es_419` and regional pseudo-locales only load under explicit `LANGUAGE`/`LANG` and that machine translations need human review — added `po/README.md`
