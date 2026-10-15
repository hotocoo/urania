//! Build identity: version + git commit (set by build.rs when git is
//! available). Evidence provenance: the report, the db's `tool_version`
//! meta, and the CLI all state which exact build produced the data.

/// `0.1.0 (abc1234)` — or just the version when built without git.
pub fn full() -> String {
    match option_env!("URANIA_GIT_HASH") {
        Some(h) => format!("{} ({h})", env!("CARGO_PKG_VERSION")),
        None => env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// The short commit hash, if the build had git available.
pub fn hash() -> Option<&'static str> {
    option_env!("URANIA_GIT_HASH")
}
