//! Parsing helpers for llvm-readobj output.

use std::io;

use super::{BinaryFormat, SectionInfo};

impl BinaryFormat {
    /// Detect binary format from llvm-readobj output.
    /// Looks for "Format:" line in the first few lines.
    pub(crate) fn detect(output: &str) -> Option<Self> {
        for line in output.lines().take(5) {
            if let Some(format_str) = line.strip_prefix("Format:") {
                let format_str = format_str.trim().to_lowercase();
                if format_str.starts_with("elf") {
                    return Some(Self::Elf);
                } else if format_str.starts_with("mach-o") {
                    return Some(Self::MachO);
                } else if format_str.starts_with("coff") {
                    return Some(Self::Coff);
                }
            }
        }
        None
    }
}

/// Helper to extract name from "Name: foo (hex...)" or "Segment: bar (hex...)"
fn extract_name(line: &str, prefix: &str) -> Option<String> {
    let part = line.strip_prefix(prefix)?;
    let name = match part.find('(') {
        Some(idx) => part[..idx].trim(),
        None => part.trim(),
    };
    Some(name.to_string())
}

/// Helper to parse size from a string (decimal or hex with 0x prefix).
fn parse_size(size_str: &str) -> io::Result<usize> {
    let size_str = size_str.trim();
    if let Some(hex) = size_str.strip_prefix("0x") {
        usize::from_str_radix(hex, 16)
    } else {
        size_str.parse::<usize>()
    }
    .map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse section size '{}': {}", size_str, e),
        )
    })
}

/// Parse ELF section info from llvm-readobj output.
///
/// ELF format:
/// ```text
/// Section {
///   Index: 16
///   Name: ver_stub (472)
///   Type: SHT_PROGBITS (0x1)
///   Flags [ (0x2)
///     SHF_ALLOC (0x2)
///     SHF_WRITE (0x1)   // if writable
///   ]
///   ...
///   Size: 512
/// }
/// ```
pub(super) fn parse_elf_sections(
    output: &str,
    section_name: &str,
) -> io::Result<Option<SectionInfo>> {
    let mut in_target_section = false;
    let mut current_size: Option<usize> = None;
    let mut current_is_writable = false;

    for line in output.lines() {
        let trimmed = line.trim();

        // Track section name
        if let Some(name) = extract_name(trimmed, "Name:") {
            in_target_section = name == section_name;
            continue;
        }

        // Track size
        if in_target_section && let Some(size_str) = trimmed.strip_prefix("Size:") {
            current_size = Some(parse_size(size_str)?);
            continue;
        }

        // Track flags - check for SHF_WRITE
        if in_target_section && trimmed.contains("SHF_WRITE") {
            current_is_writable = true;
            continue;
        }

        // End of section - return if we found our target
        if trimmed == "}"
            && in_target_section
            && let Some(size) = current_size
        {
            return Ok(Some(SectionInfo {
                size,
                is_writable: current_is_writable,
            }));
        }

        // Reset on new section
        if trimmed == "Section {" {
            in_target_section = false;
            current_size = None;
            current_is_writable = false;
        }
    }

    Ok(None)
}

/// Parse Mach-O section info from llvm-readobj output.
///
/// Mach-O format:
/// ```text
/// Section {
///   Index: 0
///   Name: ver_stub (76 65 72...)
///   Segment: __TEXT (5F5F...)
///   ...
///   Size: 0x200
/// }
/// ```
///
/// For Mach-O, section_name can be either just the section name (e.g., "ver_stub")
/// or "segment,section" format (e.g., "__TEXT,ver_stub").
/// Writability is determined by segment: __DATA is writable, __TEXT is not.
pub(super) fn parse_macho_sections(
    output: &str,
    section_name: &str,
) -> io::Result<Option<SectionInfo>> {
    let mut current_segment: Option<String> = None;
    let mut current_name: Option<String> = None;
    let mut in_target_section = false;
    let mut current_size: Option<usize> = None;
    let mut current_is_writable = false;

    // Check if section_name is in "segment,section" format
    let (target_segment, target_section) = if let Some(idx) = section_name.find(',') {
        (Some(&section_name[..idx]), &section_name[idx + 1..])
    } else {
        (None, section_name)
    };

    for line in output.lines() {
        let trimmed = line.trim();

        // Track segment
        if let Some(seg) = extract_name(trimmed, "Segment:") {
            current_segment = Some(seg.clone());
            // On Mach-O, __DATA segment is writable, __TEXT is not
            current_is_writable = seg == "__DATA";
            // Check if we now match the target section
            if let Some(ref name) = current_name {
                in_target_section =
                    matches_macho_section(&current_segment, name, target_segment, target_section);
            }
            continue;
        }

        // Track section name
        if let Some(name) = extract_name(trimmed, "Name:") {
            current_name = Some(name.clone());
            in_target_section =
                matches_macho_section(&current_segment, &name, target_segment, target_section);
            continue;
        }

        // Track size
        if in_target_section && let Some(size_str) = trimmed.strip_prefix("Size:") {
            current_size = Some(parse_size(size_str)?);
            continue;
        }

        // End of section - return if we found our target
        if trimmed == "}"
            && in_target_section
            && let Some(size) = current_size
        {
            return Ok(Some(SectionInfo {
                size,
                is_writable: current_is_writable,
            }));
        }

        // Reset on new section
        if trimmed == "Section {" {
            current_segment = None;
            current_name = None;
            in_target_section = false;
            current_size = None;
            current_is_writable = false;
        }
    }

    Ok(None)
}

/// Check if a Mach-O section matches the target.
fn matches_macho_section(
    current_segment: &Option<String>,
    current_name: &str,
    target_segment: Option<&str>,
    target_section: &str,
) -> bool {
    // Section name must match
    if current_name != target_section {
        return false;
    }
    // If target specifies a segment, it must match too
    if let Some(target_seg) = target_segment {
        if let Some(current_seg) = current_segment {
            return current_seg == target_seg;
        }
        // Haven't seen segment yet, can't confirm match
        return false;
    }
    // No segment specified in target, name match is enough
    true
}

/// Parse COFF/PE section info from llvm-readobj output.
///
/// COFF format:
/// ```text
/// Section {
///   Number: 5
///   Name: ver_stub (76 65 72 5F 73 74 75 62)
///   VirtualSize: 0x200
///   VirtualAddress: 0x27000
///   RawDataSize: 512
///   ...
///   Characteristics [ (0x40000040)
///     IMAGE_SCN_CNT_INITIALIZED_DATA (0x40)
///     IMAGE_SCN_MEM_READ (0x40000000)
///     IMAGE_SCN_MEM_WRITE (0x80000000)  // if writable
///   ]
/// }
/// ```
///
/// Note: COFF section names are limited to 8 characters. We use `ver_stub` (8 chars)
/// to fit within this limit.
pub(super) fn parse_coff_sections(
    output: &str,
    section_name: &str,
) -> io::Result<Option<SectionInfo>> {
    let mut in_target_section = false;
    let mut current_size: Option<usize> = None;
    let mut current_is_writable = false;

    for line in output.lines() {
        let trimmed = line.trim();

        // Track section name
        if let Some(name) = extract_name(trimmed, "Name:") {
            in_target_section = name == section_name;
            continue;
        }

        // Track size (COFF uses RawDataSize)
        if in_target_section && let Some(size_str) = trimmed.strip_prefix("RawDataSize:") {
            current_size = Some(parse_size(size_str)?);
            continue;
        }

        // Track characteristics - check for IMAGE_SCN_MEM_WRITE
        if in_target_section && trimmed.contains("IMAGE_SCN_MEM_WRITE") {
            current_is_writable = true;
            continue;
        }

        // End of section - return if we found our target
        if trimmed == "}"
            && in_target_section
            && let Some(size) = current_size
        {
            return Ok(Some(SectionInfo {
                size,
                is_writable: current_is_writable,
            }));
        }

        // Reset on new section
        if trimmed == "Section {" {
            in_target_section = false;
            current_size = None;
            current_is_writable = false;
        }
    }

    Ok(None)
}
