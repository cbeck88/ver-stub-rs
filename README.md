# ver-stub

`ver-stub` is a library for injecting build-time information (git hashes, timestamps, etc.)
into the binary *without* injecting code, or triggering frequent cargo rebuilds.

[![Crates.io](https://img.shields.io/crates/v/ver-stub?style=flat-square)](https://crates.io/crates/ver-stub)
[![Crates.io](https://img.shields.io/crates/d/ver-stub?style=flat-square)](https://crates.io/crates/ver-stub)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)
[![Build Status](https://img.shields.io/github/actions/workflow/status/cbeck88/ver-stub-rs/ci.yml?branch=master&style=flat-square)](https://github.com/cbeck88/ver-stub-rs/actions/workflows/ci.yml?query=branch%3Amaster)

[API Docs](https://docs.rs/ver-stub/latest/ver_stub/)

This is particularly helpful if:

* You have multiple binaries in your workspace and rebuilding them all is slow
* You are using build options like LTO which push a lot of work to link time

When I used the popular [`vergen`](https://github.com/rustyhorde/vergen) crate to embed this data, I often found myself frustrated
because actions like `git commit`, `git tag` or `git checkout -b` would cause the next `cargo build`
to rebuild many things, but that would cause momentary confusion and make
me think that I'd accidentally changed code and committed or tagged the wrong thing.

See also the ["relink don't rebuild"](https://rust-lang.github.io/rust-project-goals/2025h2/relink-dont-rebuild.html)
rust project goal.

## How does it work?

The `ver-stub` crate declares a custom linker section with a specific size in bytes.
This is filled in with requested version data only at the end of the build process, after all the
time consuming steps are done. If the version data (git, timestamps) changes, the binary doesn't
have to be recompiled -- this section just needs to be overwritten again.

If it is never filled in, then the section is all 0s, and at
runtime your program safely reports that it doesn't have build information available and otherwise works correctly.

There are several possible workflows to perform the patching, but the main ones use `llvm-tools` installed by `rustup`
which match the version of llvm used by your version of `rustc`, on whatever platform you are working on. CI covers
linux, mac, and windows at time of writing.

## Quickstart

Use `ver_stub` anywhere in your project, and call its functions

```rust
fn git_sha() -> Option<&'static str>;
fn git_describe() -> Option<&'static str>;
fn git_branch() -> Option<&'static str>;
fn git_commit_timestamp() -> Option<&'static str>;
fn git_commit_date() -> Option<&'static str>;
fn git_commit_msg() -> Option<&'static str>;
fn build_timestamp() -> Option<&'static str>;
fn build_date() -> Option<&'static str>;
fn custom() -> Option<&'static str>;
```

This crate doesn't change when the git data changes, so depending on it doesn't trigger any rebuilds.

Then, use the [`ver-stub-build`](https://docs.rs/ver-stub-build/latest/ver_stub_build/)
 crate to fill in the linker section.

There are three recommended approaches.

### Approach #1: Artifact dependencies (nightly, cleanest)

This approach uses cargo's artifact dependencies feature to create a post-build crate that
patches the binary automatically. It's the cleanest solution but requires nightly.

First install the `llvm-tools` if you haven't already.

```sh
$ rustup component add llvm-tools
```

Create a new crate in the same workspace, with a `build.rs` and an empty `lib.rs`.

It should declare a *build dependency* on your binary crate, with an artifact dependency on the bin.

```toml
[build-dependencies]
my-crate = { path = "../my-crate", artifact = "bin" }
```

The `build.rs` should look something like this:

```rust
fn main() {
    ver_stub_build::LinkSection::new()
        .with_all_git()
        .with_all_build_time()
        .patch_into_bin_dep("my-crate", "bin_name")
        .with_filename("bin_name.bin")
        .write_to_target_profile_dir();
}
```

When cargo runs this `build.rs`, it runs an `objcopy` command to patch the linker section,
and produces another binary (`bin_name.bin`) in `target/release` or `target/debug`, (or `target/<triple>/release` etc.)
according to the build profile.
This `build.rs` only runs when its input (the unpatched binary) changes, or when the git information changes.

Artifact dependencies are an unstable feature of cargo, so you will have to use nightly for this approach to work.

**Example:** [`ver-stub-example-build`](./ver-stub-example-build)

```sh
cargo +nightly build   # builds target/debug/ver-stub-example and auto-patches to target/debug/ver-stub-example.bin
```

### Approach #2: The `ver-stub` CLI tool (stable, still simple)

Install the `ver-stub` CLI tool:

```sh
$ cargo install ver-stub-tool
$ rustup component add llvm-tools
```

Then build your binary normally and patch it afterward:

```sh
$ cargo build --release
$ ver-stub --all-git --build-timestamp patch target/release/my_bin
```

This produces a patched binary at `target/release/my_bin.bin`.

You can also specify a custom output path:

```sh
$ ver-stub --all-git --build-timestamp patch target/release/my_bin -o dist/my_bin
```

For ergonomics, put this in:

* A justfile
* A pre-existing release script.

**Example:** [`ver-stub-example`](./ver-stub-example)

```sh
cargo build -p ver-stub-example
ver-stub --all-git --all-build-time patch ver-stub-example/target/debug/ver-stub-example
./ver-stub-example/target/debug/ver-stub-example.bin
```

### Approach #3: Using `cargo objcopy`

An alternative that gives you more control is to use `cargo objcopy` from [`cargo-binutils`](https://crates.io/crates/cargo-binutils):

```sh
$ cargo install cargo-binutils
$ rustup component add llvm-tools
```

Generate the section data file, then use `cargo objcopy`:

```sh
$ ver-stub --all-git --build-timestamp -o target/ver_stub_data
$ cargo objcopy --release --bin my_prog -- --update-section ver_stub=target/ver_stub_data -O dist/my_prog.bin
```

This is very similar to the `ver-stub patch` approach, but has a few differences:

* You can pass additional flags to objcopy if you want to, like `--strip-all` or other section update operations,
  and do it all in one shot.
* If you are cross-compiling, you don't have to specify the path `target/<triple>/release` because `cargo objcopy`
  works it out automatically for you, which might be nicer.
* The section is written before the binary is created, so if you change the section size from the default, you
  must also set `VER_STUB_BUFFER_SIZE` env for the `ver-stub -o ...` command.
  * This is less of a rough-edge than you might think though. If the section file produced
    by `ver-stub -o ...` is too large, then `llvm-objcopy` will give an error and refuse to update the section.
    If the section file is too small, it will actually still work fine, you just might get a build error if you actually
    have too much data to fit in the section file.
  * By contrast,
    when `ver-stub patch` is used, it reads the buffer size from the target to be patched first, before objcopy,
    so it always knows the correct size, and if the section got garbage collected, it does the right thing and
    doesn't produce an error.
 * The section name is platform-specific -- MACH-O (macos) requires the format `__TEXT,ver_stub`, and this detail
   gets exposed to you when you use objcopy on a mac.

### Summary

| Approach | Toolchain | Extra crate | Command |
|----------|-----------|-------------|---------|
| Artifact deps | **nightly** | yes | `cargo +nightly build` |
| `ver-stub patch` | **stable** | no | `cargo build && ver-stub ... patch target/...` |
| `cargo objcopy` | **stable** | no | `ver-stub -o ... && cargo objcopy ...` |

All of these approaches ultimately boil down to using `llvm-objcopy` installed for your toolchain by `rustup`,
from the same version of `llvm` as `rustc` was built. This should be portable to most platforms that rust can build for,
and is known to work well for ELF (linux), MACH-O (macos), and PE/COFF (windows).

If you have a platform or executable format where `llvm-objcopy` doesn't work well for patching, you can modify this third approach
to use an alternative tool, as long as it can consume the file generated by `ver-stub -o`.

## Reproducible builds

*Reproducible builds* is the idea that, if you publish an open source project, and binary distributions of it, you should ensure that
it is possible for someone else to confirm that the build is "good" and wasn't maliciously tampered with.

Lots of projects publish a binary you can download, and a hash of it, so that you can confirm the download wasn't corrupted.
However, this doesn't rule out the possibility that the person who built and hashed the binary was compromised.

Reproducible builds demands something further -- if I check out your repo on my machine, and I run your release build command, I should
get a byte-for-byte identical binary, and compute the same hash as you did.

For example, some security-conscious projects like Signal or Tor work to ensure that their builds are reproducible. Even if the
code in the open-source repo is good, a malicious actor could tamper with the binary sometime before or after it gets into an app repository / App Store,
and then the users would be compromised. Reproducible builds empower *users* to detect this discrepancy without even having to trust
Signal or Tor themselves -- the users can be sure on their own exactly what code they are running. This also helps to dissuade "wrench attacks"
against Signal or Tor developers, which an attacker might otherwise conduct in order to try to force the developers to release compromised code,
in the hopes that it would go undetected and allow them to compromise specific users. A similar analysis applies to e.g. Debian package maintainers.

This touches on things like `vergen` and `ver-stub` because injecting a build timestamp into the binary makes it not reproducible -- the current time
will be different if you build again later, so the hashes won't match.

`ver-stub` provides two environment variables for reproducible builds:

* **`VER_STUB_IDEMPOTENT`**: If set (to any value), build timestamp and build date are never included, even if requested.
  The binary will report `None` for these fields. This is the simplest option for fully reproducible builds.

* **`VER_STUB_BUILD_TIME`**: Can be set to a unix timestamp or an RFC3339 datetime. If set, this fixed time is used
  instead of the actual current time. You can publish the value used with each release, so that outsiders can reproduce the build
  while still having build times in your binary.

`VER_STUB_IDEMPOTENT` takes precedence over `VER_STUB_BUILD_TIME` if both are set.

These are similar to [`SOURCE_DATE_EPOCH` and `VERGEN_IDEMPOTENT`](https://docs.rs/vergen/latest/vergen/#environment-variables) in `vergen`.
However, one thing I like about the `ver-stub` approach is that it also helps with the task of debugging non-reproducible builds.

In a large project it can be very complicated to figure out why two engineers got a different binary at the same commit. I once traced this down to the
[`ahash/const-random` feature](https://docs.rs/crate/ahash/latest/features#const-random), which was intentionally injecting random numbers into the build,
and being enabled transitively by a dependency.

When using `ver-stub`, you can easily dump the `.ver_stub` sections from the two binaries and compare them, or, zero them both out and then compute hashes.
If there are still differences, you have working binaries that you can use with other tools from that point.

## Additional configuration

The size of the section created by `ver-stub` is configurable and defaults to 512 bytes. It can be changed by setting `VER_STUB_BUFFER_SIZE` while building `ver-stub`.
It must be larger than 32 bytes and no more than 64KB.

## Misc Notes

### multiple copies

It is important for the correctness of the crate that only one version of `ver-stub` is used at a time in your binary. Otherwise the custom section will have two copies
of the buffer, and only one of them actually gets written by objcopy. To force this to be the case, the [`links` attribute](https://doc.rust-lang.org/cargo/reference/build-scripts.html#the-links-manifest-key)
is used in the `Cargo.toml` for `ver-stub`, with the name of the custom linker section.

(Note that `llvm-objcopy` also has some protections, and [won't allow a section to be enlarged via `--update-section`](https://reviews.llvm.org/D112116).)

### zero copies

It's possible that the binary ends up with 0 copies of the linker section. This happens if you depend on `ver-stub` but then don't actually invoke any of its functions.
If nothing in the program, after optimizations, references the linker section, it will likely be garbage collected and removed by the linker. (This is possible
even if you put `#[used]` on a section, because linker optimizations tend to be very aggressive.) This would be fine except
that the `objcopy --update-section` command will fail if the section doesn't exist when `objcopy` runs.

The `ver-stub-build` crate and the `ver-stub-tool` use `readobj` to get the section and it's size before calling objcopy, and do the right thing if it doesn't exist, so you won't notice this with the first two methods.
If you are using `cargo objcopy` directly, however, `objcopy` will fail with an error if this happens. The simplest fix is to actually invoke a `ver-stub` function somewhere
in `main.rs`.

### will you support all the data that `vergen` does?

Most likely not.

* The rust toolchain already embeds much of this in the `.comment` section:

  ```
      String dump of section '.comment':
       [     0]  Linker: LLD 21.1.2 (/checkout/src/llvm-project/llvm 8c30b9c5098bdff1d3d9a2d460ee091cd1171e60)
       [    5f]  rustc version 1.91.1 (ed61e7d7e 2025-11-07)
       [    8b]  GCC: (Ubuntu 13.3.0-6ubuntu2~24.04) 13.3.0
  ```

  and information about the ABI appears in `.note`.

  ```
      OS: Linux, ABI: 3.2.0
  ```

  which can be read easily using `readelf -n` and `readelf -p .comment`.

* My main motivation was to avoid the hit to build times that occurs when data that "logically" isn't already a dependency of the binary,
  like git state, build timestamp, is injected into the code, and `cargo` rebuilds everything out of an abundance of caution.

  If your compiler changes, or your opt level changes, or your cargo features change, cargo already has to rebuild, whether or
  not you additionally inject this stuff as text strings into the source. So there's no advantage to the link-section approach
  over what `vergen` is doing with `env!` for such data. You might as well use `vergen` for these types of data.

* You can inject whatever you want in the custom string, and that could also be structured data with ASCII separators if you want.
  (Remember to emit appropriate `cargo::rerun-if-changed-` directives!)

That being said, the link section format is designed to be forwards and backwards compatible, so there is a clear path to extend
built-in support for more stuff.

## Licensing and distribution

MIT or Apache 2 at your option
