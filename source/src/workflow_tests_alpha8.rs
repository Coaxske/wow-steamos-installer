// Regressions derived from the independent alpha7 review. All data is synthetic.
fn rewrite_steam(f: &Fixture) {
    let p = f.ctx.home.join(".local/share/Steam/config/config.vdf");
    let text = fs::read_to_string(&p).unwrap();
    fs::write(p, text.replace("\"\t\"", "\"\t\t\"")).unwrap();
}
fn paused_reinstall(f: &mut Fixture, i: &Installation) -> Cleanup {
    review_reinstall(&f.ctx,&json!({"id":i.id,"location":f.ctx.home.join("Games"),"editions":["retail"],"keep_settings":false})).unwrap();
    let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let new = plan.replacement.as_ref().unwrap();
    fs::create_dir_all(&new.root).unwrap();
    let b = test_installer();
    fs::write(new.root.join("Battle.net-Setup.exe"), b).unwrap();
    let job = Cleanup {
        plan,
        settings_saved: false,
    };
    fsutil::json(&f.ctx.data.join("cleanup.json"), &job).unwrap();
    job
}
#[test]
fn alpha8_valve_layout_and_normal_prefix_work() {
    let mut f = Fixture::new();
    let p = f
        .ctx
        .home
        .join(".local/share/Steam/steamapps/common/Proton 11");
    assert!(!p.join("compatibilitytool.vdf").exists());
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let dir = i.prefix.join("pfx/drive_c/Program Files (x86)/Steam");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("steamclient.dll"), b"MZ").unwrap();
    review_clean(&f.ctx, &json!({"id":i.id})).unwrap();
    paused_reinstall(&mut f, &i);
}
#[test]
fn alpha8_resume_committed_cleanup_after_steam_rewrite() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let mut job = paused_reinstall(&mut f, &i);
    download(&f.ctx, job.plan.replacement.as_ref().unwrap()).unwrap();
    transaction::finish(&f.ctx.data, &mut job.plan.journal, false).unwrap();
    fsutil::json(&f.ctx.data.join("cleanup.json"), &job).unwrap();
    rewrite_steam(&f);
    run_cleanup(&f.ctx, &mut job).unwrap();
    assert!(!f.ctx.data.join("cleanup.json").exists());
    assert!(!i.root.join("games/World of Warcraft").exists());
}
#[test]
fn alpha8_pending_cleanup_can_cancel_after_steam_changes() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    paused_reinstall(&mut f, &i);
    rewrite_steam(&f);
    cancel_cleanup(&f.ctx).unwrap();
    assert!(!f.ctx.data.join("cleanup.json").exists());
    assert!(i
        .root
        .join("games/World of Warcraft/_retail_/Wow.exe")
        .exists());
}
#[test]
fn alpha8_pending_cleanup_can_rebase_and_resume() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let mut job = paused_reinstall(&mut f, &i);
    rewrite_steam(&f);
    run_cleanup(&f.ctx, &mut job).unwrap();
    assert!(!f.ctx.data.join("cleanup.json").exists());
}
#[test]
fn alpha8_stop_committed_cleanup_keeps_remaining_games() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let mut job = paused_reinstall(&mut f, &i);
    transaction::finish(&f.ctx.data, &mut job.plan.journal, false).unwrap();
    fsutil::json(&f.ctx.data.join("cleanup.json"), &job).unwrap();
    rewrite_steam(&f);
    abandon_cleanup(&f.ctx).unwrap();
    assert!(!f.ctx.data.join("cleanup.json").exists());
    assert!(i
        .root
        .join("games/World of Warcraft/_retail_/Wow.exe")
        .exists());
}
#[test]
fn alpha8_undo_after_steam_rewrite() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    rewrite_steam(&f);
    transaction::undo(&f.ctx.data, false).unwrap();
}
#[test]
fn alpha8_rename_and_missing_custom_metadata_survive_repair() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let mut root = steam::read_shortcuts(&path).unwrap();
    let count = vdf::object(&root, "shortcuts").unwrap().len();
    let mut retail = 0;
    for (_, v) in vdf::ensure(&mut root, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            if vdf::string(e,"wowRustRole")==Some("shared_game") {
                retail = vdf::number(e, "appid").unwrap();
                vdf::set(e, "appname", Value::Text("My Retail".into()));
                vdf::set(e, "IsHidden", Value::I32(1));
                let opts = format!(
                    "PROTON_ENABLE_NVAPI=1 mangohud {}",
                    vdf::string(e, "LaunchOptions").unwrap()
                );
                vdf::set(e, "LaunchOptions", Value::Text(opts));
            }
            e.retain(|(k, _)| !k.starts_with("wowRust"));
        }
    }
    fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
    f.repair(&i);
    let root = steam::read_shortcuts(&path).unwrap();
    assert_eq!(vdf::object(&root, "shortcuts").unwrap().len(), count);
    let e = vdf::object(&root, "shortcuts")
        .unwrap()
        .iter()
        .find_map(|(_, v)| match v {
            Value::Object(e) if vdf::number(e, "appid") == Some(retail) => Some(e),
            _ => None,
        })
        .unwrap();
    assert_eq!(vdf::string(e, "appname"), Some("Battle.net"));
    assert_eq!(vdf::number(e, "IsHidden"), Some(1));
    assert!(vdf::string(e, "LaunchOptions")
        .unwrap()
        .contains("PROTON_ENABLE_NVAPI=1 mangohud"));
}
#[test]
fn alpha8_saved_install_scans_before_large_common_library() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    // Deliberately exhaust one root; the later root must still get a fresh budget.
    let big = f.ctx.home.join("Games/huge");
    for n in 0..50_001 {
        fs::create_dir_all(big.join(n.to_string())).unwrap();
    }
    let sd = f
        .ctx
        .home
        .join("sdcard/prefix/pfx/drive_c/Program Files (x86)/Battle.net");
    fs::create_dir_all(&sd).unwrap();
    fs::write(sd.join("Battle.net.exe"), b"MZ").unwrap();
    f.ctx.state.extra_roots.push(f.ctx.home.join("sdcard"));
    let scan = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert_eq!(scan.launchers.len(), 2);
}
#[test]
fn alpha8_ptr_executables_detected() {
    let f = Fixture::new();
    for folder in ["_classic_ptr_", "_classic_era_ptr_"] {
        let p = f.ctx.home.join("Games/World of Warcraft").join(folder);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("WowClassicT.exe"), b"MZ").unwrap();
    }
    let s = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert_eq!(s.games.len(), 2);
}
#[test]
fn alpha8_python_adoption_preserves_three_ids_for_each_route() {
    for classic in [false, true] {
        let mut f = Fixture::new();
        let prefix = f.ctx.home.join("Games/wow-on-steam/prefix");
        let bn = prefix.join("pfx/drive_c/Program Files (x86)/Battle.net");
        fs::create_dir_all(&bn).unwrap();
        for name in ["Battle.net.exe", "Battle.net Launcher.exe"] {
            fs::write(bn.join(name), b"MZ").unwrap();
        }
        for folder in ["_retail_", if classic { "_classic_" } else { "_forever_" }] {
            let p = prefix
                .join("pfx/drive_c/Program Files (x86)/World of Warcraft")
                .join(folder);
            fs::create_dir_all(&p).unwrap();
            fs::write(p.join("Wow.exe"), b"MZ").unwrap();
        }
        let opts = steam::launch_options(&prefix, None).unwrap();
        let mut root = vec![("shortcuts".into(), Value::Object(vec![]))];
        for (n, name, exe, tail) in [
            (1, "Battle.net", "Battle.net Launcher.exe", ""),
            (
                2,
                "World of Warcraft",
                "Battle.net.exe",
                " --exec=\"launch WoW\"",
            ),
            (
                3,
                "My edition",
                "Battle.net.exe",
                if classic {
                    " --game=wow_classic"
                } else {
                    " --exec=\"launch WoWF\""
                },
            ),
        ] {
            let mut e = steam::entry(
                name,
                &bn.join(exe),
                &format!("{opts}{tail}"),
                "python",
                Some(0x80000000 + n),
            )
            .unwrap();
            e.retain(|(k, _)| !k.starts_with("wowRust"));
            vdf::ensure(&mut root, "shortcuts")
                .unwrap()
                .push((n.to_string(), Value::Object(e)));
        }
        let steamroot = f.ctx.home.join(".local/share/Steam");
        let sc = steamroot.join("userdata/123/config/shortcuts.vdf");
        fs::write(&sc, vdf::binary(&root).unwrap()).unwrap();
        let cfg = steamroot.join("config/config.vdf");
        fs::write(
            &cfg,
            steam::set_proton(
                Some(&fs::read(&cfg).unwrap()),
                &[0x80000001, 0x80000002, 0x80000003],
                "proton_11",
            )
            .unwrap(),
        )
        .unwrap();
        let candidates = adoption_candidates(&f.ctx, &json!({"prefix":prefix})).unwrap();
        assert_eq!(candidates["candidates"].as_array().unwrap().len(), 3);
        let p=review_adopt(&f.ctx,&json!({"prefix":prefix,"steam":steamroot,"user":"123","appid":0x80000001u32,"selected":[0x80000001u32,0x80000002u32,0x80000003u32]})).unwrap();
        apply(&f.ctx, &json!({"token":p["token"]})).unwrap();
        f.refresh();
        let i = f.ctx.state.installations[0].clone();
        assert_eq!(i.shortcuts.len(),3);f.confirm_fixture_exit(&i);
        assert!(review_repair(&f.ctx,&json!({"id":i.id})).unwrap_err().contains("Choose which"));
        let q=review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":0x80000002u32})).unwrap();
        apply(&f.ctx,&json!({"token":q["token"]})).unwrap();f.refresh();
        f.repair(&i);
        let root = steam::read_shortcuts(&sc).unwrap();
        let entries = vdf::object(&root, "shortcuts").unwrap();
        assert_eq!(entries.len(), 1);
        for (_, v) in entries {
            let Value::Object(e) = v else { panic!() };
            assert!(
                [0x80000001, 0x80000002].contains(&vdf::number(e, "appid").unwrap())
            );
        }
    }
}
#[test]
fn alpha8_compact_backup_reads_old_arrays_and_stays_small() {
    let f = Fixture::new();
    let path = f.ctx.data.join("config.test");
    let before = vec![b'a'; 300_000];
    fs::write(&path, before).unwrap();
    let j = Journal {
        id: fsutil::unique(),
        label: "test".into(),
        changes: vec![Change::new(path, vec![b'b'; 300_000]).unwrap()],
        done: false,
    };
    let encoded = serde_json::to_vec_pretty(&j).unwrap();
    assert!(encoded.len() < 1_210_000);
    let old = json!({"path":"/example/file","before":[1,2,3],"after":[4,5]});
    let c: Change = serde_json::from_value(old).unwrap();
    assert_eq!(c.before, Some(vec![1, 2, 3]));
}
#[test]
fn alpha8_merge_preserves_unrelated_fields_and_rejects_conflicts() {
    let path = Path::new("config.vdf");
    let b = b"\"root\" { \"ours\" \"old\" }";
    let a = b"\"root\" { \"ours\" \"new\" }";
    let c = b"\"root\" { \"ours\" \"old\" \"steam\" \"keep\" }";
    let out = crate::merge::apply(path, b, a, c).unwrap();
    let text = String::from_utf8(out.clone()).unwrap();
    assert!(text.contains("keep"));
    assert!(text.contains("new"));
    let restored = crate::merge::apply(path, a, b, &out).unwrap();
    assert!(String::from_utf8(restored).unwrap().contains("keep"));
    assert!(crate::merge::apply(path, b, a, b"\"root\" { \"ours\" \"user\" }").is_err());
}
#[test]
fn alpha8_download_checks_are_bounded() {
    for url in [
        "http://downloader.battle.net/a",
        "https://battle.net.evil.test/a",
        "https://battle.net@evil.test/a",
    ] {
        assert!(!download_host(url));
    }
    assert!(download_host(
        "https://downloader.battle.net/download/file.exe"
    ));
    assert!(valid_installer(&test_installer()));
    assert!(!valid_installer(b"MZ"));
    let mut bad = test_installer();
    bad[0x3c..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(!valid_installer(&bad));
}

#[test]
fn alpha8_live_run_checks_repair_after_launch() {
    let mut f = Fixture::new();
    let (token, i) = live_prepare_fixture(&mut f, false);
    live_advance(&mut f, &token);
    f.games(&i);
    f.repair(&i);
    rewrite_steam(&f);
    live_phase(&f, "post_launch_repair");
    live_advance(&mut f, &token);
    let result = live_advance(&mut f, &token);
    assert_eq!(result["run"]["phase"], "finish");
    assert!(result["text"]
        .as_str()
        .unwrap()
        .contains("Repair after game launch"));
}
