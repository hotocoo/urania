//! SHA-256 manifest for report artifacts.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn sha256_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut h = Sha256::new();
    h.update(&bytes);
    let digest = h.finalize();
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// Write "<hash>  <name>" lines for each file, next to the first file.
pub fn write_manifest(paths: &[PathBuf]) -> anyhow::Result<PathBuf> {
    let mut lines = Vec::new();
    for p in paths {
        let hash = sha256_file(p).ok_or_else(|| anyhow::anyhow!("unreadable: {}", p.display()))?;
        let name = p
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("no filename for {}", p.display()))?;
        lines.push(format!("{hash}  {}", name.to_string_lossy()));
    }
    let name = paths[0]
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("no filename for {}", paths[0].display()))?
        .to_string_lossy()
        .to_string();
    let out = paths[0].with_file_name(format!("{name}.sha256"));
    std::fs::write(&out, lines.join("\n") + "\n")?;
    Ok(out)
}

/// Verify a manifest file: for each "<hash>  <name>" line, re-hash the file
/// (looked up next to the manifest) and compare. Returns (name, ok) per
/// entry — a missing file counts as not ok.
pub fn verify_manifest(manifest_path: &Path) -> anyhow::Result<Vec<(String, bool)>> {
    let text = std::fs::read_to_string(manifest_path)?;
    let dir = manifest_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let Some((hash, name)) = line.split_once("  ") else {
            return Err(anyhow::anyhow!("malformed manifest line: {line}"));
        };
        let name = name.trim();
        let expected = hash.trim().to_ascii_lowercase();
        let ok = sha256_file(&dir.join(name)).is_some_and(|got| got == expected);
        out.push((name.to_string(), ok));
    }
    if out.is_empty() {
        return Err(anyhow::anyhow!(
            "manifest is empty: {}",
            manifest_path.display()
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_hashes_and_names() {
        let dir = std::env::temp_dir().join(format!("urania-manifest-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.html");
        let b = dir.join("b.csv");
        std::fs::write(&a, b"hello").unwrap();
        std::fs::write(&b, b"world").unwrap();
        let out = write_manifest(&[a.clone(), b.clone()]).unwrap();
        assert_eq!(out, dir.join("a.html.sha256"));
        let text = std::fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        // Known sha256("hello").
        assert!(lines[0].starts_with(
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824  a.html"
        ));
        assert!(lines[1].contains("b.csv"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_roundtrip_tamper_and_missing() {
        let dir = std::env::temp_dir().join(format!("urania-verify-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.html");
        let b = dir.join("b.csv");
        std::fs::write(&a, b"hello").unwrap();
        std::fs::write(&b, b"world").unwrap();
        let manifest = write_manifest(&[a.clone(), b.clone()]).unwrap();

        // Untouched: all ok.
        let res = verify_manifest(&manifest).unwrap();
        assert_eq!(res, vec![("a.html".into(), true), ("b.csv".into(), true)]);

        // Tampered file: that entry fails, the other still passes.
        std::fs::write(&a, b"hellO").unwrap();
        let res = verify_manifest(&manifest).unwrap();
        assert_eq!(res, vec![("a.html".into(), false), ("b.csv".into(), true)]);

        // Missing file: not ok.
        std::fs::remove_file(&b).unwrap();
        let res = verify_manifest(&manifest).unwrap();
        assert_eq!(res[1], ("b.csv".into(), false));

        // Malformed line: error.
        let bad = dir.join("bad.sha256");
        std::fs::write(&bad, "no separator here\n").unwrap();
        assert!(verify_manifest(&bad).is_err());

        // Empty manifest: error.
        let empty = dir.join("empty.sha256");
        std::fs::write(&empty, "\n").unwrap();
        assert!(verify_manifest(&empty).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
