//! Update section command for patching artifact dependency binaries.

use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::{Path, PathBuf};

use ver_stub::SECTION_NAME;

use crate::LinkSection;
use crate::cargo_helpers::{self, cargo_rerun_if, cargo_warning};
use crate::llvm_tools::LlvmTools;

/// Builder for updating sections in a binary.
///
/// Created by calling `LinkSection::patch_into()` or `LinkSection::patch_into_bin_dep()`.
#[must_use]
pub struct UpdateSectionCommand {
    pub(crate) link_section: LinkSection,
    pub(crate) bin_path: PathBuf,
    pub(crate) new_name: Option<String>,
}

impl UpdateSectionCommand {
    /// Sets a custom filename for the output binary.
    ///
    /// This can only be used when:
    /// - The argument to `write_to()` is a directory, or
    /// - Using `write_to_target_profile_dir()`
    ///
    /// If `write_to()` is called with a file path (not a directory), this will panic.
    ///
    /// If not called, the default name is `{original_name}.bin`.
    pub fn with_filename(mut self, name: &str) -> Self {
        self.new_name = Some(name.to_string());
        self
    }

    /// Writes the patched binary to the specified path.
    ///
    /// If the path is a directory, the output filename will be determined by
    /// `with_filename()` if set, otherwise defaults to `{original_name}.bin`.
    ///
    /// If the path is not a directory, writes directly to that path. In this case,
    /// `with_filename()` must not have been called (will panic if it was).
    ///
    /// If the section doesn't exist in the input binary, a warning is logged and the
    /// binary is copied without modification.
    pub fn write_to(self, path: impl AsRef<Path>) {
        eprintln!("ver-stub-build: input binary = {}", self.bin_path.display());

        // Emit rerun-if-changed for the input binary
        // See: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
        cargo_rerun_if(&format!("changed={}", self.bin_path.display()));

        // Determine output path
        let path = path.as_ref();
        let output_path = if path.is_dir() {
            // Directory: use new_name if set, otherwise default to {original_name}.bin
            let original_name = self
                .bin_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            // Strip .exe suffix if present, add .bin, then re-add platform suffix
            // Unix: my_prog -> my_prog.bin
            // Windows: my_prog.exe -> my_prog.bin.exe
            let base_name = original_name
                .strip_suffix(EXE_SUFFIX)
                .unwrap_or(original_name);
            let default_name = format!("{}.bin{}", base_name, EXE_SUFFIX);
            let output_name = self.new_name.as_deref().unwrap_or(&default_name);
            path.join(output_name)
        } else {
            // File path: write directly, but panic if with_filename was used
            if self.new_name.is_some() {
                panic!(
                    "ver-stub-build: with_filename() cannot be used when write_to() \
                     is called with a file path (not a directory): {}",
                    path.display()
                );
            }
            path.to_path_buf()
        };

        let llvm = LlvmTools::new().unwrap_or_else(|e| {
            panic!(
                "ver-stub-build: could not find LLVM tools directory: {}\n\
                 Please install llvm-tools: rustup component add llvm-tools",
                e
            )
        });

        // Get section info from the binary
        let section_info = llvm
            .get_section_info(&self.bin_path, SECTION_NAME)
            .unwrap_or_else(|e| {
                panic!(
                    "ver-stub-build: failed to read section info from {}: {}",
                    self.bin_path.display(),
                    e
                )
            });

        match section_info {
            Some(info) => {
                // Warn if section is writable (should be read-only for security)
                if info.is_writable {
                    cargo_warning(&format!(
                        "section '{}' is writable; consider placing it in a read-only segment",
                        SECTION_NAME
                    ));
                }

                // Build section data with the correct buffer size from the binary
                let section_bytes = self
                    .link_section
                    .with_buffer_size(info.size)
                    .build_section_bytes();

                llvm.update_section_with_bytes(
                    &self.bin_path,
                    &output_path,
                    SECTION_NAME,
                    &section_bytes,
                )
                .unwrap_or_else(|e| {
                    panic!(
                        "ver-stub-build: failed to update section in {}: {}",
                        self.bin_path.display(),
                        e
                    )
                });
                eprintln!(
                    "ver-stub-build: wrote patched binary to {}",
                    output_path.display()
                );
            }
            None => {
                // Section doesn't exist, copy binary without modification
                cargo_warning(&format!(
                    "section '{}' not found in {}, copying without modification",
                    SECTION_NAME,
                    self.bin_path.display()
                ));
                fs::copy(&self.bin_path, &output_path).unwrap_or_else(|e| {
                    panic!(
                        "ver-stub-build: failed to copy {} to {}: {}",
                        self.bin_path.display(),
                        output_path.display(),
                        e
                    )
                });
                eprintln!("ver-stub-build: copied to {}", output_path.display());
            }
        }
    }

    /// Writes the patched binary to the target profile directory (e.g., `target/debug/`).
    ///
    /// NOTE: Copying things to target dir is not expressly supported by cargo devs.
    /// If you clobber a binary that cargo generates, it may trigger unnecessary rebuilds later.
    /// However, it typically works fine.
    ///
    /// See also:
    /// - <https://github.com/rust-lang/cargo/issues/9661#issuecomment-1769481293>
    /// - <https://github.com/rust-lang/cargo/issues/9661#issuecomment-2159267601>
    /// - <https://github.com/rust-lang/cargo/issues/13663>
    pub fn write_to_target_profile_dir(self) {
        let target_dir = cargo_helpers::target_profile_dir();
        self.write_to(target_dir);
    }
}
