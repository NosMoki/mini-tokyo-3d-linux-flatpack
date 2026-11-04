desktop_conf = configuration_data()
desktop_conf.set('APP_ID', app_id)
desktop_conf.set('BIN', 'mini-tokyo-3d')
desktop_file = i18n.merge_file(
  input: configure_file(
    input: app_id + '.desktop.in',
    output: app_id + '.desktop.in',
    configuration: desktop_conf,
  ),
  output: app_id + '.desktop',
  type: 'desktop',
  po_dir: '../po',
  install: true,
  install_dir: get_option('datadir') / 'applications',
)

desktop_file_validate = find_program('desktop-file-validate', required: false)
if desktop_file_validate.found()
  test('Validate desktop file', desktop_file_validate, args: [desktop_file])
endif

metainfo_file = i18n.merge_file(
  input: app_id + '.metainfo.xml.in',
  output: app_id + '.metainfo.xml',
  po_dir: '../po',
  install: true,
  install_dir: get_option('datadir') / 'metainfo',
)

appstreamcli = find_program('appstreamcli', required: false)
if appstreamcli.found()
  test('Validate metainfo file', appstreamcli, args: ['validate', '--no-net', '--explain', metainfo_file])
endif

install_data(app_id + '.gschema.xml', install_dir: get_option('datadir') / 'glib-2.0' / 'schemas')

glib_compile_schemas = find_program('glib-compile-schemas', required: false)
if glib_compile_schemas.found()
  test('Validate schema file', glib_compile_schemas, args: ['--strict', '--dry-run', meson.current_source_dir()])
endif

# The app icon is the web app's own icon, installed at each size it comes in.
icons = [
  ['16x16', 'favicon-16x16.png'],
  ['32x32', 'favicon-32x32.png'],
  ['192x192', 'android-chrome-192x192.png'],
  ['512x512', 'android-chrome-512x512.png'],
]
foreach icon : icons
  install_data(
    meson.project_source_root() / 'public' / icon[1],
    install_dir: get_option('datadir') / 'icons' / 'hicolor' / icon[0] / 'apps',
    rename: app_id + '.png',
  )
endforeach
