# Karere translations

`LINGUAS` (72 locales) is generated from `crate::i18n::ui_locales()`:

- **52** — one per Chromium spellcheck dictionary in
  `src/spellcheck.rs::KNOWN_LANGUAGES` (gettext-named).
- **4** — v3-inherited locales with no Chromium dict (`ar`, `ga`, `kk`, `it_IT`).
- **16** — Chromium UI-only languages (a `.pak` UI catalog but no Hunspell dict):
  `am bn fi fil gu ja kn ml mr ms sw te th ur zh_CN zh_TW`.

The Preferences → Appearance → **Language** picker lists these via the same
`ui_locales()` source of truth; the `app-language` GSetting forces the UI
language (gettext `LANGUAGE`) at startup and maps to the nearest Chromium `.pak`
locale for `CefSettings.locale` (see `src/i18n.rs`). A change requires a restart.

## Caveats

- **Machine translations need human review.** Every `.po` was bulk machine-
  translated (one pass per locale). Strings are complete and compile, but wording
  is unreviewed — translators should refine. Placeholders (`%s`/`%d`) and GTK
  mnemonics were preserved.
- **`en_*` are English identity.** `en_AU`, `en_CA`, `en_GB`, `en_US` carry
  `msgstr == msgid` (no regional spelling divergence applied).
- **Regional pseudo-locales only load under explicit `LANG`/`LANGUAGE`.**
  `es_419` (Spanish, Latin America) is not a glibc locale name; its catalog loads
  only when something explicitly selects `es_419` (e.g. `LANGUAGE=es_419`).
  Same for other region variants glibc won't auto-select. No runtime breakage —
  normal systems fall back to the base language.
- **`ar`/`ga`/`kk` have no spellcheck dictionary.** Chromium ships no Hunspell
  dict for Arabic, Irish, or Kazakh, so they are translation-only locales and do
  not appear in the spellcheck language list.

## Regenerating the POT

The canonical one-command procedure is:

```sh
tools/update-po.sh
```

It regenerates `po/karere.pot` from `po/POTFILES.in` and re-merges all 72
`po/*.po` catalogs (driven by `po/LINGUAS` with `msgmerge -U --backup=none`).
Version is derived from `meson.build` (no pinned `2.0.0`).

### What the script does

1. **Materializes `data/ui/*.ui` from `.blp`** — runs
   `blueprint-compiler compile --output data/ui/<name>.ui data/ui/<name>.blp`
   for each entry in `data/meson.build`'s `blueprint_files` list
   (`window`, `preferences`, `keyboard-shortcuts`, `account_switcher`), mirroring
   what `data/meson.build` does at configure time. The `.ui` products are
   gitignored (`/data/ui/*.ui`) and are removed by the script on exit
   (including on failure via `trap`).
2. **Split xgettext extraction** — pass 1 (`--keyword=tr --keyword=gettext`
   `--from-code=UTF-8`) over the non-Rust subset of `POTFILES.in` (the 4 `.ui`
   + 2 `.in` files; xgettext auto-detects Glade/Desktop/ITS), then pass 2
   `--language=C --join-existing --keyword=tr --keyword=gettext` over **all**
   `.rs` entries derived from `po/POTFILES.in` (currently 5 files:
   `src/main.rs`, `src/window.rs`, `src/actions.rs`, `src/tray.rs`,
   `src/preferences.rs`). The file list is derived via
   `grep '\.rs$' po/POTFILES.in` — never hardcoded — so adding a new
   Rust source to `POTFILES.in` is picked up automatically. `src/preferences.rs`
   carries translatable strings (e.g. "GPU rendering takes effect after
   Karere restarts.") and was previously omitted from the documented 4-file
   join command.
3. **LINGUAS-driven merge** — `msgmerge -U --backup=none` for each locale in
   `po/LINGUAS` (72 entries, `msgfmt --check` clean).

### Known limitations & expectations

- **The meson `karere-pot` target omits `--language=C`**, so `xgettext` fails
  to parse the `.rs` sources and extracts only a few strings. Do not use the
  meson target for pot generation — route through `tools/update-po.sh`.
- **Expected warnings from the Rust join:** `xgettext --language=C` parses Rust
  via the C parser; Rust character literals (e.g. `'x'`) trigger benign
  `warning: unterminated character constant` diagnostics (one per file/line,
  e.g. `src/main.rs:228`). These are expected and do not indicate failure.
- **Tilde-suffix backups are prevented** by `--backup=none` and `.gitignore`
  (`/po/*.po~`, `/po/*.pot~`). Historical `po/*.po~` backups (en_US, es, ga,
  pt_BR, pt_PT — v3-era 80-msgid snapshots) were removed in KARE-014.
- **New msgids land untranslated** (`msgstr ""`, English fallback) until
  translated — see the machine-translation caveat above. `tools/verify-po.sh`
  is the automated catalog health gate (sentinel msgids, LINGUAS↔`.po` parity,
  `msgfmt --check`, no `~` backups, correct `Project-Id-Version`).
