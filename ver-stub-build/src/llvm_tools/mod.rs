//! LLVM tools wrapper for section manipulation.

mod parsing;

use std::env::consts::EXE_SUFFIX;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::rustc;
use parsing::{BinaryFormat, parse_coff_sections, parse_elf_sections, parse_macho_sections};

/// Information about a section in a binary.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SectionInfo {
    /// Size of the section in bytes.
    pub size: usize,
    /// Whether the section is writable (has SHF_WRITE on ELF, or is in __DATA segment on Mach-O).
    pub is_writable: bool,
}

/// Wrapper for LLVM tools (llvm-readobj, llvm-objcopy).
///
/// This provides access to LLVM tools from the Rust toolchain for reading
/// and modifying ELF sections in binaries.
pub struct LlvmTools {
    bin_dir: PathBuf,
}

impl LlvmTools {
    /// Creates a new `LlvmTools` instance by locating the LLVM tools directory.
    pub fn new() -> Result<Self, String> {
        let bin_dir = rustc::llvm_tools_bin_dir()?;
        Ok(Self { bin_dir })
    }

    /// Gets information about a section in a binary.
    ///
    /// Returns `Ok(Some(SectionInfo))` if the section exists, `Ok(None)` if it doesn't,
    /// or `Err` if there was an error executing llvm-readobj or parsing the output.
    pub fn get_section_info(
        &self,
        bin: impl AsRef<Path>,
        section_name: &str,
    ) -> io::Result<Option<SectionInfo>> {
        let bin = bin.as_ref();
        let readobj_path = self.bin_dir.join(format!("llvm-readobj{}", EXE_SUFFIX));

        let output = Command::new(&readobj_path)
            .arg("--sections")
            .arg(bin)
            .output()?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("llvm-readobj failed with status {}", output.status);
            eprintln!("stdout:\n{}", stdout);
            eprintln!("stderr:\n{}", stderr);
            return Err(io::Error::other(format!(
                "llvm-readobj failed with status {}",
                output.status
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Detect binary format and dispatch to appropriate parser
        match BinaryFormat::detect(&stdout) {
            BinaryFormat::Elf => parse_elf_sections(&stdout, section_name),
            BinaryFormat::MachO => parse_macho_sections(&stdout, section_name),
            BinaryFormat::Coff => parse_coff_sections(&stdout, section_name),
            BinaryFormat::Unknown => {
                eprintln!("Could not detect binary format. llvm-readobj output:");
                eprintln!("{}", stdout);
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "could not detect binary format from llvm-readobj output",
                ))
            }
        }
    }

    /// Gets the size of a section in a binary.
    ///
    /// Returns `Ok(Some(size))` if the section exists, `Ok(None)` if it doesn't,
    /// or `Err` if there was an error executing llvm-readobj or parsing the output.
    ///
    /// This is a convenience wrapper around `get_section_info` that returns only the size.
    pub fn get_section_size(
        &self,
        bin: impl AsRef<Path>,
        section_name: &str,
    ) -> io::Result<Option<usize>> {
        self.get_section_info(bin, section_name)
            .map(|info| info.map(|i| i.size))
    }

    /// Updates a section in a binary using llvm-objcopy.
    ///
    /// Returns `Ok(())` on success, or `Err` if there was an error executing
    /// llvm-objcopy or if it exited with a non-zero status.
    pub fn update_section(
        &self,
        input: impl AsRef<Path>,
        output: impl AsRef<Path>,
        section_name: &str,
        section_file: impl AsRef<Path>,
    ) -> io::Result<()> {
        let input = input.as_ref();
        let output = output.as_ref();
        let section_file = section_file.as_ref();

        let objcopy_path = self.bin_dir.join(format!("llvm-objcopy{}", EXE_SUFFIX));
        let update_arg = format!("{}={}", section_name, section_file.display());

        let cmd_output = Command::new(&objcopy_path)
            .arg("--update-section")
            .arg(&update_arg)
            .arg(input)
            .arg(output)
            .output()?;

        if !cmd_output.status.success() {
            let stdout = String::from_utf8_lossy(&cmd_output.stdout);
            let stderr = String::from_utf8_lossy(&cmd_output.stderr);
            eprintln!("llvm-objcopy failed with status {}", cmd_output.status);
            eprintln!("stdout:\n{}", stdout);
            eprintln!("stderr:\n{}", stderr);
            return Err(io::Error::other(format!(
                "llvm-objcopy failed with status {}",
                cmd_output.status
            )));
        }

        Ok(())
    }

    /// Updates a section in a binary using llvm-objcopy, reading section data from bytes.
    ///
    /// This pipes the bytes directly to objcopy via stdin, avoiding the need for a
    /// temporary file. Works outside of build.rs context.
    ///
    /// Returns `Ok(())` on success, or `Err` if there was an error executing
    /// llvm-objcopy or if it exited with a non-zero status.
    pub fn update_section_with_bytes(
        &self,
        input: impl AsRef<Path>,
        output: impl AsRef<Path>,
        section_name: &str,
        bytes: &[u8],
    ) -> io::Result<()> {
        let input = input.as_ref();
        let output = output.as_ref();

        let objcopy_path = self.bin_dir.join(format!("llvm-objcopy{}", EXE_SUFFIX));
        let update_arg = format!("{}=/dev/stdin", section_name);

        let mut child = Command::new(&objcopy_path)
            .arg("--update-section")
            .arg(&update_arg)
            .arg(input)
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Write bytes to stdin and close the pipe
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("failed to open stdin"))?;
        stdin.write_all(bytes)?;
        drop(stdin); // Close the pipe

        let output = child.wait_with_output()?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("llvm-objcopy failed with status {}", output.status);
            eprintln!("stdout:\n{}", stdout);
            eprintln!("stderr:\n{}", stderr);
            return Err(io::Error::other(format!(
                "llvm-objcopy failed with status {}",
                output.status
            )));
        }

        Ok(())
    }
}
