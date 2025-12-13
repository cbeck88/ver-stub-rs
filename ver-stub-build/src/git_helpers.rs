use crate::{cargo_rerun_if, cargo_warning};
use chrono::{DateTime, FixedOffset};
use std::{fs, path::PathBuf, process::Command};

/// Emits cargo rerun-if-changed directives for git state files.
/// This ensures the build script reruns when the git HEAD or refs change.
/// Matches vergen's behavior: watches .git/HEAD and .git/<ref_path>.
///
/// See: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
pub fn emit_git_rerun_if_changed() {
    // Find the git directory
    let git_dir = match find_git_dir() {
        Some(dir) => dir,
        None => return,
    };

    // Always watch .git/HEAD
    let head_path = git_dir.join("HEAD");
    if head_path.exists() {
        cargo_rerun_if(&format!("changed={}", head_path.display()));

        // If HEAD points to a ref, also watch that ref file
        if let Ok(head_contents) = fs::read_to_string(&head_path) {
            let head_contents = head_contents.trim();
            if let Some(ref_path) = head_contents.strip_prefix("ref: ") {
                let ref_file = git_dir.join(ref_path);
                if ref_file.exists() {
                    cargo_rerun_if(&format!("changed={}", ref_file.display()));
                }
            }
        }
    }
}

/// Finds the .git directory by walking up from the current directory.
fn find_git_dir() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let git_dir = dir.join(".git");
        if git_dir.is_dir() {
            return Some(git_dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Gets the current git SHA using `git rev-parse HEAD`.
pub fn get_git_sha(fail_on_error: bool) -> Option<String> {
    run_git_command(&["rev-parse", "HEAD"], fail_on_error)
}

/// Gets the git describe output using `git describe --always --dirty`.
pub fn get_git_describe(fail_on_error: bool) -> Option<String> {
    run_git_command(&["describe", "--always", "--dirty"], fail_on_error)
}

/// Gets the current git branch using `git rev-parse --abbrev-ref HEAD`.
pub fn get_git_branch(fail_on_error: bool) -> Option<String> {
    run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"], fail_on_error)
}

/// Gets the git commit timestamp as a chrono DateTime.
pub fn get_git_commit_timestamp(fail_on_error: bool) -> Option<DateTime<FixedOffset>> {
    // Get the author date in ISO 8601 strict format
    let timestamp_str = run_git_command(&["log", "-1", "--format=%aI"], fail_on_error)?;
    match DateTime::parse_from_rfc3339(&timestamp_str) {
        Ok(dt) => Some(dt),
        Err(e) => {
            let msg = format!(
                "ver-stub-build: failed to parse git timestamp '{}': {}",
                timestamp_str, e
            );
            if fail_on_error {
                panic!("{}", msg);
            } else {
                cargo_warning(&msg);
                None
            }
        }
    }
}

/// Gets the first line of the git commit message, truncated to 100 chars.
pub fn get_git_commit_msg(fail_on_error: bool) -> Option<String> {
    let msg = run_git_command(&["log", "-1", "--format=%s"], fail_on_error)?;
    // Truncate to 100 chars to leave room in the buffer
    Some(if msg.len() > 100 {
        let mut end = 100;
        while !msg.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        msg[..end].to_string()
    } else {
        msg
    })
}

/// Runs a git command and returns stdout as a trimmed string.
///
/// If `fail_on_error` is true, panics on failure. Otherwise, emits a cargo warning
/// and returns None, allowing builds to succeed without git.
fn run_git_command(args: &[&str], fail_on_error: bool) -> Option<String> {
    let cmd = format!("git {}", args.join(" "));
    let output = match Command::new("git").args(args).output() {
        Ok(output) => output,
        Err(e) => {
            let msg = format!("ver-stub-build: failed to execute '{}': {}", cmd, e);
            if fail_on_error {
                panic!("{}", msg);
            } else {
                cargo_warning(&msg);
                return None;
            }
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = format!(
            "ver-stub-build: '{}' failed with status {}: {}",
            cmd,
            output.status,
            stderr.trim()
        );
        if fail_on_error {
            panic!("{}", msg);
        } else {
            cargo_warning(&msg);
            return None;
        }
    }

    match String::from_utf8(output.stdout) {
        Ok(s) => Some(s.trim().to_string()),
        Err(_) => {
            let msg = format!("ver-stub-build: '{}' output is not valid UTF-8", cmd);
            if fail_on_error {
                panic!("{}", msg);
            } else {
                cargo_warning(&msg);
                None
            }
        }
    }
}
