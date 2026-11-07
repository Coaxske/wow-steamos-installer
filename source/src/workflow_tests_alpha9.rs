#[test]
fn alpha9_undo_after_tests_and_finish_keeps_unrelated_state() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let repaired = f.ctx.state.installations[0].clone();
    let other = f.ctx.home.join("Another games folder");
    f.ctx.state.extra_roots.push(other.clone());
    f.ctx.state.tests.push(format!("{}:World of Warcraft", i.id));
    f.ctx.state.installations[0].stage = Stage::Complete;
    f.ctx.state.installations[0].addon_pending = false;
    f.ctx.state.addon_pending = false;
    f.ctx.save().unwrap();
    rewrite_steam(&f);
    transaction::undo(&f.ctx.data, false).unwrap();
    f.refresh();
    assert_eq!(f.ctx.state.extra_roots, vec![other]);
    assert_eq!(f.ctx.state.tests.len(), 1);
    assert_eq!(f.ctx.state.installations[0].stage, i.stage);
    assert_eq!(f.ctx.state.installations[0].shortcuts, repaired.shortcuts); // Sole bootstrap identity survives repair and Undo.
    // The restored setup is still repairable.
    f.repair(&i);
}
#[test]
fn alpha9_undo_conflict_leaves_every_file_untouched() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    f.ctx.state.installations[0].launcher = Some(i.prefix.join("different.exe"));
    f.ctx.save().unwrap();
    let sc = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let before = fs::read(&sc).unwrap();
    let state = fs::read(f.ctx.data.join("state.json")).unwrap();
    assert!(transaction::undo(&f.ctx.data, false).is_err());
    assert_eq!(fs::read(sc).unwrap(), before);
    assert_eq!(fs::read(f.ctx.data.join("state.json")).unwrap(), state);
}
#[test]
fn alpha9_appimage_update_applies_keeps_id_and_does_not_duplicate() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let mut ids = vec![];
    for version in ["1", "2", "2"] {
        let app = f.ctx.home.join(format!("Applications/WowUp-{version}.AppImage"));
        appimage8(&app);
        let p = review_addon(&f.ctx, &json!({"id":i.id,"path":app,"manager":"wowup_wago"})).unwrap();
        apply(&f.ctx, &json!({"token":p["token"]})).unwrap();
        f.refresh();
        let root = steam::read_shortcuts(&path).unwrap();
        let addon: Vec<_> = vdf::object(&root,"shortcuts").unwrap().iter().filter_map(|(_,v)| {
            match v { Value::Object(e) if steam::native_addon(e) => Some(e), _ => None }
        }).collect();
        assert_eq!(addon.len(), 1);
        ids.push(vdf::number(addon[0],"appid").unwrap());
        let opts = vdf::string(addon[0],"LaunchOptions").unwrap();
        assert!(opts.contains(&format!("WowUp-{version}.AppImage")));
        assert!(!opts.contains("STEAM_COMPAT_DATA_PATH"));
        assert!(!opts.contains("%command%"));
    }
    assert!(ids.iter().all(|id| id == &ids[0]));
}
#[test]
fn alpha9_launch_options_empty_arguments_and_wrapper() {
    let desired = "STEAM_COMPAT_DATA_PATH=\"/games/prefix\" %command% --exec=\"launch WoW\"";
    for old in ["", "  ", "--exec=\"launch WoWC\""] {
        let out = steam::preserve_options(old, desired).unwrap();
        assert!(out.contains("%command%"));
        assert!(out.contains("launch WoW"));
        assert!(!out.contains("WoWC"));
    }
    let out = steam::preserve_options("--some-argument 'two words'", desired).unwrap();
    assert!(out.contains("%command% --some-argument 'two words'"));
    let out = steam::preserve_options("PROTON_LOG=1 mangohud %command% --some-argument",desired).unwrap();
    assert!(out.contains("PROTON_LOG=1 mangohud %command% --some-argument"));
    assert!(steam::preserve_options("%command% %command%",desired).is_err());
    assert!(steam::preserve_options("'unfinished",desired).is_err());
}
#[test]
fn alpha9_cleared_options_and_lost_metadata_keep_renamed_tile() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let mut root = steam::read_shortcuts(&path).unwrap();
    let mut id = 0;
    for (_,v) in vdf::ensure(&mut root,"shortcuts").unwrap() {
        if let Value::Object(e) = v {
            if vdf::string(e,"wowRustRole")==Some("shared_game") {
                id = vdf::number(e,"appid").unwrap();
                e.retain(|(k,_)| !k.starts_with("wowRust"));
                vdf::set(e,"appname",Value::Text("My renamed game".into()));
                vdf::set(e,"LaunchOptions",Value::Text(String::new()));
            }
        }
    }
    assert_ne!(id,0);
    fs::write(&path,vdf::binary(&root).unwrap()).unwrap();
    f.repair(&i);
    let root = steam::read_shortcuts(&path).unwrap();
    let entries = vdf::object(&root,"shortcuts").unwrap();
    assert_eq!(entries.len(),1);
    assert!(entries.iter().any(|(_,v)| matches!(v,Value::Object(e) if vdf::number(e,"appid")==Some(id) && vdf::string(e,"appname")==Some("Battle.net"))));
}
