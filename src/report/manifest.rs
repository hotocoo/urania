//! SHA-256 manifest for report artifacts.

use std::path::PathBuf;

use sha2::{Digest, Sha256};

/// Write "<hash>  <name>" lines for each file, next to the first file.
pub fn write_manifest(paths: &[PathBuf]) -> anyhow::Result<PathBuf> {
    let mut lines = Vec::new();
    for p in paths {
        let bytes = std::fs::read(p)?;
        let mut h = Sha256::new();
        h.update(&bytes);
        let digest = h.finalize();
        let hash: String = digest.iter().map(|b| format!("{b:02x}")).collect();
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
}
