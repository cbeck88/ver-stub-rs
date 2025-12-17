//! Write the section format used by [`ver_stub`], and inject it into binaries.
//!
//! This crate can be used from
//! - `build.rs` scripts
//! - Standalone binary tools, such as `ver-stub-tool`.
//!
//! # Quickstart
//!
//! Create a [`LinkSection`], tell it what contents you want it to have, and then
//! do something with it -- either write the bytes out to a file, or patch them
//! into a binary that is using [`ver_stub`].
//!
//! In your build.rs:
//! ```ignore
//! use ver_stub_build::LinkSection;
//!
//! fn main() {
//!     // Patch the `my-bin` executable, producing `my-bin.bin` in the target profile dir.
//!     LinkSection::new()
//!         .with_all_git()
//!         .patch_into_bin_dep("my-dep", "my-bin")
//!         .write_to_target_profile_dir()
//!         .unwrap();
//!
//!     // Or with a custom output name
//!     LinkSection::new()
//!         .with_all_git()
//!         .patch_into_bin_dep("my-dep", "my-bin")
//!         .with_filename("my-custom-name")
//!         .write_to_target_profile_dir()
//!         .unwrap();
//!
//!     // Or at a custom destination
//!     LinkSection::new()
//!         .with_all_git()
//!         .patch_into_bin_dep("my-dep", "my-bin")
//!         .write_to("dist/my-bin")
//!         .unwrap();
//! }
//! ```
//!
//! NOTE: [`LinkSection::patch_into_bin_dep`] requires cargo's unstable artifact dependencies feature.
//! You must use nightly cargo, and enable "bindeps" in `.cargo/config.toml`.
//! Then it finds the file to be patched using `CARGO_BIN_FILE_*` env vars.
//!
//! More generally, you can use it without artifact dependencies, to do things
//! similar to what [`ver-stub-tool`](https://docs.rs/ver-stub-tool/latest) does.
//!
//! ```ignore
//! use ver_stub_build::LinkSection;
//!
//! fn main() {
//!     // Patch a binary at a specific path
//!     LinkSection::new()
//!         .with_all_git()
//!         .patch_into("/path/to/binary")
//!         .write_to_target_profile_dir()
//!         .unwrap();
//!
//!     // Or with a custom output name
//!     LinkSection::new()
//!         .with_all_git()
//!         .patch_into("/path/to/binary")
//!         .with_filename("my-custom-name")
//!         .write_to_target_profile_dir()
//!         .unwrap();
//!
//!     // Or just write the section data file (for use with cargo-objcopy)
//!     LinkSection::new()
//!         .with_all_git()
//!         .write_to_out_dir()
//!         .unwrap();
//! }
//! ```

#![deny(missing_docs)]

/// Cargo build script helper functions.
mod cargo_helpers;

/// Error types for ver-stub-build operations.
mod error;

/// Helpers for interacting with git
mod git_helpers;

/// LLVM tools wrapper for section manipulation.
mod llvm_tools;

/// Helper to find LLVM tools, based on code in cargo-binutils.
mod rustc;

/// Update section command for patching artifact dependency binaries.
mod update_section;

pub use error::Error;
pub use llvm_tools::{BinaryFormat, LlvmTools, SectionInfo};
pub use update_section::{UpdateSectionCommand, platform_section_name};
pub use ver_stub::SECTION_NAME;

use chrono::{DateTime, TimeZone, Utc};
use std::{
    fs,
    path::{Path, PathBuf},
};
use ver_stub::{BUFFER_SIZE, Member, header_size};

use cargo_helpers::{cargo_rerun_if, cargo_warning};
use git_helpers::{
    emit_git_rerun_if_changed, get_git_branch, get_git_commit_msg, get_git_commit_timestamp,
    get_git_describe, get_git_sha,
};

/// Builder for configuring which git information to include in version sections.
///
/// Use this to select which git info to collect, then either:
/// - Call `write_to()` or `write_to_out_dir()` to just write the section data file
/// - Call `patch_into()` to get an `UpdateSectionCommand` for patching a binary
#[derive(Default)]
#[must_use]
pub struct LinkSection {
    include_git_sha: bool,
    include_git_describe: bool,
    include_git_branch: bool,
    include_git_commit_timestamp: bool,
    include_git_commit_date: bool,
    include_git_commit_msg: bool,
    include_build_timestamp: bool,
    include_build_date: bool,
    fail_on_error: bool,
    custom: Option<String>,
    buffer_size: Option<usize>,
}

impl LinkSection {
    /// Creates a new empty `LinkSection`
    pub fn new() -> Self {
        Self::default()
    }

    /// Includes the git SHA (`git rev-parse HEAD`) in the section data.
    pub fn with_git_sha(mut self) -> Self {
        self.include_git_sha = true;
        self
    }

    /// Includes the git describe output (`git describe --always --dirty`) in the section data.
    pub fn with_git_describe(mut self) -> Self {
        self.include_git_describe = true;
        self
    }

    /// Includes the git branch name (`git rev-parse --abbrev-ref HEAD`) in the section data.
    pub fn with_git_branch(mut self) -> Self {
        self.include_git_branch = true;
        self
    }

    /// Includes the git commit timestamp (RFC 3339 format) in the section data.
    pub fn with_git_commit_timestamp(mut self) -> Self {
        self.include_git_commit_timestamp = true;
        self
    }

    /// Includes the git commit date (YYYY-MM-DD format) in the section data.
    pub fn with_git_commit_date(mut self) -> Self {
        self.include_git_commit_date = true;
        self
    }

    /// Includes the git commit message (first line, max 100 chars) in the section data.
    pub fn with_git_commit_msg(mut self) -> Self {
        self.include_git_commit_msg = true;
        self
    }

    /// Includes all git information in the section data.
    pub fn with_all_git(mut self) -> Self {
        self.include_git_sha = true;
        self.include_git_describe = true;
        self.include_git_branch = true;
        self.include_git_commit_timestamp = true;
        self.include_git_commit_date = true;
        self.include_git_commit_msg = true;
        self
    }

    /// Includes the build timestamp (RFC 3339 format, UTC) in the section data.
    pub fn with_build_timestamp(mut self) -> Self {
        self.include_build_timestamp = true;
        self
    }

    /// Includes the build date (YYYY-MM-DD format, UTC) in the section data.
    pub fn with_build_date(mut self) -> Self {
        self.include_build_date = true;
        self
    }

    /// Includes all build time information (timestamp and date) in the section data.
    pub fn with_all_build_time(mut self) -> Self {
        self.include_build_timestamp = true;
        self.include_build_date = true;
        self
    }

    /// Enables fail-on-error mode.
    ///
    /// By default, if git commands fail (e.g., `git` not found, not in a git repository,
    /// building from a source tarball without `.git`), a `cargo:warning` is emitted and
    /// the corresponding data is skipped. This allows builds to succeed even without git.
    ///
    /// When `fail_on_error()` is called, git failures will instead cause a panic,
    /// failing the build.
    pub fn fail_on_error(mut self) -> Self {
        self.fail_on_error = true;
        self
    }

    /// Sets a custom application-specific string to embed in the binary.
    ///
    /// This can be any string your application wants to store. The total size of all
    /// data (including git info, timestamps, and custom string) must fit within the
    /// buffer size (default 512 bytes). If you need more space, set the
    /// `VER_STUB_BUFFER_SIZE` environment variable when building.
    ///
    /// As with any build script, you must emit `cargo:rerun-if-...` directives as
    /// needed if you read files or environment variables to build your custom string.
    ///
    /// Access this at runtime with `ver_stub::custom()`.
    pub fn with_custom(mut self, s: impl Into<String>) -> Self {
        self.custom = Some(s.into());
        self
    }

    /// Sets the buffer size for the section data.
    ///
    /// This should match the buffer size used when building the target binary.
    /// If not set, falls back to:
    /// 1. `VER_STUB_BUFFER_SIZE` environment variable (at runtime)
    /// 2. The `BUFFER_SIZE` constant from ver-stub (default 512)
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = Some(size);
        self
    }

    /// Gets the effective buffer size to use.
    fn effective_buffer_size(&self) -> usize {
        self.buffer_size
            .or_else(|| {
                std::env::var("VER_STUB_BUFFER_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(BUFFER_SIZE)
    }

    /// Builds the section data as bytes.
    ///
    /// This collects all enabled version info and builds the binary section data.
    /// Does not write to any file.
    pub fn build_section_bytes(self) -> Vec<u8> {
        self.check_enabled();

        // Emit rerun-if-changed directives for git state (only if git data requested)
        if self.any_git_enabled() {
            emit_git_rerun_if_changed();
        }

        // Collect the data for each member
        let mut member_data: [Option<String>; Member::COUNT] = Default::default();

        if self.include_git_sha
            && let Some(git_sha) = get_git_sha(self.fail_on_error)
        {
            eprintln!("ver-stub-build: git SHA = {}", git_sha);
            member_data[Member::GitSha as usize] = Some(git_sha);
        }

        if self.include_git_describe
            && let Some(git_describe) = get_git_describe(self.fail_on_error)
        {
            eprintln!("ver-stub-build: git describe = {}", git_describe);
            member_data[Member::GitDescribe as usize] = Some(git_describe);
        }

        if self.include_git_branch
            && let Some(git_branch) = get_git_branch(self.fail_on_error)
        {
            eprintln!("ver-stub-build: git branch = {}", git_branch);
            member_data[Member::GitBranch as usize] = Some(git_branch);
        }

        if (self.include_git_commit_timestamp || self.include_git_commit_date)
            && let Some(timestamp) = get_git_commit_timestamp(self.fail_on_error)
        {
            if self.include_git_commit_timestamp {
                let rfc3339 = timestamp.to_rfc3339();
                eprintln!("ver-stub-build: git commit timestamp = {}", rfc3339);
                member_data[Member::GitCommitTimestamp as usize] = Some(rfc3339);
            }
            if self.include_git_commit_date {
                let date = timestamp.date_naive().to_string();
                eprintln!("ver-stub-build: git commit date = {}", date);
                member_data[Member::GitCommitDate as usize] = Some(date);
            }
        }

        if self.include_git_commit_msg
            && let Some(msg) = get_git_commit_msg(self.fail_on_error)
        {
            eprintln!("ver-stub-build: git commit msg = {}", msg);
            member_data[Member::GitCommitMsg as usize] = Some(msg);
        }

        if self.any_build_time_enabled() {
            // Emit rerun-if-env-changed for reproducible build options
            cargo_rerun_if("env-changed=VER_STUB_IDEMPOTENT");
            cargo_rerun_if("env-changed=VER_STUB_BUILD_TIME");

            // VER_STUB_IDEMPOTENT takes precedence: if set, never include build time
            if std::env::var("VER_STUB_IDEMPOTENT").is_ok() {
                eprintln!(
                    "ver-stub-build: VER_STUB_IDEMPOTENT is set, skipping build timestamp/date"
                );
            } else {
                let build_time = get_build_time();
                if self.include_build_timestamp {
                    let rfc3339 = build_time.to_rfc3339();
                    eprintln!("ver-stub-build: build timestamp = {}", rfc3339);
                    member_data[Member::BuildTimestamp as usize] = Some(rfc3339);
                }
                if self.include_build_date {
                    let date = build_time.date_naive().to_string();
                    eprintln!("ver-stub-build: build date = {}", date);
                    member_data[Member::BuildDate as usize] = Some(date);
                }
            }
        }

        if let Some(ref custom) = self.custom {
            eprintln!("ver-stub-build: custom = {}", custom);
            member_data[Member::Custom as usize] = Some(custom.clone());
        }

        // Build the section buffer
        let buffer_size = self.effective_buffer_size();
        build_section_buffer(&member_data, buffer_size)
    }
    /// Writes the section data file to the specified path.
    ///
    /// If the path is a directory, writes to `{path}/ver_stub_data`.
    /// Otherwise writes directly to the path.
    ///
    /// This is useful for `cargo objcopy` workflows where you want to manually
    /// run objcopy with the generated section file.
    ///
    /// Returns the path to the written file.
    pub fn write_to(self, path: impl AsRef<Path>) -> Result<PathBuf, Error> {
        self.write_section_to_path(path.as_ref())
    }

    /// Writes the section data file to `OUT_DIR/ver_stub_data`.
    ///
    /// This is a convenience method for use in build scripts.
    ///
    /// Returns the path to the written file.
    pub fn write_to_out_dir(self) -> Result<PathBuf, Error> {
        let out_dir = cargo_helpers::out_dir();
        self.write_section_to_path(&out_dir)
    }

    /// Writes the section data file to the `target/` directory.
    /// Returns the path to the written file (e.g., `target/ver_stub_data`).
    ///
    /// This is useful for `cargo objcopy` workflows where you want to run:
    /// ```bash
    /// cargo objcopy --release --bin my_bin -- --update-section ver_stub=target/ver_stub_data my_bin.bin
    /// ```
    ///
    /// The target directory is determined by checking `CARGO_TARGET_DIR` first,
    /// then inferring from `OUT_DIR`. The result should typically be `target/ver_stub_data`.
    ///
    /// When cross-compiling, it might end up in `target/<triple>/ver_stub_data`, due to
    /// how the inference works.
    ///
    /// To adjust this, you can set `CARGO_TARGET_DIR` in `.cargo/config.toml`:
    /// ```toml
    /// [env]
    /// CARGO_TARGET_DIR = { value = "target", relative = true }
    /// ```
    pub fn write_to_target_dir(self) -> Result<PathBuf, Error> {
        let target_dir = cargo_helpers::target_dir();
        self.write_section_to_path(&target_dir)
    }

    /// Transitions to an `UpdateSectionCommand` for patching a binary at the given path.
    ///
    /// # Arguments
    /// * `binary_path` - Path to the binary to patch
    pub fn patch_into(self, binary_path: impl AsRef<Path>) -> UpdateSectionCommand {
        UpdateSectionCommand {
            link_section: self,
            bin_path: binary_path.as_ref().to_path_buf(),
            new_name: None,
            dry_run: false,
        }
    }

    /// Transitions to an `UpdateSectionCommand` for patching an artifact dependency binary.
    ///
    /// This is a convenience method for use with Cargo's artifact dependencies feature.
    /// It finds the binary using the `CARGO_BIN_FILE_<DEP>_<NAME>` environment variables
    /// that Cargo sets for artifact dependencies.
    ///
    /// # Arguments
    /// * `dep_name` - The name of the dependency as specified in Cargo.toml
    /// * `bin_name` - The name of the binary within the dependency
    pub fn patch_into_bin_dep(self, dep_name: &str, bin_name: &str) -> UpdateSectionCommand {
        let bin_path = cargo_helpers::find_artifact_binary(dep_name, bin_name);
        self.patch_into(bin_path)
    }

    fn any_git_enabled(&self) -> bool {
        self.include_git_sha
            || self.include_git_describe
            || self.include_git_branch
            || self.include_git_commit_timestamp
            || self.include_git_commit_date
            || self.include_git_commit_msg
    }

    fn any_build_time_enabled(&self) -> bool {
        self.include_build_timestamp || self.include_build_date
    }

    fn check_enabled(&self) {
        if !self.any_git_enabled() && !self.any_build_time_enabled() && self.custom.is_none() {
            panic!(
                "ver-stub-build: no version info enabled. Call with_git_sha(), with_git_describe(), \
                 with_git_branch(), with_git_commit_timestamp(), with_git_commit_date(), \
                 with_git_commit_msg(), with_all_git(), with_build_timestamp(), with_build_date(), \
                 or with_custom() before writing."
            );
        }
    }

    pub(crate) fn write_section_to_path(self, path: &Path) -> Result<PathBuf, Error> {
        let buffer = self.build_section_bytes();

        // Write to file - if path is a directory, append ver_stub_data
        let output_path = if path.is_dir() {
            path.join("ver_stub_data")
        } else {
            path.to_path_buf()
        };
        fs::write(&output_path, &buffer).map_err(|source| Error::WriteSectionFile {
            path: output_path.clone(),
            source,
        })?;

        Ok(output_path)
    }
}

/// Builds the section buffer from member data.
///
/// Format:
/// - First byte: number of members (Member::COUNT) for forward compatibility
/// - Next `Member::COUNT * 2` bytes: header with end offsets (u16, little-endian, relative to header)
/// - Remaining bytes: concatenated string data
///
/// Header size = 1 + Member::COUNT * 2
///
/// For member N:
/// - start = header_size + end[N-1] if N > 0, else header_size
/// - end = header_size + end[N]
/// - If start == end, the member is not present.
///
/// Using relative offsets means a zero-initialized buffer reads as "all members absent".
/// The num_members byte enables forward compatibility: old sections can be read by new code.
fn build_section_buffer(
    member_data: &[Option<String>; Member::COUNT],
    buffer_size: usize,
) -> Vec<u8> {
    let mut buffer = vec![0u8; buffer_size];
    let header_sz = header_size(Member::COUNT);

    // First byte: number of members
    buffer[0] = Member::COUNT as u8;

    // Data starts after the header; track position relative to header_size
    let mut relative_offset: usize = 0;

    for (idx, data) in member_data.iter().enumerate() {
        if let Some(s) = data {
            let bytes = s.as_bytes();
            let absolute_start = header_sz + relative_offset;
            let absolute_end = absolute_start + bytes.len();

            if absolute_end > buffer_size {
                panic!(
                    "ver-stub-build: section data too large ({} bytes, max {}). \
                     Use with_buffer_size() or set VER_STUB_BUFFER_SIZE env var to increase.",
                    absolute_end, buffer_size
                );
            }

            // Write the data
            buffer[absolute_start..absolute_end].copy_from_slice(bytes);

            relative_offset += bytes.len();
        }

        // Write the end offset for this member (relative to header_size)
        // If member is not present, end == previous end, so start == end indicates "not present"
        // Offset positions start at byte 1 (after the num_members byte)
        let header_offset = 1 + idx * 2;
        buffer[header_offset..header_offset + 2]
            .copy_from_slice(&(relative_offset as u16).to_le_bytes());
    }

    buffer
}

// ============================================================================
// Helper functions
// ============================================================================

/// Gets the build time, either from VER_STUB_BUILD_TIME env var or Utc::now().
///
/// If VER_STUB_BUILD_TIME is set, it tries to parse it as:
/// 1. An integer (unix timestamp in seconds)
/// 2. An RFC 3339 datetime string
///
/// This supports reproducible builds by allowing a fixed build time.
fn get_build_time() -> DateTime<Utc> {
    if let Ok(val) = std::env::var("VER_STUB_BUILD_TIME") {
        // Try parsing as unix timestamp (integer) first
        if let Ok(ts) = val.parse::<i64>() {
            let dt = Utc.timestamp_opt(ts, 0).single().unwrap_or_else(|| {
                panic!(
                    "ver-stub-build: VER_STUB_BUILD_TIME '{}' is not a valid unix timestamp",
                    val
                )
            });
            eprintln!(
                "ver-stub-build: using VER_STUB_BUILD_TIME={} (unix timestamp), overriding Utc::now()",
                val
            );
            return dt;
        }

        // Try parsing as RFC 3339
        if let Ok(dt) = DateTime::parse_from_rfc3339(&val) {
            eprintln!(
                "ver-stub-build: using VER_STUB_BUILD_TIME={} (RFC 3339), overriding Utc::now()",
                val
            );
            return dt.with_timezone(&Utc);
        }

        panic!(
            "ver-stub-build: VER_STUB_BUILD_TIME '{}' is not a valid unix timestamp or RFC 3339 datetime",
            val
        );
    }

    Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_section_buffer() {
        let mut args = [const { None }; Member::COUNT];

        args[0] = Some("asdf".into());

        let buf_vec = build_section_buffer(&args, BUFFER_SIZE);
        let buffer: &[u8; BUFFER_SIZE] = (&buf_vec[..]).try_into().unwrap();

        assert_eq!(Member::get_idx_from_buffer(0, &buffer).unwrap(), "asdf");
        for idx in 1..Member::COUNT {
            assert!(Member::get_idx_from_buffer(idx, &buffer).is_none());
        }

        args[2] = Some("jkl;".into());

        let buf_vec = build_section_buffer(&args, BUFFER_SIZE);
        let buffer: &[u8; BUFFER_SIZE] = (&buf_vec[..]).try_into().unwrap();

        assert_eq!(Member::get_idx_from_buffer(0, &buffer).unwrap(), "asdf");
        assert!(Member::get_idx_from_buffer(1, &buffer).is_none());
        assert_eq!(Member::get_idx_from_buffer(2, &buffer).unwrap(), "jkl;");
        for idx in 3..Member::COUNT {
            assert!(Member::get_idx_from_buffer(idx, &buffer).is_none());
        }

        args[5] = Some("nana".into());

        let buf_vec = build_section_buffer(&args, BUFFER_SIZE);
        let buffer: &[u8; BUFFER_SIZE] = (&buf_vec[..]).try_into().unwrap();

        assert_eq!(Member::get_idx_from_buffer(0, &buffer).unwrap(), "asdf");
        assert!(Member::get_idx_from_buffer(1, &buffer).is_none());
        assert_eq!(Member::get_idx_from_buffer(2, &buffer).unwrap(), "jkl;");
        assert!(Member::get_idx_from_buffer(3, &buffer).is_none());
        assert!(Member::get_idx_from_buffer(4, &buffer).is_none());
        assert_eq!(Member::get_idx_from_buffer(5, &buffer).unwrap(), "nana");
        for idx in 6..Member::COUNT {
            assert!(Member::get_idx_from_buffer(idx, &buffer).is_none());
        }
    }
}
