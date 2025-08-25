project('reel',
  'rust',
  version: '0.1.0',
  meson_version: '>= 0.63.0',
  default_options: [
    'warning_level=2',
    'b_lto=true',
  ]
)

i18n = import('i18n')
gnome = import('gnome')

# Application ID
application_id = 'dev.arsfeld.Reel'
prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
localedir = prefix / get_option('localedir')
pkgdatadir = datadir / meson.project_name()

# Configuration data
conf = configuration_data()
conf.set_quoted('APPLICATION_ID', application_id)
conf.set_quoted('GETTEXT_PACKAGE', meson.project_name())
conf.set_quoted('LOCALEDIR', localedir)
conf.set_quoted('PKGDATADIR', pkgdatadir)
conf.set_quoted('VERSION', meson.project_version())

# Dependencies
dependency('gtk4', version: '>= 4.10.0')
dependency('libadwaita-1', version: '>= 1.4.0')
dependency('gstreamer-1.0', version: '>= 1.20.0')
dependency('gstreamer-video-1.0', version: '>= 1.20.0')
dependency('gstreamer-player-1.0', version: '>= 1.20.0')

# Find cargo
cargo = find_program('cargo', required: true)
cargo_env = environment()
cargo_env.set('CARGO_HOME', meson.global_build_root() / 'cargo-home')

# Build type configuration
if get_option('buildtype') == 'release'
  cargo_options = ['--release']
  rust_target_dir = 'release'
else
  cargo_options = []
  rust_target_dir = 'debug'
endif

# Profile option
profile = get_option('profile')
if profile == 'devel'
  conf.set('PROFILE', 'Devel')
  application_id_suffix = '.Devel'
else
  conf.set('PROFILE', 'Default')
  application_id_suffix = ''
endif

conf.set_quoted('APPLICATION_ID_SUFFIX', application_id_suffix)
full_application_id = application_id + application_id_suffix

# Resources
subdir('data')
subdir('po')
subdir('src')

# Summary
gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)

summary({
  'Profile': profile,
  'Application ID': full_application_id,
  'Build type': get_option('buildtype'),
}, section: 'Configuration')