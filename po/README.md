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

`po/karere.pot` is produced from `POTFILES.in`. **The meson `karere-pot` target
omits `--language=C`, so xgettext fails to parse the `.rs` sources** (it only
extracts a few strings). After running the meson target, re-extract the Rust
strings and join them in:

```sh
xgettext --from-code=UTF-8 --add-comments --language=C \
  --keyword=tr --keyword=gettext -j --package-name=karere -o po/karere.pot \
  src/main.rs src/window.rs src/actions.rs src/tray.rs
```

The `.ui` files in `POTFILES.in` are compiled from `data/ui/*.blp` by the meson
`blueprint_files` list (`data/meson.build`) before extraction.
