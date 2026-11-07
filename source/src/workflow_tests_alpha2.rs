#[test]
fn checkpoint_failure_keeps_memory_and_previous_file() {
    let mut f = Fixture::new();
    let i = f.install();
    let before = fs::read(f.ctx.data.join("state.json")).unwrap();
    let original = f.ctx.data.clone();
    f.ctx.data = f.root.join("file-instead-of-directory");
    fs::write(&f.ctx.data, b"blocked").unwrap();
    let mut next = f.ctx.state.clone();
    next.installations[0].stage = Stage::Games;
    assert!(f.ctx.persist(next).is_err());
    assert_eq!(f.ctx.state.installations[0].stage, Stage::LauncherInstall);
    assert_eq!(fs::read(original.join("state.json")).unwrap(), before);
    assert_eq!(f.ctx.state.installations[0].id, i.id);
}
#[test]
fn malformed_saved_identity_is_rejected() {
    let mut f = Fixture::new();
    f.install();
    let mut s = f.ctx.state.clone();
    s.installations[0].id = "../../other".into();
    assert!(workflow::state(&s).is_err());
    let mut s = f.ctx.state.clone();
    s.installations[0].editions.push("retail".into());
    assert!(workflow::state(&s).is_err());
}
#[test]
fn launcher_handoff_keeps_bootstrap_id_before_game_download() {
    let mut f = Fixture::new();
    let i = f.install();
    let bn = i.prefix.join("pfx/drive_c/Program Files/Battle.net");
    fs::create_dir_all(&bn).unwrap();
    fs::write(bn.join("Battle.net.exe"), b"MZ").unwrap();
    let p = review_launcher(&f.ctx, &json!({"id":i.id})).unwrap();
    apply(&f.ctx, &json!({"token":p["token"]})).unwrap();
    f.refresh();
    assert_eq!(f.ctx.state.installations[0].stage, Stage::Games);
    let tiles = shortcuts(&f.ctx, &json!({"id":i.id})).unwrap();
    assert_eq!(tiles["shortcuts"][0]["appid"], i.bootstrap_id);
    assert_eq!(tiles["shortcuts"][0]["name"], "Battle.net");
}
#[test]
fn missing_selected_game_requires_explicit_partial_choice() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    fs::remove_dir_all(i.root.join("games/World of Warcraft/_forever_")).unwrap();
    assert!(review_repair(&f.ctx, &json!({"id":i.id})).is_err());
    assert!(review_repair(&f.ctx, &json!({"id":i.id,"allow_missing":true})).is_ok());
}
#[test]
fn reinstall_is_one_reviewed_replacement_with_settings_followup() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let old = i.root.join("games/World of Warcraft/_retail_/WTF");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("Config.wtf"), b"settings").unwrap();
    let r=review_reinstall(&f.ctx,&json!({"id":i.id,"location":f.ctx.home.join("Games"),"editions":["retail"],"keep_settings":true})).unwrap();
    let p: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let new = p.replacement.as_ref().unwrap();
    assert_ne!(new.prefix, i.prefix);
    assert!(old.exists());
    fs::create_dir_all(&new.root).unwrap();
    let bytes = test_installer();
    fs::write(new.root.join("Battle.net-Setup.exe"), bytes).unwrap();
    apply(
        &f.ctx,
        &json!({"token":r["token"],"confirmation":"REMOVE THIS COPY"}),
    )
    .unwrap();
    f.refresh();
    assert!(!old.exists());
    assert_eq!(f.ctx.state.installations.len(), 1);
    assert_eq!(f.ctx.state.current.as_ref(), Some(&new.id));
    assert_eq!(
        f.ctx.state.installations[0].restore_backup.as_ref(),
        Some(&p.token)
    );
    assert!(f
        .ctx
        .data
        .join("settings-backups")
        .join(&p.token)
        .join("retail/WTF/Config.wtf")
        .is_file());
}
#[test]
fn corrupt_cleanup_cannot_expand_deletion_targets() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    review_clean(&f.ctx, &json!({"id":i.id})).unwrap();
    let mut p: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    p.remove[0].path = f.ctx.home.clone();
    let mut job = Cleanup {
        plan: p,
        settings_saved: false,
    };
    assert!(run_cleanup(&f.ctx, &mut job).is_err());
    assert!(f.ctx.home.is_dir());
}
#[test]
fn interrupted_restore_accepts_its_identical_completed_folder() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    let key = fsutil::unique();
    let backup = f.ctx.data.join("settings-backups").join(&key);
    let source = backup.join("retail/WTF");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("Config.wtf"), b"saved").unwrap();
    let destination = i.root.join("games/World of Warcraft/_retail_/WTF");
    copy_tree(&source, &destination).unwrap();
    let mut job = RestoreJob {
        installation: i.id,
        backup: key,
        items: vec![RestoreItem {
            digest: tree_digest(&source).unwrap(),
            source,
            destination: destination.clone(),
            done: false,
        }],
    };
    fsutil::json(&f.ctx.data.join("restore.json"), &job).unwrap();
    run_restore(&mut f.ctx, &mut job).unwrap();
    assert_eq!(fs::read(destination.join("Config.wtf")).unwrap(), b"saved");
    assert!(!f.ctx.data.join("restore.json").exists());
    assert!(f.ctx.state.addon_pending);
}
#[test]
fn restore_refuses_changed_destination_before_copying_other_folders() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    let key = fsutil::unique();
    let source = f
        .ctx
        .data
        .join("settings-backups")
        .join(&key)
        .join("retail/WTF");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("Config.wtf"), b"saved").unwrap();
    let destination = i.root.join("games/World of Warcraft/_retail_/WTF");
    fs::create_dir_all(&destination).unwrap();
    fs::write(destination.join("Config.wtf"), b"new user edit").unwrap();
    let mut job = RestoreJob {
        installation: i.id,
        backup: key,
        items: vec![RestoreItem {
            digest: tree_digest(&source).unwrap(),
            source,
            destination: destination.clone(),
            done: false,
        }],
    };
    assert!(run_restore(&mut f.ctx, &mut job).is_err());
    assert_eq!(
        fs::read(destination.join("Config.wtf")).unwrap(),
        b"new user edit"
    );
}
#[test]
fn battlenet_setting_changes_only_on_apply_and_preserves_other_data() {
    let mut f = Fixture::new();
    let i = f.install();
    let p = i
        .prefix
        .join("pfx/drive_c/users/steamuser/AppData/Roaming/Battle.net/Battle.net.config");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    let raw = br#"{"Client":{"GameLaunchWindowBehavior":"0","other":true},"private":"fixture"}"#;
    fs::write(&p, raw).unwrap();
    let r = review_exit(&f.ctx, &json!({"id":i.id,"confirm_installation_wide":true})).unwrap();
    assert_eq!(fs::read(&p).unwrap(), raw);
    apply(&f.ctx, &json!({"token":r["token"]})).unwrap();
    let v: Json = fsutil::load(&p).unwrap();
    assert_eq!(v["Client"]["GameLaunchWindowBehavior"], "2");
    assert_eq!(v["private"], "fixture");
    assert_eq!(v["Client"]["other"], true);
}
#[test]
fn repaired_custom_game_title_keeps_id_and_route() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let mut root = steam::read_shortcuts(&path).unwrap();
    let mut id = 0;
    for (_, v) in vdf::ensure(&mut root, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            if vdf::string(e,"wowRustRole")==Some("shared_game") {
                id = vdf::number(e, "appid").unwrap();
                vdf::set(e, "appname", Value::Text("My Forever".into()));
            }
        }
    }
    fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
    f.repair(&i);
    let root = steam::read_shortcuts(&path).unwrap();
    let e = vdf::object(&root, "shortcuts")
        .unwrap()
        .iter()
        .find_map(|(_, v)| match v {
            Value::Object(e) if vdf::number(e, "appid") == Some(id) => Some(e),
            _ => None,
        })
        .unwrap();
    assert_eq!(vdf::string(e, "appname"), Some("Battle.net"));
    assert!(steam::product(vdf::string(e,"LaunchOptions").unwrap()).is_none());
}
#[test]
fn legacy_removal_requires_listed_ids_and_preserves_other_tiles() {
    let mut f = Fixture::new();
    let i = f.install();
    let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let mut root = steam::read_shortcuts(&path).unwrap();
    let e = steam::entry("Old WoW", Path::new("/old/launcher"), "", "external", None).unwrap();
    let old = steam::upsert(&mut root, e, "external").unwrap();
    fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
    assert!(review_legacy(&f.ctx, &json!({"id":i.id,"selected":[i.bootstrap_id]})).is_err());
    let r = review_legacy(&f.ctx, &json!({"id":i.id,"selected":[old]})).unwrap();
    apply(&f.ctx, &json!({"token":r["token"]})).unwrap();
    assert_eq!(
        vdf::object(&steam::read_shortcuts(&path).unwrap(), "shortcuts")
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn direct_game_adoption_preserves_id_and_changes_only_route() {
    let f = Fixture::new();
    let steam = f.ctx.home.join(".local/share/Steam");
    let prefix = steam.join("steamapps/compatdata/900");
    let bn = prefix.join("pfx/drive_c/Program Files/Battle.net");
    let game = prefix.join("pfx/drive_c/Program Files/World of Warcraft/_forever_");
    fs::create_dir_all(&bn).unwrap();
    fs::create_dir_all(&game).unwrap();
    fs::write(bn.join("Battle.net.exe"), b"MZ").unwrap();
    fs::write(game.join("Wow.exe"), b"MZ").unwrap();
    let path = steam::shortcuts_path(&steam, "123").unwrap();
    let mut root = steam::read_shortcuts(&path).unwrap();
    let e = steam::entry(
        "My game",
        &game.join("Wow.exe"),
        &steam::launch_options(&prefix, None).unwrap(),
        "previous",
        None,
    )
    .unwrap();
    let id = steam::upsert(&mut root, e, "previous").unwrap();
    fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
    let config = steam.join("config/config.vdf");
    fs::write(
        &config,
        steam::set_proton(Some(&fs::read(&config).unwrap()), &[id], "proton_10").unwrap(),
    )
    .unwrap();
    let list = adoption_candidates(&f.ctx, &json!({"prefix":prefix})).unwrap();
    assert_eq!(list["candidates"].as_array().unwrap().len(), 1);
    let r = review_adopt(
        &f.ctx,
        &json!({"prefix":prefix,"steam":steam,"user":"123","appid":id,"selected":[id]}),
    )
    .unwrap();
    apply(&f.ctx, &json!({"token":r["token"]})).unwrap();
    let root = steam::read_shortcuts(&path).unwrap();
    let e = match &vdf::object(&root, "shortcuts").unwrap()[0].1 {
        Value::Object(e) => e,
        _ => panic!(),
    };
    assert_eq!(vdf::number(e, "appid"), Some(id));
    assert_eq!(vdf::string(e, "appname"), Some("My game"));
    assert!(vdf::string(e, "exe").unwrap().contains("Battle.net.exe"));
    assert!(vdf::string(e, "LaunchOptions").unwrap().contains("WoWF"));
    assert_eq!(
        steam::proton_for(&fs::read(config).unwrap(), id)
            .unwrap()
            .as_deref(),
        Some("proton_10")
    );
}
#[test]
fn repair_retains_a_new_working_steam_proton_choice() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let current = f.ctx.state.installations[0].clone();
    let cfg = i.steam.join("config/config.vdf");
    fs::write(
        &cfg,
        steam::set_proton(
            Some(&fs::read(&cfg).unwrap()),
            &current.shortcuts,
            "proton_10",
        )
        .unwrap(),
    )
    .unwrap();
    f.repair(&i);
    assert_eq!(f.ctx.state.installations[0].proton, "proton_10");
}
#[test]
fn completed_repair_does_not_reopen_addon_followup() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    f.ctx.state.installations[0].stage = Stage::Complete;
    f.ctx.state.installations[0].addon_pending = false;
    f.ctx.state.addon_pending = false;
    f.ctx.save().unwrap();
    f.repair(&i);
    assert!(!f.ctx.state.addon_pending);
    assert_eq!(f.ctx.state.installations[0].stage, Stage::Complete);
}
#[test]
fn metadata_does_not_reclassify_reused_beta_folder() {
    let f = Fixture::new();
    let folder = f.root.join("_classic_beta_");
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join(".build.info"),
        b"Product!STRING:0|Active!DEC:1\nwow_forever|1",
    )
    .unwrap();
    assert!(products::installed(&folder).is_none());
}

#[test]
fn final_repair_reuses_the_original_launcher_tile() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let listed = shortcuts(&f.ctx, &json!({"id":i.id})).unwrap();
    let launcher = listed["shortcuts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == "Battle.net")
        .unwrap();
    assert_eq!(launcher["appid"], i.bootstrap_id);
}

#[test]
fn c37_launcher_exit_requires_explicit_installation_wide_opt_in() {
    let mut f = Fixture::new();
    let i = f.install();
    let before = fs::read(f.ctx.data.join("state.json")).unwrap();
    for owned in [true, false] {
        f.ctx.state.installations[0].owned = owned;
        let error = review_exit(&f.ctx, &json!({"id":i.id})).unwrap_err();
        assert!(error.contains("BOTH Desktop and Gaming Mode"));
        assert!(review_exit(&f.ctx, &json!({"id":i.id,"confirm_installation_wide":false})).is_err());
        assert_eq!(fs::read(f.ctx.data.join("state.json")).unwrap(), before);
    }
}
#[test]
fn c37_keep_current_is_reviewed_and_never_reads_or_changes_vendor_settings() {
    let mut f = Fixture::new();
    let i = f.install();
    let path = i.prefix.join("pfx/drive_c/users/steamuser/AppData/Roaming/Battle.net/Battle.net.config");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let private = b"not JSON; keep-current must not parse or rewrite this";
    fs::write(&path, private).unwrap();
    let before = fs::read(f.ctx.data.join("state.json")).unwrap();
    let review = review_exit(&f.ctx, &json!({"id":i.id,"choice":"keep_current"})).unwrap();
    assert_eq!(fs::read(f.ctx.data.join("state.json")).unwrap(), before);
    let plan:Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    assert_eq!(plan.journal.changes.len(), 1);
    assert_eq!(plan.journal.changes[0].path, f.ctx.data.join("state.json"));
    apply(&f.ctx, &json!({"token":review["token"]})).unwrap();
    f.refresh();
    assert_eq!(f.ctx.install(&i.id).unwrap().launcher_exit_choice.as_deref(), Some("keep_current"));
    assert!(f.ctx.install(&i.id).unwrap().launcher_exit_reviewed);
    assert_eq!(fs::read(&path).unwrap(), private);
    f.games(&i);assert!(review_repair(&f.ctx,&json!({"id":i.id})).is_err());
    assert_eq!(fs::read(&path).unwrap(), private);
    assert_eq!(f.ctx.install(&i.id).unwrap().launcher_exit_choice.as_deref(), Some("keep_current"));
}
#[test]
fn c37_close_choice_survives_reruns_without_reapplying_vendor_setting() {
    let mut f=Fixture::new();let i=f.install();
    let path=i.prefix.join("pfx/drive_c/users/steamuser/AppData/Roaming/Battle.net/Battle.net.config");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, br#"{"Client":{"GameLaunchWindowBehavior":"0","untouched":true}}"#).unwrap();
    let review=review_exit(&f.ctx,&json!({"id":i.id,"confirm_installation_wide":true})).unwrap();
    apply(&f.ctx,&json!({"token":review["token"]})).unwrap();f.refresh();
    assert_eq!(fsutil::load::<Json>(&path).unwrap()["Client"]["GameLaunchWindowBehavior"],"2");
    assert_eq!(f.ctx.install(&i.id).unwrap().launcher_exit_choice.as_deref(),Some("close_after_play"));
    let changed=b"later user/vendor preference; repair must leave it alone";
    fs::write(&path,changed).unwrap();f.games(&i);assert!(review_repair(&f.ctx,&json!({"id":i.id})).is_err());
    assert_eq!(fs::read(&path).unwrap(),changed);
    assert!(f.ctx.install(&i.id).unwrap().launcher_exit_reviewed);
    let request=json!({"id":i.id,"location":f.ctx.home.join("Games"),"editions":["era"],"keep_settings":true});
    review_reinstall(&f.ctx,&request).unwrap();
    let plan:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let replacement=plan.replacement.unwrap();
    assert_eq!(replacement.launcher_exit_choice.as_deref(),Some("close_after_play"));
    assert!(!replacement.launcher_exit_reviewed);
    assert_eq!(fs::read(&path).unwrap(),changed);
}
#[test]
fn c37_repair_summary_lists_added_editions_without_rewriting_download_selection() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);
    f.ctx.state.installations[0].editions=vec!["era".into()];
    f.ctx.persist(f.ctx.state.clone()).unwrap();
    let review=review_repair(&f.ctx,&json!({"id":i.id})).unwrap();
    assert_eq!(review["choices"]["editions"],json!(["era","forever","retail"]));
    assert_eq!(review["choices"]["requested_editions"],json!(["era"]));
    assert_eq!(f.ctx.install(&i.id).unwrap().editions,vec!["era"]);
}
#[test]
fn c37_legacy_installations_load_unreviewed_without_mutation() {
    let mut f=Fixture::new();let i=f.install();
    let mut old=serde_json::to_value(i).unwrap();
    old.as_object_mut().unwrap().remove("launcher_exit_choice");
    old.as_object_mut().unwrap().remove("launcher_exit_reviewed");
    let decoded:Installation=serde_json::from_value(old).unwrap();
    assert_eq!(decoded.launcher_exit_choice,None);
    assert!(!decoded.launcher_exit_reviewed);
    workflow::installation(&decoded).unwrap();
}
