// Logic adapted from cargo-binutils:
// https://github.com/rust-embedded/cargo-binutils/blob/07e280d97afe53c0ed24654eb85b39507ac7d6ab/src/rustc.rs#L15

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Finds the path to the LLVM tools bin directory in the Rust toolchain.
///
/// The path is: `{sysroot}/lib/rustlib/{host}/bin/`
///
/// Tools like `llvm-objcopy` and `llvm-readelf` can be found by joining
/// their name (with platform executable suffix) to this path.
pub fn llvm_tools_bin_dir() -> Result<PathBuf, String> {
    let sysroot = get_sysroot()?;
    let host = get_host()?;

    let mut path = PathBuf::from(sysroot);
    path.push("lib");
    path.push("rustlib");
    path.push(host);
    path.push("bin");

    Ok(path)
}

fn get_sysroot() -> Result<String, String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg("--print")
        .arg("sysroot")
        .output()
        .map_err(|e| format!("failed to execute 'rustc --print sysroot': {}", e))?;

    if !output.status.success() {
        return Err("'rustc --print sysroot' failed".to_string());
    }

    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| "sysroot is not valid UTF-8".to_string())
}

fn get_host() -> Result<String, String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg("-vV")
        .output()
        .map_err(|e| format!("failed to execute 'rustc -vV': {}", e))?;

    if !output.status.success() {
        return Err("'rustc -vV' failed".to_string());
    }

    let stdout =
        String::from_utf8(output.stdout).map_err(|_| "'rustc -vV' output is not valid UTF-8")?;

    for line in stdout.lines() {
        if let Some(host) = line.strip_prefix("host: ") {
            return Ok(host.to_string());
        }
    }

    Err("could not determine host target from 'rustc -vV'".to_string())
}
