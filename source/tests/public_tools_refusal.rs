#![cfg(not(feature = "developer-tools"))]
use serde_json::json;
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString, OsString},
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
use wow_steamos_core::{addons, engine, fsutil, wow_developer_tools, wow_free, wow_request};
struct Fixture {
    root: PathBuf,
    home: Option<OsString>,
    data: Option<OsString>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("wow-public-tools-{}", fsutil::unique()));
        fs::create_dir(&root).unwrap();
        let home = std::env::var_os("HOME");
        let data = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("HOME", root.join("home"));
        std::env::set_var("XDG_DATA_HOME", root.join("data"));
        Self { root, home, data }
    }
    fn file(&self, relative: &str, bytes: &[u8]) {
        let p = self.root.join(relative);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for (k, v) in [("HOME", &self.home), ("XDG_DATA_HOME", &self.data)] {
            if let Some(v) = v {
                std::env::set_var(k, v)
            } else {
                std::env::remove_var(k)
            }
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
type Snapshot = BTreeMap<PathBuf, (u64, u64, u32, Vec<u8>)>;
fn snapshot(root: &Path) -> Snapshot {
    fn walk(root: &Path, at: &Path, out: &mut Snapshot) {
        for item in fs::read_dir(at).unwrap() {
            let p = item.unwrap().path();
            let m = fs::symlink_metadata(&p).unwrap();
            assert!(!m.file_type().is_symlink());
            let b = if m.is_file() {
                fs::read(&p).unwrap()
            } else {
                Vec::new()
            };
            out.insert(
                p.strip_prefix(root).unwrap().to_path_buf(),
                (m.dev(), m.ino(), m.permissions().mode(), b),
            );
            if m.is_dir() {
                walk(root, &p, out)
            }
        }
    }
    let mut result = Snapshot::new();
    walk(root, root, &mut result);
    result
}
fn refuse_all(f: &Fixture) {
    for action in [
        "device_run",
        "device_result",
        "device_mark",
        "device_future",
        "live_prepare",
        "live_result",
        "live_step",
        "live_retry_launch",
        "live_stop",
        "live_future",
        "addon_registration_review",
        "addon_registration_apply",
        "addon_registration_result",
        "addon_registration_future",
        "addon_lifecycle_review",
        "addon_lifecycle_apply",
        "addon_lifecycle_result",
        "addon_lifecycle_cancel",
        "addon_lifecycle_future",
        "controller_review",
        "controller_apply",
        "controller_status",
        "controller_future",
        "review_addon",
        "prepare_gear",
        "download_addon",
        "mark_tested",
    ] {
        let req = json!({"action":action,"id":"stale","token":"stale","confirm":true,
            "auth_complete":true,"manager":"curseforge","url":"https://example.invalid/ignored"});
        let error = engine::dispatch(req.clone()).unwrap_err();
        assert!(
            error.contains("not included in this public build"),
            "{action}: {error}"
        );
        let input = CString::new(req.to_string()).unwrap();
        let reply = unsafe {
            let raw = wow_request(input.as_ptr());
            assert!(!raw.is_null());
            let text = CStr::from_ptr(raw).to_string_lossy().into_owned();
            wow_free(raw);
            serde_json::from_str::<serde_json::Value>(&text).unwrap()
        };
        assert_eq!(reply["ok"], false, "{action}: {reply}");
        assert!(reply["error"]
            .as_str()
            .unwrap()
            .contains("not included in this public build"));
    }
    assert!(
        addons::download(&f.root.join("no-downloads"), "curseforge", None)
            .unwrap_err()
            .contains("not included in this public build")
    );
}
#[test]
fn public_production_library_refuses_tools_before_state_or_events() {
    let f = Fixture::new();
    assert_eq!(wow_developer_tools(), 0);
    let initial = snapshot(&f.root);
    refuse_all(&f);
    assert_eq!(snapshot(&f.root), initial);
    assert!(engine::dispatch(json!({"action":"catalog"})).is_ok());
    assert!(engine::dispatch(json!({"action":"demo"})).is_ok());
    assert_eq!(snapshot(&f.root), initial);
    for n in [
        "state.json",
        "controller-job.json",
        "controller-result.json",
        "controller-permission",
        "addon-lifecycle-job.json",
        "live-test.json",
        "device-tests.json",
    ] {
        f.file(
            &format!("data/wow-steamos-rust/{n}"),
            format!("preserved historical {n}; intentionally not JSON").as_bytes(),
        );
    }
    f.file(
        "data/wow-steamos-rust/controller-backups/verified/original.bin",
        b"verified original backup",
    );
    f.file(
        "home/Steam/controller-configs/custom.vdf",
        b"custom shared controller layout",
    );
    let preserved = snapshot(&f.root);
    refuse_all(&f);
    assert_eq!(snapshot(&f.root), preserved);
    assert!(addons::validate_appimage_input("").is_err());
    let mut appimage = vec![0; 1025];
    appimage[..6].copy_from_slice(b"\x7fELF\x02\x01");
    appimage[8..11].copy_from_slice(b"AI\x02");
    appimage[18..20].copy_from_slice(&[62, 0]);
    assert!(addons::is_appimage(&appimage));
    assert_eq!(snapshot(&f.root), preserved);
}
