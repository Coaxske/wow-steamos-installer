use std::{collections::HashSet, fs, path::PathBuf};
use wow_steamos_core::{
    catalog, discovery, fsutil,
    model::Scan,
    steam,
    transaction::{self, Change, Journal},
    vdf::{self, Value},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("wow-rust-test-{}", fsutil::unique()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self, s: &str) -> PathBuf {
        self.0.join(s)
    }
    fn file(&self, s: &str, b: &[u8]) -> PathBuf {
        let p = self.path(s);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, b).unwrap();
        p
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn binary_roundtrip_preserves_unknown_fields() {
    let o = vec![(
        "root".into(),
        Value::Object(vec![
            ("future".into(), Value::U64([255; 8])),
            ("float".into(), Value::Float([1, 2, 3, 4])),
            ("dupe".into(), Value::Text("first".into())),
            ("dupe".into(), Value::Text("second".into())),
        ]),
    )];
    let b = vdf::binary(&o).unwrap();
    assert_eq!(vdf::parse_binary(&b).unwrap(), o);
    assert_eq!(vdf::binary(&vdf::parse_binary(&b).unwrap()).unwrap(), b);
}
#[test]
fn truncated_binary_is_rejected() {
    let b = vdf::binary(&vec![("a".into(), Value::Text("b".into()))]).unwrap();
    for n in 0..b.len() {
        assert!(vdf::parse_binary(&b[..n]).is_err());
    }
}
#[test]
fn trailing_binary_rejected() {
    assert!(vdf::parse_binary(&[8, 0]).is_err());
}
#[test]
fn unknown_binary_tag_rejected() {
    assert!(vdf::parse_binary(&[99, 0, 8]).is_err());
}
#[test]
fn nul_in_binary_rejected() {
    assert!(vdf::binary(&vec![("bad\0key".into(), Value::Text("value".into()))]).is_err());
}
#[test]
fn text_vdf_roundtrips_unicode_and_escapes() {
    let s = "// comment\n\"root\" { \"path\" \"Z:\\\\games\\\\é\" \"say\" \"a\\\"b\" }";
    let o = vdf::parse_text(s).unwrap();
    assert_eq!(vdf::parse_text(&vdf::text(&o).unwrap()).unwrap(), o);
}
#[test]
fn malformed_text_is_rejected() {
    for s in ["\"unclosed", "a { b c", "a", "}", "a b [windows]"] {
        assert!(vdf::parse_text(s).is_err(), "{s}");
    }
}
#[test]
fn deep_binary_rejected() {
    let mut b = vec![];
    for _ in 0..70 {
        b.extend([0, b'a', 0]);
    }
    b.extend(vec![8; 71]);
    assert!(vdf::parse_binary(&b).is_err());
}
#[test]
fn crc_matches_standard_check_vector() {
    assert_eq!(steam::crc32(b"123456789"), 0xcbf43926);
}
#[test]
fn shortcut_game_id_is_64_bit() {
    let id = 0x81234567;
    assert_eq!(steam::game_id(id), 0x8123456702000000);
}
#[test]
fn proton_mapping_preserves_unrelated_config() {
    let s=b"\"InstallConfigStore\" { \"Software\" { \"Valve\" { \"Steam\" { \"unrelated\" \"keep\" \"CompatToolMapping\" { \"10\" { \"name\" \"old\" } } } } } }";
    let updated = steam::set_proton(Some(s), &[20], "proton_11").unwrap();
    assert_eq!(steam::proton_for(&updated, 10).unwrap(), Some("old".into()));
    assert_eq!(
        steam::proton_for(&updated, 20).unwrap(),
        Some("proton_11".into())
    );
    assert!(String::from_utf8(updated).unwrap().contains("unrelated"));
}
#[test]
fn invalid_proton_is_rejected() {
    assert!(steam::set_proton(Some(b"a b"), &[1], "x\nmalicious").is_err());
}
#[test]
fn product_parser_only_extracts_a_code() {
    assert_eq!(
        steam::product("x %command% --exec=\"launch WoWF\""),
        Some("WoWF".into())
    );
    assert_eq!(steam::product("--exec=\"launch WoWF;rm\""), None);
    assert_eq!(steam::product("launch WoWF"), None);
}
#[test]
fn forever_uses_defined_route_but_beta_does_not() {
    assert_eq!(catalog::by_id("forever").unwrap().command, Some("WoWF"));
    assert!(catalog::by_id("classic_beta").unwrap().command.is_none());
    assert!(catalog::by_id("classic_beta").unwrap().command.is_none());
}
#[test]
fn era_modes_share_one_client() {
    let e = catalog::by_folder("_classic_era_").unwrap();
    assert_eq!(e.id, "era");
    assert!(e.note.contains("Hardcore"));
}
#[test]
fn support_executables_are_excluded() {
    for n in [
        "WowUp.exe",
        "WowVoiceProxy.exe",
        "Battle.net.exe",
        "Utils.exe",
        "WowError.exe",
    ] {
        assert!(!catalog::is_game_exe(n));
    }
    assert!(catalog::is_game_exe("WowClassic.exe"));
}
#[test]
fn scan_finds_only_valid_clients() {
    let t = Temp::new();
    t.file("World of Warcraft/_retail_/Wow.exe", b"MZ");
    t.file("World of Warcraft/_classic_era_/WowClassic.exe", b"MZ");
    t.file("World of Warcraft/Utils/Wow.exe", b"MZ");
    t.file("World of Warcraft/_forever_/WowUp.exe", b"MZ");
    let mut s = Scan::default();
    discovery::walk(&t.0, &mut s, &mut HashSet::new(), &mut 1000).unwrap();
    assert_eq!(s.games.len(), 2);
}
#[test]
fn unknown_client_requires_build_info() {
    let t = Temp::new();
    t.file("WoW/_future_/Wow.exe", b"MZ");
    let mut s = Scan::default();
    discovery::walk(&t.0, &mut s, &mut HashSet::new(), &mut 100).unwrap();
    assert!(s.games.is_empty());
    t.file(
        "WoW/.build.info",
        b"Product!STRING:0|Active!DEC:1\nwow_future|1",
    );
    discovery::walk(&t.0, &mut s, &mut HashSet::new(), &mut 100).unwrap();
    assert!(s.games.is_empty());
    t.file(
        "WoW/_future_/.build.info",
        b"Product!STRING:0|Active!DEC:1\nwow_future|1",
    );
    discovery::walk(&t.0, &mut s, &mut HashSet::new(), &mut 100).unwrap();
    assert_eq!(s.games[0].id, "other");
}
#[test]
fn scan_ignores_symlink_cycles() {
    let t = Temp::new();
    std::os::unix::fs::symlink(&t.0, t.path("loop")).unwrap();
    let mut s = Scan::default();
    discovery::walk(&t.0, &mut s, &mut HashSet::new(), &mut 100).unwrap();
    assert!(s.games.is_empty());
}
#[test]
fn scan_reports_budget_limit() {
    let t = Temp::new();
    let mut s = Scan::default();
    discovery::walk(&t.0, &mut s, &mut HashSet::new(), &mut 0).unwrap();
    assert!(!s.warnings.is_empty());
}
#[test]
fn launcher_is_associated_with_its_prefix() {
    let t = Temp::new();
    let path = t.file(
        "compatdata/42/pfx/drive_c/Program Files (x86)/Battle.net/Battle.net.exe",
        b"MZ",
    );
    assert_eq!(discovery::prefix_of(&path), Some(t.path("compatdata/42")));
    let mut s = Scan::default();
    discovery::walk(&t.0, &mut s, &mut HashSet::new(), &mut 100).unwrap();
    assert_eq!(s.launchers.len(), 1);
}
#[test]
fn shortcut_repair_is_idempotent_and_preserves_artwork() {
    let t = Temp::new();
    let exe = t.path("Battle.net.exe");
    let mut o = vec![("shortcuts".into(), Value::Object(vec![]))];
    let mut e = steam::entry(
        "World of Warcraft",
        &exe,
        &steam::launch_options(&t.path("prefix"), None).unwrap(),
        "owned",
        Some(300),
    )
    .unwrap();
    vdf::set(&mut e, "icon", Value::Text("custom.png".into()));
    steam::upsert(&mut o, e, "owned").unwrap();
    let mut replacement = steam::entry(
        "World of Warcraft",
        &exe,
        &steam::launch_options(&t.path("prefix"), Some("WoW")).unwrap(),
        "owned",
        None,
    )
    .unwrap();
    replacement.retain(|(k, _)| k != "icon");
    steam::upsert(&mut o, replacement.clone(), "owned").unwrap();
    let before = vdf::binary(&o).unwrap();
    steam::upsert(&mut o, replacement, "owned").unwrap();
    assert_eq!(vdf::binary(&o).unwrap(), before);
    let rows = vdf::object(&o, "shortcuts").unwrap();
    if let Value::Object(e) = &rows[0].1 {
        assert_eq!(vdf::number(e, "appid"), Some(300));
        assert_eq!(vdf::string(e, "icon"), Some("custom.png"));
    } else {
        panic!()
    }
}
#[test]
fn repair_preserves_other_installations() {
    let t = Temp::new();
    let exe = t.path("Battle.net.exe");
    let mut o = vec![("shortcuts".into(), Value::Object(vec![]))];
    steam::upsert(
        &mut o,
        steam::entry("Other game", &exe, "", "unrelated", Some(44)).unwrap(),
        "unrelated",
    )
    .unwrap();
    steam::upsert(
        &mut o,
        steam::entry("World of Warcraft", &exe, "", "owned", Some(55)).unwrap(),
        "owned",
    )
    .unwrap();
    assert_eq!(vdf::object(&o, "shortcuts").unwrap().len(), 2);
}
#[test]
fn shortcut_id_collision_is_rejected() {
    let t = Temp::new();
    let mut o = vec![("shortcuts".into(), Value::Object(vec![]))];
    steam::upsert(
        &mut o,
        steam::entry("A", &t.path("a"), "", "a", Some(7)).unwrap(),
        "a",
    )
    .unwrap();
    assert!(steam::upsert(
        &mut o,
        steam::entry("B", &t.path("b"), "", "b", Some(7)).unwrap(),
        "b"
    )
    .is_err());
}
#[test]
fn profile_path_traversal_is_blocked() {
    assert!(steam::shortcuts_path(std::path::Path::new("/tmp"), "../../x").is_err());
    assert!(steam::shortcuts_path(std::path::Path::new("/tmp"), "0").is_err());
}
#[test]
fn path_quoting_handles_shell_metacharacters() {
    let p = PathBuf::from("/tmp/a b'c$(touch x)`y`");
    let q = fsutil::shell_quote(&p).unwrap();
    assert_eq!(q, "'/tmp/a b'\\''c$(touch x)`y`'");
    assert!(fsutil::steam_quote(std::path::Path::new("/tmp/\"x")).is_err());
}
#[test]
fn control_characters_in_paths_are_rejected() {
    assert!(fsutil::safe_path(std::path::Path::new("/tmp/line\nother")).is_err());
}
#[test]
fn transaction_commits_all_files_and_saves_backup() {
    let t = Temp::new();
    let a = t.file("a", b"old");
    let b = t.file("b", b"other");
    let mut j = Journal {
        id: "job".into(),
        label: "repair".into(),
        changes: vec![
            Change::new(a.clone(), b"new".to_vec()).unwrap(),
            Change::new(b.clone(), b"new2".to_vec()).unwrap(),
        ],
        done: false,
    };
    transaction::commit(&t.0, &mut j, false).unwrap();
    assert_eq!(fs::read(a).unwrap(), b"new");
    assert_eq!(fs::read(b).unwrap(), b"new2");
    assert!(t.path("backups/job.json").exists());
    assert!(!t.path("transaction.json").exists());
}
#[test]
fn review_becomes_stale_after_external_change() {
    let t = Temp::new();
    let a = t.file("a", b"old");
    let c = Change::new(a.clone(), b"new".to_vec()).unwrap();
    fs::write(&a, b"external").unwrap();
    let mut j = Journal {
        id: "job".into(),
        label: "repair".into(),
        changes: vec![c],
        done: false,
    };
    assert!(transaction::commit(&t.0, &mut j, false).is_err());
    assert_eq!(fs::read(a).unwrap(), b"external");
}
#[test]
fn interrupted_transaction_recovers_idempotently() {
    let t = Temp::new();
    let a = t.file("a", b"old");
    let b = t.file("b", b"other");
    let j = Journal {
        id: "job".into(),
        label: "repair".into(),
        changes: vec![
            Change::new(a.clone(), b"new".to_vec()).unwrap(),
            Change::new(b.clone(), b"new2".to_vec()).unwrap(),
        ],
        done: false,
    };
    fsutil::json(&t.path("transaction.json"), &j).unwrap();
    fs::write(&a, b"new").unwrap();
    transaction::recover(&t.0, false).unwrap();
    assert_eq!(fs::read(b).unwrap(), b"new2");
    assert!(!t.path("transaction.json").exists());
}
#[test]
fn recovery_refuses_external_conflicts() {
    let t = Temp::new();
    let a = t.file("a", b"old");
    let j = Journal {
        id: "job".into(),
        label: "repair".into(),
        changes: vec![Change::new(a.clone(), b"new".to_vec()).unwrap()],
        done: false,
    };
    fsutil::json(&t.path("transaction.json"), &j).unwrap();
    fs::write(&a, b"external").unwrap();
    assert!(transaction::recover(&t.0, false).is_err());
}
#[test]
fn undo_preserves_later_user_changes() {
    let t = Temp::new();
    let a = t.file("a", b"old");
    let mut j = Journal {
        id: "job".into(),
        label: "repair".into(),
        changes: vec![Change::new(a.clone(), b"new".to_vec()).unwrap()],
        done: false,
    };
    transaction::commit(&t.0, &mut j, false).unwrap();
    fs::write(a, b"later").unwrap();
    assert!(transaction::undo(&t.0, false).is_err());
}
#[test]
fn undo_never_resurrects_removed_game_shortcuts() {
    let t = Temp::new();
    let a = t.file("a", b"old");
    let mut j = Journal {
        id: "job".into(),
        label: "reinstall: removal".into(),
        changes: vec![Change::new(a, b"new".to_vec()).unwrap()],
        done: false,
    };
    transaction::commit(&t.0, &mut j, false).unwrap();
    assert!(transaction::undo(&t.0, false).is_err());
}
#[test]
fn removal_rejects_symlinks_inside_tree() {
    let t = Temp::new();
    let d = t.path("game");
    fs::create_dir(&d).unwrap();
    std::os::unix::fs::symlink("/tmp", d.join("linked")).unwrap();
    assert!(fsutil::check_tree(&d).is_err());
}
#[test]
fn atomic_write_does_not_follow_destination_symlink() {
    let t = Temp::new();
    let outside = t.file("outside", b"keep");
    std::os::unix::fs::symlink(&outside, t.path("link")).unwrap();
    assert!(fsutil::atomic(&t.path("link"), b"erase").is_err());
    assert_eq!(fs::read(outside).unwrap(), b"keep");
}
#[test]
fn lock_prevents_concurrent_writers() {
    let t = Temp::new();
    let _lock = fsutil::Lock::acquire(&t.path("lock")).unwrap();
    assert!(fsutil::Lock::acquire(&t.path("lock")).is_err());
}
#[test]
fn inode_change_is_detected() {
    let t = Temp::new();
    let p = t.path("game");
    fs::create_dir(&p).unwrap();
    let old = fsutil::identity(&p).unwrap();
    fs::rename(&p, t.path("old")).unwrap();
    fs::create_dir(&p).unwrap();
    assert_ne!(old, fsutil::identity(&p).unwrap());
}
#[test]
fn pure_catalog_does_not_need_user_state() {
    let result =
        wow_steamos_core::engine::dispatch(serde_json::json!({"action":"catalog"})).unwrap();
    assert!(result["catalog"].as_array().unwrap().len() >= 12);
}
#[test]
fn known_legacy_title_cleanup_does_not_guess_modes() {
    assert_eq!(
        steam::canonical_title("WoW Forever [WoW SteamOS Installer]"),
        "World of Warcraft Forever"
    );
    assert_eq!(
        steam::canonical_title("World of Warcraft (Retail)"),
        "World of Warcraft"
    );
    assert_eq!(steam::canonical_title("WoW Classic"), "WoW Classic");
}

#[test]
fn explicit_scan_root_revisits_a_previous_depth_boundary() {
    let t = Temp::new();
    let mut boundary = t.0.clone();
    for _ in 0..11 {
        boundary = boundary.join("nested");
    }
    let game = boundary.join("_retail_");
    fs::create_dir_all(&game).unwrap();
    fs::write(game.join("Wow.exe"), b"MZ").unwrap();
    let mut scan = Scan::default();
    let mut seen = HashSet::new();
    let mut budget = 1000;
    discovery::walk(&t.0, &mut scan, &mut seen, &mut budget).unwrap();
    assert!(scan.games.is_empty());
    discovery::walk(&boundary, &mut scan, &mut seen, &mut budget).unwrap();
    assert_eq!(scan.games.len(), 1);
}

#[test]
fn zero_byte_game_placeholder_is_not_discovered_as_installed() {
    let t = Temp::new();
    let exe = t.file("World of Warcraft/_classic_era_/WoWClassic.exe", b"");
    let mut scan = wow_steamos_core::model::Scan::default();
    discovery::walk(&t.0, &mut scan, &mut HashSet::new(), &mut 1000).unwrap();
    assert!(scan.games.is_empty());
    fs::write(exe, b"MZ").unwrap();
    let mut scan = wow_steamos_core::model::Scan::default();
    discovery::walk(&t.0, &mut scan, &mut HashSet::new(), &mut 1000).unwrap();
    assert_eq!(scan.games.len(), 1);
}
