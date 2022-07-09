include!(concat!(env!("OUT_DIR"), "/built.rs"));

pub fn version_str() -> String {
    let git_commit = match GIT_COMMIT_HASH {
        Some(v) => &v[..9],
        None => ("Unknown commit"),
    };
    let debug = if DEBUG { " (debug)" } else { "" };
    format!(
        "Version {} ({}){}, built for {} {} {}-bit using {} at {}",
        PKG_VERSION,
        git_commit,
        debug,
        CFG_OS,
        CFG_TARGET_ARCH,
        CFG_POINTER_WIDTH,
        RUSTC_VERSION,
        BUILT_TIME_UTC
    )
}
