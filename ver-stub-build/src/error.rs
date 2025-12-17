//! Error types for ver-stub-build operations.

use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// Error type for ver-stub-build operations.
#[derive(Debug)]
pub enum Error {
    /// Failed to write section data file.
    WriteSectionFile { path: PathBuf, source: io::Error },

    /// Failed to find LLVM tools.
    LlvmToolsNotFound { source: io::Error },

    /// Failed to get section info from binary.
    GetSectionInfo {
        binary_path: PathBuf,
        source: io::Error,
    },

    /// Failed to update section in binary.
    UpdateSection {
        binary_path: PathBuf,
        source: io::Error,
    },

    /// Failed to copy binary.
    CopyBinary {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WriteSectionFile { path, source } => {
                write!(
                    f,
                    "failed to write section file to {}: {}",
                    path.display(),
                    source
                )
            }
            Error::LlvmToolsNotFound { source } => {
                write!(
                    f,
                    "could not find LLVM tools directory: {}\n\
                     Please install llvm-tools: rustup component add llvm-tools",
                    source
                )
            }
            Error::GetSectionInfo {
                binary_path,
                source,
            } => {
                write!(
                    f,
                    "failed to find section from {}: {}",
                    binary_path.display(),
                    source
                )
            }
            Error::UpdateSection {
                binary_path,
                source,
            } => {
                write!(
                    f,
                    "failed to update section in {}: {}",
                    binary_path.display(),
                    source
                )
            }
            Error::CopyBinary { from, to, source } => {
                write!(
                    f,
                    "failed to copy {} to {}: {}",
                    from.display(),
                    to.display(),
                    source
                )
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::WriteSectionFile { source, .. } => Some(source),
            Error::LlvmToolsNotFound { source } => Some(source),
            Error::GetSectionInfo { source, .. } => Some(source),
            Error::UpdateSection { source, .. } => Some(source),
            Error::CopyBinary { source, .. } => Some(source),
        }
    }
}
