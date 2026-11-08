//! Installer update policy. Replacement is completed by the portable wrapper.
mod transport;
mod wrapper;
use crate::{fsutil, Result};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    cmp::Ordering,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering as AtomicOrdering},
    time::{SystemTime, UNIX_EPOCH},
};
pub const REPOSITORY: &str = "Coaxske/wow-steamos-installer";
pub const RELEASE_PAGE: &str = "https://github.com/Coaxske/wow-steamos-installer/releases";
pub const PORTABLE: &str = "WoW-SteamOS-Rust-Linux-x86_64";
const DAY: u64 = 86400;
const MAX_BINARY: u64 = 512 * 1024 * 1024;
const MAX_STATE: u64 = 128 * 1024;
// Production provisioning remains a separate reviewed change. No test key fallback.
const PRODUCTION_KEY: Option<[u8; 32]> = None;
// One atomic winner: cancellation or prepared restart.
static CANCEL: AtomicU8 = AtomicU8::new(0);
fn cancel_state(state: &AtomicU8) -> bool {
    match state.compare_exchange(0, 1, AtomicOrdering::SeqCst, AtomicOrdering::SeqCst) {
        Ok(_) | Err(1) => true,
        Err(_) => false,
    }
}
fn reset_state(state: &AtomicU8) {
    let _ = state.compare_exchange(1, 0, AtomicOrdering::SeqCst, AtomicOrdering::SeqCst);
}
pub fn cancel() -> bool {
    cancel_state(&CANCEL)
}
pub fn reset_cancel() {
    reset_state(&CANCEL)
}
fn cancelled() -> Result<()> {
    if CANCEL.load(AtomicOrdering::SeqCst) == 1 {
        Err("Update cancelled".into())
    } else {
        Ok(())
    }
}
fn prepare_state(state: &AtomicU8) -> Result<()> {
    state
        .compare_exchange(0, 2, AtomicOrdering::SeqCst, AtomicOrdering::SeqCst)
        .map(|_| ())
        .map_err(|_| "Update was cancelled before its restart was prepared".into())
}
fn mark_prepared() -> Result<()> {
    prepare_state(&CANCEL)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    version: String,
    tag: String,
    page: String,
    token: String,
    size: u64,
    #[serde(default)]
    automatic_assets_available: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    schema: u32,
    enabled: bool,
    last_attempt: Option<u64>,
    candidate: Option<Candidate>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: 1,
            enabled: true,
            last_attempt: None,
            candidate: None,
        }
    }
}
fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|x| x.as_secs())
        .map_err(|_| "System clock is unavailable".into())
}
fn due(last: Option<u64>, current: u64) -> bool {
    last.map(|x| current.checked_sub(x).is_some_and(|elapsed| elapsed >= DAY))
        .unwrap_or(true)
}
fn load(data: &Path) -> Result<Settings> {
    let p = data.join("updates.json");
    if !p.try_exists().map_err(|e| e.to_string())? {
        return Ok(Settings::default());
    }
    let mut s: Settings = serde_json::from_slice(&read_regular(&p, MAX_STATE)?)
        .map_err(|_| "Saved update settings are unreadable")?;
    if s.schema != 1 {
        return Err("Saved update settings have an unsupported version".into());
    }
    if let Some(c) = &s.candidate {
        validate_candidate(c)?;
        if Version::parse(&c.version)? <= Version::parse(env!("CARGO_PKG_VERSION"))? {
            s.candidate = None;
        }
    }
    Ok(s)
}
fn save(data: &Path, state: &Settings) -> Result<()> {
    fsutil::json(&data.join("updates.json"), state)
}
fn status(s: &Settings) -> Value {
    json!({"enabled":s.enabled,"candidate":if s.enabled{s.candidate.as_ref()}else{None},"signing_configured":PRODUCTION_KEY.is_some(),"release_page":RELEASE_PAGE})
}
fn prepare_data(data: &Path) -> Result<()> {
    if !data.exists() {
        fsutil::private_dir(data)?;
    }
    let m = fs::symlink_metadata(data).map_err(|e| e.to_string())?;
    if !m.is_dir()
        || m.file_type().is_symlink()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o022 != 0
    {
        return Err("Update settings require an owned private directory".into());
    }
    Ok(())
}
fn idle(data: &Path) -> Result<()> {
    for name in [
        "transaction.json",
        "cleanup.json",
        "restore.json",
        "controller-job.json",
        "steam-restart.json",
        "update-request.json",
    ] {
        if data.join(name).try_exists().map_err(|e| e.to_string())? {
            return Err(
                "Finish the current setup, repair, removal or restart before updating".into(),
            );
        }
    }
    let state = data.join("state.json");
    if state.try_exists().map_err(|e| e.to_string())? {
        let value: Value = serde_json::from_slice(&read_regular(&state, 16 * 1024 * 1024)?)
            .map_err(|_| "Setup state must be readable before updating")?;
        let installations = value["installations"]
            .as_array()
            .ok_or("Setup state must be readable before updating")?;
        if value["addon_pending"].as_bool().unwrap_or(false)
            || installations.iter().any(|i| {
                !matches!(i["stage"].as_str(), Some("complete" | "removed"))
                    || i["addon_pending"].as_bool().unwrap_or(false)
            })
        {
            return Err("Finish the current setup before updating".into());
        }
    }
    Ok(())
}
fn idle_lock(data: &Path) -> Result<fsutil::Lock> {
    let lock = fsutil::Lock::acquire(&data.join("operation.lock"))?;
    idle(data)?;
    Ok(lock)
}
pub fn dispatch(data: &Path, request: &Value) -> Result<Value> {
    let action = request["action"].as_str().ok_or("Missing update action")?;
    if [
        "updater_bind",
        "updater_commit",
        "updater_rollback",
        "updater_ready",
    ]
    .contains(&action)
    {
        return wrapper::dispatch(data, request);
    }
    if action == "updater_status" {
        return load(data).map(|s| status(&s));
    }
    prepare_data(data)?;
    let _update_lock = match fsutil::Lock::acquire(&data.join("updates.lock")) {
        Ok(lock) => lock,
        Err(_) if action == "updater_check" => return Ok(status(&load(data)?)),
        Err(e) => return Err(e),
    };
    let mut s = load(data)?;
    match action {
        "updater_enabled" => {
            s.enabled = request["enabled"]
                .as_bool()
                .ok_or("Choose whether automatic update checks are enabled")?;
            if !s.enabled {
                s.candidate = None;
            }
            save(data, &s)?;
            Ok(status(&s))
        }
        "updater_check" => {
            if !s.enabled || !due(s.last_attempt, now()?) {
                return Ok(status(&s));
            }
            let guard = match idle_lock(data) {
                Ok(x) => x,
                Err(_) => return Ok(status(&s)),
            };
            s.last_attempt = Some(now()?);
            save(data, &s)?;
            drop(guard);
            let found = transport::releases(data)
                .and_then(|bytes| select_release(&bytes, env!("CARGO_PKG_VERSION")));
            let _guard = match idle_lock(data) {
                Ok(x) => x,
                Err(_) => return Ok(status(&s)),
            };
            if let Ok(candidate) = found {
                s.candidate = candidate;
                save(data, &s)?;
            }
            Ok(status(&s))
        }
        "updater_install" => {
            let failure = |message: String| {
                Ok(
                    json!({"restart_required":false,"manual_required":true,"message":message,"release_page":RELEASE_PAGE}),
                )
            };
            if request["confirmed"] != true {
                return failure("Choose Update now before installing an update".into());
            }
            if !s.enabled {
                return failure(
                    "Update checks are disabled. Download manually from the release page".into(),
                );
            }
            let candidate = match s.candidate.as_ref() {
                Some(c) if request["token"].as_str() == Some(c.token.as_str()) => c,
                _ => {
                    return failure(
                        "The update choice expired. Check again or use the release page".into(),
                    )
                }
            };
            if !candidate.automatic_assets_available {
                return failure("This release is missing usable installer, checksum or signature files. Use the release page".into());
            }
            let Some(key) = PRODUCTION_KEY else {
                return failure("Signed automatic updates are not configured in this build. Use the release page".into());
            };
            let _guard = match idle_lock(data) {
                Ok(x) => x,
                Err(e) => return failure(e),
            };
            match stage(data,candidate,key){Ok(_)=>Ok(json!({"restart_required":true,"version":candidate.version})),Err(e)=>failure(format!("Automatic update could not be prepared: {e}. Your current portable has not been replaced"))}
        }
        _ => Err("Unknown update action".into()),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    main: [u64; 3],
    pre: Vec<String>,
}
impl Version {
    fn parse(input: &str) -> Result<Self> {
        let s = input.strip_prefix('v').unwrap_or(input);
        if s.len() > 100 || s.contains('+') {
            return Err("Unrecognized release version".into());
        }
        let (core, pre) = s.split_once('-').unwrap_or((s, ""));
        let numbers: Vec<_> = core.split('.').collect();
        if numbers.len() != 3 {
            return Err("Unrecognized release version".into());
        }
        let mut main = [0; 3];
        for (i, n) in numbers.iter().enumerate() {
            if n.is_empty()
                || (n.len() > 1 && n.starts_with('0'))
                || !n.bytes().all(|b| b.is_ascii_digit())
            {
                return Err("Unrecognized release version".into());
            }
            main[i] = n.parse().map_err(|_| "Release version is too large")?;
        }
        let normalized = if let Some(rest) = pre.strip_prefix("alpha.") {
            if let Some((generation, candidate)) = rest.split_once("-candidate.") {
                if generation.bytes().all(|x| x.is_ascii_digit())
                    && candidate.bytes().all(|x| x.is_ascii_digit())
                {
                    format!("alpha.{generation}.candidate.{candidate}")
                } else {
                    pre.to_owned()
                }
            } else {
                pre.to_owned()
            }
        } else {
            pre.to_owned()
        };
        let mut parts = vec![];
        if s.contains('-') {
            for p in normalized.split('.') {
                if p.is_empty()
                    || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    || (p.bytes().all(|b| b.is_ascii_digit()) && p.len() > 1 && p.starts_with('0'))
                {
                    return Err("Unrecognized prerelease version".into());
                }
                if p.bytes().all(|b| b.is_ascii_digit()) {
                    let _: u64 = p.parse().map_err(|_| "Prerelease number is too large")?;
                }
                parts.push(p.to_owned());
            }
        }
        Ok(Self { main, pre: parts })
    }
}
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let base = self.main.cmp(&other.main);
        if base != Ordering::Equal {
            return base;
        }
        if self.pre.is_empty() || other.pre.is_empty() {
            return self.pre.is_empty().cmp(&other.pre.is_empty());
        }
        for (a, b) in self.pre.iter().zip(&other.pre) {
            let order = match (a.parse::<u64>(), b.parse::<u64>()) {
                (Ok(a), Ok(b)) => a.cmp(&b),
                (Ok(_), Err(_)) => Ordering::Less,
                (Err(_), Ok(_)) => Ordering::Greater,
                (Err(_), Err(_)) => a.cmp(b),
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        self.pre.len().cmp(&other.pre.len())
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
fn page(tag: &str) -> String {
    format!("https://github.com/{REPOSITORY}/releases/tag/{tag}")
}
fn asset(tag: &str, name: &str) -> String {
    format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{name}")
}
fn validate_candidate(c: &Candidate) -> Result<()> {
    let parsed = Version::parse(&c.version)?;
    if c.tag != format!("v{}", c.version)
        || Version::parse(&c.tag)? != parsed
        || c.page != page(&c.tag)
        || c.token.len() != 32
        || !c.token.bytes().all(|b| b.is_ascii_hexdigit())
        || (c.automatic_assets_available && c.size == 0)
        || c.size > MAX_BINARY
    {
        return Err("Saved release choice is invalid".into());
    }
    Ok(())
}
fn select_release(bytes: &[u8], current: &str) -> Result<Option<Candidate>> {
    let releases: Vec<Value> =
        serde_json::from_slice(bytes).map_err(|_| "Invalid releases response")?;
    if releases.len() > 1000 {
        return Err("Too many release entries".into());
    }
    let current = Version::parse(current)?;
    let mut best: Option<(Version, Candidate)> = None;
    for r in releases {
        if r["draft"].as_bool() != Some(false) {
            continue;
        }
        let Some(tag) = r["tag_name"].as_str() else {
            continue;
        };
        let Ok(v) = Version::parse(tag) else {
            continue;
        };
        if v <= current
            || !tag.starts_with('v')
            || r["html_url"].as_str() != Some(page(tag).as_str())
        {
            continue;
        }
        // A newer recognized release still deserves a notice and manual page
        // when its automatic-update assets are absent or unusable.
        let assets = r["assets"].as_array().cloned().unwrap_or_default();
        let mut size = None;
        let mut valid = true;
        for name in [
            PORTABLE.to_owned(),
            "SHA256SUMS".into(),
            format!("{PORTABLE}.sig"),
        ] {
            let found: Vec<_> = assets
                .iter()
                .filter(|a| a["name"].as_str() == Some(name.as_str()))
                .collect();
            if found.len() != 1
                || found[0]["browser_download_url"].as_str() != Some(asset(tag, &name).as_str())
                || found[0]["state"].as_str() != Some("uploaded")
            {
                valid = false;
                break;
            }
            let length = found[0]["size"].as_u64().unwrap_or(0);
            if name == PORTABLE {
                size = Some(length);
            }
            if length == 0
                || (name == "SHA256SUMS" && length > 128 * 1024)
                || (name.ends_with(".sig") && length != 64)
            {
                valid = false;
                break;
            }
        }
        let automatic_assets_available =
            valid && size.unwrap_or(0) > 0 && size.unwrap_or(u64::MAX) <= MAX_BINARY;
        let c = Candidate {
            version: tag[1..].into(),
            tag: tag.into(),
            page: page(tag),
            token: fsutil::unique(),
            size: if automatic_assets_available {
                size.unwrap()
            } else {
                0
            },
            automatic_assets_available,
        };
        validate_candidate(&c)?;
        if best.as_ref().map(|(old, _)| v > *old).unwrap_or(true) {
            best = Some((v, c));
        }
    }
    Ok(best.map(|(_, c)| c))
}
fn checksum(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Checksum file is not UTF-8")?;
    let mut result = None;
    for line in text.lines() {
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() != 2 {
            return Err("Malformed checksum file".into());
        }
        let name = columns[1].strip_prefix('*').unwrap_or(columns[1]);
        if name == PORTABLE {
            if result.is_some()
                || columns[0].len() != 64
                || !columns[0].bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err("Ambiguous portable checksum".into());
            }
            result = Some(columns[0].to_ascii_lowercase());
        }
    }
    result.ok_or_else(|| "Portable checksum is missing".into())
}
fn verify_payload(key: [u8; 32], bytes: &[u8], expected: &str, signature: &[u8]) -> Result<()> {
    cancelled()?;
    if expected.len() != 64
        || !expected.bytes().all(|b| b.is_ascii_hexdigit())
        || fsutil::hash(bytes) != expected.to_ascii_lowercase()
    {
        return Err("Portable checksum does not match".into());
    }
    let key = VerifyingKey::from_bytes(&key).map_err(|_| "Invalid release verification key")?;
    if key.is_weak() {
        return Err("Invalid release verification key".into());
    }
    let signature =
        Signature::from_slice(signature).map_err(|_| "Invalid release signature length")?;
    key.verify_strict(bytes, &signature)
        .map_err(|_| "Portable signature does not verify".into())
}
fn metadata_same(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.uid() == b.uid()
        && a.mode() == b.mode()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
fn opened_regular(path: &Path, limit: u64) -> Result<(Vec<u8>, fs::Metadata)> {
    let named = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !named.is_file() || named.file_type().is_symlink() || named.len() > limit {
        return Err("Update file is not an ordinary bounded file".into());
    }
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    let m = f.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > limit || !metadata_same(&named, &m) {
        return Err("Update file is not an ordinary bounded file".into());
    }
    let mut bytes = Vec::new();
    (&mut f)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let end = f.metadata().map_err(|e| e.to_string())?;
    let named_end = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if bytes.len() as u64 != m.len() || !metadata_same(&m, &end) || !metadata_same(&end, &named_end)
    {
        return Err("Update file changed while reading".into());
    }
    Ok((bytes, end))
}
fn read_regular(path: &Path, limit: u64) -> Result<Vec<u8>> {
    opened_regular(path, limit).map(|x| x.0)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct FilePin {
    device: u64,
    inode: u64,
    size: u64,
    mode: u32,
    uid: u32,
    sha256: String,
}
fn pin(path: &Path) -> Result<FilePin> {
    let (bytes, m) = opened_regular(path, MAX_BINARY)?;
    if m.uid() != unsafe { libc::geteuid() } || m.nlink() != 1 || m.mode() & 0o6022 != 0 {
        return Err("Update requires an owned, unshared ordinary portable file".into());
    }
    Ok(FilePin {
        device: m.dev(),
        inode: m.ino(),
        size: m.len(),
        mode: m.mode(),
        uid: m.uid(),
        sha256: fsutil::hash(&bytes),
    })
}
fn private_parent(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err("Update handoff location is invalid".into());
    }
    let parent = path.parent().ok_or("Missing request directory")?;
    if parent.canonicalize().map_err(|e| e.to_string())? != parent {
        return Err("Update handoff location contains links".into());
    }
    let m = fs::symlink_metadata(parent).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err("Update handoff must be private".into());
    }
    Ok(())
}
fn env_pin() -> Result<(PathBuf, FilePin, PathBuf, String)> {
    let target = PathBuf::from(
        std::env::var_os("WOW_PORTABLE_PATH").ok_or("Open the portable normally to update it")?,
    );
    fsutil::safe_path(&target)?;
    if !target.is_absolute() || target.canonicalize().map_err(|e| e.to_string())? != target {
        return Err("Portable path must be canonical and contain no links".into());
    }
    let current = pin(&target)?;
    for (name, expected) in [
        ("WOW_PORTABLE_DEV", current.device.to_string()),
        ("WOW_PORTABLE_INO", current.inode.to_string()),
        ("WOW_PORTABLE_SIZE", current.size.to_string()),
        ("WOW_PORTABLE_MODE", current.mode.to_string()),
        ("WOW_PORTABLE_UID", current.uid.to_string()),
        ("WOW_PORTABLE_SHA256", current.sha256.clone()),
    ] {
        if std::env::var(name).ok().as_deref() != Some(expected.as_str()) {
            return Err("Original portable identity no longer matches".into());
        }
    }
    let request = PathBuf::from(
        std::env::var_os("WOW_UPDATE_REQUEST").ok_or("Portable update handoff is unavailable")?,
    );
    private_parent(&request)?;
    if request.file_name().and_then(|x| x.to_str()) != Some("update-request.json")
        || request.try_exists().map_err(|e| e.to_string())?
    {
        return Err("Update handoff location is invalid".into());
    }
    let nonce = std::env::var("WOW_UPDATE_NONCE").map_err(|_| "Missing wrapper nonce")?;
    if nonce.len() != 32 || !nonce.bytes().all(|x| x.is_ascii_hexdigit()) {
        return Err("Invalid wrapper nonce".into());
    }
    Ok((target, current, request, nonce))
}
fn write_new_pinned(path: &Path, bytes: &[u8], mode: u32) -> Result<fs::Metadata> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| e.to_string())?;
    let own = f.metadata().map_err(|e| e.to_string())?;
    let result = (|| {
        f.set_permissions(fs::Permissions::from_mode(mode))
            .and_then(|_| f.write_all(bytes))
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        let end = f.metadata().map_err(|e| e.to_string())?;
        let named = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !metadata_same(&end, &named) {
            return Err("New update file changed during write".into());
        }
        Ok(end)
    })();
    if result.is_err()
        && fs::symlink_metadata(path)
            .map(|m| m.dev() == own.dev() && m.ino() == own.ino())
            .unwrap_or(false)
    {
        let _ = fs::remove_file(path);
    }
    result
}
fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    write_new_pinned(path, bytes, mode).map(|_| ())
}
fn cleanup_owned(path: &Path, owner: Option<&fs::Metadata>, expected_hash: Option<&str>) -> bool {
    let Some(owner) = owner else {
        return false;
    };
    let Ok(m) = fs::symlink_metadata(path) else {
        return false;
    };
    if !m.is_file() || m.dev() != owner.dev() || m.ino() != owner.ino() {
        return false;
    }
    if let Some(expected) = expected_hash {
        if pin(path).map(|x| x.sha256 != expected).unwrap_or(true) {
            return false;
        }
    }
    fs::remove_file(path).is_ok()
}
fn stage(data: &Path, candidate: &Candidate, key: [u8; 32]) -> Result<()> {
    cancelled()?;
    validate_candidate(candidate)?;
    if !candidate.automatic_assets_available {
        return Err(
            "Required automatic-update files are missing or unusable; use the release page".into(),
        );
    }
    if Version::parse(&candidate.version)? <= Version::parse(env!("CARGO_PKG_VERSION"))? {
        return Err("Updates cannot downgrade this version".into());
    }
    let (target, current, request, nonce) = env_pin()?;
    let sums = transport::get(data, &asset(&candidate.tag, "SHA256SUMS"), 128 * 1024)?;
    let signature = transport::get(data, &asset(&candidate.tag, &format!("{PORTABLE}.sig")), 64)?;
    let bytes = transport::get(data, &asset(&candidate.tag, PORTABLE), MAX_BINARY)?;
    if bytes.len() as u64 != candidate.size {
        return Err("Portable size differs from the selected release".into());
    }
    verify_payload(key, &bytes, &checksum(&sums)?, &signature)?;
    cancelled()?;
    idle(data)?;
    if pin(&target)? != current {
        return Err("Original portable changed during download".into());
    }
    let parent = target.parent().ok_or("Missing portable directory")?;
    let pm = fs::symlink_metadata(parent).map_err(|e| e.to_string())?;
    if !pm.is_dir() || pm.uid() != unsafe { libc::geteuid() } || pm.mode() & 0o022 != 0 {
        return Err(
            "The portable's folder is not privately writable. Download the update by hand".into(),
        );
    }
    let staged = parent.join(format!(".wow-update-{nonce}.new"));
    let mut request_pin = None;
    let mut staged_owned = None;
    let result = (|| {
        staged_owned = Some(write_new_pinned(&staged, &bytes, current.mode & 0o777)?);
        let info = transport::build_info(&staged)?;
        if info["version"].as_str() != Some(candidate.version.as_str())
            || info["developer_tools"] != false
            || info["build_mode"].as_str() != Some("public")
        {
            return Err("Signed portable does not match this public release version".into());
        }
        let source = info["source_sha256"]
            .as_str()
            .ok_or("Signed portable build identity is missing")?;
        if source.len() != 64 || !source.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Signed portable build identity is invalid".into());
        }
        cancelled()?;
        idle(data)?;
        if pin(&target)? != current {
            return Err("Original portable changed before handoff".into());
        }
        let staged_pin = pin(&staged)?;
        if staged_pin.sha256 != fsutil::hash(&bytes) {
            return Err("Staged portable changed".into());
        }
        let message = json!({"schema":1,"repository":REPOSITORY,"nonce":nonce,"target":target,"current":current,"old_version":env!("CARGO_PKG_VERSION"),"old_source_sha256":env!("WOW_SOURCE_SHA256"),"staged":{"path":staged,"pin":staged_pin},"version":candidate.version,"source_sha256":source,"release_url":candidate.page,"signature_hex":hex(&signature)});
        request_pin = Some(write_new_pinned(
            &request,
            &serde_json::to_vec_pretty(&message).map_err(|e| e.to_string())?,
            0o600,
        )?);
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        File::open(request.parent().unwrap())
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        mark_prepared()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = cleanup_owned(&staged, staged_owned.as_ref(), Some(&fsutil::hash(&bytes)));
        let _ = cleanup_owned(&request, request_pin.as_ref(), None);
    }
    result
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 15) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    fn temp() -> PathBuf {
        let p = std::env::temp_dir().join(format!("wow-updater-{}", fsutil::unique()));
        fsutil::private_dir(&p).unwrap();
        p
    }
    #[test]
    fn numeric_project_generations_and_candidates_are_ordered() {
        let p = |x| Version::parse(x).unwrap();
        assert!(p("0.2.0-alpha.10-candidate.49") > p("0.2.0-alpha.9-candidate.99"));
        assert!(p("0.2.0-alpha.100-candidate.1") > p("0.2.0-alpha.99-candidate.99"));
        assert!(p("v0.2.0-alpha.10-candidate.100") > p("0.2.0-alpha.10-candidate.99"));
        assert!(p("0.2.0") > p("0.2.0-rc.9"));
        assert!(p("0.10.0") > p("0.9.99"));
        for x in [
            "1.2",
            "1.2.03",
            "1.2.3-",
            "1.2.3-01",
            "1.2.3+x",
            "1.2.3-a/b",
            "1.2.3-999999999999999999999999",
        ] {
            assert!(Version::parse(x).is_err(), "{x}");
        }
    }
    #[test]
    fn throttle_is_rolling_and_clock_rollback_does_not_recheck() {
        assert!(due(None, 5));
        assert!(!due(Some(1), DAY));
        assert!(due(Some(1), DAY + 1));
        assert!(!due(Some(100), 99));
    }
    fn release(tag: &str, draft: bool) -> Value {
        json!({"draft":draft,"prerelease":true,"tag_name":tag,"html_url":page(tag),"assets":[{"name":PORTABLE,"browser_download_url":asset(tag,PORTABLE),"size":10,"state":"uploaded"},{"name":"SHA256SUMS","browser_download_url":asset(tag,"SHA256SUMS"),"size":100,"state":"uploaded"},{"name":format!("{PORTABLE}.sig"),"browser_download_url":asset(tag,&format!("{PORTABLE}.sig")),"size":64,"state":"uploaded"}]})
    }
    #[test]
    fn drafts_foreign_pages_and_old_releases_are_ignored() {
        let mut foreign = release("v0.2.0-alpha.10-candidate.999", false);
        foreign["html_url"] = json!("https://github.com/other/repo/releases/tag/x");
        let bytes = serde_json::to_vec(&vec![
            release("v0.2.0-alpha.10-candidate.1000", true),
            foreign,
            release("v0.2.0-alpha.9-candidate.9999", false),
            release("v0.2.0-alpha.10-candidate.50", false),
        ])
        .unwrap();
        let candidate = select_release(&bytes, "0.2.0-alpha.10-candidate.49")
            .unwrap()
            .unwrap();
        assert_eq!(candidate.version, "0.2.0-alpha.10-candidate.50");
        assert!(candidate.automatic_assets_available);
    }
    #[test]
    fn newest_release_without_usable_assets_still_has_manual_notice() {
        let good = release("v0.2.0-alpha.10-candidate.50", false);
        for defect in ["signature", "checksum", "host", "duplicate", "all"] {
            let mut newer = release("v0.2.0-alpha.10-candidate.51", false);
            match defect {
                "signature" => {
                    newer["assets"].as_array_mut().unwrap().remove(2);
                }
                "checksum" => {
                    newer["assets"].as_array_mut().unwrap().remove(1);
                }
                "host" => {
                    newer["assets"][0]["browser_download_url"] =
                        json!("https://github.com/other/repo/releases/download/x/file")
                }
                "duplicate" => {
                    let entry = newer["assets"][0].clone();
                    newer["assets"].as_array_mut().unwrap().push(entry);
                }
                _ => newer["assets"] = Value::Null,
            }
            let bytes = serde_json::to_vec(&vec![good.clone(), newer]).unwrap();
            let candidate = select_release(&bytes, "0.2.0-alpha.10-candidate.49")
                .unwrap()
                .unwrap();
            assert_eq!(candidate.version, "0.2.0-alpha.10-candidate.51");
            assert_eq!(candidate.page, page("v0.2.0-alpha.10-candidate.51"));
            assert!(!candidate.automatic_assets_available);
            assert_eq!(candidate.size, 0);
            validate_candidate(&candidate).unwrap();
            let p = temp();
            let error = stage(&p, &candidate, [0; 32]).unwrap_err();
            assert!(error.contains("Required automatic-update files"));
            assert_eq!(fs::read_dir(&p).unwrap().count(), 0);
            save(
                &p,
                &Settings {
                    candidate: Some(candidate.clone()),
                    ..Settings::default()
                },
            )
            .unwrap();
            let refused = dispatch(
                &p,
                &json!({"action":"updater_install","confirmed":true,"token":candidate.token}),
            )
            .unwrap();
            assert_eq!(refused["manual_required"], true);
            assert!(refused["message"]
                .as_str()
                .unwrap()
                .contains("missing usable"));
            assert!(!p.join("update-request.json").exists());
            fs::remove_dir_all(p).unwrap();
        }
    }
    #[test]
    fn older_cached_candidate_defaults_to_manual_only() {
        let c: Candidate = serde_json::from_value(json!({
            "version":"0.2.0-alpha.10-candidate.50",
            "tag":"v0.2.0-alpha.10-candidate.50",
            "page":page("v0.2.0-alpha.10-candidate.50"),
            "token":"a".repeat(32),"size":100
        }))
        .unwrap();
        assert!(!c.automatic_assets_available);
    }
    #[test]
    fn checksum_and_signature_cover_the_exact_bytes() {
        reset_cancel();
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let bytes = b"synthetic offline fixture, not an executable";
        let sig = key.sign(bytes).to_bytes();
        assert!(verify_payload(
            key.verifying_key().to_bytes(),
            bytes,
            &fsutil::hash(bytes),
            &sig
        )
        .is_ok());
        assert!(verify_payload(
            key.verifying_key().to_bytes(),
            b"changed",
            &fsutil::hash(bytes),
            &sig
        )
        .is_err());
        assert!(verify_payload(
            key.verifying_key().to_bytes(),
            b"changed",
            &fsutil::hash(b"changed"),
            &sig
        )
        .is_err());
        assert!(verify_payload(
            key.verifying_key().to_bytes(),
            bytes,
            &fsutil::hash(bytes),
            &sig[..63]
        )
        .is_err());
        assert!(verify_payload(
            SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes(),
            bytes,
            &fsutil::hash(bytes),
            &sig
        )
        .is_err());
    }
    #[test]
    fn duplicate_or_missing_checksum_is_not_a_match() {
        let line = format!("{}  {PORTABLE}\n", fsutil::hash(b"a"));
        assert!(checksum(line.as_bytes()).is_ok());
        assert!(checksum(format!("{line}{line}").as_bytes()).is_err());
        assert!(checksum(b"garbage").is_err());
    }
    #[test]
    fn opt_out_and_busy_do_not_start_the_feed_request() {
        reset_cancel();
        let p = temp();
        dispatch(&p, &json!({"action":"updater_enabled","enabled":false})).unwrap();
        assert_eq!(
            dispatch(&p, &json!({"action":"updater_check"})).unwrap()["enabled"],
            false
        );
        assert!(load(&p).unwrap().last_attempt.is_none());
        dispatch(&p, &json!({"action":"updater_enabled","enabled":true})).unwrap();
        fs::write(p.join("transaction.json"), b"{}").unwrap();
        dispatch(&p, &json!({"action":"updater_check"})).unwrap();
        assert!(load(&p).unwrap().last_attempt.is_none());
        fs::remove_file(p.join("transaction.json")).unwrap();
        let lock = fsutil::Lock::acquire(&p.join("operation.lock")).unwrap();
        dispatch(&p, &json!({"action":"updater_check"})).unwrap();
        assert!(load(&p).unwrap().last_attempt.is_none());
        drop(lock);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn unset_key_refuses_install_without_network_or_portable_env() {
        let p = temp();
        let bytes =
            serde_json::to_vec(&vec![release("v0.2.0-alpha.10-candidate.50", false)]).unwrap();
        let candidate = select_release(&bytes, env!("CARGO_PKG_VERSION"))
            .unwrap()
            .unwrap();
        save(
            &p,
            &Settings {
                candidate: Some(candidate.clone()),
                ..Settings::default()
            },
        )
        .unwrap();
        let result = dispatch(
            &p,
            &json!({"action":"updater_install","confirmed":true,"token":candidate.token}),
        )
        .unwrap();
        if PRODUCTION_KEY.is_none() {
            assert!(result["message"]
                .as_str()
                .unwrap()
                .contains("not configured"));
        }
        assert_eq!(result["manual_required"], true);
        assert_eq!(result["restart_required"], false);
        assert!(!p.join("update-request.json").exists());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn partial_and_symlink_do_not_replace_existing_bytes() {
        reset_cancel();
        use std::os::unix::fs::symlink;
        let p = temp();
        let old = p.join("portable");
        fs::write(&old, b"old usable bytes").unwrap();
        let partial = p.join(".download");
        fs::write(&partial, b"partial").unwrap();
        assert!(verify_payload(
            [1; 32],
            &fs::read(&partial).unwrap(),
            &fsutil::hash(b"complete"),
            &[0; 64]
        )
        .is_err());
        assert_eq!(fs::read(&old).unwrap(), b"old usable bytes");
        let link = p.join("link");
        symlink(&old, &link).unwrap();
        assert!(pin(&link).is_err());
        assert!(read_regular(&link, 1024).is_err());
        assert!(write_new(&old, b"replace", 0o700).is_err());
        assert_eq!(fs::read(&old).unwrap(), b"old usable bytes");
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn incomplete_or_unreadable_workflow_blocks_update() {
        let p = temp();
        fs::write(
            p.join("state.json"),
            br#"{"installations":[{"stage":"games"}]}"#,
        )
        .unwrap();
        assert!(idle(&p).is_err());
        fs::write(
            p.join("state.json"),
            br#"{"installations":[{"stage":"complete"}],"addon_pending":false}"#,
        )
        .unwrap();
        assert!(idle(&p).is_ok());
        fs::write(p.join("state.json"), b"bad").unwrap();
        assert!(idle(&p).is_err());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn identical_preexisting_stage_is_not_owned_by_failed_write() {
        let p = temp();
        let stage = p.join(".wow-update-stage.new");
        fs::write(&stage, b"same payload").unwrap();
        assert!(write_new_pinned(&stage, b"same payload", 0o700).is_err());
        assert!(!cleanup_owned(
            &stage,
            None,
            Some(&fsutil::hash(b"same payload"))
        ));
        assert_eq!(fs::read(&stage).unwrap(), b"same payload");
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn owned_stage_cleanup_preserves_independent_edits() {
        let p = temp();
        let stage = p.join(".wow-update-stage.new");
        let owner = write_new_pinned(&stage, b"new payload", 0o700).unwrap();
        fs::write(&stage, b"user edited").unwrap();
        assert!(!cleanup_owned(
            &stage,
            Some(&owner),
            Some(&fsutil::hash(b"new payload"))
        ));
        assert_eq!(fs::read(&stage).unwrap(), b"user edited");
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn cancellation_is_separate_and_refuses_before_payload_use() {
        cancel();
        assert!(cancelled().is_err());
        reset_cancel();
        assert!(cancelled().is_ok());
        let state = AtomicU8::new(0);
        assert!(cancel_state(&state));
        assert!(prepare_state(&state).is_err());
        reset_state(&state);
        prepare_state(&state).unwrap();
        assert!(!cancel_state(&state));
        reset_state(&state);
        assert_eq!(state.load(AtomicOrdering::SeqCst), 2);
    }
    #[test]
    fn cached_current_and_older_updates_are_hidden_without_rechecking() {
        let p = temp();
        for version in [env!("CARGO_PKG_VERSION"), "0.2.0-alpha.9-candidate.999"] {
            let state = Settings {
                schema: 1,
                enabled: true,
                last_attempt: Some(1234),
                candidate: Some(Candidate {
                    version: version.into(),
                    tag: format!("v{version}"),
                    page: page(&format!("v{version}")),
                    token: "a".repeat(32),
                    size: 100,
                    automatic_assets_available: true,
                }),
            };
            save(&p, &state).unwrap();
            let loaded = load(&p).unwrap();
            assert!(loaded.candidate.is_none());
            assert_eq!(loaded.last_attempt, Some(1234));
        }
        fs::remove_dir_all(p).unwrap();
    }
}
