# Desktop file
desktop_file = i18n.merge_file(
  type: 'desktop',
  input: configure_file(
    input: application_id + '.desktop.in.in',
    output: application_id + '.desktop.in',
    configuration: conf,
  ),
  output: full_application_id + '.desktop',
  po_dir: meson.project_source_root() / 'po',
  install: true,
  install_dir: datadir / 'applications',
)

# Validate desktop file
desktop_file_validate = find_program('desktop-file-validate', required: false)
if desktop_file_validate.found()
  test(
    'validate-desktop-file',
    desktop_file_validate,
    args: [desktop_file],
    depends: [desktop_file],
  )
endif

# Metainfo file
metainfo_file = i18n.merge_file(
  input: configure_file(
    input: application_id + '.metainfo.xml.in.in',
    output: application_id + '.metainfo.xml.in',
    configuration: conf,
  ),
  output: full_application_id + '.metainfo.xml',
  po_dir: meson.project_source_root() / 'po',
  install: true,
  install_dir: datadir / 'metainfo',
)

# Validate metainfo file
appstreamcli = find_program('appstreamcli', required: false)
if appstreamcli.found()
  test(
    'validate-metainfo-file',
    appstreamcli,
    args: ['validate', '--no-net', '--explain', metainfo_file],
    depends: [metainfo_file],
  )
endif

# Icons
icon_sizes = ['scalable']
foreach size : icon_sizes
  install_data(
    'icons' / 'hicolor' / size / 'apps' / application_id + '.svg',
    install_dir: datadir / 'icons' / 'hicolor' / size / 'apps',
    rename: full_application_id + '.svg',
  )
endforeach

# GSchema
schema_file = configure_file(
  input: application_id + '.gschema.xml.in',
  output: full_application_id + '.gschema.xml',
  configuration: conf,
)

install_data(
  schema_file,
  install_dir: datadir / 'glib-2.0' / 'schemas',
)

# Validate GSchema
compile_schemas = find_program('glib-compile-schemas', required: false)
if compile_schemas.found()
  test(
    'validate-gschema',
    compile_schemas,
    args: ['--strict', '--dry-run', meson.current_source_dir()],
  )
endif