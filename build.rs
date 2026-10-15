//! Records the build commit for evidence provenance: the report header,
//! the daemon's `tool_version` db meta, and `status`/`config` output all
//! state which exact build produced the data. Builds without git access
//! (vendored tarballs) fall back to the version alone.
fn main() {
    println!("cargo:rerun-if-changed=.git");
    if let Ok(out) = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        && out.status.success()
    {
        let h = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !h.is_empty() {
            println!("cargo:rustc-env=URANIA_GIT_HASH={h}");
        }
    }
}
