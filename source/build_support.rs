use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub const DIRECTORIES: &[&str] = &[
    "src",
    "ui",
    "scripts",
    "packaging",
    "tests",
    "vendor",
    ".cargo",
];
pub const FILES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "CMakeLists.txt",
    "build.rs",
    "build_support.rs",
    "LICENSE",
    "README.md",
    "FEATURES.md",
    "VALIDATION.md",
    "TEST-ON-DEVICE.md",
    "THIRD-PARTY.md",
];
fn walk(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for e in fs::read_dir(path).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let t = e.file_type().map_err(|e| e.to_string())?;
        if e.file_name() == "__pycache__" {
            continue;
        }
        if t.is_symlink() {
            return Err("Source inputs must not be links".into());
        }
        if t.is_dir() {
            walk(&e.path(), out)?;
        } else if t.is_file() {
            out.push(e.path());
        }
    }
    Ok(())
}
pub fn fingerprint(root: &Path) -> Result<String, String> {
    let mut paths = FILES.iter().map(|s| root.join(s)).collect::<Vec<_>>();
    for d in DIRECTORIES {
        walk(&root.join(d), &mut paths)?;
    }
    paths.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    let mut h = Sha256::new();
    for p in paths {
        if fs::symlink_metadata(&p)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Source input is a link".into());
        }
        h.update(p.strip_prefix(root).unwrap().as_os_str().as_encoded_bytes());
        h.update([0]);
        h.update(Sha256::digest(fs::read(&p).map_err(|e| e.to_string())?));
    }
    Ok(format!("{:x}", h.finalize()))
}
