use std::{env, path::PathBuf};

/// Get the project root directory
#[allow(dead_code)]
pub fn project_root() -> PathBuf {
    let start = env::var_os("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    start
        .ancestors()
        .find(|path| path.join("Cargo.toml").is_file() && path.join("xtask").is_dir())
        .expect("Run xtask from the Razemify workspace")
        .to_path_buf()
}

/// Format bytes as human-readable size
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_idx])
}
