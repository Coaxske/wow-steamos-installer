//! Parent-wrapper update operations. Target, paths and key are never accepted from JSON.
use super::{
    cancelled, idle, pin, private_parent, read_regular, verify_payload, write_new, FilePin,
    Version, MAX_BINARY, PRODUCTION_KEY, REPOSITORY,
};
use crate::{fsutil, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Staged {
    path: PathBuf,
    pin: FilePin,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: u32,
    repository: String,
    nonce: String,
    target: PathBuf,
    current: FilePin,
    old_version: String,
    old_source_sha256: String,
    staged: Staged,
    version: String,
    source_sha256: String,
    release_url: String,
    signature_hex: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema: u32,
    nonce: String,
    target: PathBuf,
    old: FilePin,
    new: FilePin,
    staged: PathBuf,
    backup: PathBuf,
    old_version: String,
    old_source: String,
    new_version: String,
    new_source: String,
}
struct Context {
    target: PathBuf,
    parent: PathBuf,
    runtime: PathBuf,
    nonce: String,
    request: PathBuf,
    receipt: PathBuf,
    binding: PathBuf,
    dir: File,
}
fn is_hex(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|x| x.is_ascii_hexdigit())
}
fn env(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| format!("Missing wrapper binding: {name}"))
}
fn env_path(name: &str) -> Result<PathBuf> {
    let p = PathBuf::from(
        std::env::var_os(name).ok_or_else(|| format!("Missing wrapper binding: {name}"))?,
    );
    fsutil::safe_path(&p)?;
    Ok(p)
}
fn read_private(path: &Path, max: u64) -> Result<Vec<u8>> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.uid() != unsafe { libc::geteuid() }
        || m.nlink() != 1
        || m.mode() & 0o077 != 0
    {
        return Err("Wrapper protocol file is not private".into());
    }
    read_regular(path, max)
}
fn lines(bytes: &[u8], count: usize) -> Result<Vec<String>> {
    let s = std::str::from_utf8(bytes).map_err(|_| "Invalid wrapper receipt")?;
    if !s.ends_with('\n') || s.contains('\r') || s.contains('\0') {
        return Err("Invalid wrapper receipt".into());
    }
    let parts: Vec<_> = s[..s.len() - 1].split('\n').map(str::to_owned).collect();
    if parts.len() != count || parts.iter().any(|x| x.is_empty()) {
        return Err("Invalid wrapper receipt".into());
    }
    Ok(parts)
}
fn wire(header: &str, nonce: &str, sha: &str, version: &str, source: &str) -> String {
    format!("{header}\n{nonce}\n{sha}\n{version}\n{source}\n")
}
fn check_env_pin(p: &FilePin, sha_required: bool) -> Result<()> {
    for (name, value) in [
        ("WOW_PORTABLE_DEV", p.device.to_string()),
        ("WOW_PORTABLE_INO", p.inode.to_string()),
        ("WOW_PORTABLE_SIZE", p.size.to_string()),
        ("WOW_PORTABLE_MODE", p.mode.to_string()),
        ("WOW_PORTABLE_UID", p.uid.to_string()),
    ] {
        if env(name)? != value {
            return Err("Portable metadata no longer matches its wrapper binding".into());
        }
    }
    if sha_required && env("WOW_PORTABLE_SHA256")? != p.sha256 {
        return Err("Portable checksum no longer matches its wrapper binding".into());
    }
    Ok(())
}
impl Context {
    fn from_env() -> Result<Self> {
        let target = env_path("WOW_PORTABLE_PATH")?;
        if !target.is_absolute() || target.canonicalize().map_err(|e| e.to_string())? != target {
            return Err("Wrapper portable path contains links or is unavailable".into());
        }
        let parent = target
            .parent()
            .ok_or("Missing portable directory")?
            .to_path_buf();
        let pm = fs::symlink_metadata(&parent).map_err(|e| e.to_string())?;
        if !pm.is_dir() || pm.uid() != unsafe { libc::geteuid() } || pm.mode() & 0o022 != 0 {
            return Err("Portable directory must be owned and not shared writable".into());
        }
        let request = env_path("WOW_UPDATE_REQUEST")?;
        private_parent(&request)?;
        let runtime = request.parent().unwrap().to_path_buf();
        if request != runtime.join("update-request.json") {
            return Err("Invalid wrapper request path".into());
        }
        let receipt = env_path("WOW_UPDATE_RECEIPT")?;
        let binding = env_path("WOW_UPDATE_BINDING")?;
        if receipt != runtime.join("update-receipt") || binding != runtime.join("update-binding") {
            return Err("Wrapper protocol paths disagree".into());
        }
        let nonce = env("WOW_UPDATE_NONCE")?;
        if !is_hex(&nonce, 32) {
            return Err("Invalid wrapper nonce".into());
        }
        let dir = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&parent)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            target,
            parent,
            runtime,
            nonce,
            request,
            receipt,
            binding,
            dir,
        })
    }
    fn path(&self, name: &std::ffi::OsStr) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.dir.as_raw_fd())).join(name)
    }
    fn target_fd(&self) -> PathBuf {
        self.path(self.target.file_name().unwrap())
    }
    fn staged(&self) -> PathBuf {
        self.parent.join(format!(".wow-update-{}.new", self.nonce))
    }
    fn backup(&self) -> PathBuf {
        self.parent
            .join(format!(".wow-update-{}.backup", self.nonce))
    }
    fn recovery(&self) -> PathBuf {
        self.path(std::ffi::OsStr::new(&format!(
            ".wow-update-{}.recovery.json",
            self.nonce
        )))
    }
    fn scoped(&self, p: &Path) -> Result<PathBuf> {
        if p.parent() != Some(self.parent.as_path()) {
            return Err("Update file left the pinned portable directory".into());
        }
        Ok(self.path(p.file_name().ok_or("Missing update filename")?))
    }
    fn parent_unchanged(&self) -> Result<()> {
        let a = self.dir.metadata().map_err(|e| e.to_string())?;
        let b = fs::symlink_metadata(&self.parent).map_err(|e| e.to_string())?;
        if !b.is_dir() || b.file_type().is_symlink() || a.dev() != b.dev() || a.ino() != b.ino() {
            return Err("Portable directory changed during update".into());
        }
        Ok(())
    }
    fn sync(&self) -> Result<()> {
        self.dir.sync_all().map_err(|e| e.to_string())
    }
    fn binding(&self) -> Result<Vec<String>> {
        let x = lines(&read_private(&self.binding, 4096)?, 5)?;
        if x[0] != "WOW_UPDATE_BIND_V1"
            || x[1] != self.nonce
            || !is_hex(&x[2], 64)
            || Version::parse(&x[3]).is_err()
            || !is_hex(&x[4], 64)
        {
            return Err("Invalid original portable binding".into());
        }
        Ok(x)
    }
    fn request(&self) -> Result<Request> {
        let r: Request = serde_json::from_slice(&read_private(&self.request, 64 * 1024)?)
            .map_err(|_| "Invalid update handoff")?;
        if r.schema != 1
            || r.repository != REPOSITORY
            || r.nonce != self.nonce
            || r.target != self.target
            || r.staged.path != self.staged()
            || r.release_url != super::page(&format!("v{}", r.version))
            || !is_hex(&r.source_sha256, 64)
            || !is_hex(&r.old_source_sha256, 64)
            || !is_hex(&r.signature_hex, 128)
            || Version::parse(&r.version)? <= Version::parse(&r.old_version)?
        {
            return Err("Update handoff binding does not match".into());
        }
        let b = self.binding()?;
        if r.current.sha256 != b[2]
            || r.old_version != b[3]
            || r.old_source_sha256 != b[4]
            || r.old_version != env!("CARGO_PKG_VERSION")
            || r.old_source_sha256 != env!("WOW_SOURCE_SHA256")
        {
            return Err("Update handoff is not bound to this old build".into());
        }
        Ok(r)
    }
    fn journal(&self) -> Result<Journal> {
        let j: Journal = serde_json::from_slice(&read_private(&self.recovery(), 64 * 1024)?)
            .map_err(|_| "Invalid retained update recovery record")?;
        if j.schema != 1
            || j.nonce != self.nonce
            || j.target != self.target
            || j.staged != self.staged()
            || j.backup != self.backup()
            || !is_hex(&j.old.sha256, 64)
            || !is_hex(&j.new.sha256, 64)
            || !is_hex(&j.old_source, 64)
            || !is_hex(&j.new_source, 64)
        {
            return Err("Retained update recovery record has wrong bindings".into());
        }
        let b = self.binding()?;
        if j.old.sha256 != b[2] || j.old_version != b[3] || j.old_source != b[4] {
            return Err("Retained update recovery record does not match original build".into());
        }
        Ok(j)
    }
    fn receipt(&self, header: &str, sha: &str, version: &str, source: &str) -> Result<()> {
        let bytes = wire(header, &self.nonce, sha, version, source);
        if self.receipt.try_exists().map_err(|e| e.to_string())? {
            let old = lines(&read_private(&self.receipt, 4096)?, 5)?;
            if old[1] != self.nonce
                || !["WOW_UPDATE_COMMIT_V1", "WOW_UPDATE_ROLLBACK_V1"].contains(&old[0].as_str())
            {
                return Err("Existing wrapper receipt is not owned by this update".into());
            }
            fsutil::atomic(&self.receipt, bytes.as_bytes())?;
        } else {
            write_new(&self.receipt, bytes.as_bytes(), 0o600)?;
        }
        File::open(&self.runtime)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    }
}
fn rename(ctx: &Context, from: &Path, to: &Path, flags: libc::c_uint) -> Result<()> {
    ctx.parent_unchanged()?;
    let a = CString::new(
        from.file_name()
            .ok_or("Missing source filename")?
            .as_bytes(),
    )
    .map_err(|_| "Invalid filename")?;
    let b = CString::new(to.file_name().ok_or("Missing target filename")?.as_bytes())
        .map_err(|_| "Invalid filename")?;
    if from.parent() != Some(ctx.parent.as_path()) || to.parent() != Some(ctx.parent.as_path()) {
        return Err("Update rename left pinned directory".into());
    }
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            ctx.dir.as_raw_fd(),
            a.as_ptr(),
            ctx.dir.as_raw_fd(),
            b.as_ptr(),
            flags,
        )
    };
    if result != 0 {
        return Err(format!(
            "Atomic update rename failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    ctx.sync()
}
fn decode_hex(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 || !s.bytes().all(|x| x.is_ascii_hexdigit()) {
        return Err("Malformed signature".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "Malformed signature".into()))
        .collect()
}
fn bind(ctx: &Context) -> Result<Value> {
    let current = pin(&ctx.target_fd())?;
    check_env_pin(&current, false)?;
    ctx.parent_unchanged()?;
    let bytes = wire(
        "WOW_UPDATE_BIND_V1",
        &ctx.nonce,
        &current.sha256,
        env!("CARGO_PKG_VERSION"),
        env!("WOW_SOURCE_SHA256"),
    );
    write_new(&ctx.binding, bytes.as_bytes(), 0o600)?;
    File::open(&ctx.runtime)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(json!({"bound":true}))
}
fn commit(data: &Path, ctx: &Context, key: [u8; 32]) -> Result<Value> {
    let _updates = fsutil::Lock::acquire(&data.join("updates.lock"))?;
    let _operation = fsutil::Lock::acquire(&data.join("operation.lock"))?;
    idle(data)?;
    cancelled()?;
    let r = ctx.request()?;
    let current = pin(&ctx.target_fd())?;
    check_env_pin(&current, true)?;
    if current != r.current {
        return Err("Original portable changed before update commit".into());
    }
    let staged_path = ctx.scoped(&r.staged.path)?;
    if pin(&staged_path)? != r.staged.pin {
        return Err("Staged portable identity changed before update commit".into());
    }
    let bytes = read_regular(&staged_path, MAX_BINARY)?;
    verify_payload(
        key,
        &bytes,
        &r.staged.pin.sha256,
        &decode_hex(&r.signature_hex)?,
    )?;
    ctx.parent_unchanged()?;
    if pin(&staged_path)? != r.staged.pin {
        return Err("Staged portable changed before build query".into());
    }
    let info = super::transport::build_info(&r.staged.path)?;
    ctx.parent_unchanged()?;
    if pin(&staged_path)? != r.staged.pin {
        return Err("Staged portable changed after build query".into());
    }
    if info["version"].as_str() != Some(r.version.as_str())
        || info["source_sha256"].as_str() != Some(r.source_sha256.as_str())
        || info["developer_tools"] != false
        || info["build_mode"] != "public"
    {
        return Err("Authenticated candidate build identity changed".into());
    }
    let j = Journal {
        schema: 1,
        nonce: ctx.nonce.clone(),
        target: ctx.target.clone(),
        old: r.current.clone(),
        new: r.staged.pin.clone(),
        staged: r.staged.path.clone(),
        backup: ctx.backup(),
        old_version: r.old_version.clone(),
        old_source: r.old_source_sha256.clone(),
        new_version: r.version.clone(),
        new_source: r.source_sha256.clone(),
    };
    if ctx
        .scoped(&j.backup)?
        .try_exists()
        .map_err(|e| e.to_string())?
    {
        return Err("Backup path is already occupied".into());
    }
    write_new(
        &ctx.recovery(),
        &serde_json::to_vec_pretty(&j).map_err(|e| e.to_string())?,
        0o600,
    )?;
    ctx.sync()?;
    idle(data)?;
    cancelled()?;
    ctx.parent_unchanged()?;
    if pin(&ctx.target_fd())? != j.old || pin(&staged_path)? != j.new {
        return Err("Update files changed immediately before replacement".into());
    }
    rename(ctx, &ctx.target, &j.staged, libc::RENAME_EXCHANGE)?;
    rename(ctx, &j.staged, &j.backup, libc::RENAME_NOREPLACE)?;
    if pin(&ctx.target_fd())? != j.new || pin(&ctx.scoped(&j.backup)?)? != j.old {
        return Err("Post-commit identities do not match; recovery record retained".into());
    }
    ctx.receipt(
        "WOW_UPDATE_COMMIT_V1",
        &j.new.sha256,
        &j.new_version,
        &j.new_source,
    )?;
    Ok(json!({"committed":true,"version":j.new_version}))
}
fn rollback(data: &Path, ctx: &Context) -> Result<Value> {
    let _updates = fsutil::Lock::acquire(&data.join("updates.lock"))?;
    let _operation = fsutil::Lock::acquire(&data.join("operation.lock"))?;
    idle(data)?;
    ctx.parent_unchanged()?;
    if !ctx.recovery().try_exists().map_err(|e| e.to_string())? {
        let current = pin(&ctx.target_fd())?;
        check_env_pin(&current, true)?;
        let b = ctx.binding()?;
        if current.sha256 != b[2]
            || b[3] != env!("CARGO_PKG_VERSION")
            || b[4] != env!("WOW_SOURCE_SHA256")
        {
            return Err("Original portable cannot be confirmed; no rollback was attempted".into());
        }
        ctx.receipt("WOW_UPDATE_ROLLBACK_V1", &current.sha256, &b[3], &b[4])?;
        return Ok(json!({"rolled_back":true,"already_original":true}));
    }
    let j = ctx.journal()?;
    let current = pin(&ctx.target_fd())?;
    if current == j.old {
        ctx.receipt(
            "WOW_UPDATE_ROLLBACK_V1",
            &j.old.sha256,
            &j.old_version,
            &j.old_source,
        )?;
        return Ok(json!({"rolled_back":true,"already_original":true}));
    }
    if current != j.new {
        return Err("Portable was changed independently; verified backups and recovery record were preserved".into());
    }
    let backup = ctx.scoped(&j.backup)?;
    let staged = ctx.scoped(&j.staged)?;
    let old_location = if pin(&backup).map(|p| p == j.old).unwrap_or(false) {
        j.backup.clone()
    } else if pin(&staged).map(|p| p == j.old).unwrap_or(false) {
        j.staged.clone()
    } else {
        return Err("Verified original backup is unavailable; recovery record preserved".into());
    };
    idle(data)?;
    ctx.parent_unchanged()?;
    if pin(&ctx.target_fd())? != j.new || pin(&ctx.scoped(&old_location)?)? != j.old {
        return Err("Rollback files changed before restoration".into());
    }
    rename(ctx, &ctx.target, &old_location, libc::RENAME_EXCHANGE)?;
    if pin(&ctx.target_fd())? != j.old || pin(&ctx.scoped(&old_location)?)? != j.new {
        return Err("Rollback verification failed; both files and recovery record retained".into());
    }
    ctx.receipt(
        "WOW_UPDATE_ROLLBACK_V1",
        &j.old.sha256,
        &j.old_version,
        &j.old_source,
    )?;
    Ok(json!({"rolled_back":true,"already_original":false}))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CompletedBackup {
    journal: Journal,
    record_pin: FilePin,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupHistory {
    schema: u32,
    entries: Vec<CompletedBackup>,
}
fn history(data: &Path) -> Result<BackupHistory> {
    let path = data.join("update-backups.json");
    if !path.try_exists().map_err(|e| e.to_string())? {
        return Ok(BackupHistory {
            schema: 1,
            entries: vec![],
        });
    }
    let h: BackupHistory = serde_json::from_slice(&read_private(&path, 1024 * 1024)?)
        .map_err(|_| "Backup ownership records are unreadable; all backups were kept")?;
    if h.schema != 1 || h.entries.len() > 64 {
        return Err("Backup ownership records are unsupported; all backups were kept".into());
    }
    Ok(h)
}
fn valid_historical(ctx: &Context, j: &Journal) -> Result<()> {
    if j.schema != 1
        || j.target != ctx.target
        || !is_hex(&j.nonce, 32)
        || j.staged != ctx.parent.join(format!(".wow-update-{}.new", j.nonce))
        || j.backup != ctx.parent.join(format!(".wow-update-{}.backup", j.nonce))
        || !is_hex(&j.old.sha256, 64)
        || !is_hex(&j.new.sha256, 64)
        || !is_hex(&j.old_source, 64)
        || !is_hex(&j.new_source, 64)
        || Version::parse(&j.new_version)? <= Version::parse(&j.old_version)?
    {
        return Err("Historical backup ownership does not match this portable".into());
    }
    Ok(())
}
fn historical_record(ctx: &Context, j: &Journal) -> PathBuf {
    ctx.parent
        .join(format!(".wow-update-{}.recovery.json", j.nonce))
}
fn move_no_replace(
    from: i32,
    from_name: &std::ffi::OsStr,
    to: i32,
    to_name: &std::ffi::OsStr,
) -> Result<()> {
    let a = CString::new(from_name.as_bytes()).map_err(|_| "Invalid cleanup filename")?;
    let b = CString::new(to_name.as_bytes()).map_err(|_| "Invalid cleanup filename")?;
    let rc = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            from,
            a.as_ptr(),
            to,
            b.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if rc != 0 {
        return Err(format!(
            "Older backup cleanup was deferred: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}
fn prune_pair(
    ctx: &Context,
    backup: &Path,
    backup_pin: &FilePin,
    record: &Path,
    record_pin: &FilePin,
) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    ctx.parent_unchanged()?;
    let paths = [backup, record];
    let expected = [backup_pin, record_pin];
    for i in 0..2 {
        if pin(&ctx.scoped(paths[i])?)? != *expected[i] {
            return Err("Older backup or ownership record changed; both were kept".into());
        }
    }
    let name = format!(".wow-update-clean-{}", fsutil::unique());
    let qpath = ctx.path(std::ffi::OsStr::new(&name));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&qpath)
        .map_err(|e| e.to_string())?;
    let q = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&qpath)
        .map_err(|e| e.to_string())?;
    q.set_permissions(fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let q_identity = q.metadata().map_err(|e| e.to_string())?;
    let slots = [
        std::ffi::OsStr::new("backup"),
        std::ffi::OsStr::new("record"),
    ];
    let qfile = |slot: &std::ffi::OsStr| {
        PathBuf::from(format!("/proc/self/fd/{}", q.as_raw_fd())).join(slot)
    };
    let mut moved = 0usize;
    let result = (|| {
        for i in 0..2 {
            ctx.parent_unchanged()?;
            if pin(&ctx.scoped(paths[i])?)? != *expected[i] {
                return Err("Older cleanup file changed before quarantine".into());
            }
            move_no_replace(
                ctx.dir.as_raw_fd(),
                paths[i].file_name().unwrap(),
                q.as_raw_fd(),
                slots[i],
            )?;
            moved += 1;
        }
        q.sync_all().map_err(|e| e.to_string())?;
        ctx.sync()?;
        for i in 0..2 {
            if pin(&qfile(slots[i]))? != *expected[i] {
                return Err("Quarantined cleanup file did not match its owned identity".into());
            }
        }
        let m = q.metadata().map_err(|e| e.to_string())?;
        if m.dev() != q_identity.dev()
            || m.ino() != q_identity.ino()
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o777 != 0o700
        {
            return Err("Cleanup quarantine permissions changed".into());
        }
        for i in 0..2 {
            if pin(&qfile(slots[i]))? != *expected[i] {
                return Err("Quarantined file changed before cleanup".into());
            }
            let slot = CString::new(slots[i].as_bytes()).map_err(|_| "Invalid cleanup filename")?;
            if unsafe { libc::unlinkat(q.as_raw_fd(), slot.as_ptr(), 0) } != 0 {
                return Err(format!(
                    "Older backup cleanup was deferred: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        q.sync_all().map_err(|e| e.to_string())?;
        ctx.sync()?;
        Ok(())
    })();
    if result.is_err() {
        for i in (0..moved).rev() {
            if pin(&qfile(slots[i]))
                .map(|p| p == *expected[i])
                .unwrap_or(false)
                && ctx.parent_unchanged().is_ok()
            {
                let _ = move_no_replace(
                    q.as_raw_fd(),
                    slots[i],
                    ctx.dir.as_raw_fd(),
                    paths[i].file_name().unwrap(),
                );
            }
        }
        let _ = q.sync_all();
        let _ = ctx.sync();
    }
    if fs::symlink_metadata(&qpath)
        .map(|m| m.is_dir() && m.dev() == q_identity.dev() && m.ino() == q_identity.ino())
        .unwrap_or(false)
    {
        let _ = fs::remove_dir(&qpath);
    }
    result
}
fn retain_completed(data: &Path, ctx: &Context, newest: CompletedBackup) -> Result<Value> {
    let j = &newest.journal;
    valid_historical(ctx, j)?;
    let newest_version = Version::parse(&j.new_version)?;
    let mut h = history(data)?;
    if let Some(existing) = h
        .entries
        .iter()
        .find(|e| e.journal.target == j.target && e.journal.nonce == j.nonce)
    {
        if existing != &newest {
            return Err("Completed backup ownership changed; all backups were kept".into());
        }
    } else {
        if h.entries.len() >= 64 {
            return Err("Backup history needs review; all backups were kept".into());
        }
        h.entries.push(newest.clone());
    }
    fsutil::json(&data.join("update-backups.json"), &h)?;
    let mut kept = Vec::new();
    let mut cleaned = 0usize;
    let mut deferred = 0usize;
    for entry in h.entries {
        let older = entry.journal.target == ctx.target
            && entry.journal.nonce != j.nonce
            && Version::parse(&entry.journal.new_version)
                .map(|v| v <= newest_version)
                .unwrap_or(false);
        if older {
            let result = (|| {
                valid_historical(ctx, &entry.journal)?;
                let record = historical_record(ctx, &entry.journal);
                let scoped_record = ctx.scoped(&record)?;
                if pin(&scoped_record)? != entry.record_pin
                    || pin(&ctx.scoped(&entry.journal.backup)?)? != entry.journal.old
                {
                    return Err("Older backup or record changed; both were kept".into());
                }
                let recorded: Journal =
                    serde_json::from_slice(&read_private(&scoped_record, 64 * 1024)?)
                        .map_err(|_| "Older recovery record is unreadable")?;
                if recorded != entry.journal {
                    return Err("Older recovery record contents changed".into());
                }
                ctx.parent_unchanged()?;
                if pin(&ctx.target_fd())? != j.new || pin(&ctx.scoped(&j.backup)?)? != j.old {
                    return Err("Newest retained backup changed; cleanup stopped".into());
                }
                prune_pair(
                    ctx,
                    &entry.journal.backup,
                    &entry.journal.old,
                    &record,
                    &entry.record_pin,
                )
            })();
            if result.is_ok() {
                cleaned += 1;
                continue;
            }
            deferred += 1;
        }
        kept.push(entry);
    }
    fsutil::json(
        &data.join("update-backups.json"),
        &BackupHistory {
            schema: 1,
            entries: kept,
        },
    )?;
    Ok(
        json!({"finalized":true,"older_pairs_removed":cleaned,"older_pairs_deferred":deferred,"retained_backup":j.backup}),
    )
}
fn finalize(data: &Path, ctx: &Context, key: [u8; 32]) -> Result<Value> {
    let _updates = fsutil::Lock::acquire(&data.join("updates.lock"))?;
    ctx.parent_unchanged()?;
    let r = ctx.request()?;
    let j = ctx.journal()?;
    if r.current != j.old
        || r.staged.pin != j.new
        || r.version != j.new_version
        || r.source_sha256 != j.new_source
    {
        return Err("Finalized update identity does not match its authenticated request".into());
    }
    let ready = lines(&read_private(&ctx.runtime.join("update-ready"), 4096)?, 4)?;
    let decision = lines(&read_private(&ctx.runtime.join("update-decision"), 128)?, 2)?;
    if ready
        != vec![
            "WOW_UPDATE_READY_V1".to_owned(),
            ctx.nonce.clone(),
            j.new_version.clone(),
            j.new_source.clone(),
        ]
        || decision != vec!["READY".to_owned(), ctx.nonce.clone()]
    {
        return Err("The new installer has not confirmed readiness; all backups were kept".into());
    }
    if pin(&ctx.target_fd())? != j.new || pin(&ctx.scoped(&j.backup)?)? != j.old {
        return Err("Current or previous portable changed; all backups were kept".into());
    }
    verify_payload(
        key,
        &read_regular(&ctx.target_fd(), MAX_BINARY)?,
        &j.new.sha256,
        &decode_hex(&r.signature_hex)?,
    )?;
    let newest = CompletedBackup {
        journal: j,
        record_pin: pin(&ctx.recovery())?,
    };
    retain_completed(data, ctx, newest)
}
fn verified_previous_path(ctx: &Context, expected: &FilePin) -> Result<PathBuf> {
    ctx.parent_unchanged()?;
    for p in [ctx.backup(), ctx.staged(), ctx.target.clone()] {
        if pin(&ctx.scoped(&p)?)
            .map(|found| found == *expected)
            .unwrap_or(false)
        {
            ctx.parent_unchanged()?;
            if pin(&ctx.scoped(&p)?)? == *expected {
                return Ok(p);
            }
        }
    }
    Err("No owned previous installer could be verified; recovery files were kept".into())
}
fn recovery_info(ctx: &Context) -> Result<Value> {
    let b = ctx.binding()?;
    if b[3] != env!("CARGO_PKG_VERSION") || b[4] != env!("WOW_SOURCE_SHA256") {
        return Err("Original build binding does not match this recovery helper".into());
    }
    let expected = FilePin {
        device: env("WOW_PORTABLE_DEV")?
            .parse()
            .map_err(|_| "Invalid portable device")?,
        inode: env("WOW_PORTABLE_INO")?
            .parse()
            .map_err(|_| "Invalid portable inode")?,
        size: env("WOW_PORTABLE_SIZE")?
            .parse()
            .map_err(|_| "Invalid portable size")?,
        mode: env("WOW_PORTABLE_MODE")?
            .parse()
            .map_err(|_| "Invalid portable mode")?,
        uid: env("WOW_PORTABLE_UID")?
            .parse()
            .map_err(|_| "Invalid portable owner")?,
        sha256: b[2].clone(),
    };
    check_env_pin(&expected, true)?;
    let path = verified_previous_path(ctx, &expected)?;
    fsutil::safe_path(&path)?;
    let wire = format!(
        "WOW_UPDATE_RECOVERY_V1\n{}\n{}\n{}\n{}\n{}\n",
        ctx.nonce,
        path.to_str().ok_or("Recovery path is not UTF-8")?,
        b[2],
        b[3],
        b[4]
    );
    let output = ctx.runtime.join("update-recovery-info");
    if output.try_exists().map_err(|e| e.to_string())? {
        let old = lines(&read_private(&output, 16384)?, 6)?;
        if old[0] != "WOW_UPDATE_RECOVERY_V1"
            || old[1] != ctx.nonce
            || old[3] != b[2]
            || old[4] != b[3]
            || old[5] != b[4]
        {
            return Err("Existing recovery report is not owned by this update".into());
        }
        fsutil::atomic(&output, wire.as_bytes())?;
    } else {
        write_new(&output, wire.as_bytes(), 0o600)?;
    }
    File::open(&ctx.runtime)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(json!({"available":true,"path":path}))
}

fn abort_absent(path: &Path) -> Result<()> {
    if path.try_exists().map_err(|e| e.to_string())? {
        return Err("Startup was cancelled before readiness".into());
    }
    Ok(())
}
fn ready() -> Result<Value> {
    let ack = env_path("WOW_UPDATE_STARTUP_ACK")?;
    private_parent(&ack)?;
    let runtime = ack.parent().unwrap();
    let name = ack
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or("Invalid readiness path")?;
    let (header, abort_name, decision_name) = match name {
        "update-ready" => ("WOW_UPDATE_COMMIT_V1", "update-abort", "update-decision"),
        "rollback-ready" => (
            "WOW_UPDATE_ROLLBACK_V1",
            "rollback-abort",
            "rollback-decision",
        ),
        _ => return Err("Invalid readiness filename".into()),
    };
    let nonce = env("WOW_UPDATE_STARTUP_NONCE")?;
    if !is_hex(&nonce, 32) {
        return Err("Invalid readiness nonce".into());
    }
    let abort = runtime.join(abort_name);
    abort_absent(&abort)?;
    let receipt = lines(&read_private(&runtime.join("update-receipt"), 4096)?, 5)?;
    if receipt[0] != header
        || receipt[1] != nonce
        || !is_hex(&receipt[2], 64)
        || receipt[3] != env!("CARGO_PKG_VERSION")
        || receipt[4] != env!("WOW_SOURCE_SHA256")
    {
        return Err("New window does not match the committed build".into());
    }
    let target = env_path("WOW_PORTABLE_PATH")?;
    if !target.is_absolute() || target.canonicalize().map_err(|e| e.to_string())? != target {
        return Err("Ready portable path changed".into());
    }
    let current = pin(&target)?;
    check_env_pin(&current, true)?;
    if current.sha256 != receipt[2] {
        return Err("Ready portable bytes do not match the committed receipt".into());
    }
    abort_absent(&abort)?;
    write_new(
        &runtime.join(decision_name),
        format!("READY\n{nonce}\n").as_bytes(),
        0o600,
    )?;
    let response = format!(
        "WOW_UPDATE_READY_V1\n{nonce}\n{}\n{}\n",
        env!("CARGO_PKG_VERSION"),
        env!("WOW_SOURCE_SHA256")
    );
    write_new(&ack, response.as_bytes(), 0o600)?;
    File::open(runtime)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(json!({"ready":true}))
}
pub(super) fn dispatch(data: &Path, request: &Value) -> Result<Value> {
    let action = request["action"].as_str().ok_or("Missing wrapper action")?;
    if action == "updater_ready" {
        return ready();
    }
    if ![
        "updater_bind",
        "updater_commit",
        "updater_rollback",
        "updater_finalize",
        "updater_recovery_info",
    ]
    .contains(&action)
        || env("WOW_UPDATE_WRAPPER_ACTION")? != action
    {
        return Err("This update action is reserved for its running portable wrapper".into());
    }
    let object = request.as_object().ok_or("Invalid wrapper action")?;
    if object.len() != 1 {
        return Err("Wrapper actions do not accept paths or options".into());
    }
    let ctx = Context::from_env()?;
    match action {
        "updater_bind" => bind(&ctx),
        "updater_recovery_info" => recovery_info(&ctx),
        "updater_finalize" => {
            let key = PRODUCTION_KEY
                .ok_or("Signed automatic updates are not configured in this build")?;
            super::prepare_data(data)?;
            finalize(data, &ctx, key)
        }
        "updater_commit" => {
            let key = PRODUCTION_KEY.ok_or(
                "Signed automatic updates are not configured in this build; use the release page",
            )?;
            super::prepare_data(data)?;
            commit(data, &ctx, key)
        }
        "updater_rollback" => {
            super::prepare_data(data)?;
            rollback(data, &ctx)
        }
        _ => Err("Unknown wrapper action".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn fixture() -> (PathBuf, PathBuf, Context) {
        let root = std::env::temp_dir().join(format!("wow-updater-wrapper-{}", fsutil::unique()));
        fsutil::private_dir(&root).unwrap();
        let data = root.join("data");
        let runtime = root.join("runtime");
        fsutil::private_dir(&data).unwrap();
        fsutil::private_dir(&runtime).unwrap();
        let target = root.join("portable");
        fs::write(&target, b"original fixture bytes").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        let dir = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&root)
            .unwrap();
        let ctx = Context {
            target,
            parent: root.clone(),
            runtime: runtime.clone(),
            nonce: "0123456789abcdef0123456789abcdef".into(),
            request: runtime.join("update-request.json"),
            receipt: runtime.join("update-receipt"),
            binding: runtime.join("update-binding"),
            dir,
        };
        (root, data, ctx)
    }
    fn prepare(ctx: &Context) -> Journal {
        let old = pin(&ctx.target_fd()).unwrap();
        let staged = ctx.staged();
        write_new(&staged, b"new fixture bytes", 0o700).unwrap();
        let new = pin(&staged).unwrap();
        write_new(
            &ctx.binding,
            wire(
                "WOW_UPDATE_BIND_V1",
                &ctx.nonce,
                &old.sha256,
                env!("CARGO_PKG_VERSION"),
                env!("WOW_SOURCE_SHA256"),
            )
            .as_bytes(),
            0o600,
        )
        .unwrap();
        let j = Journal {
            schema: 1,
            nonce: ctx.nonce.clone(),
            target: ctx.target.clone(),
            old,
            new,
            staged,
            backup: ctx.backup(),
            old_version: env!("CARGO_PKG_VERSION").into(),
            old_source: env!("WOW_SOURCE_SHA256").into(),
            new_version: "99.0.0".into(),
            new_source: "a".repeat(64),
        };
        write_new(&ctx.recovery(), &serde_json::to_vec(&j).unwrap(), 0o600).unwrap();
        j
    }
    #[test]
    fn wire_rejects_cr_extra_or_incomplete_records() {
        assert!(lines(b"H\nn\nh\nv\ns\n", 5).is_ok());
        assert!(lines(b"H\r\nn\nh\nv\ns\n", 5).is_err());
        assert!(lines(b"H\nn\nh\nv\ns", 5).is_err());
        assert!(lines(b"H\nn\nh\nv\ns\nx\n", 5).is_err());
    }
    #[test]
    fn atomic_exchange_and_rollback_restore_the_original_inode() {
        let (root, data, ctx) = fixture();
        let j = prepare(&ctx);
        rename(&ctx, &ctx.target, &j.staged, libc::RENAME_EXCHANGE).unwrap();
        rename(&ctx, &j.staged, &j.backup, libc::RENAME_NOREPLACE).unwrap();
        assert_eq!(pin(&ctx.target_fd()).unwrap(), j.new);
        rollback(&data, &ctx).unwrap();
        assert_eq!(pin(&ctx.target_fd()).unwrap(), j.old);
        assert_eq!(pin(&ctx.scoped(&j.backup).unwrap()).unwrap(), j.new);
        let receipt = lines(&read_private(&ctx.receipt, 4096).unwrap(), 5).unwrap();
        assert_eq!(receipt[0], "WOW_UPDATE_ROLLBACK_V1");
        assert_eq!(receipt[2], j.old.sha256);
        drop(ctx);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_exchange_recovers_original_from_staged_name() {
        let (root, data, ctx) = fixture();
        let j = prepare(&ctx);
        rename(&ctx, &ctx.target, &j.staged, libc::RENAME_EXCHANGE).unwrap();
        rollback(&data, &ctx).unwrap();
        assert_eq!(pin(&ctx.target_fd()).unwrap(), j.old);
        assert_eq!(pin(&ctx.scoped(&j.staged).unwrap()).unwrap(), j.new);
        drop(ctx);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unrelated_target_change_is_never_overwritten_by_rollback() {
        let (root, data, ctx) = fixture();
        let j = prepare(&ctx);
        fs::write(&ctx.target, b"independent edit").unwrap();
        assert!(rollback(&data, &ctx).is_err());
        assert_eq!(fs::read(&ctx.target).unwrap(), b"independent edit");
        assert_eq!(pin(&ctx.scoped(&j.staged).unwrap()).unwrap(), j.new);
        drop(ctx);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn backup_collision_recovers_old_stage_without_touching_collision() {
        let (root, data, ctx) = fixture();
        let j = prepare(&ctx);
        rename(&ctx, &ctx.target, &j.staged, libc::RENAME_EXCHANGE).unwrap();
        fs::write(&j.backup, b"unrelated occupied backup").unwrap();
        rollback(&data, &ctx).unwrap();
        assert_eq!(pin(&ctx.target_fd()).unwrap(), j.old);
        assert_eq!(fs::read(&j.backup).unwrap(), b"unrelated occupied backup");
        drop(ctx);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn existing_atomic_decision_is_never_replaced() {
        let (root, _, ctx) = fixture();
        let p = ctx.runtime.join("update-decision");
        write_new(&p, format!("ABORT\n{}\n", ctx.nonce).as_bytes(), 0o600).unwrap();
        assert!(write_new(&p, b"READY\n", 0o600).is_err());
        assert!(fs::read_to_string(&p).unwrap().starts_with("ABORT\n"));
        drop(ctx);
        fs::remove_dir_all(root).unwrap();
    }
    fn next_context(old: &Context, nonce: &str) -> Context {
        let runtime = old.parent.join(format!("runtime-{nonce}"));
        fsutil::private_dir(&runtime).unwrap();
        Context {
            target: old.target.clone(),
            parent: old.parent.clone(),
            runtime: runtime.clone(),
            nonce: nonce.into(),
            request: runtime.join("update-request.json"),
            receipt: runtime.join("update-receipt"),
            binding: runtime.join("update-binding"),
            dir: old.dir.try_clone().unwrap(),
        }
    }
    fn completed_pair(
        ctx: &Context,
        old_version: &str,
        new_version: &str,
        bytes: &[u8],
    ) -> CompletedBackup {
        let old = pin(&ctx.target_fd()).unwrap();
        write_new(&ctx.staged(), bytes, 0o700).unwrap();
        let new = pin(&ctx.staged()).unwrap();
        let j = Journal {
            schema: 1,
            nonce: ctx.nonce.clone(),
            target: ctx.target.clone(),
            old,
            new,
            staged: ctx.staged(),
            backup: ctx.backup(),
            old_version: old_version.into(),
            old_source: "a".repeat(64),
            new_version: new_version.into(),
            new_source: "b".repeat(64),
        };
        write_new(&ctx.recovery(), &serde_json::to_vec(&j).unwrap(), 0o600).unwrap();
        rename(ctx, &ctx.target, &j.staged, libc::RENAME_EXCHANGE).unwrap();
        rename(ctx, &j.staged, &j.backup, libc::RENAME_NOREPLACE).unwrap();
        CompletedBackup {
            journal: j,
            record_pin: pin(&ctx.recovery()).unwrap(),
        }
    }
    #[test]
    fn two_completed_updates_keep_only_newest_owned_previous_pair() {
        let (root, data, first) = fixture();
        let a = completed_pair(&first, "1.0.0", "1.0.1", b"version one fixture");
        retain_completed(&data, &first, a).unwrap();
        let second = next_context(&first, "22222222222222222222222222222222");
        let b = completed_pair(&second, "1.0.1", "1.0.2", b"version two fixture");
        retain_completed(&data, &second, b.clone()).unwrap();
        assert!(!first.backup().exists());
        assert!(!first.recovery().exists());
        assert_eq!(pin(&second.backup()).unwrap(), b.journal.old);
        assert_eq!(pin(&second.target).unwrap(), b.journal.new);
        assert!(second.recovery().exists());
        let h = history(&data).unwrap();
        assert_eq!(h.entries.len(), 1);
        assert_eq!(h.entries[0], b);
        drop(second);
        drop(first);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unrecorded_lookalike_pairs_survive_completed_cleanup() {
        let (root, data, first) = fixture();
        let a = completed_pair(&first, "1.0.0", "1.0.1", b"one");
        retain_completed(&data, &first, a).unwrap();
        let unknown = next_context(&first, "33333333333333333333333333333333");
        write_new(
            &unknown.backup(),
            b"personal unrelated fixture bytes",
            0o700,
        )
        .unwrap();
        let lookalike = Journal {
            schema: 1,
            nonce: unknown.nonce.clone(),
            target: unknown.target.clone(),
            old: pin(&unknown.backup()).unwrap(),
            new: pin(&unknown.target).unwrap(),
            staged: unknown.staged(),
            backup: unknown.backup(),
            old_version: "0.1.0".into(),
            old_source: "a".repeat(64),
            new_version: "0.1.1".into(),
            new_source: "b".repeat(64),
        };
        write_new(
            &unknown.recovery(),
            &serde_json::to_vec(&lookalike).unwrap(),
            0o600,
        )
        .unwrap();
        let saved = read_regular(&unknown.recovery(), 65536).unwrap();
        let second = next_context(&first, "44444444444444444444444444444444");
        let b = completed_pair(&second, "1.0.1", "1.0.2", b"two");
        retain_completed(&data, &second, b).unwrap();
        assert_eq!(
            fs::read(unknown.backup()).unwrap(),
            b"personal unrelated fixture bytes"
        );
        assert_eq!(read_regular(&unknown.recovery(), 65536).unwrap(), saved);
        drop(second);
        drop(unknown);
        drop(first);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn replaced_identical_backup_is_preserved_as_unresolved() {
        let (root, data, first) = fixture();
        let a = completed_pair(&first, "1.0.0", "1.0.1", b"one");
        retain_completed(&data, &first, a.clone()).unwrap();
        let duplicate = first.parent.join("independent-copy");
        write_new(&duplicate, &fs::read(first.backup()).unwrap(), 0o700).unwrap();
        fs::rename(&duplicate, first.backup()).unwrap();
        let independently_owned = pin(&first.backup()).unwrap();
        assert_ne!(independently_owned.inode, a.journal.old.inode);
        let second = next_context(&first, "55555555555555555555555555555555");
        let b = completed_pair(&second, "1.0.1", "1.0.2", b"two");
        retain_completed(&data, &second, b).unwrap();
        assert_eq!(pin(&first.backup()).unwrap(), independently_owned);
        assert!(first.recovery().exists());
        assert_eq!(history(&data).unwrap().entries.len(), 2);
        drop(second);
        drop(first);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn recovery_reports_actual_verified_backup_not_a_guessed_name() {
        let (root, _, ctx) = fixture();
        let entry = completed_pair(&ctx, "1.0.0", "1.0.1", b"new bytes");
        assert_eq!(
            verified_previous_path(&ctx, &entry.journal.old).unwrap(),
            ctx.backup()
        );
        fs::rename(ctx.backup(), ctx.staged()).unwrap();
        write_new(&ctx.backup(), b"unrelated backup-name collision", 0o700).unwrap();
        assert_eq!(
            verified_previous_path(&ctx, &entry.journal.old).unwrap(),
            ctx.staged()
        );
        assert_eq!(
            fs::read(ctx.backup()).unwrap(),
            b"unrelated backup-name collision"
        );
        let replacement = ctx.parent.join("independent-staged-replacement");
        write_new(&replacement, b"original fixture bytes", 0o700).unwrap();
        assert_ne!(pin(&replacement).unwrap().inode, entry.journal.old.inode);
        fs::rename(&replacement, ctx.staged()).unwrap();
        assert!(verified_previous_path(&ctx, &entry.journal.old).is_err());
        drop(ctx);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unconfirmed_current_update_never_authorizes_retention_cleanup() {
        use ed25519_dalek::{Signer, SigningKey};
        let (root, data, ctx) = fixture();
        let mut entry = completed_pair(
            &ctx,
            env!("CARGO_PKG_VERSION"),
            "999.0.0",
            b"signed fake payload, not executable",
        );
        entry.journal.old_source = env!("WOW_SOURCE_SHA256").into();
        fsutil::json(&ctx.recovery(), &entry.journal).unwrap();
        entry.record_pin = pin(&ctx.recovery()).unwrap();
        let j = &entry.journal;
        write_new(
            &ctx.binding,
            wire(
                "WOW_UPDATE_BIND_V1",
                &ctx.nonce,
                &j.old.sha256,
                &j.old_version,
                &j.old_source,
            )
            .as_bytes(),
            0o600,
        )
        .unwrap();
        let key = SigningKey::from_bytes(&[17u8; 32]);
        let bytes = read_regular(&ctx.target_fd(), MAX_BINARY).unwrap();
        let signature = key.sign(&bytes).to_bytes();
        let request = Request {
            schema: 1,
            repository: REPOSITORY.into(),
            nonce: ctx.nonce.clone(),
            target: ctx.target.clone(),
            current: j.old.clone(),
            old_version: j.old_version.clone(),
            old_source_sha256: j.old_source.clone(),
            staged: Staged {
                path: j.staged.clone(),
                pin: j.new.clone(),
            },
            version: j.new_version.clone(),
            source_sha256: j.new_source.clone(),
            release_url: super::super::page(&format!("v{}", j.new_version)),
            signature_hex: super::super::hex(&signature),
        };
        write_new(&ctx.request, &serde_json::to_vec(&request).unwrap(), 0o600).unwrap();
        let before_target = pin(&ctx.target_fd()).unwrap();
        let before_backup = pin(&ctx.backup()).unwrap();
        let before_record = pin(&ctx.recovery()).unwrap();
        assert!(finalize(&data, &ctx, key.verifying_key().to_bytes()).is_err());
        assert!(!data.join("update-backups.json").exists());
        write_new(
            &ctx.runtime.join("update-ready"),
            format!(
                "WOW_UPDATE_READY_V1\n{}\n{}\n{}\n",
                "f".repeat(32),
                j.new_version,
                j.new_source
            )
            .as_bytes(),
            0o600,
        )
        .unwrap();
        write_new(
            &ctx.runtime.join("update-decision"),
            format!("READY\n{}\n", ctx.nonce).as_bytes(),
            0o600,
        )
        .unwrap();
        assert!(finalize(&data, &ctx, key.verifying_key().to_bytes()).is_err());
        assert!(!data.join("update-backups.json").exists());
        assert_eq!(pin(&ctx.target_fd()).unwrap(), before_target);
        assert_eq!(pin(&ctx.backup()).unwrap(), before_backup);
        assert_eq!(pin(&ctx.recovery()).unwrap(), before_record);
        drop(ctx);
        fs::remove_dir_all(root).unwrap();
    }
}
