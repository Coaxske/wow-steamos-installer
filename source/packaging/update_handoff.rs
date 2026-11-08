//! Portable-owned replacement/restart protocol. No Steam, game or global signals.
use std::{
    env, fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};
const NOFOLLOW: i32 = 0x20000;
const RESTART_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);
extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}
fn uid() -> Result<u32, String> {
    fs::metadata("/proc/self")
        .map(|m| m.uid())
        .map_err(|e| e.to_string())
}
fn hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn same(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.uid() == b.uid()
        && a.mode() == b.mode()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
fn private_dir(path: &Path) -> Result<fs::Metadata, String> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.uid() != uid()? || m.mode() & 0o777 != 0o700 {
        return Err("Invalid private updater directory".into());
    }
    Ok(m)
}
// Resolve legitimate HOME/cache aliases while retaining the owned-directory
// checks. The final runtime component itself may not be a symlink.
fn canonical_runtime(path: &Path) -> Result<PathBuf, String> {
    let before = private_dir(path)?;
    let canonical = fs::canonicalize(path).map_err(|e| e.to_string())?;
    if !same(&before, &private_dir(&canonical)?) {
        return Err("Updater runtime changed during resolution".into());
    }
    Ok(canonical)
}
fn private_bytes(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file()
        || before.uid() != uid()?
        || before.mode() & 0o077 != 0
        || before.nlink() != 1
        || before.len() > limit
    {
        return Err("Invalid private updater file".into());
    }
    let mut bytes = Vec::new();
    (&file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let after = file.metadata().map_err(|e| e.to_string())?;
    let named = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit || !same(&before, &after) || !same(&after, &named) {
        return Err("Updater file changed during verification".into());
    }
    Ok(bytes)
}
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.set_permissions(fs::Permissions::from_mode(0o600))
        .and_then(|_| f.write_all(bytes))
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())
}
#[derive(Clone)]
pub struct PortablePin {
    path: PathBuf,
    dev: u64,
    ino: u64,
    size: u64,
    mode: u32,
    owner: u32,
}
impl PortablePin {
    pub fn capture() -> Option<Self> {
        let path = env::current_exe().ok()?;
        if !path.is_absolute()
            || path.to_str()?.chars().any(char::is_control)
            || fs::canonicalize(&path).ok()? != path
        {
            return None;
        }
        let disk = fs::symlink_metadata(&path).ok()?;
        let running = fs::metadata("/proc/self/exe").ok()?;
        let owner = uid().ok()?;
        if !disk.is_file()
            || disk.uid() != owner
            || disk.mode() & 0o111 == 0
            || disk.mode() & 0o6000 != 0
            || disk.dev() != running.dev()
            || disk.ino() != running.ino()
            || disk.len() != running.len()
        {
            return None;
        }
        Some(Self {
            path,
            dev: disk.dev(),
            ino: disk.ino(),
            size: disk.len(),
            mode: disk.mode(),
            owner,
        })
    }
    fn unchanged(&self) -> bool {
        fs::symlink_metadata(&self.path)
            .map(|m| {
                m.is_file()
                    && m.dev() == self.dev
                    && m.ino() == self.ino
                    && m.len() == self.size
                    && m.mode() == self.mode
                    && m.uid() == self.owner
            })
            .unwrap_or(false)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Receipt {
    sha256: String,
    version: String,
    source: String,
}
impl Receipt {
    fn parse(bytes: &[u8], header: &str, nonce: &str) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let lines: Vec<_> = text.lines().collect();
        if text.contains('\r')
            || !text.ends_with('\n')
            || lines.len() != 5
            || lines[0] != header
            || lines[1] != nonce
            || !hex(lines[2], 64)
            || !hex(lines[4], 64)
            || lines[3].is_empty()
            || lines[3].len() > 128
            || !lines[3]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".+-".contains(&b))
        {
            return Err("Invalid updater receipt".into());
        }
        Ok(Self {
            sha256: lines[2].into(),
            version: lines[3].into(),
            source: lines[4].into(),
        })
    }
    fn read(path: &Path, header: &str, nonce: &str) -> Result<Self, String> {
        Self::parse(&private_bytes(path, 4096)?, header, nonce)
    }
}
pub fn clear_context(cmd: &mut Command, preserve_startup: bool) {
    for (key, _) in env::vars_os() {
        let name = key.to_string_lossy();
        let startup = matches!(
            name.as_ref(),
            "WOW_UPDATE_STARTUP_ACK" | "WOW_UPDATE_STARTUP_NONCE" | "WOW_UPDATE_STARTUP_ABORT"
        );
        if (name.starts_with("WOW_PORTABLE_")
            || name.starts_with("WOW_UPDATE_")
            || name.starts_with("WOW_ORIGINAL_PORTABLE"))
            && !(preserve_startup && startup)
        {
            cmd.env_remove(key);
        }
    }
}
fn native_command(runtime: &Path) -> Command {
    let mut cmd = Command::new(runtime.join("bin/wow-steamos"));
    cmd.env("WOW_BUNDLED", "1");
    for key in [
        "LD_LIBRARY_PATH",
        "QT_PLUGIN_PATH",
        "QML2_IMPORT_PATH",
        "QML_IMPORT_PATH",
    ] {
        cmd.env(
            format!("WOW_ORIGINAL_{key}"),
            env::var_os(key).unwrap_or_default(),
        );
    }
    cmd.env("LD_LIBRARY_PATH", runtime.join("lib"))
        .env("QT_PLUGIN_PATH", runtime.join("plugins"))
        .env("QML2_IMPORT_PATH", runtime.join("qml"))
        .env("QML_IMPORT_PATH", runtime.join("qml"));
    clear_context(&mut cmd, false);
    cmd
}
pub struct Session {
    runtime: PathBuf,
    nonce: String,
    pin: PortablePin,
    binding: Option<Receipt>,
}
impl Session {
    pub fn bind(pin: PortablePin, runtime: &Path, nonce: &str) -> Option<Self> {
        let runtime = canonical_runtime(runtime).ok()?;
        if !hex(nonce, 32) || !pin.unchanged() {
            return None;
        }
        let mut s = Self {
            runtime: runtime.into(),
            nonce: nonce.into(),
            pin,
            binding: None,
        };
        s.helper("updater_bind").ok()?;
        let b = Receipt::read(
            &s.runtime.join("update-binding"),
            "WOW_UPDATE_BIND_V1",
            &s.nonce,
        )
        .ok()?;
        if !s.pin.unchanged() {
            return None;
        }
        s.binding = Some(b);
        Some(s)
    }
    pub fn configure(&self, cmd: &mut Command) {
        cmd.env("WOW_PORTABLE_PATH", &self.pin.path)
            .env("WOW_PORTABLE_DEV", self.pin.dev.to_string())
            .env("WOW_PORTABLE_INO", self.pin.ino.to_string())
            .env("WOW_PORTABLE_SIZE", self.pin.size.to_string())
            .env("WOW_PORTABLE_MODE", self.pin.mode.to_string())
            .env("WOW_PORTABLE_UID", self.pin.owner.to_string())
            .env("WOW_UPDATE_NONCE", &self.nonce)
            .env(
                "WOW_UPDATE_REQUEST",
                self.runtime.join("update-request.json"),
            )
            .env("WOW_UPDATE_BINDING", self.runtime.join("update-binding"))
            .env("WOW_UPDATE_RECEIPT", self.runtime.join("update-receipt"));
        if let Some(b) = &self.binding {
            cmd.env("WOW_PORTABLE_SHA256", &b.sha256);
        } else {
            cmd.env_remove("WOW_PORTABLE_SHA256");
        }
        cmd.env_remove("WOW_UPDATE_WRAPPER_ACTION");
    }
    fn helper(&self, action: &str) -> Result<(), String> {
        let mut cmd = native_command(&self.runtime);
        self.configure(&mut cmd);
        cmd.env("WOW_UPDATE_WRAPPER_ACTION", action)
            .arg("--engine-json")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let written = match child.stdin.take() {
            Some(mut input) => input.write_all(format!("{{\"action\":\"{action}\"}}").as_bytes()),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Missing updater input",
            )),
        };
        let status = child.wait().map_err(|e| e.to_string())?;
        written.map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("The updater helper refused the operation".into());
        }
        Ok(())
    }
    pub fn has_request(&self) -> Result<bool, String> {
        let path = self.runtime.join("update-request.json");
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.to_string()),
            Ok(_) => {
                private_bytes(&path, 65536)?;
                Ok(true)
            }
        }
    }
    fn child_exit_proven(&self, expected: &Receipt, rollback: bool) -> bool {
        let name = if rollback {
            "rollback-child-exit"
        } else {
            "update-child-exit"
        };
        Receipt::read(
            &self.runtime.join(name),
            "WOW_UPDATE_CHILD_EXIT_V1",
            &self.nonce,
        )
        .as_ref()
        .ok()
            == Some(expected)
    }
    fn restart(&self, expected: &Receipt, rollback: bool) -> Result<(), bool> {
        let ready_name = if rollback {
            "rollback-ready"
        } else {
            "update-ready"
        };
        let abort_name = if rollback {
            "rollback-abort"
        } else {
            "update-abort"
        };
        let ack = self.runtime.join(ready_name);
        let abort = self.runtime.join(abort_name);
        if ready_matches(&ack, &self.nonce, expected) {
            return Ok(());
        }
        if ack.exists()
            || abort.exists()
            || self
                .runtime
                .join(if rollback {
                    "rollback-child-exit"
                } else {
                    "update-child-exit"
                })
                .symlink_metadata()
                .is_ok()
        {
            return Err(false);
        }
        let mut cmd = Command::new(&self.pin.path);
        clear_context(&mut cmd, false);
        for key in [
            "LD_LIBRARY_PATH",
            "QT_PLUGIN_PATH",
            "QML2_IMPORT_PATH",
            "QML_IMPORT_PATH",
        ] {
            let saved = format!("WOW_ORIGINAL_{key}");
            if env::var_os("WOW_BUNDLED").is_some() {
                if let Some(value) = env::var_os(&saved).filter(|v| !v.is_empty()) {
                    cmd.env(key, value);
                } else {
                    cmd.env_remove(key);
                }
            }
            cmd.env_remove(saved);
        }
        cmd.env_remove("WOW_BUNDLED")
            .env("WOW_UPDATE_STARTUP_ACK", &ack)
            .env("WOW_UPDATE_STARTUP_NONCE", &self.nonce)
            .env("WOW_UPDATE_STARTUP_ABORT", &abort)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|_| true)?;
        let deadline = Instant::now() + RESTART_TIMEOUT;
        loop {
            if ready_matches(&ack, &self.nonce, expected) {
                return Ok(());
            }
            match child.try_wait() {
                Ok(Some(_)) => {
                    return if ready_matches(&ack, &self.nonce, expected) {
                        Ok(())
                    } else if self.child_exit_proven(expected, rollback) {
                        Err(true)
                    } else {
                        Err(false)
                    }
                }
                Err(_) => {
                    return if ready_matches(&ack, &self.nonce, expected) {
                        Ok(())
                    } else {
                        Err(false)
                    }
                }
                Ok(None) => {}
            }
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        if ready_matches(&ack, &self.nonce, expected) {
            return Ok(());
        }
        if write_private(&abort, format!("{}\n", self.nonce).as_bytes()).is_err() {
            return if ready_matches(&ack, &self.nonce, expected) {
                Ok(())
            } else {
                Err(false)
            };
        }
        let deadline = Instant::now() + STOP_TIMEOUT;
        loop {
            if ready_matches(&ack, &self.nonce, expected) {
                return Ok(());
            }
            match child.try_wait() {
                Ok(Some(_)) => {
                    return if ready_matches(&ack, &self.nonce, expected) {
                        Ok(())
                    } else if self.child_exit_proven(expected, rollback) {
                        Err(true)
                    } else {
                        Err(false)
                    }
                }
                Err(_) => {
                    return if ready_matches(&ack, &self.nonce, expected) {
                        Ok(())
                    } else {
                        Err(false)
                    }
                }
                Ok(None) => {}
            }
            if Instant::now() >= deadline {
                return Err(false);
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn verified_recovery_path(&self) -> Option<PathBuf> {
        self.helper("updater_recovery_info").ok()?;
        let bytes = private_bytes(&self.runtime.join("update-recovery-info"), 16384).ok()?;
        let text = std::str::from_utf8(&bytes).ok()?;
        if !text.ends_with('\n') || text.contains('\r') || text.contains('\0') {
            return None;
        }
        let fields: Vec<_> = text[..text.len() - 1].split('\n').collect();
        let binding = self.binding.as_ref()?;
        if fields.len() != 6
            || fields[0] != "WOW_UPDATE_RECOVERY_V1"
            || fields[1] != self.nonce
            || fields[3] != binding.sha256
            || fields[4] != binding.version
            || fields[5] != binding.source
        {
            return None;
        }
        let path = PathBuf::from(fields[2]);
        let parent = self.pin.path.parent()?;
        if ![
            self.pin.path.clone(),
            parent.join(format!(".wow-update-{}.backup", self.nonce)),
            parent.join(format!(".wow-update-{}.new", self.nonce)),
        ]
        .contains(&path)
        {
            return None;
        }
        Some(path)
    }
    fn recovery_message(&self, reason: &str) -> String {
        recovery_text(
            reason,
            &self.pin.path,
            &self.runtime,
            self.verified_recovery_path().as_deref(),
        )
    }
    pub fn finish(&self) -> Result<bool, String> {
        write_private(
            &self.runtime.join("update-pending"),
            format!("WOW_UPDATE_PENDING_V1\n{}\n", self.nonce).as_bytes(),
        )
        .map_err(|e| self.recovery_message(&e))?;
        let committed = self.helper("updater_commit").and_then(|_| {
            Receipt::read(
                &self.runtime.join("update-receipt"),
                "WOW_UPDATE_COMMIT_V1",
                &self.nonce,
            )
        });
        if let Ok(new) = committed {
            match self.restart(&new,false) {
                Ok(()) => {
                    // A healthy READY client is never rolled back because cleanup
                    // cannot be proved. Retain the private evidence on refusal.
                    if self.helper("updater_finalize").is_ok() {return Ok(true);}
                    eprintln!("The new installer is ready. Older backup cleanup was deferred; recovery records were kept at: {}",self.runtime.display());
                    return Ok(false);
                }
                Err(false) => return Err(self.recovery_message("The new installer did not confirm startup and its exit could not be verified. No rollback was attempted while it might still be active.")),
                Err(true) => {}
            }
        }
        self.helper("updater_rollback")
            .map_err(|e| self.recovery_message(&e))?;
        let old = Receipt::read(
            &self.runtime.join("update-receipt"),
            "WOW_UPDATE_ROLLBACK_V1",
            &self.nonce,
        )
        .map_err(|e| self.recovery_message(&e))?;
        if self.binding.as_ref() != Some(&old) {
            return Err(
                self.recovery_message("Restored installer identity did not match the original")
            );
        }
        match self.restart(&old, true) {
            Ok(()) => Ok(true),
            Err(_) => Err(self.recovery_message(
                "The previous installer was restored, but automatic reopening was not confirmed.",
            )),
        }
    }
}
pub struct Startup {
    ready_seen: std::cell::Cell<bool>,
    parent: PathBuf,
    parent_dev: u64,
    parent_ino: u64,
    ack: PathBuf,
    abort: PathBuf,
    nonce: String,
    expected: Receipt,
}
impl Startup {
    pub fn read(cache_base: &Path) -> Result<Option<Self>, String> {
        let Some(ack) = env::var_os("WOW_UPDATE_STARTUP_ACK").map(PathBuf::from) else {
            return Ok(None);
        };
        let nonce =
            env::var("WOW_UPDATE_STARTUP_NONCE").map_err(|_| "Missing updater startup nonce")?;
        if !hex(&nonce, 32) || !ack.is_absolute() {
            return Err("Invalid updater startup channel".into());
        }
        let parent = ack.parent().ok_or("Missing updater startup parent")?;
        let base = fs::canonicalize(cache_base).map_err(|e| e.to_string())?;
        if fs::canonicalize(parent.parent().ok_or("Missing cache parent")?)
            .map_err(|e| e.to_string())?
            != base
            || parent.file_name().and_then(|x| x.to_str()) != Some(format!("run-{nonce}").as_str())
        {
            return Err("Updater startup channel is outside its private runtime".into());
        }
        let rollback = match ack.file_name().and_then(|x| x.to_str()) {
            Some("update-ready") => false,
            Some("rollback-ready") => true,
            _ => return Err("Invalid updater startup acknowledgment name".into()),
        };
        let abort = parent.join(if rollback {
            "rollback-abort"
        } else {
            "update-abort"
        });
        if env::var_os("WOW_UPDATE_STARTUP_ABORT")
            .map(PathBuf::from)
            .as_ref()
            != Some(&abort)
        {
            return Err("Invalid updater startup abort channel".into());
        }
        let m = private_dir(parent)?;
        let expected = Receipt::read(
            &parent.join("update-receipt"),
            if rollback {
                "WOW_UPDATE_ROLLBACK_V1"
            } else {
                "WOW_UPDATE_COMMIT_V1"
            },
            &nonce,
        )?;
        Ok(Some(Self {
            ready_seen: std::cell::Cell::new(false),
            parent: parent.into(),
            parent_dev: m.dev(),
            parent_ino: m.ino(),
            ack,
            abort,
            nonce,
            expected,
        }))
    }
    pub fn aborted(&self) -> bool {
        private_bytes(&self.abort, 128)
            .map(|b| b == format!("{}\n", self.nonce).as_bytes())
            .unwrap_or(false)
    }
    fn parent_current(&self) -> bool {
        private_dir(&self.parent)
            .map(|m| m.dev() == self.parent_dev && m.ino() == self.parent_ino)
            .unwrap_or(false)
    }
    fn record_child_exit(&self) {
        if self.ready_seen.get() {
            return;
        }
        if ready_matches(&self.ack, &self.nonce, &self.expected) {
            self.ready_seen.set(true);
            return;
        }
        if !self.parent_current() {
            return;
        }
        let name = if self.ack.file_name().and_then(|s| s.to_str()) == Some("rollback-ready") {
            "rollback-child-exit"
        } else {
            "update-child-exit"
        };
        let body = format!(
            "WOW_UPDATE_CHILD_EXIT_V1\n{}\n{}\n{}\n{}\n",
            self.nonce, self.expected.sha256, self.expected.version, self.expected.source
        );
        let _ = write_private(&self.parent.join(name), body.as_bytes());
    }
    fn claim_abort(&self) -> bool {
        if !self.parent_current() {
            return false;
        }
        let name = if self.ack.file_name().and_then(|x| x.to_str()) == Some("rollback-ready") {
            "rollback-decision"
        } else {
            "update-decision"
        };
        write_private(
            &self.parent.join(name),
            format!("ABORT\n{}\n", self.nonce).as_bytes(),
        )
        .is_ok()
    }
}

fn recovery_text(reason: &str, target: &Path, runtime: &Path, verified: Option<&Path>) -> String {
    let mut message=format!("{reason}\n\nClose all installer windows before using a previous copy. If you cannot confirm they are closed, do not open another installer.\n\n");
    if let Some(path) = verified {
        if path == target {
            message.push_str(&format!("The previous installer was verified at:\n{}\n\nAfter closing other installer windows, open that file again.\n",path.display()));
        } else {
            message.push_str(&format!("The previous installer backup was verified at:\n{}\n\nIn Desktop Mode, open that folder in the file manager. Press Ctrl+H to show hidden files. Copy the backup to a different name, keep the backup, mark the copy executable in Properties, and open the copy after all other installer windows are closed. Do not overwrite or delete your current installer.\n",path.display()));
        }
    } else {
        message.push_str("A previous installer copy could not be verified. Do not rename or overwrite files based on a guessed backup name.\n");
    }
    message.push_str(&format!("\nRecovery records were kept at:\n{}\n\nYou can instead download the installer by hand from:\nhttps://github.com/Coaxske/wow-steamos-installer/releases",runtime.display()));
    message
}

fn ready_matches(path: &Path, nonce: &str, expected: &Receipt) -> bool {
    private_bytes(path, 4096)
        .map(|b| {
            b == format!(
                "WOW_UPDATE_READY_V1\n{nonce}\n{}\n{}\n",
                expected.version, expected.source
            )
            .as_bytes()
        })
        .unwrap_or(false)
}
pub fn wait_native(child: &mut Child, startup: Option<&Startup>) -> Result<ExitStatus, String> {
    let Some(startup) = startup else {
        return child.wait().map_err(|e| e.to_string());
    };
    loop {
        if ready_matches(&startup.ack, &startup.nonce, &startup.expected) {
            startup.ready_seen.set(true);
            return child.wait().map_err(|e| e.to_string());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            startup.record_child_exit();
            return Ok(status);
        }
        if startup.aborted() {
            if ready_matches(&startup.ack, &startup.nonce, &startup.expected) {
                startup.ready_seen.set(true);
                return child.wait().map_err(|e| e.to_string());
            }
            if !startup.claim_abort() {
                let status = child.wait().map_err(|e| e.to_string())?;
                startup.record_child_exit();
                return Ok(status);
            }
            unsafe {
                kill(child.id() as i32, 15);
            }
            let status = child.wait().map_err(|e| e.to_string())?;
            startup.record_child_exit();
            return Ok(status);
        }
        if !startup.parent.exists() {
            return child.wait().map_err(|e| e.to_string());
        }
        thread::sleep(Duration::from_millis(25));
    }
}
pub fn retained_recovery(runtime: &Path) -> bool {
    ["update-request.json", "update-pending", "update-receipt"]
        .iter()
        .any(|n| fs::symlink_metadata(runtime.join(n)).is_ok())
}
pub fn is_update_exit(code: Option<i32>) -> bool {
    code == Some(42)
}
pub fn suppress_startup_dialog() -> bool {
    env::var_os("WOW_UPDATE_STARTUP_ACK").is_some()
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;
    const NONCE: &str = "0123456789abcdef0123456789abcdef";
    fn receipt() -> String {
        format!(
            "WOW_UPDATE_COMMIT_V1\n{NONCE}\n{}\n0.2.0-alpha.10-candidate.50\n{}\n",
            "a".repeat(64),
            "b".repeat(64)
        )
    }
    #[test]
    fn exact_receipt_is_accepted() {
        assert!(Receipt::parse(receipt().as_bytes(), "WOW_UPDATE_COMMIT_V1", NONCE).is_ok());
    }
    #[test]
    fn wrong_nonce_header_hash_or_lines_refuse() {
        let good = receipt();
        for bad in [
            good.replace(NONCE, "ffffffffffffffffffffffffffffffff"),
            good.replace("WOW_UPDATE_COMMIT_V1", "WOW_UPDATE_BIND_V1"),
            good.replace(&"a".repeat(64), "bad"),
            format!("{good}\n"),
            good.replace('\n', "\r\n"),
        ] {
            assert!(Receipt::parse(bad.as_bytes(), "WOW_UPDATE_COMMIT_V1", NONCE).is_err());
        }
    }
    #[test]
    fn hashes_and_nonces_are_exact_lowercase_hex() {
        assert!(hex(NONCE, 32));
        assert!(!hex("0123456789ABCDEF0123456789ABCDEF", 32));
        assert!(!hex(&"0".repeat(31), 32));
    }
    fn fixture_startup() -> Startup {
        let parent =
            std::env::temp_dir().join(format!("wow-update-channel-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&parent).unwrap();
        let m = fs::metadata(&parent).unwrap();
        Startup {
            parent: parent.clone(),
            parent_dev: m.dev(),
            parent_ino: m.ino(),
            ack: parent.join("update-ready"),
            abort: parent.join("update-abort"),
            nonce: NONCE.into(),
            expected: Receipt {
                sha256: "a".repeat(64),
                version: "0.2.0-alpha.10-candidate.50".into(),
                source: "b".repeat(64),
            },
            ready_seen: std::cell::Cell::new(false),
        }
    }
    #[test]
    fn positively_reaped_startup_child_has_bound_exit_proof() {
        let startup = fixture_startup();
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 88"])
            .spawn()
            .unwrap();
        assert_eq!(
            wait_native(&mut child, Some(&startup)).unwrap().code(),
            Some(88)
        );
        let proof = Receipt::read(
            &startup.parent.join("update-child-exit"),
            "WOW_UPDATE_CHILD_EXIT_V1",
            NONCE,
        )
        .unwrap();
        assert_eq!(proof, startup.expected);
        fs::remove_dir_all(startup.parent).unwrap();
    }
    #[test]
    fn normal_quit_after_ready_does_not_need_deleted_channel() {
        let startup = fixture_startup();
        startup.ready_seen.set(true);
        fs::remove_dir_all(&startup.parent).unwrap();
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        assert!(wait_native(&mut child, Some(&startup)).unwrap().success());
        assert!(!startup.parent.exists());
    }
    #[test]
    fn unresolved_request_protects_runtime_from_pruning() {
        let startup = fixture_startup();
        write_private(
            &startup.parent.join("update-request.json"),
            b"interrupted partial request",
        )
        .unwrap();
        assert!(retained_recovery(&startup.parent));
        fs::remove_dir_all(startup.parent).unwrap();
    }
    #[test]
    fn cache_parent_alias_resolves_without_relaxing_runtime_ownership() {
        let startup = fixture_startup();
        let parent = startup.parent.parent().unwrap();
        let alias = parent.join(format!("wow-update-cache-alias-{}", std::process::id()));
        std::os::unix::fs::symlink(parent, &alias).unwrap();
        let aliased = alias.join(startup.parent.file_name().unwrap());
        assert_eq!(canonical_runtime(&aliased).unwrap(), startup.parent);
        let final_alias = parent.join(format!("wow-update-runtime-alias-{}", std::process::id()));
        std::os::unix::fs::symlink(&startup.parent, &final_alias).unwrap();
        assert!(canonical_runtime(&final_alias).is_err());
        fs::set_permissions(&startup.parent, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(canonical_runtime(&aliased).is_err());
        fs::remove_file(final_alias).unwrap();
        fs::remove_file(alias).unwrap();
        fs::remove_dir_all(startup.parent).unwrap();
    }
    #[test]
    fn ordinary_close_never_activates_a_prepared_update() {
        assert!(!is_update_exit(Some(0)));
        assert!(!is_update_exit(None));
        assert!(!is_update_exit(Some(1)));
        assert!(is_update_exit(Some(42)));
    }
    #[test]
    fn recovery_message_names_verified_backup_and_safe_steps() {
        let base = std::env::temp_dir().join("recovery fixture with spaces");
        let target = base.join("portable");
        let backup = base.join(".wow-update-fixture.backup");
        let message = recovery_text(
            "Startup could not be confirmed",
            &target,
            &base.join("runtime"),
            Some(&backup),
        );
        assert!(message.contains(backup.to_str().unwrap()));
        assert!(
            message.find("Close all installer windows").unwrap()
                < message.find("Copy the backup").unwrap()
        );
        assert!(message.contains("https://github.com/Coaxske/wow-steamos-installer/releases"));
        assert!(message.contains("Do not overwrite or delete"));
        let missing = recovery_text("Unknown", &target, &base.join("runtime"), None);
        assert!(!missing.contains("backup was verified at"));
        assert!(!missing.contains(backup.to_str().unwrap()));
    }
}
