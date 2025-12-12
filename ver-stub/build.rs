// This file is required because cargo won't allow to set `links` on this crate
// unless there is a `build.rs`. We don't actually need to link a system library,
// but we don't want anything else to add something to our custom linker section,
// and the `links` attribute is the tool we have to exclude that.
fn main() {}
