#[test]
fn completed_setup_cannot_retarget_a_game_as_the_installer() {
    let mut f = Fixture::new(); let i = f.install(); f.games(&i); f.repair(&i);
    let before = fs::read(steam::shortcuts_path(&i.steam, &i.user).unwrap()).unwrap();
    assert!(review_launcher(&f.ctx, &json!({"id":i.id})).is_err());
    assert_eq!(before, fs::read(steam::shortcuts_path(&i.steam, &i.user).unwrap()).unwrap());
}
#[test]
fn explicit_forever_beta_association_survives_repair_and_preservation() {
    let mut f = Fixture::new(); let i = f.install(); f.games(&i);
    let old = i.root.join("games/World of Warcraft/_forever_");
    let beta = old.with_file_name("_classic_beta_"); fs::rename(old, &beta).unwrap();
    fs::create_dir_all(beta.join("WTF")).unwrap(); fs::write(beta.join("WTF/Config.wtf"), b"settings").unwrap();
    assert!(review_repair(&f.ctx, &json!({"id":i.id})).is_err());
    let plan = review_repair(&f.ctx, &json!({"id":i.id,"confirm_forever_beta":true})).unwrap();
    apply(&f.ctx, &json!({"token":plan["token"]})).unwrap(); f.refresh();
    let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let before = fs::read(&path).unwrap(); f.repair(&i);
    assert_eq!(before, fs::read(&path).unwrap());
    let p = review_clean(&f.ctx, &json!({"id":i.id,"keep_settings":true})).unwrap();
    apply(&f.ctx, &json!({"token":p["token"],"confirmation":"REMOVE THIS COPY"})).unwrap();
    assert!(f.ctx.data.join("settings-backups").join(p["token"].as_str().unwrap()).join("forever/WTF/Config.wtf").is_file());
}
#[test]
fn routed_forever_beta_adoption_preserves_id_without_extra_game_tile() {
    let mut f = Fixture::new();
    let steam = f.ctx.home.join(".local/share/Steam");
    let prefix = steam.join("steamapps/compatdata/900");
    let bn = prefix.join("pfx/drive_c/Program Files/Battle.net");
    let game = prefix.join("pfx/drive_c/Program Files/World of Warcraft/_classic_beta_");
    fs::create_dir_all(&bn).unwrap(); fs::create_dir_all(&game).unwrap();
    fs::write(bn.join("Battle.net.exe"), b"MZ").unwrap(); fs::write(game.join("WowClassicB.exe"), b"MZ").unwrap();
    let path = steam::shortcuts_path(&steam, "123").unwrap();
    let mut root = steam::read_shortcuts(&path).unwrap();
    let entry = steam::entry("My Forever", &bn.join("Battle.net.exe"), &steam::launch_options(&prefix, Some("WoWF")).unwrap(), "old", None).unwrap();
    let id = steam::upsert(&mut root, entry, "old").unwrap();
    fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
    let cfg = steam.join("config/config.vdf");
    fs::write(&cfg, steam::set_proton(Some(&fs::read(&cfg).unwrap()), &[id], "proton_10").unwrap()).unwrap();
    let request = json!({"prefix":prefix,"steam":steam,"user":"123","appid":id,"selected":[id]});
    let p=review_adopt(&f.ctx,&request).unwrap(); apply(&f.ctx,&json!({"token":p["token"]})).unwrap(); f.refresh();
    let i=f.ctx.state.installations[0].clone(); f.confirm_fixture_exit(&i);f.repair(&i);
    let root=steam::read_shortcuts(&path).unwrap(); let entries=vdf::object(&root,"shortcuts").unwrap();
    assert_eq!(entries.len(),1);
    let entry=entries.iter().find_map(|(_,v)|match v{Value::Object(e) if vdf::number(e,"appid")==Some(id)=>Some(e),_=>None}).unwrap();
    assert_eq!(vdf::string(entry,"appname"),Some("Battle.net"));
    assert_eq!(vdf::string(entry,"wowRustRole"),Some("shared_game"));
    assert!(f.ctx.state.installations[0].game_associations.iter().any(|g|g.id=="forever"));
    assert_eq!(steam::product(vdf::string(entry,"LaunchOptions").unwrap()),None);
    assert_eq!(f.ctx.state.installations[0].proton,"proton_10");
}
#[test]
fn cleanup_keeps_native_addon_tile_and_new_copy_can_reuse_it() {
    use std::os::unix::fs::PermissionsExt;
    let mut f=Fixture::new(); let i=f.install(); f.games(&i); f.repair(&i);
    let app=f.root.join("manager.AppImage"); let mut bytes=vec![0;2048];
    bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");bytes[8..11].copy_from_slice(b"AI\x02");bytes[18]=62;
    fs::write(&app,bytes).unwrap();fs::set_permissions(&app,fs::Permissions::from_mode(0o700)).unwrap();
    let p=review_addon(&f.ctx,&json!({"id":i.id,"path":app,"manager":"wowup_wago"})).unwrap();
    apply(&f.ctx,&json!({"token":p["token"]})).unwrap();
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();
    let p=review_clean(&f.ctx,&json!({"id":i.id})).unwrap();
    apply(&f.ctx,&json!({"token":p["token"],"confirmation":"REMOVE THIS COPY"})).unwrap();f.refresh();
    let root=steam::read_shortcuts(&path).unwrap(); assert_eq!(vdf::object(&root,"shortcuts").unwrap().len(),1);
    let second=f.install();
    let p=review_addon(&f.ctx,&json!({"id":second.id,"path":app,"manager":"wowup_wago"})).unwrap();
    apply(&f.ctx,&json!({"token":p["token"]})).unwrap();
    let root=steam::read_shortcuts(&path).unwrap(); assert_eq!(vdf::object(&root,"shortcuts").unwrap().len(),2);
}
#[test]
fn other_application_blocks_cleanup_before_review() {
    let mut f=Fixture::new(); let i=f.install(); f.games(&i);
    fs::create_dir_all(i.prefix.join("pfx/drive_c/Program Files/Other Game")).unwrap();
    assert!(review_clean(&f.ctx,&json!({"id":i.id})).is_err());
    assert!(i.root.join("games/World of Warcraft").exists());
}
#[test]
fn cleanup_cancellation_is_available_only_before_configuration_changes() {
    let mut f=Fixture::new(); let i=f.install(); f.games(&i);
    review_clean(&f.ctx,&json!({"id":i.id})).unwrap();
    let plan:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let mut job=Cleanup{plan,settings_saved:false};
    fsutil::json(&f.ctx.data.join("cleanup.json"),&job).unwrap();
    cancel_cleanup(&f.ctx).unwrap();assert!(!f.ctx.data.join("cleanup.json").exists());
    assert!(i.root.join("games/World of Warcraft").exists());
    transaction::finish(&f.ctx.data,&mut job.plan.journal,false).unwrap();
    fsutil::json(&f.ctx.data.join("cleanup.json"),&job).unwrap();
    assert!(cancel_cleanup(&f.ctx).is_err());
}

#[test]
fn launcher_handoff_preserves_proton_changed_in_steam() {
    let mut f=Fixture::new(); let i=f.install();
    let bn=i.prefix.join("pfx/drive_c/Program Files/Battle.net");
    fs::create_dir_all(&bn).unwrap(); fs::write(bn.join("Battle.net.exe"), b"MZ").unwrap();
    let cfg=i.steam.join("config/config.vdf");
    fs::write(&cfg,steam::set_proton(Some(&fs::read(&cfg).unwrap()),&[i.bootstrap_id],"proton_10").unwrap()).unwrap();
    let p=review_launcher(&f.ctx,&json!({"id":i.id})).unwrap();
    apply(&f.ctx,&json!({"token":p["token"]})).unwrap(); f.refresh();
    assert_eq!(f.ctx.state.installations[0].proton,"proton_10");
}
#[test]
fn restore_does_not_silently_finish_with_missing_saved_editions() {
    let mut f=Fixture::new(); let i=f.install(); f.games(&i);
    let key=fsutil::unique(); let backup=f.ctx.data.join("settings-backups").join(&key);
    for id in ["retail","ptr"] {
        fs::create_dir_all(backup.join(id).join("WTF")).unwrap();
        fs::write(backup.join(id).join("WTF/Config.wtf"),b"saved").unwrap();
    }
    fsutil::json(&backup.join(".complete.json"),&key).unwrap();
    assert!(restore_settings(&mut f.ctx,&json!({"id":i.id,"backup":key})).is_err());
    assert!(!i.root.join("games/World of Warcraft/_retail_/WTF").exists());
    assert!(!f.ctx.data.join("restore.json").exists());
}

#[test]
fn shortcut_response_identifies_one_battle_net_for_all_detected_games() {
    let mut f = Fixture::new(); let i = f.install(); f.games(&i); f.repair(&i);
    let result = shortcuts(&f.ctx, &json!({"id":i.id})).unwrap();
    let tiles = result["shortcuts"].as_array().unwrap();
    assert_eq!(tiles.iter().filter(|t| t["game"] == true).count(), 1);
    assert_eq!(tiles.iter().filter(|t| t["game"] == false && t["addon"] == false).count(), 0);
}

#[test]
fn device_check_reads_working_tiles_without_changing_steam_or_progress() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let paths=[steam::shortcuts_path(&i.steam,&i.user).unwrap(),i.steam.join("config/config.vdf"),f.ctx.data.join("state.json")];
    let before:Vec<_>=paths.iter().map(|p|fs::read(p).unwrap()).collect();
    let r=crate::device_tests::run(&f.ctx.home,&f.ctx.data,Some(&i.id)).unwrap();
    assert_eq!(r.game_count,3);
    assert_eq!(r.automatic[6].outcome,crate::device_tests::Outcome::Pass);
    assert!(r.manual.iter().any(|m|m.id=="game_forever"));
    assert!(r.manual.iter().all(|m|m.outcome==crate::device_tests::Outcome::NotTested));
    assert_eq!(before,paths.iter().map(|p|fs::read(p).unwrap()).collect::<Vec<_>>());
    fs::remove_file(f.ctx.state.installations[0].launcher.as_ref().unwrap()).unwrap();
    let r=crate::device_tests::run(&f.ctx.home,&f.ctx.data,Some(&i.id)).unwrap();
    assert_eq!(r.automatic[6].outcome,crate::device_tests::Outcome::Attention);
}
#[test]
fn device_check_flags_duplicates_without_exporting_tile_names() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let mut root=steam::read_shortcuts(&path).unwrap();
    let entries=vdf::ensure(&mut root,"shortcuts").unwrap();
    let mut duplicate=entries.iter().find_map(|(_,v)|match v{Value::Object(e)if vdf::string(e,"wowRustRole")==Some("shared_game")=>Some(e.clone()),_=>None}).unwrap();
    vdf::set(&mut duplicate,"appid",Value::I32(42));vdf::set(&mut duplicate,"appname",Value::Text("private@example.test".into()));
    entries.push(("99".into(),Value::Object(duplicate)));fs::write(&path,vdf::binary(&root).unwrap()).unwrap();
    let before=fs::read(&path).unwrap();let r=crate::device_tests::run(&f.ctx.home,&f.ctx.data,Some(&i.id)).unwrap();
    assert_eq!(r.automatic[7].outcome,crate::device_tests::Outcome::Attention);
    assert!(!serde_json::to_string(&r).unwrap().contains("private@"));assert_eq!(before,fs::read(&path).unwrap());
    fs::write(&path,b"not a valid shortcut file").unwrap();
    let r=crate::device_tests::run(&f.ctx.home,&f.ctx.data,Some(&i.id)).unwrap();
    assert_eq!(r.automatic[7].outcome,crate::device_tests::Outcome::NotTested);
}
