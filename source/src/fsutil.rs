use crate::Result;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
};
pub fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn fingerprint(path: &Path) -> Result<Option<String>> {
    match fs::read(path) {
        Ok(x) => Ok(Some(hash(&x))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub fn unique() -> String {
    let mut b = [0u8; 16];
    use std::io::Read;
    if File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok()
    {
        use std::fmt::Write;
        b.iter().fold(String::with_capacity(32), |mut s, x| {
            write!(&mut s, "{x:02x}").expect("string write");
            s
        })
    } else {
        format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )
    }
}
pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    if fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Setup data directory must not be a symlink".into());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())
}
pub fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("Missing parent directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    if fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(format!("Refusing to replace a link: {}", path.display()));
    }
    let temp = parent.join(format!(".wow-{}.tmp", unique()));
    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
pub fn json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    atomic(
        path,
        &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
}
pub fn load<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&read(path)?)
        .map_err(|e| format!("Invalid saved data in {}: {e}", path.display()))
}
pub fn canonical(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .map_err(|e| format!("Cannot access {}: {e}", path.display()))
}
pub fn safe_path(path: &Path) -> Result<()> {
    let s = path.to_str().ok_or("Paths must be valid UTF-8")?;
    if s.chars().any(|c| c.is_control()) {
        return Err("Path contains control characters".into());
    }
    Ok(())
}
pub fn shell_quote(path: &Path) -> Result<String> {
    safe_path(path)?;
    Ok(format!(
        "'{}'",
        path.to_str().unwrap().replace('\'', "'\\''")
    ))
}
pub fn steam_quote(path: &Path) -> Result<String> {
    safe_path(path)?;
    let s = path.to_str().unwrap();
    if s.contains('"') {
        return Err("Steam targets cannot contain a double quote".into());
    }
    Ok(format!("\"{s}\""))
}
pub struct Lock(File);
impl Lock {
    pub fn acquire(path: &Path) -> Result<Self> {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| e.to_string())?;
        if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(
                "Another setup operation is running. Close the other window and retry.".into(),
            );
        }
        Ok(Self(f))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Identity {
    pub device: u64,
    pub inode: u64,
}
pub fn identity(path: &Path) -> Result<Identity> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("Removal requires an ordinary directory".into());
    }
    Ok(Identity {
        device: m.dev(),
        inode: m.ino(),
    })
}
/// Reject links and mount points throughout a removal tree, including same-device bind mounts.
pub fn check_tree(root: &Path) -> Result<()> {
    let id = identity(root)?;
    let mounts = fs::read_to_string("/proc/self/mountinfo").map_err(|e| e.to_string())?;
    for line in mounts.lines() {
        if let Some(p) = line.split_whitespace().nth(4) {
            let p = PathBuf::from(
                p.replace("\\040", " ")
                    .replace("\\011", "\t")
                    .replace("\\134", "\\"),
            );
            if p == root || p.starts_with(root) {
                return Err(format!(
                    "A mounted location blocks removal: {}",
                    p.display()
                ));
            }
        }
    }
    let mut stack = vec![root.to_path_buf()];
    let mut count = 0;
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
            crate::cancelled()?;
            let p = e.map_err(|e| e.to_string())?.path();
            let m = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            count += 1;
            if count > 1_000_000 {
                return Err("Removal tree is too large to validate".into());
            }
            if m.file_type().is_symlink() || m.dev() != id.device || (!m.is_file() && !m.is_dir()) {
                return Err(format!(
                    "Linked, mounted or special content blocks removal: {}",
                    p.display()
                ));
            }
            if m.is_dir() {
                stack.push(p);
            }
        }
    }
    Ok(())
}
pub fn free_bytes(path: &Path) -> Result<u64> {
    let c =
        std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
    let mut st = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(c.as_ptr(), st.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let st = unsafe { st.assume_init() };
    Ok(st.f_bavail.saturating_mul(st.f_frsize))
}
