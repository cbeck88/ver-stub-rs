# ver-stub-build

Build script helper for [`ver-stub`](https://crates.io/crates/ver-stub).

[![Crates.io](https://img.shields.io/crates/v/ver-stub-build?style=flat-square)](https://crates.io/crates/ver-stub-build)
[![Crates.io](https://img.shields.io/crates/d/ver-stub-build?style=flat-square)](https://crates.io/crates/ver-stub-build)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)

[API Docs](https://docs.rs/ver-stub-build/latest/ver_stub_build/)

This crate generates link section content matching what `ver-stub` expects at runtime.
It collects git information (SHA, branch, commit timestamp, etc.) and build timestamps,
then writes the data to a file or patches it directly into a binary.

## Example

```rust
// build.rs
fn main() {
    ver_stub_build::LinkSection::new()
        .with_all_git()
        .with_build_timestamp()
        .write_to_out_dir();
}
```

## See Also

- [`ver-stub`](https://crates.io/crates/ver-stub) - Runtime library for reading version data
- [`ver-stub-tool`](https://crates.io/crates/ver-stub-tool) - CLI tool (if you don't need build.rs integration)
- [Main documentation](https://github.com/cbeck88/ver-stub-rs) - Full usage instructions and examples
