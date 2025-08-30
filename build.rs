use std::path::Path;

fn main() {
    // Only compile GTK resources when GTK feature is enabled
    #[cfg(feature = "gtk")]
    {
        compile_gtk_resources();
    }

    // Swift-bridge specific build steps when swift feature is enabled
    #[cfg(feature = "swift")]
    {
        compile_swift_resources();
    }
}

#[cfg(feature = "gtk")]
fn compile_gtk_resources() {
    // Compile Blueprint files
    println!("cargo:rerun-if-changed=src/platforms/gtk/ui/blueprints/");
    println!("cargo:rerun-if-changed=src/platforms/gtk/ui/resources.gresource.xml");
    println!("cargo:rerun-if-changed=src/platforms/gtk/ui/style.css");

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
        "player.blp",
    ];

    // Compile each Blueprint file to UI
    for file in &blueprint_files {
        let input_path = format!("src/platforms/gtk/ui/blueprints/{}", file);
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
    std::fs::copy("src/platforms/gtk/ui/style.css", ui_dir.join("style.css"))
        .expect("Failed to copy style.css");

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
    <file>player.ui</file>
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

#[cfg(feature = "swift")]
fn compile_swift_resources() {
    // Swift-bridge code generation and other macOS-specific build steps
    println!("cargo:rerun-if-changed=src/platforms/macos/bridge.rs");
    println!("cargo:warning=Running macOS build with swift-bridge");

    // Get OUT_DIR
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    println!("cargo:warning=OUT_DIR={}", out_dir);

    // Generate Swift bridging code from #[swift_bridge::bridge] modules
    // The generated Swift files are placed in OUT_DIR for consumption by Xcode/SwiftPM
    let bridges = vec!["src/platforms/macos/bridge.rs"];
    for path in &bridges {
        println!("cargo:rerun-if-changed={}", path);
    }

    swift_bridge_build::parse_bridges(bridges).write_all_concatenated(&out_dir, "SwiftBridgeCore");

    // Copy generated files to Xcode project for easier development
    let xcode_generated_dir = "macos/ReelApp/Generated";
    if std::path::Path::new("macos/ReelApp").exists() {
        std::fs::create_dir_all(xcode_generated_dir).ok();

        // Copy the generated Swift and header files
        for entry in ["SwiftBridgeCore.swift", "SwiftBridgeCore.h"] {
            let src = std::path::Path::new(&out_dir).join(entry);
            let dst = std::path::Path::new(xcode_generated_dir).join(entry);
            if src.exists() {
                std::fs::copy(&src, &dst).ok();
                println!("cargo:warning=Copied {} to {}", entry, dst.display());
            }
        }

        // Also copy the SwiftBridgeCore directory if it exists
        let src_dir = std::path::Path::new(&out_dir).join("SwiftBridgeCore");
        if src_dir.exists() {
            let dst_dir = std::path::Path::new(xcode_generated_dir).join("SwiftBridgeCore");
            std::fs::create_dir_all(&dst_dir).ok();
            for entry in std::fs::read_dir(&src_dir).unwrap() {
                if let Ok(entry) = entry {
                    let dst = dst_dir.join(entry.file_name());
                    std::fs::copy(entry.path(), dst).ok();
                }
            }
            println!(
                "cargo:warning=Copied SwiftBridgeCore directory to {}",
                xcode_generated_dir
            );
        }
    }

    println!(
        "cargo:warning=Swift bridge generation complete - check {}",
        out_dir
    );
}
