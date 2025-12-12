use conf::{Conf, Subcommands};
use std::path::PathBuf;
use ver_stub_build::LinkSection;

/// Inject git and build metadata into binaries via the .ver_stub linker section.
///
/// Two modes of operation:
///
/// 1. Generate section data file (for use with cargo objcopy):
///    ver-stub --all-git -o target/ver_stub_data
///
/// 2. Patch a binary directly (recommended):
///    ver-stub --all-git --build-timestamp patch target/release/my-bin
///
/// The patch command produces a new binary with .bin extension containing the version info.
///
/// For reproducible builds:
/// - VER_STUB_IDEMPOTENT: If set, build timestamp/date are never included (always None)
/// - VER_STUB_BUILD_TIME: Override build timestamp with a fixed value (unix or RFC 3339)
#[derive(Debug, Conf)]
struct Args {
    /// Include git SHA (git rev-parse HEAD)
    #[conf(long)]
    git_sha: bool,

    /// Include git describe (git describe --always --dirty)
    #[conf(long)]
    git_describe: bool,

    /// Include git branch (git rev-parse --abbrev-ref HEAD)
    #[conf(long)]
    git_branch: bool,

    /// Include git commit timestamp
    #[conf(long)]
    git_commit_timestamp: bool,

    /// Include git commit date
    #[conf(long)]
    git_commit_date: bool,

    /// Include git commit message (first line)
    #[conf(long)]
    git_commit_msg: bool,

    /// Include all git information
    #[conf(long)]
    all_git: bool,

    /// Include build timestamp
    #[conf(long)]
    build_timestamp: bool,

    /// Include build date
    #[conf(long)]
    build_date: bool,

    /// Include all build time information
    #[conf(long)]
    all_build_time: bool,

    /// Custom string to include
    #[conf(long)]
    custom: Option<String>,

    /// Output path (writes to this path, or {path}/ver_stub_data if it's a directory).
    /// Mutually exclusive with subcommands.
    #[conf(short, long)]
    output: Option<PathBuf>,

    #[conf(subcommands)]
    command: Option<Command>,
}

#[derive(Debug, Subcommands)]
enum Command {
    /// Patch version info into an existing binary using llvm-objcopy.
    ///
    /// Example: ver-stub --all-git patch target/release/my-bin
    ///
    /// This reads the input binary, updates its .ver_stub section with
    /// the requested version info, and writes the result to {input}.bin
    /// (or to the specified output path).
    ///
    /// Requires llvm-tools: rustup component add llvm-tools
    Patch {
        /// Path to the binary to patch (e.g., target/release/my-bin)
        #[conf(pos)]
        input: PathBuf,

        /// Output directory or file path. If a directory, writes {input_name}.bin there.
        /// Defaults to the input file's parent directory.
        #[conf(short, long)]
        output: Option<PathBuf>,
    },

    /// Print the platform-specific linker section name and exit.
    ///
    /// Useful for scripts that need to use cargo objcopy directly.
    /// Returns ".ver_stub" on ELF (Linux) or "__TEXT,__ver_stub" on Mach-O (macOS).
    PrintSectionName,
}

fn build_section(args: &Args) -> LinkSection {
    let mut section = LinkSection::new();

    // Git options
    if args.all_git {
        section = section.with_all_git();
    } else {
        if args.git_sha {
            section = section.with_git_sha();
        }
        if args.git_describe {
            section = section.with_git_describe();
        }
        if args.git_branch {
            section = section.with_git_branch();
        }
        if args.git_commit_timestamp {
            section = section.with_git_commit_timestamp();
        }
        if args.git_commit_date {
            section = section.with_git_commit_date();
        }
        if args.git_commit_msg {
            section = section.with_git_commit_msg();
        }
    }

    // Build time options
    if args.all_build_time {
        section = section.with_all_build_time();
    } else {
        if args.build_timestamp {
            section = section.with_build_timestamp();
        }
        if args.build_date {
            section = section.with_build_date();
        }
    }

    // Custom string
    if let Some(ref custom) = args.custom {
        section = section.with_custom(custom);
    }

    section
}

fn main() {
    // Unset OUT_DIR to prevent LinkSection from trying to use build.rs paths
    // SAFETY: We're single-threaded at this point, before any other code runs
    unsafe { std::env::remove_var("OUT_DIR") };

    let args = Args::parse();

    // Error if --output is specified with a subcommand
    if args.output.is_some() && args.command.is_some() {
        eprintln!(
            "error: when using patch command, top-level --output flag is ignored; \
             this is probably not what you intended"
        );
        std::process::exit(1);
    }

    let section = build_section(&args);

    match args.command {
        Some(Command::Patch {
            ref input,
            ref output,
        }) => {
            let output_path = output
                .clone()
                .unwrap_or_else(|| input.parent().unwrap().to_path_buf());
            section.patch_into(input).write_to(&output_path);
            eprintln!(
                "ver-stub: patched {} -> {}",
                input.display(),
                output_path.display()
            );
        }
        Some(Command::PrintSectionName) => {
            println!("{}", ver_stub_build::SECTION_NAME);
        }
        None => {
            let Some(output) = args.output else {
                eprintln!("error: --output is required when not using a subcommand");
                std::process::exit(1);
            };
            let output_path = section.write_to(&output);
            eprintln!("ver-stub: wrote {}", output_path.display());
        }
    }
}
