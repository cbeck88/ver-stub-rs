# ver-stub-tool

CLI tool for injecting version data into binaries using [`ver-stub`](https://crates.io/crates/ver-stub).

[![Crates.io](https://img.shields.io/crates/v/ver-stub-tool?style=flat-square)](https://crates.io/crates/ver-stub-tool)
[![Crates.io](https://img.shields.io/crates/d/ver-stub-tool?style=flat-square)](https://crates.io/crates/ver-stub-tool)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)

## Installation

```sh
cargo install ver-stub-tool
rustup component add llvm-tools
```

## Example Usage

### Patch a binary directly

```sh
cargo build --release
ver-stub --all-git --build-timestamp patch target/release/my-bin
```

This produces a patched binary at `target/release/my-bin.bin`.

### Generate section data file

For use with `cargo objcopy` or other tools:

```sh
ver-stub --all-git --build-timestamp -o target/ver_stub_data
cargo objcopy --release --bin my-bin -- --update-section ver_stub=target/ver_stub_data my-bin.bin
```

## Options

This tool exposes CLI parameters for the functionality in [`ver-stub-build`](https://crates.io/crates/ver-stub-build).
Run `ver-stub --help` for the full list of options.

## Reproducible Builds

For reproducible builds, two environment variables are supported:

- **`VER_STUB_IDEMPOTENT`**: If set, build timestamp/date are never included (always None).
  This is the simplest option for fully reproducible builds.

- **`VER_STUB_BUILD_TIME`**: Override the build timestamp with a fixed value.
  Accepts unix timestamps or RFC 3339 datetimes.

`VER_STUB_IDEMPOTENT` takes precedence if both are set.

## See Also

- [`ver-stub`](https://crates.io/crates/ver-stub) - Runtime library for reading version data
- [`ver-stub-build`](https://crates.io/crates/ver-stub-build) - Build script helper (used by this tool)
- [Main documentation](https://github.com/cbeck88/ver-stub-rs) - Full usage instructions and examples
