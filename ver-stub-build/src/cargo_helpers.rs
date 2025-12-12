//! Cargo build script helper functions.

use heck::ToShoutySnakeCase;
use std::fs;
use std::path::PathBuf;

/// Returns true if we're running inside a cargo build script context.
/// We detect this by checking for the OUT_DIR environment variable.
pub fn in_build_script() -> bool {
    std::env::var_os("OUT_DIR").is_some()
}

/// Emit a `cargo::rerun-if-{suffix}` directive if in a build script context.
///
/// Example: `cargo_rerun_if("changed=/path/to/file")` emits `cargo::rerun-if-changed=/path/to/file`
pub fn cargo_rerun_if(suffix: &str) {
    if in_build_script() {
        println!("cargo::rerun-if-{}", suffix);
    }
}

/// Emit a warning. In build script context, emits `cargo::warning=msg`.
/// Otherwise, prints to stderr with `eprintln!`.
pub fn cargo_warning(msg: &str) {
    if in_build_script() {
        println!("cargo::warning={}", msg);
    } else {
        eprintln!("warning: {}", msg);
    }
}

/// Gets OUT_DIR from environment.
pub fn out_dir() -> PathBuf {
    // OUT_DIR is set by Cargo for build scripts to write generated files.
    // See: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set - must be run from build.rs");
    PathBuf::from(out_dir)
}

/// Gets the target directory (e.g., `target/`).
///
/// Checks CARGO_TARGET_DIR first, then tries to infer it from the value of
/// OUT_DIR. If this inference fails, users can set CARGO_TARGET_DIR in
/// `.cargo/config.toml`:
/// ```toml
/// [env]
/// CARGO_TARGET_DIR = { value = "target", relative = true }
/// ```
pub fn target_dir() -> PathBuf {
    // Check CARGO_TARGET_DIR first (user override)
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(target_dir);
    }

    // Infer from OUT_DIR (target/debug/build/<pkg>/out -> go up 4 levels)
    let out_dir = out_dir();
    out_dir
        .ancestors()
        .nth(4)
        .expect(
            "ver-stub-build: could not find target dir from OUT_DIR. \
             Set CARGO_TARGET_DIR in .cargo/config.toml:\n\n\
             [env]\n\
             CARGO_TARGET_DIR = { value = \"target\", relative = true }",
        )
        .to_path_buf()
}

/// Gets the target profile directory (e.g., `target/debug/` or `target/release/`).
///
/// Derives this from OUT_DIR which is like `target/debug/build/<pkg>/out`.
/// For cross-compilation, it's `target/<triple>/debug/build/<pkg>/out`.
pub fn target_profile_dir() -> PathBuf {
    let out_dir = out_dir();
    // OUT_DIR is target/[<triple>/]debug/build/<pkg>/out, go up 3 levels to get target/[<triple>/]debug
    out_dir
        .ancestors()
        .nth(3)
        .expect("ver-stub-build: could not find target dir from OUT_DIR")
        .to_path_buf()
}

/// Finds the artifact binary path using cargo's artifact dependency environment variables:
/// `CARGO_BIN_FILE_<DEP>_<NAME>` and `CARGO_BIN_DIR_<DEP>`.
/// See: https://doc.rust-lang.org/cargo/reference/unstable.html#artifact-dependencies
pub fn find_artifact_binary(dep_name: &str, bin_name: &str) -> PathBuf {
    // Convert dep name to SHOUTY_SNAKE_CASE for env var lookup.
    // Cargo converts dependency names to uppercase with dashes replaced by underscores.
    let dep_upper = dep_name.to_shouty_snake_case();

    // Try CARGO_BIN_FILE_<DEP>_<NAME> with original bin name case first
    // (cargo uses original case for bin name, not upper case)
    let file_env_var_original = format!("CARGO_BIN_FILE_{}_{}", dep_upper, bin_name);
    if let Ok(path) = std::env::var(&file_env_var_original) {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
        panic!(
            "ver-stub-build: {} is set to '{}' but file does not exist",
            file_env_var_original,
            path.display()
        );
    }

    // Try CARGO_BIN_FILE_<DEP> (default binary, no name suffix)
    let file_env_var_default = format!("CARGO_BIN_FILE_{}", dep_upper);
    if let Ok(path) = std::env::var(&file_env_var_default) {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
        panic!(
            "ver-stub-build: {} is set to '{}' but file does not exist",
            file_env_var_default,
            path.display()
        );
    }

    // Try CARGO_BIN_DIR_<DEP> and search for the binary
    let dir_env_var = format!("CARGO_BIN_DIR_{}", dep_upper);
    if let Ok(dir) = std::env::var(&dir_env_var) {
        let dir_path = PathBuf::from(&dir);
        // The binary might have a hash suffix, so look for any file starting with the bin name
        if let Ok(entries) = fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                // Match bin_name with underscores (cargo converts - to _)
                let bin_name_underscore = bin_name.replace('-', "_");
                if file_name_str.starts_with(&bin_name_underscore) {
                    return entry.path();
                }
            }
        }
        panic!(
            "ver-stub-build: {} is set to '{}' but no binary matching '{}' found in that directory",
            dir_env_var, dir, bin_name
        );
    }

    // No env var found
    panic!(
        "ver-stub-build: could not find artifact binary for dep='{}', bin='{}'\n\
         Expected one of:\n\
         - {} (not set)\n\
         - {} (not set)\n\
         - {} (not set)\n\
         \n\
         Make sure you have an artifact dependency in Cargo.toml:\n\
         [build-dependencies]\n\
         {} = {{ path = \"...\", artifact = \"bin\" }}",
        dep_name, bin_name, file_env_var_original, file_env_var_default, dir_env_var, dep_name
    );
}
