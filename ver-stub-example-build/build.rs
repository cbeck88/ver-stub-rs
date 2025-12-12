fn main() {
    ver_stub_build::LinkSection::new()
        .with_all_git()
        .with_all_build_time()
        .patch_into_bin_dep("ver-stub-example", "ver-stub-example")
        .with_filename("ver-stub-example.bin")
        .write_to_target_profile_dir();
}
