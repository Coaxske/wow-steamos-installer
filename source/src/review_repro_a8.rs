// Alpha8 reviewer probes retained with regression assertions added for alpha9.
// Original attachment is unchanged; this working copy adds assertions.
// To run: copy into src/, add  include!("review_repro_a8.rs");  at the end of `mod workflow_tests`
// in src/engine.rs, then: cargo test --offline --lib -- a8_probe repro_ --nocapture --test-threads=1
// a8_probe_3 to a8_probe_6 show the remaining problems; the repro_ tests now show fixed behavior.

// alpha8 follow-up probes. Included inside engine::workflow_tests (uses the project's Fixture).

fn q8(p: &std::path::Path) -> String {
    format!("\"{}\"", p.display())
}
fn tile8(id: u32, name: &str, exe: &std::path::Path, o: String) -> Value {
    Value::Object(vec![
        ("appid".into(), Value::I32(id)),
        ("AppName".into(), Value::Text(name.into())),
        ("Exe".into(), Value::Text(q8(exe))),
        ("StartDir".into(), Value::Text(q8(exe.parent().unwrap()))),
        ("LaunchOptions".into(), Value::Text(o)),
        ("tags".into(), Value::Object(vec![])),
    ])
}
fn steam_rewrite8(f: &Fixture) {
    let cfg = f.ctx.home.join(".local/share/Steam/config/config.vdf");
    let s = fs::read_to_string(&cfg).unwrap();
    fs::write(&cfg, s.replace("\"\t\"", "\"\t\t\"")).unwrap();
}
fn appimage8(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut b = vec![0u8; 4096];
    b[..6].copy_from_slice(b"\x7fELF\x02\x01");
    b[8..11].copy_from_slice(b"AI\x02");
    b[18] = 62;
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a8_probe_1_proton_labels() {
    let f = Fixture::new();
    let common = f.ctx.home.join(".local/share/Steam/steamapps/common");
    for (dir, ver) in [("Proton 11.0", "1760000000 proton-11.0-2e"), ("Proton 10.0", "1750000000 proton-10.0-4b"),
                       ("Proton - Experimental", "1760000000 experimental-11.0-20261001"), ("Proton 7.0", "1690000000 proton-7.0-6")] {
        let d = common.join(dir);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("proton"), b"x").unwrap();
        fs::write(d.join("toolmanifest.vdf"), b"\"manifest\" { \"commandline\" \"/proton %verb%\" }").unwrap();
        fs::write(d.join("version"), ver).unwrap();
    }
    let info = discovery::steam_info(&fsutil::canonical(&f.ctx.home.join(".local/share/Steam")).unwrap()).unwrap();
    assert!(info.protons.iter().any(|p| p.name == "proton_7"));
    println!("A8P1 detected: {:?}", info.protons.iter().map(|p| format!("{} <- {}", p.name, p.label)).collect::<Vec<_>>());
}

#[test]
fn a8_probe_2_removal_and_lockout_scenarios() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    for extra in ["pfx/drive_c/vrclient/bin", "pfx/drive_c/openxr"] { fs::create_dir_all(i.prefix.join(extra)).unwrap(); }
    println!("A8P2 removal review on Proton-shaped prefix: {}", review_clean(&f.ctx, &json!({"id":i.id})).is_ok());
    // post-commit interruption + Steam rewrite
    review_reinstall(&f.ctx, &json!({"id":i.id,"location":f.ctx.home.join("Games"),"editions":["retail"],"keep_settings":false})).unwrap();
    let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let new = plan.replacement.clone().unwrap();
    fs::create_dir_all(&new.root).unwrap();
    fs::write(new.root.join("Battle.net-Setup.exe"), test_installer()).unwrap();
    let mut job = Cleanup { plan, settings_saved: false };
    fsutil::json(&f.ctx.data.join("cleanup.json"), &job).unwrap();
    download(&f.ctx, job.plan.replacement.as_ref().unwrap()).unwrap();
    transaction::finish(&f.ctx.data, &mut job.plan.journal, false).unwrap();
    fsutil::json(&f.ctx.data.join("cleanup.json"), &job).unwrap();
    steam_rewrite8(&f);
    let mut job: Cleanup = fsutil::load(&f.ctx.data.join("cleanup.json")).unwrap();
    println!("A8P2 resume after commit + Steam rewrite: {:?} | old game removed: {}",
        run_cleanup(&f.ctx, &mut job).map(|_| "ok"), !i.root.join("games/World of Warcraft").exists());
}

#[test]
fn a8_probe_3_undo_after_benign_local_change() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    steam_rewrite8(&f);
    // Something ordinary that saves state.json: recording a successful test.
    let r = dispatch_mark(&mut f, &i);
    println!("A8P3 mark_tested: {r}");
    println!("A8P3 undo after Steam rewrite + test record: {:?}", transaction::undo(&f.ctx.data, false).unwrap());
}
fn dispatch_mark(f: &mut Fixture, i: &Installation) -> String {
    let key = format!("{}:{}", i.id, "World of Warcraft");
    f.ctx.state.tests.push(key);
    f.ctx.save().map(|_| "saved state.json".to_string()).unwrap_or_else(|e| e)
}

#[test]
fn a8_probe_4_addon_tile_repointed_to_new_appimage() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let first = f.ctx.home.join("Applications/WowUp-2.20.0.AppImage");
    appimage8(&first);
    let p = review_addon(&f.ctx, &json!({"id":i.id,"path":first,"manager":"wowup_wago"})).unwrap();
    apply(&f.ctx, &json!({"token":p["token"]})).unwrap();
    f.refresh();
    let second = f.ctx.home.join("Applications/WowUp-2.21.0.AppImage");
    appimage8(&second);
    println!("A8P4 point the WowUp tile at an updated AppImage: {:?}",
        review_addon(&f.ctx, &json!({"id":i.id,"path":second,"manager":"wowup_wago"})).map(|_| "ok").unwrap());
}

#[test]
fn a8_probe_5_adopt_steam_created_battlenet_tile() {
    // Common manual setup: Battle.net added through Steam's "Add a Non-Steam Game",
    // Proton forced in Properties, no launch options, Steam's own compatdata prefix.
    let mut f = Fixture::new();
    let home = f.ctx.home.clone();
    let steam = home.join(".local/share/Steam");
    let prefix = steam.join("steamapps/compatdata/2900000001");
    let bn = prefix.join("pfx/drive_c/Program Files (x86)/Battle.net");
    fs::create_dir_all(&bn).unwrap();
    for exe in ["Battle.net.exe", "Battle.net Launcher.exe"] { fs::write(bn.join(exe), b"MZ").unwrap(); }
    fs::create_dir_all(prefix.join("pfx/drive_c/Program Files (x86)/Steam")).unwrap();
    let game = prefix.join("pfx/drive_c/Program Files (x86)/World of Warcraft/_retail_");
    fs::create_dir_all(&game).unwrap();
    fs::write(game.join("Wow.exe"), b"MZ").unwrap();
    let opts = format!("STEAM_COMPAT_DATA_PATH={} %command%", q8(&prefix));
    let tiles = vec![("shortcuts".to_string(), Value::Object(vec![
        ("0".into(), tile8(0xACDC_0001, "Battle.net", &bn.join("Battle.net Launcher.exe"), String::new())),
        ("1".into(), tile8(0xACDC_0002, "World of Warcraft", &bn.join("Battle.net.exe"), format!("{opts} --exec=\"launch WoW\""))),
    ]))];
    let sc = steam.join("userdata/123/config/shortcuts.vdf");
    fs::write(sc.clone(), vdf::binary(&tiles).unwrap()).unwrap();
    let cfg = steam.join("config/config.vdf");
    fs::write(&cfg, steam::set_proton(Some(&fs::read(&cfg).unwrap()), &[0xACDC_0001, 0xACDC_0002], "proton_11").unwrap()).unwrap();
    let data = adoption_candidates(&f.ctx, &json!({"prefix":prefix})).unwrap();
    let all: Vec<u64> = data["candidates"].as_array().unwrap().iter().map(|x| x["appid"].as_u64().unwrap()).collect();
    println!("A8P5 candidates: {:?}", data["candidates"].as_array().unwrap().iter().map(|c| c["name"].clone()).collect::<Vec<_>>());
    let c = &data["candidates"][0];
    let p = review_adopt(&f.ctx, &json!({"prefix":prefix,"steam":c["steam"],"user":c["user"],"appid":c["appid"],"selected":all})).unwrap();
    apply(&f.ctx, &json!({"token":p["token"]})).unwrap();
    f.refresh();
    let adopted=f.ctx.state.installations[0].clone();f.confirm_fixture_exit(&adopted);
    let id = adopted.id.clone();
    println!("A8P5 first repair after adoption: {:?}", review_repair(&f.ctx, &json!({"id":id,"allow_missing":true,"shared_tile_appid":0xACDC0002u32})).map(|_| "ok").unwrap());
}

#[test]
fn a8_probe_6_custom_options_without_command() {
    // A user who replaced the options with a wrapper that has no %command% (or cleared them).
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let mut root = steam::read_shortcuts(&path).unwrap();
    for (_, v) in vdf::ensure(&mut root, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            if vdf::string(e, "appname") == Some("World of Warcraft") {
                vdf::set(e, "LaunchOptions", Value::Text(String::new()));
            }
        }
    }
    fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
    println!("A8P6 repair after the Retail tile's options were cleared in Steam: {:?}",
        review_repair(&f.ctx, &json!({"id":i.id})).map(|_| "ok").unwrap());
}

fn steam_rewrites_config(f: &Fixture) {
    let cfg = f.ctx.home.join(".local/share/Steam/config/config.vdf");
    let s = fs::read_to_string(&cfg).unwrap();
    fs::write(&cfg, s.replace("\"\t\"", "\"\t\t\"")).unwrap();
}
#[test]
fn repro_6_renamed_tile_after_steam_drops_custom_keys() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    // Control: rename only (custom keys intact) -> repair keeps the name.
    let mut root = steam::read_shortcuts(&path).unwrap();
    for (_, v) in vdf::ensure(&mut root, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            if vdf::string(e, "appname") == Some("World of Warcraft") {
                vdf::set(e, "appname", Value::Text("My Retail".into()));
            }
        }
    }
    fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
    let ok = review_repair(&f.ctx, &json!({"id":i.id})).is_ok();
    println!("REPRO6 rename with custom keys intact -> repair ok: {ok}");
    // Same rename, but Steam has rewritten shortcuts.vdf without the wowRust* keys.
    for (_, v) in vdf::ensure(&mut root, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            e.retain(|(k, _)| !k.starts_with("wowRust"));
        }
    }
    fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
    match review_repair(&f.ctx, &json!({"id":i.id})) {
        Ok(p) => println!("REPRO6 after key loss -> plan: {:?}", p["summary"]),
        Err(e) => panic!("REPRO6 after key loss -> repair: {e}"),
    }
}

#[test]
fn repro_7_scan_budget_starves_later_libraries() {
    let base = std::env::temp_dir().join(format!("wow-budget-{}", fsutil::unique()));
    let home = base.join("home");
    let steam = home.join(".local/share/Steam");
    fs::create_dir_all(steam.join("userdata/123/config")).unwrap();
    fs::create_dir_all(steam.join("config")).unwrap();
    // Second library (e.g. an SD card) holding a Steam-created Battle.net prefix.
    let lib2 = base.join("sdcard");
    let bn = lib2.join("steamapps/compatdata/4000000001/pfx/drive_c/Program Files (x86)/Battle.net");
    fs::create_dir_all(&bn).unwrap();
    fs::write(bn.join("Battle.net.exe"), b"MZ").unwrap();
    fs::write(steam.join("steamapps/libraryfolders.vdf").as_path().parent().map(|p| { fs::create_dir_all(p).unwrap(); p.join("libraryfolders.vdf") }).unwrap(),
        format!("\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} }}", steam.display(), lib2.display())).unwrap();
    let state = State { schema: 1, ..State::default() };
    let before = discovery::scan(&home, &state).unwrap();
    // A large internal library: 50,001 directories under steamapps/common.
    for n in 0..50_001 { fs::create_dir_all(steam.join(format!("steamapps/common/Game{}/d{}", n % 500, n))).unwrap(); }
    let after = discovery::scan(&home, &state).unwrap();
    assert_eq!(before.launchers.len(), after.launchers.len());
    println!("REPRO7 launchers found: small library={} large library={} | warning: {:?}",
        before.launchers.len(), after.launchers.len(), after.warnings.iter().find(|w| w.contains("limit")));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn repro_8_classic_ptr_executable_not_recognized() {
    let base = std::env::temp_dir().join(format!("wow-ptr-{}", fsutil::unique()));
    for (folder, exe) in [("_classic_ptr_", "WowClassicT.exe"), ("_classic_era_ptr_", "WowClassicT.exe"), ("_ptr_", "WowT.exe")] {
        let d = base.join("World of Warcraft").join(folder);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join(exe), b"MZ").unwrap();
    }
    let mut scan = Scan::default();
    discovery::walk(&base, &mut scan, &mut std::collections::HashSet::new(), &mut 1000).unwrap();
    assert_eq!(scan.games.len(), 3);
    println!("REPRO8 detected: {:?}", scan.games.iter().map(|g| g.id.clone()).collect::<Vec<_>>());
    let _ = fs::remove_dir_all(base);
}

#[test]
fn repro_9_journal_size() {
    let base = std::env::temp_dir().join(format!("wow-journal-{}", fsutil::unique()));
    fs::create_dir_all(&base).unwrap();
    let file = base.join("config.vdf");
    let before: Vec<u8> = (0..300_000).map(|n| b"abcdefgh\t\"{}\n"[n % 13]).collect();
    fs::write(&file, &before).unwrap();
    let mut after = before.clone();
    after.extend_from_slice(b"\n\"x\" \"y\"\n");
    let mut j = Journal { id: fsutil::unique(), label: "Repair Steam shortcuts".into(), changes: vec![Change::new(file, after).unwrap()], done: false };
    transaction::commit(&base, &mut j, false).unwrap();
    let backup = fs::metadata(base.join("backups").join(format!("{}.json", j.id))).unwrap().len();
    let last = fs::metadata(base.join("last-transaction.json")).unwrap().len();
    println!("REPRO9 300 KB config edit -> backups/<id>.json {} KB + last-transaction.json {} KB (kept per change, never pruned)", backup / 1024, last / 1024);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn repro_10_adopting_a_python_built_installation() {
    let mut f = Fixture::new();
    let home = f.ctx.home.clone();
    let prefix = home.join("Games/wow-on-steam/prefix");
    let bn = prefix.join("pfx/drive_c/Program Files (x86)/Battle.net");
    fs::create_dir_all(&bn).unwrap();
    for exe in ["Battle.net.exe", "Battle.net Launcher.exe"] { fs::write(bn.join(exe), b"MZ").unwrap(); }
    for folder in ["_retail_", "_classic_"] {
        let d = prefix.join("pfx/drive_c/Program Files (x86)/World of Warcraft").join(folder);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join(if folder == "_retail_" { "Wow.exe" } else { "WowClassic.exe" }), b"MZ").unwrap();
    }
    // Tiles exactly as the Python installer wrote them (alpha12+ routing).
    let q = |p: &std::path::Path| format!("\"{}\"", p.display());
    let opts = format!("STEAM_COMPAT_DATA_PATH={} %command%", q(&prefix));
    let mk = |id: u32, name: &str, exe: &std::path::Path, o: String| -> Value {
        Value::Object(vec![("appid".into(), Value::I32(id)), ("AppName".into(), Value::Text(name.into())),
            ("Exe".into(), Value::Text(q(exe))), ("StartDir".into(), Value::Text(q(exe.parent().unwrap()))),
            ("LaunchOptions".into(), Value::Text(o)), ("tags".into(), Value::Object(vec![]))])
    };
    let tiles = vec![("shortcuts".to_string(), Value::Object(vec![
        ("0".into(), mk(0x8000_0001, "Battle.net", &bn.join("Battle.net Launcher.exe"), opts.clone())),
        ("1".into(), mk(0x8000_0002, "World of Warcraft", &bn.join("Battle.net.exe"), format!("{opts} --exec=\"launch WoW\""))),
        ("2".into(), mk(0x8000_0003, "WoW — Classic Progression", &bn.join("Battle.net.exe"), format!("{opts} --game=wow_classic"))),
    ]))];
    let sc = home.join(".local/share/Steam/userdata/123/config/shortcuts.vdf");
    fs::write(sc.clone(), vdf::binary(&tiles).unwrap()).unwrap();
    let cfg = home.join(".local/share/Steam/config/config.vdf");
    let c = steam::set_proton(Some(&fs::read(&cfg).unwrap()), &[0x8000_0001, 0x8000_0002, 0x8000_0003], "proton_11").unwrap();
    fs::write(&cfg, c).unwrap();

    let data = adoption_candidates(&f.ctx, &json!({"prefix":prefix})).unwrap();
    for c in data["candidates"].as_array().unwrap() { println!("REPRO10 candidate: {} (product {:?}, edition {:?})", c["name"], c["product"], c["edition"]); }
    let c = &data["candidates"][0];
    let all: Vec<u64> = data["candidates"].as_array().unwrap().iter().map(|x| x["appid"].as_u64().unwrap()).collect();
    let p = review_adopt(&f.ctx, &json!({"prefix":prefix,"steam":c["steam"],"user":c["user"],"appid":c["appid"],"selected":all})).unwrap();
    apply(&f.ctx, &json!({"token":p["token"]})).unwrap(); f.refresh();
    let adopted=f.ctx.state.installations[0].clone();f.confirm_fixture_exit(&adopted);
    let id = adopted.id.clone();
    let p = review_repair(&f.ctx, &json!({"id":id,"allow_missing":true,"shared_tile_appid":0x80000002u32})).unwrap();
    apply(&f.ctx, &json!({"token":p["token"]})).unwrap(); f.refresh();
    let after = steam::read_shortcuts(&sc).unwrap();
    assert_eq!(vdf::object(&after, "shortcuts").unwrap().len(), 1);
    for (_, v) in vdf::object(&after, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            println!("REPRO10 after repair: id {:#x} | {:?} | exe ends {:?} | opts tail {:?}",
                vdf::number(e, "appid").unwrap(), vdf::string(e, "appname").unwrap(),
                vdf::string(e, "exe").unwrap().rsplit('/').next().unwrap(),
                vdf::string(e, "LaunchOptions").unwrap().split("%command%").nth(1).unwrap_or(""));
        }
    }
}

#[test]
fn repro_11_adopting_python_retail_and_forever() {
    let mut f = Fixture::new();
    let home = f.ctx.home.clone();
    let prefix = home.join("Games/wow-on-steam/prefix");
    let bn = prefix.join("pfx/drive_c/Program Files (x86)/Battle.net");
    fs::create_dir_all(&bn).unwrap();
    for exe in ["Battle.net.exe", "Battle.net Launcher.exe"] { fs::write(bn.join(exe), b"MZ").unwrap(); }
    for folder in ["_retail_", "_forever_"] {
        let d = prefix.join("pfx/drive_c/Program Files (x86)/World of Warcraft").join(folder);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("Wow.exe"), b"MZ").unwrap();
    }
    // Tiles exactly as the Python installer wrote them (alpha12+ routing).
    let q = |p: &std::path::Path| format!("\"{}\"", p.display());
    let opts = format!("STEAM_COMPAT_DATA_PATH={} %command%", q(&prefix));
    let mk = |id: u32, name: &str, exe: &std::path::Path, o: String| -> Value {
        Value::Object(vec![("appid".into(), Value::I32(id)), ("AppName".into(), Value::Text(name.into())),
            ("Exe".into(), Value::Text(q(exe))), ("StartDir".into(), Value::Text(q(exe.parent().unwrap()))),
            ("LaunchOptions".into(), Value::Text(o)), ("tags".into(), Value::Object(vec![]))])
    };
    let tiles = vec![("shortcuts".to_string(), Value::Object(vec![
        ("0".into(), mk(0x8000_0001, "Battle.net", &bn.join("Battle.net Launcher.exe"), opts.clone())),
        ("1".into(), mk(0x8000_0002, "World of Warcraft", &bn.join("Battle.net.exe"), format!("{opts} --exec=\"launch WoW\""))),
        ("2".into(), mk(0x8000_0003, "World of Warcraft Forever", &bn.join("Battle.net.exe"), format!("{opts} --exec=\"launch WoWF\""))),
    ]))];
    let sc = home.join(".local/share/Steam/userdata/123/config/shortcuts.vdf");
    fs::write(sc.clone(), vdf::binary(&tiles).unwrap()).unwrap();
    let cfg = home.join(".local/share/Steam/config/config.vdf");
    let c = steam::set_proton(Some(&fs::read(&cfg).unwrap()), &[0x8000_0001, 0x8000_0002, 0x8000_0003], "proton_11").unwrap();
    fs::write(&cfg, c).unwrap();

    let data = adoption_candidates(&f.ctx, &json!({"prefix":prefix})).unwrap();
    for c in data["candidates"].as_array().unwrap() { println!("REPRO11 candidate: {} (product {:?}, edition {:?})", c["name"], c["product"], c["edition"]); }
    let c = &data["candidates"][0];
    let all: Vec<u64> = data["candidates"].as_array().unwrap().iter().map(|x| x["appid"].as_u64().unwrap()).collect();
    let p = review_adopt(&f.ctx, &json!({"prefix":prefix,"steam":c["steam"],"user":c["user"],"appid":c["appid"],"selected":all})).unwrap();
    apply(&f.ctx, &json!({"token":p["token"]})).unwrap(); f.refresh();
    let adopted=f.ctx.state.installations[0].clone();f.confirm_fixture_exit(&adopted);
    let id = adopted.id.clone();
    let p = review_repair(&f.ctx, &json!({"id":id,"allow_missing":true,"shared_tile_appid":0x80000002u32})).unwrap();
    apply(&f.ctx, &json!({"token":p["token"]})).unwrap(); f.refresh();
    let after = steam::read_shortcuts(&sc).unwrap();
    assert_eq!(vdf::object(&after, "shortcuts").unwrap().len(), 1);
    for (_, v) in vdf::object(&after, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            println!("REPRO11 after repair: id {:#x} | {:?} | exe ends {:?} | opts tail {:?}",
                vdf::number(e, "appid").unwrap(), vdf::string(e, "appname").unwrap(),
                vdf::string(e, "exe").unwrap().rsplit('/').next().unwrap(),
                vdf::string(e, "LaunchOptions").unwrap().split("%command%").nth(1).unwrap_or(""));
        }
    }
}

#[test]
fn repro_5_undo_after_steam_has_run() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    steam_rewrites_config(&f);
    println!("REPRO5 undo after Steam rewrite: {:?}", transaction::undo(&f.ctx.data, false).unwrap());
}
