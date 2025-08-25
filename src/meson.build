# Configure Rust build
cargo_manifest = meson.project_source_root() / 'Cargo.toml'
cargo_target_dir = meson.global_build_root() / 'target'

# Generate config.rs with build configuration
config_rs = configure_file(
  input: 'config.rs.in',
  output: 'config.rs',
  configuration: conf,
)

# Cargo build command
cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: 'reel',
  console: true,
  env: cargo_env,
  command: [
    cargo, 'build',
    '--manifest-path', cargo_manifest,
    '--target-dir', cargo_target_dir,
    cargo_options,
  ],
)

# Extract the built binary
reel_bin = custom_target(
  'reel-binary',
  depends: cargo_build,
  build_by_default: true,
  install: true,
  install_dir: bindir,
  output: 'reel',
  command: [
    'cp', 
    cargo_target_dir / rust_target_dir / 'reel',
    '@OUTPUT@'
  ],
)

# Compile GResources (Blueprint files)
blueprint_files = files(
  'ui/blueprints/window.blp',
  'ui/blueprints/auth_dialog.blp',
  'ui/blueprints/library_view.blp',
  'ui/blueprints/media_card.blp',
  'ui/blueprints/movie_details.blp',
  'ui/blueprints/show_details.blp',
)

blueprints = []
foreach blueprint : blueprint_files
  blueprints += custom_target(
    blueprint.full_path().split('/')[-1].replace('.blp', '.ui'),
    input: blueprint,
    output: '@BASENAME@'.replace('.blp', '.ui'),
    command: [
      find_program('blueprint-compiler'),
      'compile',
      '--output', '@OUTPUT@',
      '@INPUT@',
    ],
  )
endforeach

# Generate GResource
resources = gnome.compile_resources(
  'resources',
  'ui/resources.gresource.xml',
  gresource_bundle: true,
  install: true,
  install_dir: pkgdatadir,
  dependencies: blueprints,
)