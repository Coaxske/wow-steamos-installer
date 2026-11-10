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
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
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
// Public verifier only. The signing key is kept outside the project. No test key fallback.
const PRODUCTION_KEY: Option<[u8; 32]> = Some([
    0xe6, 0x00, 0x7a, 0x9a, 0xd0, 0xbf, 0xa1, 0xf9, 0xcd, 0xf1, 0xa8, 0x7f, 0xb9, 0x00, 0x15, 0x6a,
    0xa6, 0x97, 0x22, 0x4b, 0x80, 0x0c, 0x55, 0xfe, 0xf9, 0x63, 0x0e, 0x34, 0xd7, 0x7e, 0x06, 0xf8,
]);
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
const UNFINISHED_SETUP: &str = "Finish or remove your unfinished setup before updating. You can still download the new version from the release page.";
fn status(data: &Path, s: &Settings) -> Value {
    let readiness = fsutil::Lock::acquire(&data.join("operation.lock"))
        .and_then(|_guard| workflow_readiness(data));
    let (ready, message) = match readiness {
        Ok(true) => (true, String::new()),
        Ok(false) => (false, UNFINISHED_SETUP.to_string()),
        Err(e) => (false, e),
    };
    json!({"enabled":s.enabled,"candidate":if s.enabled{s.candidate.as_ref()}else{None},"signing_configured":PRODUCTION_KEY.is_some(),"release_page":RELEASE_PAGE,"install_ready":ready,"install_blocked_message":message})
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
fn canonical_data(data: &Path) -> Result<PathBuf> {
    prepare_data(data)?;
    let before = ArtifactIdentity::read(data)?;
    let resolved = data.canonicalize().map_err(|e| e.to_string())?;
    if !before.directory
        || ArtifactIdentity::read(&resolved)? != before
        || ArtifactIdentity::read(data)? != before
    {
        return Err("Update settings directory changed during resolution".into());
    }
    Ok(resolved)
}

fn workflow_readiness(data: &Path) -> Result<bool> {
    let mut ready = true;
    for name in [
        "transaction.json",
        "cleanup.json",
        "restore.json",
        "controller-job.json",
        "steam-restart.json",
        "update-request.json",
    ] {
        if data.join(name).try_exists().map_err(|e| e.to_string())? {
            ready = false;
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
            return Ok(false);
        }
    }
    Ok(ready)
}
fn idle(data: &Path) -> Result<()> {
    if !workflow_readiness(data)? {
        return Err(UNFINISHED_SETUP.into());
    }
    Ok(())
}
fn check_idle_lock(data: &Path) -> Result<fsutil::Lock> {
    let lock = fsutil::Lock::acquire(&data.join("operation.lock"))?;
    let _ = workflow_readiness(data)?;
    Ok(lock)
}
fn idle_lock(data: &Path) -> Result<fsutil::Lock> {
    let lock = fsutil::Lock::acquire(&data.join("operation.lock"))?;
    idle(data)?;
    Ok(lock)
}
pub fn dispatch(data: &Path, request: &Value) -> Result<Value> {
    let action = request["action"].as_str().ok_or("Missing update action")?;
    let resolved_data = canonical_data(data)?;
    let data = resolved_data.as_path();
    if [
        "updater_bind",
        "updater_commit",
        "updater_rollback",
        "updater_ready",
        "updater_finalize",
        "updater_recovery_info",
    ]
    .contains(&action)
    {
        return wrapper::dispatch(data, request);
    }
    if action == "updater_status" {
        return load(data).map(|s| status(data, &s));
    }
    let _update_lock = match fsutil::Lock::acquire(&data.join("updates.lock")) {
        Ok(lock) => lock,
        Err(_) if action == "updater_check" => return Ok(status(data, &load(data)?)),
        Err(e) => return Err(e),
    };
    if action == "updater_check" {
        if let Ok(_operation) = fsutil::Lock::acquire(&data.join("operation.lock")) {
            let _ = fsutil::json(&data.join("update-cleanup.json"), &cleanup_abandoned(data));
        }
    }
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
            Ok(status(data, &s))
        }
        "updater_check" => {
            if !s.enabled || !due(s.last_attempt, now()?) {
                return Ok(status(data, &s));
            }
            let guard = match check_idle_lock(data) {
                Ok(x) => x,
                Err(_) => return Ok(status(data, &s)),
            };
            s.last_attempt = Some(now()?);
            save(data, &s)?;
            drop(guard);
            let found = transport::releases(data)
                .and_then(|bytes| select_release(&bytes, env!("CARGO_PKG_VERSION")));
            let _guard = match check_idle_lock(data) {
                Ok(x) => x,
                Err(_) => return Ok(status(data, &s)),
            };
            if let Ok(candidate) = found {
                s.candidate = candidate;
                save(data, &s)?;
            }
            drop(_guard);
            Ok(status(data, &s))
        }
        "updater_install" => install(data, &s, request, PRODUCTION_KEY),
        _ => Err("Unknown update action".into()),
    }
}
fn install(data: &Path, s: &Settings, request: &Value, key: Option<[u8; 32]>) -> Result<Value> {
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
    let _guard = match idle_lock(data) {
        Ok(x) => x,
        Err(e) => return failure(e),
    };
    let candidate = match s.candidate.as_ref() {
        Some(c) if request["token"].as_str() == Some(c.token.as_str()) => c,
        _ => {
            return failure("The update choice expired. Check again or use the release page".into())
        }
    };
    if !candidate.automatic_assets_available {
        return failure("This release is missing usable installer, checksum or signature files. Use the release page".into());
    }
    let Some(key) = key else {
        return failure(
            "Signed automatic updates are not configured in this build. Use the release page"
                .into(),
        );
    };
    match stage(data,candidate,key){Ok(_)=>Ok(json!({"restart_required":true,"version":candidate.version})),Err(e)=>failure(format!("Automatic update could not be prepared: {e}. Your current portable has not been replaced"))}
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
    write_new_observed(path, bytes, mode, |_| Ok(()))
}
fn write_new_observed(
    path: &Path,
    bytes: &[u8],
    mode: u32,
    created: impl FnOnce(&fs::Metadata) -> Result<()>,
) -> Result<fs::Metadata> {
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
            .map_err(|e| e.to_string())?;
        created(&f.metadata().map_err(|e| e.to_string())?)?;
        f.write_all(bytes)
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

// Durable ownership for updater-created temporary files. Legacy/unrecorded
// files are intentionally outside this cleanup policy.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ArtifactIdentity {
    device: u64,
    inode: u64,
    uid: u32,
    mode: u32,
    directory: bool,
}
impl ArtifactIdentity {
    fn read(path: &Path) -> Result<Self> {
        let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        Self::metadata(&m)
    }
    fn metadata(m: &fs::Metadata) -> Result<Self> {
        if m.file_type().is_symlink()
            || (!m.is_dir() && !m.is_file())
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o6022 != 0
            || (!m.is_dir() && m.nlink() != 1)
        {
            return Err("Update artifact identity is not exclusively owned".into());
        }
        Ok(Self {
            device: m.dev(),
            inode: m.ino(),
            uid: m.uid(),
            mode: m.mode(),
            directory: m.is_dir(),
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactProcess {
    pid: u32,
    start: String,
}
fn process_pin(pid: u32) -> Result<ArtifactProcess> {
    let p = PathBuf::from(format!("/proc/{pid}"));
    if fs::metadata(&p).map_err(|e| e.to_string())?.uid() != unsafe { libc::geteuid() } {
        return Err("Update process owner changed".into());
    }
    let s = fs::read_to_string(p.join("stat")).map_err(|e| e.to_string())?;
    let start = s
        .rsplit_once(')')
        .and_then(|(_, x)| x.split_whitespace().nth(19))
        .ok_or("Update process identity unavailable")?;
    Ok(ArtifactProcess {
        pid,
        start: start.into(),
    })
}
fn artifact_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
fn process_may_live(p: &ArtifactProcess) -> bool {
    match process_pin(p.pid) {
        Ok(now) => now.start == p.start,
        Err(_) => artifact_exists(&PathBuf::from(format!("/proc/{}", p.pid))).unwrap_or(true),
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactMember {
    relative: PathBuf,
    identity: ArtifactIdentity,
    sealed: Option<FilePin>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactRecord {
    schema: u32,
    nonce: String,
    kind: String,
    root: PathBuf,
    parent: ArtifactIdentity,
    owners: Vec<ArtifactProcess>,
    handoff: Option<PathBuf>,
    members: Vec<ArtifactMember>,
    lease: ArtifactIdentity,
}
struct Artifact {
    data: PathBuf,
    record: ArtifactRecord,
    lease: File,
}
fn artifact_store(data: &Path) -> PathBuf {
    data.join("update-artifacts")
}
fn artifact_record_path(data: &Path, nonce: &str) -> PathBuf {
    artifact_store(data).join(format!("{nonce}.json"))
}
fn artifact_lease_path(data: &Path, nonce: &str) -> PathBuf {
    artifact_store(data).join(format!("{nonce}.lock"))
}
fn artifact_hex(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|x| x.is_ascii_hexdigit())
}
#[derive(Default)]
struct ArtifactRegistration {
    files: Vec<(PathBuf, File, ArtifactIdentity, Option<String>)>,
    committed: bool,
}
impl ArtifactRegistration {
    fn track(&mut self, path: &Path, file: &File, hash: Option<String>) -> Result<()> {
        let opened = file.metadata().map_err(|e| e.to_string())?;
        let identity = ArtifactIdentity::metadata(&opened)?;
        if ArtifactIdentity::read(path)? != identity {
            return Err("Registration file identity changed".into());
        }
        let held = match file.try_clone() {
            Ok(held) => held,
            Err(error) => {
                let _ = cleanup_owned(path, Some(&opened), hash.as_deref());
                return Err(error.to_string());
            }
        };
        self.files.push((path.into(), held, identity, hash));
        Ok(())
    }
    fn seal(&mut self, path: &Path, hash: String) {
        if let Some(entry) = self.files.iter_mut().find(|entry| entry.0 == path) {
            entry.3 = Some(hash);
        }
    }
}
impl Drop for ArtifactRegistration {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for (path, file, identity, hash) in self.files.iter().rev() {
            let Ok(opened) = file.metadata() else {
                continue;
            };
            if ArtifactIdentity::metadata(&opened).ok().as_ref() != Some(identity)
                || ArtifactIdentity::read(path).ok().as_ref() != Some(identity)
            {
                continue;
            }
            let _ = cleanup_owned(path, Some(&opened), hash.as_deref());
        }
    }
}

impl Artifact {
    fn new(
        data: &Path,
        root: &Path,
        kind: &str,
        nonce: &str,
        handoff: Option<&Path>,
    ) -> Result<Self> {
        Self::new_with_sync(data, root, kind, nonce, handoff, |store| {
            File::open(store)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())
        })
    }
    fn new_with_sync(
        data: &Path,
        root: &Path,
        kind: &str,
        nonce: &str,
        handoff: Option<&Path>,
        sync: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<Self> {
        if !artifact_hex(nonce) {
            return Err("Invalid update artifact nonce".into());
        }
        let store = artifact_store(data);
        prepare_data(&store)?;
        let lp = artifact_lease_path(data, nonce);
        let mut registration = ArtifactRegistration::default();
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&lp)
            .map_err(|e| e.to_string())?;
        registration.track(&lp, &lease, Some(fsutil::hash(b"")))?;
        if unsafe { libc::flock(lease.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("Update artifact is active".into());
        }
        let mut owners = vec![process_pin(std::process::id())?];
        if let Some(request) = std::env::var_os("WOW_UPDATE_REQUEST").map(PathBuf::from) {
            private_parent(&request)?;
            let p = request.parent().unwrap().join("processes");
            if p.exists() {
                for value in String::from_utf8(read_regular(&p, 1024)?)
                    .map_err(|_| "Invalid wrapper processes")?
                    .split_whitespace()
                {
                    let pid = value
                        .parse::<u32>()
                        .map_err(|_| "Invalid wrapper process")?;
                    if let Ok(owner) = process_pin(pid) {
                        if !owners.iter().any(|x| x.pid == pid) {
                            owners.push(owner);
                        }
                    }
                }
            }
        }
        let parent = ArtifactIdentity::read(root.parent().ok_or("Missing artifact parent")?)?;
        let record = ArtifactRecord {
            schema: 1,
            nonce: nonce.into(),
            kind: kind.into(),
            root: root.into(),
            parent,
            owners,
            handoff: handoff.map(Path::to_path_buf),
            members: vec![ArtifactMember {
                relative: PathBuf::new(),
                identity: ArtifactIdentity::read(root)?,
                sealed: None,
            }],
            lease: ArtifactIdentity::read(&lp)?,
        };
        let value = Self {
            data: data.into(),
            record,
            lease,
        };
        validate_artifact(&value.record)?;
        let record_path = artifact_record_path(data, nonce);
        let bytes = serde_json::to_vec_pretty(&value.record).map_err(|e| e.to_string())?;
        let mut record_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&record_path)
            .map_err(|e| e.to_string())?;
        registration.track(&record_path, &record_file, None)?;
        let record_identity =
            ArtifactIdentity::metadata(&record_file.metadata().map_err(|e| e.to_string())?)?;
        record_file
            .write_all(&bytes)
            .and_then(|_| record_file.sync_all())
            .map_err(|e| e.to_string())?;
        registration.seal(&record_path, fsutil::hash(&bytes));
        let expected_record = FilePin {
            device: record_identity.device,
            inode: record_identity.inode,
            size: bytes.len() as u64,
            mode: record_identity.mode,
            uid: record_identity.uid,
            sha256: fsutil::hash(&bytes),
        };
        let verify_registration = || -> Result<()> {
            let opened = record_file.metadata().map_err(|e| e.to_string())?;
            if !metadata_same(
                &opened,
                &fs::symlink_metadata(&record_path).map_err(|e| e.to_string())?,
            ) || pin(&record_path)? != expected_record
                || ArtifactIdentity::metadata(&value.lease.metadata().map_err(|e| e.to_string())?)?
                    != value.record.lease
                || ArtifactIdentity::read(&lp)? != value.record.lease
            {
                return Err("Update registration changed before commit".into());
            }
            Ok(())
        };
        verify_registration()?;
        sync(&store)?;
        verify_registration()?;
        registration.committed = true;
        Ok(value)
    }
    fn save(&self) -> Result<()> {
        fsutil::json(
            &artifact_record_path(&self.data, &self.record.nonce),
            &self.record,
        )
    }
    fn add(&mut self, path: &Path) -> Result<()> {
        let relative = path
            .strip_prefix(&self.record.root)
            .map_err(|_| "Artifact left its owned root")?
            .to_path_buf();
        if self.record.members.iter().any(|m| m.relative == relative) {
            return Err("Duplicate update artifact".into());
        }
        self.record.members.push(ArtifactMember {
            relative,
            identity: ArtifactIdentity::read(path)?,
            sealed: None,
        });
        validate_artifact(&self.record)?;
        self.save()
    }
    fn seal(&mut self, path: &Path) -> Result<()> {
        let relative = path
            .strip_prefix(&self.record.root)
            .map_err(|_| "Artifact left its owned root")?;
        let member = self
            .record
            .members
            .iter_mut()
            .find(|m| m.relative == relative)
            .ok_or("Artifact ownership missing")?;
        if ArtifactIdentity::read(path)? != member.identity {
            return Err("Update artifact changed before sealing".into());
        }
        member.sealed = Some(pin(path)?);
        self.save()
    }
    fn lease_fd(&self) -> i32 {
        self.lease.as_raw_fd()
    }
    fn finish(&self) {
        if remove_artifact(&self.record).is_ok() {
            let _ = forget_artifact(&self.data, &self.record);
        }
    }
}
fn validate_artifact(r: &ArtifactRecord) -> Result<()> {
    if r.schema != 1
        || !artifact_hex(&r.nonce)
        || r.members.is_empty()
        || r.members.len() > 64
        || r.owners.is_empty()
        || r.owners.len() > 8
        || !r.root.is_absolute()
    {
        return Err("Unsupported update artifact record".into());
    }
    if r.owners.iter().any(|p| {
        p.pid == 0
            || p.start.is_empty()
            || p.start.len() > 24
            || !p.start.bytes().all(|b| b.is_ascii_digit())
    }) {
        return Err("Invalid artifact process identity".into());
    }
    match (r.kind.as_str(), r.handoff.as_ref()) {
        ("scratch", None) => {}
        ("stage", Some(p))
            if p.is_absolute()
                && p.file_name().and_then(|x| x.to_str()) == Some("update-request.json")
                && p.parent()
                    .and_then(|x| x.file_name())
                    .and_then(|x| x.to_str())
                    == Some(format!("run-{}", r.nonce).as_str()) => {}
        _ => return Err("Invalid artifact handoff reference".into()),
    }
    let expected = match r.kind.as_str() {
        "scratch" => format!(".wow-update-tmp-{}", r.nonce),
        "stage" => format!(".wow-update-{}.new", r.nonce),
        _ => return Err("Invalid update artifact kind".into()),
    };
    if r.root.file_name().and_then(|s| s.to_str()) != Some(expected.as_str())
        || r.root
            .parent()
            .ok_or("Artifact parent missing")?
            .canonicalize()
            .map_err(|e| e.to_string())?
            != r.root.parent().unwrap()
    {
        return Err("Update artifact path changed".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for m in &r.members {
        if !seen.insert(m.relative.clone()) {
            return Err("Duplicate update artifact record".into());
        }
        let parts: Vec<_> = m.relative.components().collect();
        if !parts
            .iter()
            .all(|x| matches!(x, std::path::Component::Normal(_)))
            || parts.len() > 2
        {
            return Err("Invalid artifact member".into());
        }
        if !parts.is_empty() {
            if r.kind != "scratch" {
                return Err("Staged artifact has unexpected members".into());
            }
            let names: Vec<_> = parts
                .iter()
                .map(|x| x.as_os_str().to_string_lossy())
                .collect();
            let leaf = names.last().unwrap();
            let file = ["body", "headers", "stdout", "stderr"].contains(&leaf.as_ref());
            let hop = names.len() == 1 && ["0", "1", "2", "3", "4", "5"].contains(&leaf.as_ref());
            if (!file && !hop)
                || (names.len() == 2
                    && !["0", "1", "2", "3", "4", "5"].contains(&names[0].as_ref()))
                || m.identity.directory != hop
            {
                return Err("Unexpected update artifact member".into());
            }
        }
    }
    if !seen.contains(Path::new("")) || r.members[0].identity.directory != (r.kind == "scratch") {
        return Err("Update artifact root kind changed".into());
    }
    Ok(())
}
fn artifact_member_matches(path: &Path, m: &ArtifactMember) -> Result<()> {
    if ArtifactIdentity::read(path)? != m.identity {
        return Err("Update artifact identity changed; file kept".into());
    }
    if let Some(expected) = &m.sealed {
        if pin(path)? != *expected {
            return Err("Completed update artifact changed; file kept".into());
        }
    }
    Ok(())
}
fn artifact_tree_matches(root: &Path, r: &ArtifactRecord) -> Result<()> {
    let mut stack = vec![(PathBuf::new(), root.to_path_buf())];
    while let Some((relative, path)) = stack.pop() {
        let m = r
            .members
            .iter()
            .find(|x| x.relative == relative)
            .ok_or("Unrecorded update artifact content was kept")?;
        artifact_member_matches(&path, m)?;
        if m.identity.directory {
            for item in fs::read_dir(&path).map_err(|e| e.to_string())? {
                let item = item.map_err(|e| e.to_string())?;
                stack.push((relative.join(item.file_name()), item.path()));
            }
        }
    }
    Ok(())
}
fn artifact_fd_path(dir: &File, name: &std::ffi::OsStr) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd())).join(name)
}
fn artifact_open_dir(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| e.to_string())
}
fn artifact_move(dir: &File, from: &std::ffi::OsStr, to: &std::ffi::OsStr) -> Result<()> {
    let from = std::ffi::CString::new(from.as_bytes()).map_err(|_| "Invalid artifact name")?;
    let to = std::ffi::CString::new(to.as_bytes()).map_err(|_| "Invalid artifact name")?;
    if unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            dir.as_raw_fd(),
            from.as_ptr(),
            dir.as_raw_fd(),
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err("Update artifact quarantine was deferred".into());
    }
    dir.sync_all().map_err(|e| e.to_string())
}
fn artifact_recovery_pending(r: &ArtifactRecord) -> bool {
    if std::env::var_os("WOW_PORTABLE_PATH")
        .map(PathBuf::from)
        .as_ref()
        == Some(&r.root)
    {
        return true;
    }
    if r.kind != "stage" {
        return false;
    }
    let parent = r.root.parent().unwrap();
    for suffix in ["backup", "recovery.json"] {
        if artifact_exists(&parent.join(format!(".wow-update-{}.{suffix}", r.nonce)))
            .unwrap_or(true)
        {
            return true;
        }
    }
    if let Some(request) = &r.handoff {
        let Some(runtime) = request.parent() else {
            return true;
        };
        if request.file_name().and_then(|x| x.to_str()) != Some("update-request.json")
            || runtime.file_name().and_then(|x| x.to_str())
                != Some(format!("run-{}", r.nonce).as_str())
        {
            return true;
        }
        if artifact_exists(runtime).unwrap_or(true) && private_parent(request).is_err() {
            return true;
        }
        for name in [
            "update-request.json",
            "update-pending",
            "update-receipt",
            "update-recovery-info",
        ] {
            if artifact_exists(&runtime.join(name)).unwrap_or(true) {
                return true;
            }
        }
    }
    false
}
fn remove_artifact(r: &ArtifactRecord) -> Result<()> {
    validate_artifact(r)?;
    let parent = r.root.parent().unwrap();
    if ArtifactIdentity::read(parent)? != r.parent {
        return Err("Update artifact parent changed".into());
    }
    let parent_fd = artifact_open_dir(parent)?;
    if ArtifactIdentity::metadata(&parent_fd.metadata().map_err(|e| e.to_string())?)? != r.parent {
        return Err("Update artifact parent changed".into());
    }
    let original = r.root.file_name().unwrap();
    let quarantine = std::ffi::OsString::from(format!(".wow-update-clean-{}", r.nonce));
    let original_path = artifact_fd_path(&parent_fd, original);
    let qpath = artifact_fd_path(&parent_fd, &quarantine);
    let original_exists = artifact_exists(&original_path)?;
    let q_exists = artifact_exists(&qpath)?;
    if !original_exists && !q_exists {
        return Ok(());
    }
    if artifact_recovery_pending(r) {
        return Err("Update recovery still refers to this file".into());
    }
    if original_exists && q_exists {
        return Err("Ambiguous cleanup paths were kept".into());
    }
    if original_exists {
        artifact_tree_matches(&original_path, r)?;
        artifact_move(&parent_fd, original, &quarantine)?;
    }
    artifact_tree_matches(&qpath, r)?;
    let root_member = r
        .members
        .iter()
        .find(|m| m.relative.as_os_str().is_empty())
        .ok_or("Missing root identity")?;
    if !root_member.identity.directory {
        artifact_member_matches(&qpath, root_member)?;
        let name =
            std::ffi::CString::new(quarantine.as_bytes()).map_err(|_| "Invalid quarantine")?;
        if unsafe { libc::unlinkat(parent_fd.as_raw_fd(), name.as_ptr(), 0) } != 0 {
            return Err("Owned staged file cleanup deferred".into());
        }
        return parent_fd.sync_all().map_err(|e| e.to_string());
    }
    let root_fd = artifact_open_dir(&qpath)?;
    let mut members: Vec<_> = r
        .members
        .iter()
        .filter(|m| !m.relative.as_os_str().is_empty())
        .collect();
    members.sort_by_key(|m| std::cmp::Reverse(m.relative.components().count()));
    for m in members {
        let leaf = m.relative.file_name().ok_or("Invalid artifact member")?;
        let parent_relative = m.relative.parent().unwrap();
        let dir = if parent_relative.as_os_str().is_empty() {
            root_fd.try_clone().map_err(|e| e.to_string())?
        } else {
            let ancestor = r
                .members
                .iter()
                .find(|x| x.relative == parent_relative)
                .ok_or("Missing artifact directory identity")?;
            let path = artifact_fd_path(&root_fd, parent_relative.as_os_str());
            if fs::symlink_metadata(&path).is_err() {
                continue;
            }
            artifact_member_matches(&path, ancestor)?;
            artifact_open_dir(&path)?
        };
        let path = artifact_fd_path(&dir, leaf);
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
            Ok(_) => {}
        }
        artifact_member_matches(&path, m)?;
        let name =
            std::ffi::CString::new(leaf.as_bytes()).map_err(|_| "Invalid artifact member")?;
        let flag = if m.identity.directory {
            libc::AT_REMOVEDIR
        } else {
            0
        };
        if unsafe { libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), flag) } != 0 {
            return Err("Owned update artifact cleanup deferred".into());
        }
    }
    artifact_member_matches(&qpath, root_member)?;
    let name = std::ffi::CString::new(quarantine.as_bytes()).map_err(|_| "Invalid quarantine")?;
    if unsafe { libc::unlinkat(parent_fd.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
        return Err("Owned scratch cleanup deferred".into());
    }
    parent_fd.sync_all().map_err(|e| e.to_string())
}
fn forget_artifact(data: &Path, r: &ArtifactRecord) -> Result<()> {
    let path = artifact_record_path(data, &r.nonce);
    let current: Value = serde_json::from_slice(&read_regular(&path, MAX_STATE)?)
        .map_err(|_| "Invalid artifact record")?;
    if current != serde_json::to_value(r).map_err(|e| e.to_string())? {
        return Err("Artifact record changed".into());
    }
    if ArtifactIdentity::read(&artifact_lease_path(data, &r.nonce))? != r.lease {
        return Err("Artifact lease identity changed".into());
    }
    fs::remove_file(path).map_err(|e| e.to_string())?;
    fs::remove_file(artifact_lease_path(data, &r.nonce)).map_err(|e| e.to_string())
}
fn cleanup_abandoned(data: &Path) -> Value {
    let mut removed = 0usize;
    let mut kept = 0usize;
    let store = artifact_store(data);
    let result = (|| -> Result<()> {
        if !store.exists() {
            return Ok(());
        }
        prepare_data(&store)?;
        let entries: Vec<_> = fs::read_dir(&store).map_err(|e| e.to_string())?.collect();
        if entries.len() > 256 {
            return Err("Too many artifact records; all files kept".into());
        }
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let clean = (|| -> Result<()> {
                let id = p
                    .file_stem()
                    .and_then(|x| x.to_str())
                    .ok_or("Invalid artifact record filename")?;
                if !artifact_hex(id) {
                    return Err("Unrecognized artifact record".into());
                }
                let meta = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
                if meta.uid() != unsafe { libc::geteuid() }
                    || meta.mode() & 0o077 != 0
                    || meta.nlink() != 1
                {
                    return Err("Artifact record is not private".into());
                }
                let r: ArtifactRecord = serde_json::from_slice(&read_regular(&p, MAX_STATE)?)
                    .map_err(|_| "Unreadable artifact ownership record")?;
                validate_artifact(&r)?;
                if r.nonce != id || r.owners.iter().any(process_may_live) {
                    return Err("Artifact creator is active or uncertain".into());
                }
                let lp = artifact_lease_path(data, id);
                if ArtifactIdentity::read(&lp)? != r.lease {
                    return Err("Artifact lease changed".into());
                }
                let lease = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(&lp)
                    .map_err(|e| e.to_string())?;
                if ArtifactIdentity::metadata(&lease.metadata().map_err(|e| e.to_string())?)?
                    != r.lease
                {
                    return Err("Artifact opened lease changed".into());
                }
                if unsafe { libc::flock(lease.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                    return Err("Artifact writer remains active".into());
                }
                remove_artifact(&r)?;
                forget_artifact(data, &r)?;
                Ok(())
            })();
            if clean.is_ok() {
                removed += 1;
            } else {
                kept += 1;
            }
        }
        Ok(())
    })();
    json!({"removed":removed,"kept":kept,"ownership_limit":"Unrecorded files, legacy leftovers, unknown contents and changed identities are preserved. An interruption before ownership registration may leave an unrecorded file.","error":result.err()})
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
    let mut artifact = None;
    let result = (|| {
        staged_owned = Some(write_new_observed(
            &staged,
            &bytes,
            current.mode & 0o777,
            |_| {
                artifact = Some(Artifact::new(
                    data,
                    &staged,
                    "stage",
                    &nonce,
                    Some(&request),
                )?);
                Ok(())
            },
        )?);
        artifact
            .as_mut()
            .ok_or("Missing staged ownership")?
            .seal(&staged)?;
        let info = transport::build_info(data, &staged)?;
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
            let mut newer = release("v0.2.0-alpha.10-candidate.999", false);
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
            assert_eq!(candidate.version, "0.2.0-alpha.10-candidate.999");
            assert_eq!(candidate.page, page("v0.2.0-alpha.10-candidate.999"));
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
    fn configured_production_verifier_refuses_unsigned_and_fixture_signed_bytes() {
        reset_cancel();
        let key = PRODUCTION_KEY.expect("Production public verifier must be configured");
        assert!(!VerifyingKey::from_bytes(&key).unwrap().is_weak());
        let fixture = SigningKey::from_bytes(&[7; 32]);
        assert_ne!(key, fixture.verifying_key().to_bytes());
        let bytes = b"public-verifier refusal fixture, not executable";
        let hash = fsutil::hash(bytes);
        assert_eq!(
            verify_payload(key, bytes, &hash, &[]).unwrap_err(),
            "Invalid release signature length"
        );
        assert_eq!(
            verify_payload(key, bytes, &hash, &[0; 64]).unwrap_err(),
            "Portable signature does not verify"
        );
        assert_eq!(
            verify_payload(key, bytes, &hash, &fixture.sign(bytes).to_bytes()).unwrap_err(),
            "Portable signature does not verify"
        );
        let p = temp();
        assert_eq!(status(&p, &Settings::default())["signing_configured"], true);
        fs::remove_dir_all(p).unwrap();
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
        let active = fsutil::Lock::acquire(&p.join("operation.lock")).unwrap();
        dispatch(&p, &json!({"action":"updater_check"})).unwrap();
        assert!(load(&p).unwrap().last_attempt.is_none());
        drop(active);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn unset_key_refuses_install_without_network_or_portable_env() {
        let p = temp();
        let bytes =
            serde_json::to_vec(&vec![release("v0.2.0-alpha.10-candidate.999", false)]).unwrap();
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
        let result = install(
            &p,
            &load(&p).unwrap(),
            &json!({"action":"updater_install","confirmed":true,"token":candidate.token}),
            None,
        )
        .unwrap();
        assert_eq!(
            result["message"],
            "Signed automatic updates are not configured in this build. Use the release page"
        );
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
    #[test]
    fn unfinished_idle_setup_allows_checks_but_not_installation() {
        let p = temp();
        fsutil::json(
            &p.join("state.json"),
            &json!({"installations":[{"stage":"launcher_install"}]}),
        )
        .unwrap();
        assert!(!workflow_readiness(&p).unwrap());
        {
            let _guard = check_idle_lock(&p).unwrap();
        }
        let value = status(&p, &Settings::default());
        assert_eq!(value["install_ready"], false);
        assert_eq!(value["install_blocked_message"], UNFINISHED_SETUP);
        let result = dispatch(
            &p,
            &json!({"action":"updater_install","confirmed":true,"token":"not-needed"}),
        )
        .unwrap();
        assert_eq!(result["manual_required"], true);
        assert_eq!(result["message"], UNFINISHED_SETUP);
        assert!(!p.join("update-request.json").exists());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn saved_recovery_markers_are_unfinished_not_proof_of_running_work() {
        let p = temp();
        for name in [
            "transaction.json",
            "cleanup.json",
            "restore.json",
            "controller-job.json",
            "steam-restart.json",
            "update-request.json",
        ] {
            fs::write(p.join(name), b"retained interrupted fixture").unwrap();
            assert!(!workflow_readiness(&p).unwrap());
            {
                let _guard = check_idle_lock(&p).unwrap();
            }
            assert_eq!(idle(&p).unwrap_err(), UNFINISHED_SETUP);
            fs::remove_file(p.join(name)).unwrap();
        }
        let guard = fsutil::Lock::acquire(&p.join("operation.lock")).unwrap();
        assert!(check_idle_lock(&p).is_err());
        let result = dispatch(&p, &json!({"action":"updater_check"})).unwrap();
        assert_eq!(result["install_ready"], false);
        assert!(load(&p).unwrap().last_attempt.is_none());
        let refused = dispatch(
            &p,
            &json!({"action":"updater_install","confirmed":true,"token":"unused"}),
        )
        .unwrap();
        assert!(refused["message"]
            .as_str()
            .unwrap()
            .contains("Another setup operation"));
        drop(guard);
        fs::remove_dir_all(p).unwrap();
    }

    fn scratch_fixture(data: &Path) -> Artifact {
        let nonce = fsutil::unique();
        let root = data.join(format!(".wow-update-tmp-{nonce}"));
        fsutil::private_dir(&root).unwrap();
        Artifact::new(data, &root, "scratch", &nonce, None).unwrap()
    }
    fn abandon_fixture(a: &mut Artifact) {
        a.record.owners = vec![ArtifactProcess {
            pid: u32::MAX,
            start: "1".into(),
        }];
        a.save().unwrap();
    }
    #[test]
    fn interrupted_download_owned_partial_is_removed_and_cleanup_repeats_harmlessly() {
        let data = temp();
        let mut a = scratch_fixture(&data);
        let root = a.record.root.clone();
        let hop = root.join("0");
        fsutil::private_dir(&hop).unwrap();
        a.add(&hop).unwrap();
        let body = hop.join("body");
        write_new(&body, b"", 0o600).unwrap();
        a.add(&body).unwrap();
        fs::write(&body, b"interrupted transfer bytes").unwrap();
        abandon_fixture(&mut a);
        drop(a);
        let result = cleanup_abandoned(&data);
        assert_eq!(result["removed"], 1);
        assert!(!root.exists());
        assert_eq!(cleanup_abandoned(&data)["removed"], 0);
        fs::remove_dir_all(data).unwrap();
    }
    #[test]
    fn cleanup_preserves_unrecorded_and_changed_partial_identities() {
        let data = temp();
        let mut a = scratch_fixture(&data);
        let root = a.record.root.clone();
        let body = root.join("body");
        write_new(&body, b"owned", 0o600).unwrap();
        a.add(&body).unwrap();
        let displaced = data.join("saved-partial");
        fs::rename(&body, displaced).unwrap();
        write_new(&body, b"unrelated replacement", 0o600).unwrap();
        abandon_fixture(&mut a);
        drop(a);
        assert_eq!(cleanup_abandoned(&data)["removed"], 0);
        assert_eq!(fs::read(&body).unwrap(), b"unrelated replacement");
        let legacy = data.join(format!(".wow-update-tmp-{}", fsutil::unique()));
        fsutil::private_dir(&legacy).unwrap();
        fs::write(legacy.join("body"), b"unrecorded old transfer").unwrap();
        cleanup_abandoned(&data);
        assert!(legacy.join("body").exists());
        fs::remove_dir_all(data).unwrap();
    }
    #[test]
    fn cleanup_preserves_unknown_scratch_content() {
        let data = temp();
        let mut a = scratch_fixture(&data);
        let root = a.record.root.clone();
        write_new(&root.join("personal-notes"), b"unrelated", 0o600).unwrap();
        abandon_fixture(&mut a);
        drop(a);
        assert_eq!(cleanup_abandoned(&data)["removed"], 0);
        assert_eq!(fs::read(root.join("personal-notes")).unwrap(), b"unrelated");
        fs::remove_dir_all(data).unwrap();
    }
    #[test]
    fn artifact_activity_lease_survives_creator_and_blocks_cleanup() {
        use std::os::unix::process::CommandExt;
        let data = temp();
        let mut a = scratch_fixture(&data);
        let root = a.record.root.clone();
        abandon_fixture(&mut a);
        let fd = a.lease_fd();
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", "sleep 0.2"]);
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let mut child = command.spawn().unwrap();
        drop(a);
        assert_eq!(cleanup_abandoned(&data)["removed"], 0);
        assert!(root.exists());
        assert!(child.wait().unwrap().success());
        assert_eq!(cleanup_abandoned(&data)["removed"], 1);
        fs::remove_dir_all(data).unwrap();
    }
    #[test]
    fn active_owner_process_preserves_artifacts_without_a_profile_lock() {
        let data = temp();
        let a = scratch_fixture(&data);
        let root = a.record.root.clone();
        drop(a);
        assert_eq!(cleanup_abandoned(&data)["removed"], 0);
        assert!(root.exists());
        fs::remove_dir_all(data).unwrap();
    }
    fn stage_fixture(data: &Path, parent: &Path) -> Artifact {
        let nonce = fsutil::unique();
        let root = parent.join(format!(".wow-update-{nonce}.new"));
        write_new(&root, b"partial staged installer", 0o700).unwrap();
        let runtime = data.join(format!("run-{nonce}"));
        fsutil::private_dir(&runtime).unwrap();
        Artifact::new(
            data,
            &root,
            "stage",
            &nonce,
            Some(&runtime.join("update-request.json")),
        )
        .unwrap()
    }
    #[test]
    fn interrupted_stage_is_cleaned_but_recovery_references_and_backups_are_preserved() {
        let data = temp();
        let parent = temp();
        let mut a = stage_fixture(&data, &parent);
        let root = a.record.root.clone();
        let request = a.record.handoff.clone().unwrap();
        write_new(&request, b"retained unresolved handoff", 0o600).unwrap();
        abandon_fixture(&mut a);
        let nonce = a.record.nonce.clone();
        drop(a);
        assert_eq!(cleanup_abandoned(&data)["removed"], 0);
        fs::remove_file(&request).unwrap();
        let recovery = parent.join(format!(".wow-update-{nonce}.recovery.json"));
        write_new(&recovery, b"unresolved recovery", 0o600).unwrap();
        assert_eq!(cleanup_abandoned(&data)["removed"], 0);
        fs::remove_file(recovery).unwrap();
        let backup = parent.join(format!(".wow-update-{nonce}.backup"));
        write_new(&backup, b"previous installer", 0o700).unwrap();
        assert_eq!(cleanup_abandoned(&data)["removed"], 0);
        assert!(root.exists());
        assert_eq!(fs::read(&backup).unwrap(), b"previous installer");
        fs::remove_file(backup).unwrap();
        assert_eq!(cleanup_abandoned(&data)["removed"], 1);
        fs::remove_dir_all(data).unwrap();
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn sealed_stage_edit_is_preserved() {
        let data = temp();
        let parent = temp();
        let mut a = stage_fixture(&data, &parent);
        let root = a.record.root.clone();
        a.seal(&root).unwrap();
        fs::write(&root, b"changed same inode").unwrap();
        abandon_fixture(&mut a);
        drop(a);
        assert_eq!(cleanup_abandoned(&data)["removed"], 0);
        assert_eq!(fs::read(root).unwrap(), b"changed same inode");
        fs::remove_dir_all(data).unwrap();
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn startup_cleanup_respects_active_operation_and_opt_out() {
        let data = temp();
        let mut a = scratch_fixture(&data);
        let root = a.record.root.clone();
        abandon_fixture(&mut a);
        drop(a);
        dispatch(&data, &json!({"action":"updater_enabled","enabled":false})).unwrap();
        let operation = fsutil::Lock::acquire(&data.join("operation.lock")).unwrap();
        dispatch(&data, &json!({"action":"updater_check"})).unwrap();
        assert!(root.exists());
        drop(operation);
        dispatch(&data, &json!({"action":"updater_check"})).unwrap();
        assert!(!root.exists());
        assert!(!load(&data).unwrap().enabled);
        assert!(load(&data).unwrap().last_attempt.is_none());
        fs::remove_dir_all(data).unwrap();
    }
    #[test]
    fn current_portable_is_never_cleaned_as_an_abandoned_stage() {
        let data = temp();
        let parent = temp();
        let mut a = stage_fixture(&data, &parent);
        let root = a.record.root.clone();
        abandon_fixture(&mut a);
        drop(a);
        let previous = std::env::var_os("WOW_PORTABLE_PATH");
        std::env::set_var("WOW_PORTABLE_PATH", &root);
        let result = cleanup_abandoned(&data);
        if let Some(p) = previous {
            std::env::set_var("WOW_PORTABLE_PATH", p);
        } else {
            std::env::remove_var("WOW_PORTABLE_PATH");
        }
        assert_eq!(result["removed"], 0);
        assert!(root.exists());
        fs::remove_dir_all(data).unwrap();
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn updater_dispatch_resolves_data_parent_alias_once() {
        use std::os::unix::fs::symlink;
        let base = temp();
        let real = base.join("actual");
        fsutil::private_dir(&real).unwrap();
        let alias = base.join("alias");
        symlink(&real, &alias).unwrap();
        let data_alias = alias.join("profile");
        let resolved = canonical_data(&data_alias).unwrap();
        assert_eq!(resolved, real.join("profile"));
        dispatch(
            &data_alias,
            &json!({"action":"updater_enabled","enabled":false}),
        )
        .unwrap();
        assert!(!load(&resolved).unwrap().enabled);
        assert_eq!(
            dispatch(&data_alias, &json!({"action":"updater_status"})).unwrap()["enabled"],
            false
        );
        let mut artifact = scratch_fixture(&resolved);
        let root = artifact.record.root.clone();
        abandon_fixture(&mut artifact);
        drop(artifact);
        dispatch(&data_alias, &json!({"action":"updater_check"})).unwrap();
        assert!(!root.exists());
        assert!(load(&resolved).unwrap().last_attempt.is_none());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn updater_data_leaf_symlink_is_rejected_without_touching_target() {
        use std::os::unix::fs::symlink;
        let base = temp();
        let data = base.join("actual-profile");
        fsutil::private_dir(&data).unwrap();
        fs::write(data.join("sentinel"), b"keep").unwrap();
        let alias = base.join("profile-link");
        symlink(&data, &alias).unwrap();
        assert!(dispatch(&alias, &json!({"action":"updater_status"})).is_err());
        assert_eq!(fs::read(data.join("sentinel")).unwrap(), b"keep");
        assert_eq!(fs::read_dir(&data).unwrap().count(), 1);
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn failed_artifact_registration_rolls_back_only_attempt_owned_files() {
        let data = temp();
        let nonce = fsutil::unique();
        let root = data.join(format!(".wow-update-tmp-{nonce}"));
        fsutil::private_dir(&root).unwrap();
        let failed = Artifact::new_with_sync(&data, &root, "scratch", &nonce, None, |_| {
            Err("simulated registration sync failure".into())
        });
        assert!(failed.is_err());
        assert!(!artifact_record_path(&data, &nonce).exists());
        assert!(!artifact_lease_path(&data, &nonce).exists());
        assert!(root.is_dir());
        fs::remove_dir_all(data).unwrap();
    }
    #[test]
    fn registration_failure_preserves_replaced_record_and_lease() {
        let data = temp();
        let nonce = fsutil::unique();
        let root = data.join(format!(".wow-update-tmp-{nonce}"));
        fsutil::private_dir(&root).unwrap();
        let record = artifact_record_path(&data, &nonce);
        let lease = artifact_lease_path(&data, &nonce);
        let failed = Artifact::new_with_sync(&data, &root, "scratch", &nonce, None, |_| {
            fs::rename(&record, data.join("moved-owned-record")).unwrap();
            fs::rename(&lease, data.join("moved-owned-lease")).unwrap();
            write_new(&record, b"unrelated record replacement", 0o600).unwrap();
            write_new(&lease, b"unrelated lease replacement", 0o600).unwrap();
            Err("simulated final registration failure".into())
        });
        assert!(failed.is_err());
        assert_eq!(fs::read(record).unwrap(), b"unrelated record replacement");
        assert_eq!(fs::read(lease).unwrap(), b"unrelated lease replacement");
        fs::remove_dir_all(data).unwrap();
    }
}
