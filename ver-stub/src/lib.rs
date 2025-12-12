//! Runtime access to version data injected via a link section.
//!
//! This crate provides a way to access git version information that has been
//! injected into the binary via a platform-specific link section
//! (`ver_stub` on ELF/COFF, `__TEXT,ver_stub` on Mach-O).
//!
//! The section format is:
//! - First byte: number of members in the section (for forward compatibility)
//! - Next `num_members * 2` bytes: array of end offsets (u16, little-endian, relative to header)
//! - Remaining bytes: concatenated string data
//!
//! Header size = 1 + num_members * 2
//!
//! For member N:
//! - start = header_size + end[N-1] if N > 0, else header_size
//! - end = header_size + end[N]
//! - If start == end, the member is not present.
//! - If N >= num_members (from first byte), the member is not present.
//!
//! Using relative offsets means a zero-initialized buffer reads as "all members absent".
//! The num_members byte enables forward and backwards compatibility: old sections can be read by new code
//! which has more members added in the future, and new sections can be read by old code as well,
//! as long as we never change the index of any existing member.

#![no_std]

// Size of the version data buffer in bytes.
// Can be overridden by setting VER_STUB_BUFFER_SIZE env var at compile time.
// Parsed as u16 since offsets in the header are u16 (max buffer size is 65535).
#[doc(hidden)]
pub const BUFFER_SIZE: usize = match option_env!("VER_STUB_BUFFER_SIZE") {
    Some(s) => match u16::from_str_radix(s, 10) {
        Ok(n) => n as usize,
        Err(_) => panic!("VER_STUB_BUFFER_SIZE must be a valid u16 integer (0-65535)"),
    },
    None => 512,
};

// Calculate header size for a given number of members.
// Header = 1 byte (num_members) + 2 bytes per member (end offsets).
#[doc(hidden)]
pub const fn header_size(num_members: usize) -> usize {
    1 + num_members * 2
}

// Compile-time checks for buffer size validity.
// We use 32 as a minimum threshold because:
// - The header must fit (currently 19 bytes for 9 members)
// - There must be room for actual data
// - Anything smaller than 32 bytes is impractical
// - We want to give clear error messages, so a simpler condition is better.
const _: () = assert!(
    header_size(Member::COUNT) <= 32,
    "header_size(Member::COUNT) exceeds 32, these asserts must be updated"
);
const _: () = assert!(
    BUFFER_SIZE > 32,
    "VER_STUB_BUFFER_SIZE must be greater than 32"
);

/// The section name used for version data (platform-specific).
///
/// On ELF (Linux, etc.) and COFF (Windows): `ver_stub`
/// On Mach-O (macOS): `__TEXT,ver_stub`
///
/// This is useful for scripts that need to use `cargo objcopy` directly.
#[cfg(target_os = "macos")]
pub const SECTION_NAME: &str = "__TEXT,ver_stub";

/// The section name used for version data (platform-specific).
///
/// On ELF (Linux, etc.) and COFF (Windows): `ver_stub`
/// On Mach-O (macOS): `__TEXT,ver_stub`
///
/// This is useful for scripts that need to use `cargo objcopy` directly.
#[cfg(not(target_os = "macos"))]
pub const SECTION_NAME: &str = "ver_stub";

// Members that can be stored in the version data.
#[doc(hidden)]
#[repr(u16)]
#[derive(Clone, Copy)]
pub enum Member {
    GitSha = 0,
    GitDescribe = 1,
    GitBranch = 2,
    GitCommitTimestamp = 3,
    GitCommitDate = 4,
    GitCommitMsg = 5,
    BuildTimestamp = 6,
    BuildDate = 7,
    Custom = 8,
}

impl Member {
    /// Number of members in the version data.
    #[doc(hidden)]
    pub const COUNT: usize = 9;
}

/// Static buffer for version data, placed in a custom link section.
//
// Note: We use "links" in the cargo toml for this crate to try to ensure that
// only one version of this crate appears in the build graph, and so only one
// version of the BUFFER exists, and BUFFER_SIZE = section size.
#[cfg_attr(target_os = "macos", unsafe(link_section = "__TEXT,ver_stub"))]
#[cfg_attr(not(target_os = "macos"), unsafe(link_section = "ver_stub"))]
#[used]
static BUFFER: [u8; BUFFER_SIZE] = [0u8; BUFFER_SIZE];

// Reads a byte from the buffer using volatile read to prevent optimization.
// This is necessary because the compiler would otherwise inline the zeros
// since the buffer is initialized to all zeros at compile time.
#[inline(never)]
fn read_buffer_byte(index: usize) -> u8 {
    // SAFETY: index is bounds-checked by caller, BUFFER is static
    unsafe { core::ptr::read_volatile(BUFFER.as_ptr().add(index)) }
}

// Reads a u16 from the buffer at the given offset (little-endian).
fn read_buffer_u16(offset: usize) -> u16 {
    let lo = read_buffer_byte(offset) as u16;
    let hi = read_buffer_byte(offset + 1) as u16;
    lo | (hi << 8)
}

// Reads a member from the version buffer.
//
// Returns:
// - `None` if the member is not present (start == end, or member >= actual num_members)
// - `Some(&str)` containing the member's string data
//
// Panics:
// - If end < start (invalid range)
// - If end > BUFFER_SIZE (out of bounds)
// - If the data is not valid UTF-8
fn get_member(member: Member) -> Option<&'static str> {
    let idx = member as usize;

    // Read the actual number of members from the first byte
    let actual_num_members = read_buffer_byte(0) as usize;

    // If first byte is 0, section is uninitialized (all zeros)
    if actual_num_members == 0 {
        return None;
    }

    // Forward compatibility: if requested member >= actual num_members, return None
    if idx >= actual_num_members {
        return None;
    }

    // Compute header size based on actual number of members in the section
    let actual_header_size = header_size(actual_num_members);

    // Read end offset for this member (stored at byte 1 + idx * 2, relative to header)
    let end_offset_pos = 1 + idx * 2;
    let end = actual_header_size + read_buffer_u16(end_offset_pos) as usize;

    // Calculate start: header_size + previous member's end, or header_size for member 0
    let start = if idx == 0 {
        actual_header_size
    } else {
        let prev_end_pos = 1 + (idx - 1) * 2;
        actual_header_size + read_buffer_u16(prev_end_pos) as usize
    };

    // If start == end, member is not present
    if start == end {
        return None;
    }

    // Validate range
    if end < start {
        panic!(
            "ver-stub: invalid range for {:?}: start={}, end={}",
            member as u16, start, end
        );
    }
    if end > BUFFER_SIZE {
        panic!(
            "ver-stub: end offset {} exceeds buffer size {} for {:?}",
            end, BUFFER_SIZE, member as u16
        );
    }

    // Get the slice and convert to UTF-8.
    // Use black_box to prevent the compiler from optimizing away the read,
    // since the buffer is initialized to zeros at compile time, but changed at link time.
    let bytes = core::hint::black_box(&BUFFER[start..end]);
    match core::str::from_utf8(bytes) {
        Ok(s) => Some(s),
        Err(e) => panic!("ver-stub: invalid UTF-8 for {:?}: {:?}", member as u16, e),
    }
}

/// Returns the git SHA, if present.
///
/// This is the full SHA from `git rev-parse HEAD`.
pub fn git_sha() -> Option<&'static str> {
    get_member(Member::GitSha)
}

/// Returns the git describe output, if present.
///
/// This is the output of `git describe --always --dirty`, which includes:
/// - The most recent tag (if any)
/// - Number of commits since that tag
/// - Abbreviated commit hash
/// - `-dirty` suffix if there are uncommitted changes
pub fn git_describe() -> Option<&'static str> {
    get_member(Member::GitDescribe)
}

/// Returns the git branch name, if present.
///
/// This is the output of `git rev-parse --abbrev-ref HEAD`.
pub fn git_branch() -> Option<&'static str> {
    get_member(Member::GitBranch)
}

/// Returns the git commit timestamp, if present.
///
/// This is the author date of HEAD formatted as RFC 3339
/// (e.g., `2024-01-15T10:30:00+00:00`).
pub fn git_commit_timestamp() -> Option<&'static str> {
    get_member(Member::GitCommitTimestamp)
}

/// Returns the git commit date, if present.
///
/// This is the author date of HEAD formatted as a date only
/// (e.g., `2024-01-15`).
pub fn git_commit_date() -> Option<&'static str> {
    get_member(Member::GitCommitDate)
}

/// Returns the git commit message, if present.
///
/// This is the first line of the commit message (subject line),
/// truncated to at most 100 characters.
pub fn git_commit_msg() -> Option<&'static str> {
    get_member(Member::GitCommitMsg)
}

/// Returns the build timestamp, if present.
///
/// This is the time the binary was built, formatted as RFC 3339
/// (e.g., `2024-01-15T10:30:00Z`).
pub fn build_timestamp() -> Option<&'static str> {
    get_member(Member::BuildTimestamp)
}

/// Returns the build date, if present.
///
/// This is the date the binary was built, formatted as YYYY-MM-DD
/// (e.g., `2024-01-15`).
pub fn build_date() -> Option<&'static str> {
    get_member(Member::BuildDate)
}

/// Returns the custom application-specific string, if present.
///
/// This can be any string your application wants to embed into the binary.
/// Set it using `LinkSection::with_custom()` in your build script.
pub fn custom() -> Option<&'static str> {
    get_member(Member::Custom)
}
