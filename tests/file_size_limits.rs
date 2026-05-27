/// File size enforcement tests.
///
/// These tests walk `src/**/*.rs` and enforce line-count limits.
/// - `MAX_LINES` (hard cap): any file exceeding this fails the build.
/// - Files between the soft target and the hard cap are listed as warnings
///   (via `println!`) but don't fail tests — they're candidates for splitting.
use std::fs;
use std::path::PathBuf;

/// Hard cap: any `.rs` file under `src/` exceeding this panics the test.
const MAX_LINES: usize = 2000;

/// Soft target: files above this are logged as "needs splitting" but don't fail.
const SOFT_TARGET: usize = 800;

/// Files grandfathered above the soft target (known-large, split planned).
const GRANDFATHERED: &[&str] = &[
    // TODO: split these files, then remove from this list
    "src/components/player/video_player.rs",
    "src/components/library/mod.rs",
    "src/services/library_filter.rs",
];

fn collect_rs_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_dir(&mut files, "src");
    files.sort();
    files
}

fn walk_dir(files: &mut Vec<PathBuf>, dir: &str) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(files, &path.to_string_lossy());
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
}

fn count_lines(path: &PathBuf) -> usize {
    fs::read_to_string(path)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

#[test]
fn no_file_exceeds_hard_cap() {
    let files = collect_rs_files();
    let mut failures = Vec::new();

    for path in &files {
        let lines = count_lines(path);
        let rel = path.to_string_lossy();

        if lines > MAX_LINES {
            failures.push((rel.to_string(), lines));
        }
    }

    if !failures.is_empty() {
        let mut msg = String::from("Files exceeding hard cap:\n");
        for (path, lines) in &failures {
            msg.push_str(&format!("  {path}: {lines} lines (cap: {MAX_LINES})\n"));
        }
        panic!("{msg}");
    }
}

#[test]
fn report_files_above_soft_target() {
    let files = collect_rs_files();
    let mut above_target: Vec<(String, usize)> = Vec::new();

    for path in &files {
        let rel = path.to_string_lossy().to_string();
        let lines = count_lines(path);

        if lines > SOFT_TARGET && !GRANDFATHERED.contains(&rel.as_str()) {
            above_target.push((rel, lines));
        }
    }

    if !above_target.is_empty() {
        println!("\n═══ Files above soft target ({SOFT_TARGET} lines) ═══");
        for (path, lines) in &above_target {
            println!("  {path}: {lines} lines — consider splitting");
        }
        println!(
            "  ({files_checked} files checked, {grandfathered} grandfathered)\n",
            files_checked = files.len(),
            grandfathered = GRANDFATHERED.len(),
        );
    }

    // Show grandfathered files too for visibility
    let mut grandfathered_found = Vec::new();
    for gf in GRANDFATHERED {
        if let Some(lines) = files
            .iter()
            .find(|p| p.to_string_lossy() == *gf)
            .map(|p| count_lines(p))
        {
            grandfathered_found.push((*gf, lines));
        }
    }
    if !grandfathered_found.is_empty() {
        println!("═══ Grandfathered files (known-large, split planned) ═══");
        for (path, lines) in &grandfathered_found {
            println!("  {path}: {lines} lines");
        }
    }
}
