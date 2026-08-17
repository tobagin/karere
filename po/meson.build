# po/meson.build — hand-rolled gettext wiring (replaces i18n.gettext).
# WHY not i18n.gettext: its internal `meson --internal gettext pot` script passes
# extra args verbatim to ONE xgettext invocation. A single-pass --language=C would
# destroy all XML (.ui/.desktop/.metainfo) msgids (verified 490 vs 513); the
# correct extraction is the two-pass procedure (non-Rust auto-detect + Rust
# --language=C --join-existing) implemented in tools/update-po.sh. So
# karere-pot / karere-update-po run targets invoke that script directly, while
# LINGUAS-driven per-locale .mo targets are recreated here (mirroring what
# i18n.gettext generated, minus the broken pot machinery).

fs_po = import('fs')
msgfmt = find_program('msgfmt', required: false)
if not msgfmt.found()
  warning('Gettext not found, all translation (po) targets will be ignored.')
else
  linguas_content = fs_po.read(meson.project_source_root() / 'po' / 'LINGUAS')
  linguas = []
  foreach line : linguas_content.split('\n')
    stripped = line.strip()
    if stripped == '' or stripped.startswith('#')
      continue
    endif
    linguas += stripped
  endforeach

  gmotargets = []
  foreach lang : linguas
    subdir(lang / 'LC_MESSAGES')
  endforeach

  alias_target('karere-gmo', gmotargets)

  update_po = find_program(meson.project_source_root() / 'tools/update-po.sh')
  # karere-pot and karere-update-po both run the canonical regenerate+merge
  # procedure (pot + all 72 .po via msgmerge --backup=none); the script is
  # idempotent so the two names are aliases for the same command.
  run_target('karere-pot', command: [update_po])
  run_target('karere-update-po', command: [update_po])
endif
