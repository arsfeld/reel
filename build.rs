use std::path::Path;

fn main() {
    // Compile Blueprint files
    println!("cargo:rerun-if-changed=src/ui/blueprints/");
    println!("cargo:rerun-if-changed=src/ui/resources.gresource.xml");
    println!("cargo:rerun-if-changed=src/ui/style.css");

    // Compile GResource with Blueprint files
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir);

    // Create the target directory for compiled UI files
    let ui_dir = out_path.join("ui");
    std::fs::create_dir_all(&ui_dir).expect("Failed to create UI directory");

    // Copy resources to OUT_DIR for compilation
    let resources_dir = out_path.join("resources");
    std::fs::create_dir_all(&resources_dir).expect("Failed to create resources directory");

    // Blueprint files to compile
    let blueprint_files = [
        "window.blp",
        "auth_dialog.blp",
        "library_view.blp",
        "media_card.blp",
        "movie_details.blp",
        "show_details.blp",
    ];

    // Compile each Blueprint file to UI
    for file in &blueprint_files {
        let input_path = format!("src/ui/blueprints/{}", file);
        let output_name = file.replace(".blp", ".ui");
        let output_path = ui_dir.join(&output_name);

        println!(
            "cargo:warning=Compiling {} to {:?}",
            input_path, output_path
        );

        let output = std::process::Command::new("blueprint-compiler")
            .arg("compile")
            .arg("--output")
            .arg(&output_path)
            .arg(&input_path)
            .output()
            .expect("Failed to run blueprint-compiler. Make sure it's installed.");

        if !output.status.success() {
            panic!(
                "blueprint-compiler failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // Copy style.css to build directory
    std::fs::copy("src/ui/style.css", ui_dir.join("style.css")).expect("Failed to copy style.css");

    // Create a modified gresource file that points to compiled UI files
    let gresource_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<gresources>
  <gresource prefix="/dev/arsfeld/Reel">
    <file>window.ui</file>
    <file>auth_dialog.ui</file>
    <file>library_view.ui</file>
    <file>media_card.ui</file>
    <file>movie_details.ui</file>
    <file>show_details.ui</file>
    <file>style.css</file>
  </gresource>
</gresources>"#
        .to_string();

    let gresource_path = ui_dir.join("resources.gresource.xml");
    std::fs::write(&gresource_path, gresource_content).expect("Failed to write gresource file");

    // Compile the GResource file
    glib_build_tools::compile_resources(
        &[ui_dir.to_str().unwrap()],
        gresource_path.to_str().unwrap(),
        "resources.gresource",
    );
}
