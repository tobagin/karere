# Generated for nl — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-nl.mo',
  input: meson.project_source_root() / 'po' / 'nl.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'nl' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget
