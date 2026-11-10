//! Closed-vocabulary copyable diagnostics. Unknown free text never passes through.
use serde_json::{json, Value};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::Read,
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawFd, FromRawFd},
    },
    path::{Path, PathBuf},
};
const MAX_BYTES: usize = 256 * 1024;
const MAX_LINES: usize = 2000;
const MAX_LINE: usize = 4096;
const LOGS: [&str; 2] = ["startup.log", "startup.previous.log"];
const FILES: [&str; 3] = ["startup.log", "startup.previous.log", "events.json"];
const OPERATIONS: &[&str] = &[
    "apply",
    "guided_apply",
    "guided_existing",
    "recover",
    "undo",
    "review_setup",
    "review_repair",
    "review_adopt",
    "review_reinstall",
    "review_clean",
    "launch",
    "cancel_cleanup",
    "abandon_cleanup",
    "reset_state",
];
#[derive(Debug, PartialEq, Eq)]
enum ReadResult {
    Present(String),
    Missing,
    Unavailable,
    TooLarge,
    Changed,
    InvalidText,
}
struct Directory {
    file: File,
    path: PathBuf,
    metadata: Metadata,
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
impl Directory {
    fn open(data: &Path) -> Option<Self> {
        let before = fs::symlink_metadata(data).ok()?;
        if !before.is_dir()
            || before.uid() != unsafe { libc::geteuid() }
            || before.mode() & 0o777 != 0o700
        {
            return None;
        }
        let path = fs::canonicalize(data).ok()?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .ok()?;
        let metadata = file.metadata().ok()?;
        let named = fs::symlink_metadata(data).ok()?;
        if !same(&before, &metadata) || !same(&metadata, &named) {
            return None;
        }
        Some(Self {
            file,
            path,
            metadata,
        })
    }
    fn unchanged(&self) -> bool {
        fs::symlink_metadata(&self.path)
            .map(|m| same(&m, &self.metadata))
            .unwrap_or(false)
    }
    fn read(&self, name: &'static str) -> ReadResult {
        if !FILES.contains(&name) || !self.unchanged() {
            return ReadResult::Unavailable;
        }
        let name = std::ffi::CString::new(name).expect("fixed diagnostic name");
        // The fixed leaf is resolved beneath a held owned directory, without links or atime writes.
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK
                    | libc::O_NOATIME,
            )
        };
        if fd < 0 {
            return if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
                ReadResult::Missing
            } else {
                ReadResult::Unavailable
            };
        }
        // SAFETY: openat produced a new descriptor, transferred once into File.
        let mut file = unsafe { File::from_raw_fd(fd) };
        let Ok(before) = file.metadata() else {
            return ReadResult::Unavailable;
        };
        if !before.is_file()
            || before.uid() != unsafe { libc::geteuid() }
            || before.mode() & 0o777 != 0o600
            || before.nlink() != 1
        {
            return ReadResult::Unavailable;
        }
        if before.len() > MAX_BYTES as u64 {
            return ReadResult::TooLarge;
        }
        let mut raw = Vec::with_capacity(before.len() as usize);
        if file
            .by_ref()
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut raw)
            .is_err()
        {
            return ReadResult::Unavailable;
        }
        if raw.len() > MAX_BYTES {
            return ReadResult::TooLarge;
        }
        let Ok(after) = file.metadata() else {
            return ReadResult::Changed;
        };
        let mut named = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: fstatat initializes its output only on success. The fixed leaf does not follow links.
        let named_ok = unsafe {
            libc::fstatat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                named.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } == 0;
        if !named_ok || !same(&before, &after) || !self.unchanged() {
            return ReadResult::Changed;
        }
        // SAFETY: successful fstatat above initialized this value.
        let n = unsafe { named.assume_init() };
        if n.st_dev != before.dev()
            || n.st_ino != before.ino()
            || n.st_mode != before.mode()
            || n.st_uid != before.uid()
            || n.st_nlink != before.nlink()
            || n.st_size != before.len() as i64
            || n.st_mtime != before.mtime()
            || n.st_mtime_nsec != before.mtime_nsec()
            || n.st_ctime != before.ctime()
            || n.st_ctime_nsec != before.ctime_nsec()
        {
            return ReadResult::Changed;
        }
        String::from_utf8(raw)
            .map(ReadResult::Present)
            .unwrap_or(ReadResult::InvalidText)
    }
}
fn known_version(value: &Value) -> Option<String> {
    let suffix = value.as_str()?.strip_prefix("0.2.0-alpha.10-candidate.")?;
    if suffix.is_empty() || suffix.len() > 3 || !suffix.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u16 = suffix.parse().ok()?;
    Some(format!("0.2.0-alpha.10-candidate.{n}"))
}
fn cleaned_summary(s: &Value) -> Value {
    let mut out = json!({"version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH});
    for key in ["native_steam_count", "launcher_count"] {
        if let Some(n) = s[key].as_u64().filter(|n| *n <= 10000) {
            out[key] = json!(n);
        }
    }
    for key in ["pending_transaction", "pending_cleanup"] {
        if let Some(b) = s[key].as_bool() {
            out[key] = json!(b);
        }
    }
    let editions: Vec<_> = s["game_editions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|id| crate::catalog::by_id(id).is_some())
        .take(crate::catalog::EDITIONS.len())
        .collect();
    out["game_editions"] = json!(editions);
    let events:Vec<_>=s["events"].as_array().into_iter().flatten().rev().take(120).filter_map(|e|{
  let operation=e["operation"].as_str()?;let result=e["result"].as_str()?;
  if !OPERATIONS.contains(&operation)||!["started","completed","failed"].contains(&result){return None;}
  Some(json!({"version":known_version(&e["version"])? ,"operation":operation,"result":result}))
 }).collect::<Vec<_>>().into_iter().rev().collect();
    out["events"] = json!(events);
    out
}
fn errno(text: &str) -> Option<u16> {
    let rest = text.strip_suffix(')')?.rsplit_once("(os error ")?.1;
    if rest.is_empty() || rest.len() > 4 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok().filter(|n| *n <= 4095)
}
fn exit_line(line: &str) -> Option<String> {
    let status = line
        .strip_prefix("Native interface exited: ")?
        .strip_suffix('.')?;
    if let Some(code) = status.strip_prefix("exit status: ") {
        if code.is_empty() || code.len() > 3 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let code: u8 = code.parse().ok()?;
        return Some(format!("Native interface exited: exit status: {code}."));
    }
    let signal = status.strip_prefix("signal: ")?;
    let (number, suffix) = signal.split_once(' ').unwrap_or((signal, ""));
    if number.is_empty() || number.len() > 2 || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number: u8 = number.parse().ok()?;
    if !(1..=64).contains(&number) {
        return None;
    }
    let suffix = suffix.strip_suffix(" (core dumped)").unwrap_or(suffix);
    if ![
        "",
        "(SIGHUP)",
        "(SIGINT)",
        "(SIGQUIT)",
        "(SIGILL)",
        "(SIGTRAP)",
        "(SIGABRT)",
        "(SIGBUS)",
        "(SIGFPE)",
        "(SIGKILL)",
        "(SIGUSR1)",
        "(SIGSEGV)",
        "(SIGUSR2)",
        "(SIGPIPE)",
        "(SIGALRM)",
        "(SIGTERM)",
    ]
    .contains(&suffix)
    {
        return None;
    }
    Some(format!("Native interface exited: signal: {number}."))
}
fn recognized_line(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() || line.len() > MAX_LINE || line.chars().any(char::is_control) {
        return None;
    }
    const FIXED:&[&str]=&[
  "Deckhand launcher starting.","Starting Deckhand interface.","Verified update restart requested.",
  "Update startup was cancelled before the interface opened","Verified update restart had no bound portable session",
  "Verified update restart had no owned request. The current portable is unchanged.","Preview/self-check failed. See terminal output.","The application stopped unexpectedly. See terminal output.",
  "HOME must identify an absolute directory","Runtime cache must not be a symbolic link","HOME or XDG_DATA_HOME must identify an absolute folder",
  "Setup data folder must be an owned directory, not a symbolic link","Setup data folder changed during resolution","Startup diagnostics must be private, owned regular files",
  "Deckhand is already open for this profile. Close its other window first.","An existing setup window lock could not be verified; startup logs were preserved.","The setup window lock changed; startup logs were preserved.","Another setup window may be open. Close it before reopening Deckhand; startup logs were preserved.",
  "Invalid private updater directory","Updater runtime changed during resolution","Invalid private updater file","Updater file changed during verification","Invalid updater receipt","The updater helper refused the operation","Restored installer identity did not match the original",
  "The new installer did not confirm startup and its exit could not be verified. No rollback was attempted while it might still be active.","The previous installer was restored, but automatic reopening was not confirmed.","QQmlApplicationEngine failed to load component"
 ];
    if let Some(found) = FIXED.iter().find(|known| **known == line) {
        return Some((*found).to_string());
    }
    if let Some(exit) = exit_line(line) {
        return Some(exit);
    }
    const TEMPLATES:&[(&str,&str)]=&[
  ("Cannot unpack runtime (tar is required): ","Cannot unpack runtime (tar is required)."),("Runtime extraction failed: ","Runtime extraction failed."),("Cannot open the application: ","Cannot open the application."),
  ("Choose a destination on the cache filesystem: ","Cannot use the selected extraction destination."),("Startup diagnostics unavailable: ","Startup diagnostics are unavailable."),
  ("Setup could not start or stopped unexpectedly.","Setup could not start or stopped unexpectedly."),("Update recovery files retained at: ","Update recovery files were retained locally."),
  ("The new installer is ready. Older backup cleanup was deferred; recovery records were kept at: ","The new installer is ready. Older backup cleanup was deferred; recovery records were kept locally."),
  ("QStandardPaths: XDG_RUNTIME_DIR not set, defaulting to ","Qt runtime directory was not set."),("QStandardPaths: wrong permissions on runtime directory ","Qt runtime directory permissions are invalid."),
  ("qt.qpa.plugin: Could not load the Qt platform plugin ","Qt could not load its platform plugin."),("qt.qpa.xcb: could not connect to display ","Qt could not connect to the display."),
  ("Failed to create OpenGL context","Qt could not create an OpenGL context."),("This application failed to start because no Qt platform plugin could be initialized.","Qt could not initialize a platform plugin.")
 ];
    for (prefix, cleaned) in TEMPLATES {
        if line.starts_with(prefix) {
            return Some(match errno(line) {
                Some(code) => format!("{cleaned} OS error: {code}."),
                None => cleaned.to_string(),
            });
        }
    }
    if let Some(code) = errno(line) {
        for known in [
            "Permission denied",
            "No such file or directory",
            "Read-only file system",
            "No space left on device",
            "Too many open files",
            "Resource temporarily unavailable",
            "Interrupted system call",
            "Operation not permitted",
            "File exists",
        ] {
            if line == format!("{known} (os error {code})") {
                return Some(format!("{known} (os error {code})."));
            }
        }
    }
    for prefix in ["Exception code: ", "Unhandled exception code: "] {
        if let Some(code) = line.strip_prefix(prefix).and_then(|s| s.strip_prefix("0x")) {
            if code.len() == 8 && code.bytes().all(|b| b.is_ascii_hexdigit()) {
                let n = u32::from_str_radix(code, 16).ok()?;
                return Some(format!("Exception code: 0x{n:08x}."));
            }
        }
    }
    if line.starts_with("qrc:/ui/") {
        for class in ["ReferenceError", "TypeError", "SyntaxError", "RangeError"] {
            if line.contains(&format!(": {class}:")) {
                return Some(format!("QML {class}."));
            }
        }
    }
    None
}
fn clean_log(raw: &str) -> String {
    let mut out = Vec::new();
    let mut omitted = 0;
    for (index, line) in raw.lines().enumerate() {
        if index >= MAX_LINES {
            out.push("[Remaining lines omitted: snapshot limit reached.]".into());
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        match recognized_line(line) {
            Some(line) => out.push(line),
            None => omitted += 1,
        }
    }
    if omitted > 0 {
        out.push(format!("[{omitted} unrecognized lines omitted.]"));
    }
    if out.is_empty() {
        "[No recognized startup messages.]".into()
    } else {
        out.join("\n")
    }
}
fn display_log(log: ReadResult) -> String {
    match log {
        ReadResult::Present(raw) => clean_log(&raw),
        ReadResult::Missing => "[Log not present.]".into(),
        ReadResult::Unavailable => "[Log not read: private file checks failed.]".into(),
        ReadResult::TooLarge => "[Log not read: snapshot size limit exceeded.]".into(),
        ReadResult::Changed => "[Log changed while being read. Open this screen again.]".into(),
        ReadResult::InvalidText => "[Log not read: text was not valid UTF-8.]".into(),
    }
}
/// Capture once. The caller displays and copies this same value without rereading.
pub fn snapshot(data: &Path, summary: &Value) -> String {
    let directory = Directory::open(data);
    let mut summary = summary.clone();
    // Events are read under the same private-file checks, never through an arbitrary symlink.
    summary["events"] = directory
        .as_ref()
        .and_then(|d| match d.read("events.json") {
            ReadResult::Present(raw) => serde_json::from_str::<Value>(&raw)
                .ok()
                .filter(Value::is_array),
            _ => None,
        })
        .unwrap_or_else(|| json!([]));
    let mut out=format!("Deckhand problem report\nNothing is sent automatically. Review this text before copying.\n\nSetup summary\n{}",serde_json::to_string_pretty(&cleaned_summary(&summary)).expect("fixed report JSON"));
    for name in LOGS {
        let log = directory
            .as_ref()
            .map(|d| d.read(name))
            .unwrap_or(ReadResult::Unavailable);
        out.push_str(&format!("\n\n{name}\n{}", display_log(log)));
    }
    out.push('\n');
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p =
                std::env::temp_dir().join(format!("deckhand-report-{}", crate::fsutil::unique()));
            fs::create_dir(&p).unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
            Self(p)
        }
        fn write(&self, name: &str, contents: &[u8]) -> PathBuf {
            let p = self.0.join(name);
            fs::write(&p, contents).unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
            p
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn recognizable_messages_and_codes_readable() {
        let got=clean_log("Deckhand launcher starting.\nStarting Deckhand interface.\nNative interface exited: exit status: 0.\nNative interface exited: exit status: 42.\nNative interface exited: signal: 11 (SIGSEGV) (core dumped).\nPermission denied (os error 13)\nException code: 0xc0000005\n");
        for s in [
            "Deckhand launcher starting.",
            "Starting Deckhand interface.",
            "exit status: 0.",
            "exit status: 42.",
            "signal: 11.",
            "Permission denied (os error 13).",
            "Exception code: 0xc0000005.",
        ] {
            assert!(got.contains(s), "{s}: {got}");
        }
    }
    #[test]
    fn fake_private_and_unlabeled_text_omitted() {
        let private = [
            "/home/example-person/private",
            "example-person",
            "ExampleAccount#1234",
            "person@example.invalid",
            "192.0.2.44",
            "2001:db8::22",
            "example-host.invalid",
            "76561198000000000",
            "UnlabeledAccountCanary",
        ];
        let got = clean_log(&private.join("\n"));
        for secret in private {
            assert!(!got.contains(secret));
        }
        assert!(got.contains("9 unrecognized lines omitted"));
    }
    #[test]
    fn template_dynamic_error_details_never_copied() {
        let got=clean_log("Cannot open the application: /home/example-person/private on example-host.invalid (os error 13)\nRuntime extraction failed: person@example.invalid 192.0.2.44\nqt.qpa.plugin: Could not load the Qt platform plugin \"secret-account\" in \"/home/example-person\"\nqrc:/ui/Main.qml:10: ReferenceError: UnlabeledAccountCanary is not defined\n");
        for s in [
            "Cannot open the application. OS error: 13.",
            "Runtime extraction failed.",
            "Qt could not load its platform plugin.",
            "QML ReferenceError.",
        ] {
            assert!(got.contains(s), "{s}: {got}");
        }
        for secret in [
            "example-person",
            "example-host",
            "person@",
            "192.0.2",
            "secret-account",
            "UnlabeledAccountCanary",
        ] {
            assert!(!got.contains(secret));
        }
    }
    #[test]
    fn unknown_suffix_malformed_codes_and_controls_omitted() {
        let got=clean_log("Deckhand launcher starting. UnlabeledAccountCanary\nNative interface exited: exit status: 76561198000000000.\nNative interface exited: exit status: 0. UnlabeledAccountCanary\nNative interface exited: signal: 11 (UnlabeledAccountCanary).\nException code: 0xc0000005 UnlabeledAccountCanary\n\u{1b}[31mDeckhand launcher starting.\n");
        assert!(!got.contains("UnlabeledAccountCanary"));
        assert!(!got.contains("exit status:"));
        assert!(!got.contains("signal:"));
        assert!(got.contains("unrecognized lines omitted"));
    }
    #[test]
    fn summary_and_events_only_known_fields_values() {
        let raw = json!({"version":"UnlabeledAccountCanary","os":"example-host.invalid","architecture":"person@example.invalid","native_steam_count":2,"launcher_count":76561198000000000u64,"game_editions":["retail","UnlabeledAccountCanary","era"],"pending_transaction":true,"pending_cleanup":"/home/example-person","secret":"UnlabeledAccountCanary","events":[{"version":"0.2.0-alpha.10-candidate.50","operation":"review_setup","result":"completed","secret":"UnlabeledAccountCanary"},{"version":"person@example.invalid","operation":"review_setup","result":"completed"},{"version":"0.2.0-alpha.10-candidate.50","operation":"UnlabeledAccountCanary","result":"completed"}]});
        let got = cleaned_summary(&raw).to_string();
        for secret in [
            "UnlabeledAccountCanary",
            "example-host",
            "person@",
            "/home/",
            "76561198000000000",
        ] {
            assert!(!got.contains(secret));
        }
        for s in ["review_setup", "completed", "retail", "native_steam_count"] {
            assert!(got.contains(s));
        }
    }
    #[test]
    fn snapshot_combines_both_once_without_changes() {
        let f = Fixture::new();
        let current = f.write(
            "startup.log",
            b"Deckhand launcher starting.\nUnlabeledAccountCanary\n",
        );
        let previous = f.write(
            "startup.previous.log",
            b"Native interface exited: exit status: 0.\n",
        );
        let a = fs::read(&current).unwrap();
        let b = fs::read(&previous).unwrap();
        let text = snapshot(&f.0, &json!({}));
        assert!(text.contains("startup.log\nDeckhand launcher starting."));
        assert!(text.contains("startup.previous.log\nNative interface exited: exit status: 0."));
        assert!(!text.contains("UnlabeledAccountCanary"));
        assert_eq!(fs::read(current).unwrap(), a);
        assert_eq!(fs::read(previous).unwrap(), b);
        f.write("startup.log", b"Verified update restart requested.\n");
        assert!(!text.contains("Verified update restart requested."));
        assert!(snapshot(&f.0, &json!({})).contains("Verified update restart requested."));
    }
    #[test]
    fn linked_log_target_not_read_or_changed() {
        let f = Fixture::new();
        let target = f.write("unrelated", b"UnlabeledAccountCanary");
        symlink(&target, f.0.join("startup.log")).unwrap();
        let text = snapshot(&f.0, &json!({}));
        assert!(text.contains("private file checks failed"));
        assert!(!text.contains("UnlabeledAccountCanary"));
        assert_eq!(fs::read(target).unwrap(), b"UnlabeledAccountCanary");
    }
    #[test]
    fn public_hardlinked_nonregular_logs_refused() {
        let f = Fixture::new();
        let current = f.write("startup.log", b"Deckhand launcher starting.\n");
        fs::set_permissions(&current, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(snapshot(&f.0, &json!({})).contains("private file checks failed"));
        fs::set_permissions(&current, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&current, f.0.join("other-link")).unwrap();
        assert!(snapshot(&f.0, &json!({})).contains("private file checks failed"));
        fs::remove_file(&current).unwrap();
        fs::create_dir(&current).unwrap();
        assert!(snapshot(&f.0, &json!({})).contains("private file checks failed"));
    }
    #[test]
    fn missing_oversized_invalid_control_text_bounded() {
        let f = Fixture::new();
        assert!(snapshot(&f.0, &json!({})).contains("Log not present"));
        let file = f.write("startup.log", &vec![b'a'; MAX_BYTES + 1]);
        assert!(snapshot(&f.0, &json!({})).contains("size limit exceeded"));
        fs::write(&file, [0xff, 0xfe]).unwrap();
        assert!(snapshot(&f.0, &json!({})).contains("not valid UTF-8"));
        fs::write(
            &file,
            b"Deckhand launcher starting.\0UnlabeledAccountCanary",
        )
        .unwrap();
        let text = snapshot(&f.0, &json!({}));
        assert!(!text.contains("UnlabeledAccountCanary"));
        assert!(text.contains("unrecognized lines omitted"));
    }
    #[test]
    fn final_data_alias_refused_parent_alias_supported() {
        let f = Fixture::new();
        let data = f.0.join("data");
        fs::create_dir(&data).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).unwrap();
        let log = data.join("startup.log");
        fs::write(&log, b"Deckhand launcher starting.\n").unwrap();
        fs::set_permissions(&log, fs::Permissions::from_mode(0o600)).unwrap();
        let direct = f.0.join("data-alias");
        symlink(&data, &direct).unwrap();
        assert!(snapshot(&direct, &json!({})).contains("private file checks failed"));
        let parent = f.0.join("parent-alias");
        symlink(&f.0, &parent).unwrap();
        assert!(snapshot(&parent.join("data"), &json!({}))
            .contains("startup.log\nDeckhand launcher starting."));
    }
    #[test]
    fn line_length_and_count_limits() {
        let long = format!(
            "Cannot open the application: {}",
            "UnlabeledAccountCanary".repeat(MAX_LINE)
        );
        assert!(!clean_log(&long).contains("UnlabeledAccountCanary"));
        let got = clean_log(&"Deckhand launcher starting.\n".repeat(MAX_LINES + 1));
        assert!(got.contains("snapshot limit reached"));
        assert_eq!(
            got.matches("Deckhand launcher starting.").count(),
            MAX_LINES
        );
    }
    #[test]
    fn event_file_is_private_and_strictly_filtered() {
        let f = Fixture::new();
        f.write("events.json",br#"[{"version":"0.2.0-alpha.10-candidate.51","operation":"review_setup","result":"completed","message":"UnlabeledAccountCanary"}]"#);
        let got = snapshot(&f.0, &json!({}));
        assert!(got.contains("review_setup"));
        assert!(!got.contains("UnlabeledAccountCanary"));
    }
    #[test]
    fn event_symlink_is_not_followed() {
        let f = Fixture::new();
        let target=f.write("unrelated",br#"[{"version":"0.2.0-alpha.10-candidate.51","operation":"review_setup","result":"completed"}]"#);
        symlink(target, f.0.join("events.json")).unwrap();
        assert!(!snapshot(&f.0, &json!({})).contains("review_setup"));
    }
}
