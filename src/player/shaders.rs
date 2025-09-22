// Embedded shaders for MPV player
// These are compiled into the binary to ensure they're always available

pub struct EmbeddedShader {
    pub name: &'static str,
    pub content: &'static str,
}

// Embed all shaders from assets/shaders directory at compile time
pub const FSRCNNX_X2_8_0_4_1: &str = include_str!("../../assets/shaders/FSRCNNX_x2_8-0-4-1.glsl");
pub const FSR: &str = include_str!("../../assets/shaders/FSR.glsl");
pub const ANIME4K_CLAMP_HIGHLIGHTS: &str =
    include_str!("../../assets/shaders/Anime4K_Clamp_Highlights.glsl");
pub const ANIME4K_UPSCALE_CNN_X2_M: &str =
    include_str!("../../assets/shaders/Anime4K_Upscale_CNN_x2_M.glsl");

// Collection of all available shaders
pub const EMBEDDED_SHADERS: &[EmbeddedShader] = &[
    EmbeddedShader {
        name: "FSRCNNX_x2_8-0-4-1.glsl",
        content: FSRCNNX_X2_8_0_4_1,
    },
    EmbeddedShader {
        name: "FSR.glsl",
        content: FSR,
    },
    EmbeddedShader {
        name: "Anime4K_Clamp_Highlights.glsl",
        content: ANIME4K_CLAMP_HIGHLIGHTS,
    },
    EmbeddedShader {
        name: "Anime4K_Upscale_CNN_x2_M.glsl",
        content: ANIME4K_UPSCALE_CNN_X2_M,
    },
];

use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Write an embedded shader to a temporary file and return its path
/// MPV requires shader files to be on disk, so we create temporary files
pub fn write_shader_to_temp_file(shader_name: &str, shader_content: &str) -> Result<PathBuf> {
    // Create temp directory for shaders if it doesn't exist
    let temp_dir = std::env::temp_dir().join("reel-shaders");
    fs::create_dir_all(&temp_dir)?;

    // Create temp file path
    let temp_path = temp_dir.join(shader_name);

    // Write shader content to temp file
    let mut file = fs::File::create(&temp_path)?;
    file.write_all(shader_content.as_bytes())?;
    file.sync_all()?;

    Ok(temp_path)
}

/// Write multiple shaders and return their paths as a colon-separated string for MPV
pub fn prepare_shader_chain(shader_names: &[&str]) -> Result<String> {
    let mut paths = Vec::new();

    for name in shader_names {
        // Find the shader in our embedded collection
        if let Some(shader) = EMBEDDED_SHADERS.iter().find(|s| s.name == *name) {
            let path = write_shader_to_temp_file(shader.name, shader.content)?;
            paths.push(path.to_string_lossy().to_string());
        } else {
            // Log warning but don't fail - continue with other shaders
            tracing::warn!("Shader {} not found in embedded shaders", name);
        }
    }

    // Join paths with colon for MPV's glsl-shaders property
    Ok(paths.join(":"))
}

/// Clean up temporary shader files (optional - OS will clean temp dir anyway)
pub fn cleanup_temp_shaders() -> Result<()> {
    let temp_dir = std::env::temp_dir().join("reel-shaders");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    Ok(())
}
