fn main() {
    println!(
        "git sha:         {}",
        ver_stub::git_sha().unwrap_or("(not set)")
    );
    println!(
        "git describe:    {}",
        ver_stub::git_describe().unwrap_or("(not set)")
    );
    println!(
        "git branch:      {}",
        ver_stub::git_branch().unwrap_or("(not set)")
    );
    println!(
        "git timestamp:   {}",
        ver_stub::git_commit_timestamp().unwrap_or("(not set)")
    );
    println!(
        "git date:        {}",
        ver_stub::git_commit_date().unwrap_or("(not set)")
    );
    println!(
        "git msg:         {}",
        ver_stub::git_commit_msg().unwrap_or("(not set)")
    );
    println!(
        "build timestamp: {}",
        ver_stub::build_timestamp().unwrap_or("(not set)")
    );
    println!(
        "build date:      {}",
        ver_stub::build_date().unwrap_or("(not set)")
    );
}
