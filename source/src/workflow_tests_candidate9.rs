fn c9_fixture() -> (Fixture, Installation, PathBuf, u32) {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    let parent = i.root.join("games/World of Warcraft");
    let ptr = parent.join("_classic_era_ptr_");
    fs::create_dir_all(&ptr).unwrap();
    fs::write(ptr.join("WowClassicT.exe"), b"MZ").unwrap();
    fs::write(parent.join(".build.info"),"Product!STRING:0|Active!DEC:1\nwow|1\nwow_forever|1\nwow_classic_era|1\nwow_classic_era_ptr|1\n").unwrap();
    c42_seed_legacy(&mut f,&i);
    let i = f.ctx.state.installations[0].clone();
    let app = i
        .bindings
        .iter()
        .find(|b| b.edition.as_deref() == Some("era_ptr"))
        .unwrap()
        .appid;
    fs::remove_file(ptr.join("WowClassicT.exe")).unwrap();
    (f, i, ptr, app)
}
fn c9_confirm(ptr: &std::path::Path) {
    fs::write(
        ptr.parent().unwrap().join(".build.info"),
        "Product!STRING:0|Active!DEC:1\nwow|1\nwow_forever|1\nwow_classic_era|1\n",
    )
    .unwrap();
}
#[test]
fn c9_confirmed_uninstall_review_apply_repeat_preserves_remaining_and_unrelated() {
    let (mut f, i, ptr, app) = c9_fixture();
    c9_confirm(&ptr);
    let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let mut root = steam::read_shortcuts(&path).unwrap();
    let unrelated = steam::entry(
        "Unrelated",
        std::path::Path::new("/unrelated"),
        "custom options",
        "elsewhere",
        None,
    )
    .unwrap();
    let uid = steam::upsert(&mut root, unrelated, "elsewhere").unwrap();
    fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
    let before = root;
    let response = review_repair(&f.ctx, &json!({"id":i.id,"shared_tile_appid":i.bindings.iter().find(|b|b.edition.as_deref()==Some("retail")).unwrap().appid})).unwrap();
    assert_eq!(response["shortcut_removals"][0]["appid"], app);
    assert!(response["summary"].to_string().contains("stale binding"));
    apply(&f.ctx, &json!({"token":response["token"]})).unwrap();
    f.refresh();
    let now = &f.ctx.state.installations[0];
    assert!(!now.bindings.iter().any(|b| b.appid == app));
    assert!(now
        .game_associations
        .iter()
        .any(|g| g.id == "era"));
    let after = steam::read_shortcuts(&path).unwrap();
    for (_, v) in vdf::object(&before, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            let id = vdf::number(e, "appid").unwrap();
            if id == uid {
                let found = vdf::object(&after, "shortcuts")
                    .unwrap()
                    .iter()
                    .find_map(|(_, v)| match v {
                        Value::Object(x) if vdf::number(x, "appid") == Some(id) => Some(x),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(vdf::binary(e).unwrap(), vdf::binary(found).unwrap());
            }
        }
    }
    assert!(vdf::object(&after, "shortcuts")
        .unwrap()
        .iter()
        .any(|(_, v)| matches!(v,Value::Object(e) if vdf::number(e,"appid")==Some(uid))));
    let cfg = fs::read(i.steam.join("config/config.vdf")).unwrap();
    f.repair(&now.clone());
    assert_eq!(fs::read(&path).unwrap(), vdf::binary(&after).unwrap());
    assert_eq!(fs::read(i.steam.join("config/config.vdf")).unwrap(), cfg);
}
#[test]
fn c9_missing_exe_active_metadata_incomplete_unknown_or_unavailable_refuses() {
    let (f, i, ptr, _) = c9_fixture();
    assert!(review_repair(&f.ctx, &json!({"id":i.id,"shared_tile_appid":i.bindings.iter().find(|b|b.edition.as_deref()==Some("retail")).unwrap().appid})).is_err());
    c9_confirm(&ptr);
    fs::create_dir(ptr.join("Data")).unwrap();
    assert!(review_repair(&f.ctx, &json!({"id":i.id,"shared_tile_appid":i.bindings.iter().find(|b|b.edition.as_deref()==Some("retail")).unwrap().appid})).is_err());
    fs::remove_dir(ptr.join("Data")).unwrap();
    fs::write(ptr.parent().unwrap().join(".build.info"), "malformed").unwrap();
    assert!(review_repair(&f.ctx, &json!({"id":i.id,"shared_tile_appid":i.bindings.iter().find(|b|b.edition.as_deref()==Some("retail")).unwrap().appid})).is_err());
    c9_confirm(&ptr);
    fs::rename(&i.root, f.root.join("away")).unwrap();
    assert!(review_repair(&f.ctx, &json!({"id":i.id,"shared_tile_appid":i.bindings.iter().find(|b|b.edition.as_deref()==Some("retail")).unwrap().appid})).is_err());
}
#[test]
fn c9_changed_evidence_after_review_refuses_before_mutation() {
    let (f, i, ptr, _) = c9_fixture();
    c9_confirm(&ptr);
    let q = review_repair(&f.ctx, &json!({"id":i.id,"shared_tile_appid":i.bindings.iter().find(|b|b.edition.as_deref()==Some("retail")).unwrap().appid})).unwrap();
    let p = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let before = fs::read(&p).unwrap();
    fs::write(ptr.join("WowClassicT.exe"), b"MZ").unwrap();
    assert!(load_plan(&f.ctx, &json!({"token":q["token"]})).is_err());
    assert_eq!(before, fs::read(&p).unwrap());
}
#[test]
fn c9_ambiguous_ownership_is_retained() {
    let (_f, mut i, ptr, app) = c9_fixture();
    c9_confirm(&ptr);
    let p = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let mut root = steam::read_shortcuts(&p).unwrap();
    for (_, v) in vdf::ensure(&mut root, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            if vdf::number(e, "appid") == Some(app) {
                vdf::set(e, "wowRustEdition", Value::Text("unknown".into()));
            }
        }
    }
    let games = vec![];
    assert!(obsolete_clients(&i, &games, &mut root).is_err());
    i.bindings.retain(|b| b.appid == app);
    assert!(obsolete_clients(&i, &games, &mut root).unwrap().is_empty());
}
