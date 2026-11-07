fn c42_shared(_f:&Fixture,i:&Installation)->vdf::Object {
    let root=steam::read_shortcuts(&steam::shortcuts_path(&i.steam,&i.user).unwrap()).unwrap();
    let entries=vdf::object(&root,"shortcuts").unwrap();
    let games:Vec<_>=entries.iter().filter_map(|(_,v)|match v {Value::Object(e) if steam::owned(e,&i.id) && vdf::string(e,"wowRustRole")==Some("shared_game")=>Some(e),_=>None}).collect();
    assert_eq!(games.len(),1);games[0].clone()
}
fn c42_legacy(f:&mut Fixture)->(Installation,Vec<u32>) {
    let i=f.install();f.games(&i);c42_seed_legacy(f,&i)
}
fn c42_seed_legacy(f:&mut Fixture, base:&Installation)->(Installation,Vec<u32>) {
    let mut i=base.clone();i.launcher_exit_choice=Some("close_after_play".into());i.launcher_exit_reviewed=true;
    i.launcher=Some(i.prefix.join("pfx/drive_c/Program Files (x86)/Battle.net/Battle.net.exe"));
    let mut root=vec![("shortcuts".into(),Value::Object(vec![]))];
    let bn=steam::entry("Battle.net",i.launcher.as_ref().unwrap(),&steam::launch_options(&i.prefix,None).unwrap(),&i.id,Some(i.bootstrap_id)).unwrap();
    steam::upsert(&mut root,bn,&i.id).unwrap();
    let games=associated_games(&discovery::scan(&f.ctx.home,&f.ctx.state).unwrap(),&i);
    let mut ids=vec![];
    for (n,g) in games.iter().enumerate() {
        let id=3000001001+n as u32;ids.push(id);
        let mut e=steam::entry(&g.name,i.launcher.as_ref().unwrap(),&products::options(&i.prefix,&g.id,None).unwrap(),&i.id,Some(id)).unwrap();
        vdf::set(&mut e,"wowRustGame",Value::Text(g.executable.to_string_lossy().into()));
        vdf::set(&mut e,"wowRustEdition",Value::Text(g.id.clone()));
        steam::upsert(&mut root,e,&i.id).unwrap();
    }
    fs::write(steam::shortcuts_path(&i.steam,&i.user).unwrap(),vdf::binary(&root).unwrap()).unwrap();
    i.shortcuts=vec![i.bootstrap_id];i.shortcuts.extend(ids.clone());i.bindings=steam::bindings(&root,&i.id);i.stage=Stage::Complete;
    f.ctx.state.installations[0]=i.clone();fsutil::json(&f.ctx.data.join("state.json"),&f.ctx.state).unwrap();
    fsutil::json(&i.root.join(".wow-rust-owner.json"),&i).unwrap();
    let cfg=i.steam.join("config/config.vdf");fs::write(&cfg,steam::set_proton(Some(&fs::read(&cfg).unwrap()),&i.shortcuts,&i.proton).unwrap()).unwrap();
    (i,ids)
}
#[test]
fn c42_fresh_shared_contract_and_rerun_reuse() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let shared=c42_shared(&f,&i);let id=vdf::number(&shared,"appid").unwrap();
    assert_eq!(vdf::string(&shared,"appname"),Some("Battle.net"));
    assert_eq!(vdf::string(&shared,"wowRustGame"),None);assert_eq!(vdf::string(&shared,"wowRustEdition"),None);
    assert_eq!(vdf::string(&shared,"LaunchOptions"),Some(steam::launch_options(&i.prefix,None).unwrap().as_str()));
    let tiles=shortcuts(&f.ctx,&json!({"id":i.id})).unwrap();
    assert_eq!(tiles["shortcuts"].as_array().unwrap().len(),1);assert_eq!(tiles["games"].as_array().unwrap().len(),3);
    assert_eq!(f.ctx.state.installations[0].bindings.iter().filter(|b|b.shared).count(),1);
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let before=fs::read(&path).unwrap();let config=fs::read(i.steam.join("config/config.vdf")).unwrap();
    f.repair(&i);assert_eq!(fs::read(&path).unwrap(),before);assert_eq!(fs::read(i.steam.join("config/config.vdf")).unwrap(),config);assert_eq!(vdf::number(&c42_shared(&f,&i),"appid"),Some(id));
}
#[test]
fn c42_each_selected_edition_has_same_neutral_shared_route() {
    for edition in ["retail","era","forever"] {
        let mut f=Fixture::new();let i=f.install();f.games(&i);
        f.ctx.state.installations[0].editions=vec![edition.into()];fsutil::json(&f.ctx.data.join("state.json"),&f.ctx.state).unwrap();
        f.repair(&i);let e=c42_shared(&f,&i);
        assert_eq!(steam::edition(vdf::string(&e,"LaunchOptions").unwrap()),None);
        assert_eq!(steam::compat_prefix(vdf::string(&e,"LaunchOptions").unwrap()),Some(i.prefix.clone()));
    }
}
#[test]
fn c42_upgrade_requires_explicit_valid_keeper_and_verified_backup() {
    let mut f=Fixture::new();let(i,ids)=c42_legacy(&mut f);
    assert!(review_repair(&f.ctx,&json!({"id":i.id})).unwrap_err().contains("Choose which"));
    assert!(review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":123456})).is_err());
    let layout=i.steam.join("steamapps/common/Steam Controller Configs").join(&i.user).join("config/custom.vdf");
    fs::create_dir_all(layout.parent().unwrap()).unwrap();fs::write(&layout,b"custom per-device controls").unwrap();
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let mut root=steam::read_shortcuts(&path).unwrap();
    let unrelated=steam::entry("Other game",Path::new("/unrelated/game"),"%command%","other",Some(12345)).unwrap();vdf::ensure(&mut root,"shortcuts").unwrap().push(("99".into(),Value::Object(unrelated.clone())));
    for(_,v) in vdf::ensure(&mut root,"shortcuts").unwrap(){if let Value::Object(e)=v {if vdf::number(e,"appid")==Some(ids[0]) {vdf::set(e,"icon",Value::Text("custom.png".into()));let old=vdf::string(e,"LaunchOptions").unwrap();vdf::set(e,"LaunchOptions",Value::Text(format!("CUSTOM=1 {old} --custom")));}}}
    fs::write(&path,vdf::binary(&root).unwrap()).unwrap();let before=fs::read(&path).unwrap();
    let q=review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":ids[0]})).unwrap();
    let plan:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();assert!(plan.shared_migration);
    let backup=backup_shared_migration(&f.ctx,&plan).unwrap();assert!(backup.join(".complete.json").exists());assert_eq!(fs::read(&path).unwrap(),before);
    apply(&f.ctx,&json!({"token":q["token"]})).unwrap();f.refresh();
    let e=c42_shared(&f,&i);assert_eq!(vdf::number(&e,"appid"),Some(ids[0]));assert_eq!(vdf::string(&e,"icon"),Some("custom.png"));assert!(vdf::string(&e,"LaunchOptions").unwrap().contains("CUSTOM=1"));assert!(vdf::string(&e,"LaunchOptions").unwrap().contains("--custom"));assert_eq!(steam::edition(vdf::string(&e,"LaunchOptions").unwrap()),None);
    let after=steam::read_shortcuts(&path).unwrap();assert!(vdf::object(&after,"shortcuts").unwrap().iter().any(|(_,v)|matches!(v,Value::Object(e)if *e==unrelated)));
    assert_eq!(fs::read(&layout).unwrap(),b"custom per-device controls");
    transaction::undo(&f.ctx.data,true).unwrap();assert_eq!(fs::read(&layout).unwrap(),b"custom per-device controls");assert_eq!(steam::read_shortcuts(&path).unwrap(),vdf::parse_binary(&before).unwrap());
}
#[test]
fn c42_stripped_owned_metadata_still_requires_migration_backup() {
    let mut f=Fixture::new();let(i,ids)=c42_legacy(&mut f);let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let mut root=steam::read_shortcuts(&path).unwrap();
    for(_,v) in vdf::ensure(&mut root,"shortcuts").unwrap(){if let Value::Object(e)=v{e.retain(|(k,_)|!["tags","wowRustRole","wowRustGame","wowRustEdition"].contains(&k.as_str()));}}
    fs::write(&path,vdf::binary(&root).unwrap()).unwrap();
    review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":ids[0]})).unwrap();
    let p:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();assert!(p.shared_migration);assert!(shared_migration_needed(&p).unwrap());
}
#[test]
fn c42_incomplete_or_corrupt_migration_backup_blocks_commit() {
    let mut f=Fixture::new();let(i,ids)=c42_legacy(&mut f);let q=review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":ids[0]})).unwrap();
    let plan:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();let b=backup_shared_migration(&f.ctx,&plan).unwrap();
    fs::write(b.join("inventory.json"),b"[]").unwrap();let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let before=fs::read(&path).unwrap();
    assert!(apply(&f.ctx,&json!({"token":q["token"]})).is_err());assert_eq!(fs::read(&path).unwrap(),before);
}
#[test]
fn c42_missing_saved_hint_never_claims_installed_client() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let mut saved=f.ctx.state.installations[0].clone();let fake=saved.root.join("missing/_ptr_/Wow.exe");saved.game_associations.push(Game{id:"ptr".into(),name:"missing".into(),path:fake.parent().unwrap().into(),executable:fake,prefix:None});
    let scan=discovery::scan(&f.ctx.home,&f.ctx.state).unwrap();let found=installation_games(&f.ctx,&saved,&scan).unwrap();assert_eq!(found.len(),3);assert!(!found.iter().any(|g|g.name=="missing"));
}
#[test]
fn c43_old_maintenance_route_is_retired_only_after_explicit_other_keeper() {
    let mut f=Fixture::new();let(i,ids)=c42_legacy(&mut f);let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let mut root=steam::read_shortcuts(&path).unwrap();
    for(_,v) in vdf::ensure(&mut root,"shortcuts").unwrap(){if let Value::Object(e)=v{if vdf::number(e,"appid")==Some(i.bootstrap_id){vdf::set(e,"LaunchOptions",Value::Text(steam::launch_options(&i.prefix,Some("WoW")).unwrap()));}}}
    fs::write(&path,vdf::binary(&root).unwrap()).unwrap();let q=review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":ids[0]})).unwrap();apply(&f.ctx,&json!({"token":q["token"]})).unwrap();
    let out=steam::read_shortcuts(&path).unwrap();assert!(!vdf::object(&out,"shortcuts").unwrap().iter().any(|(_,v)|matches!(v,Value::Object(e)if vdf::number(e,"appid")==Some(i.bootstrap_id))));assert_eq!(vdf::number(&c42_shared(&f,&i),"appid"),Some(ids[0]));
}
#[test]
fn c42_shared_tile_survives_metadata_strip_and_preserves_associations() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);let saved=f.ctx.state.installations[0].clone();let id=vdf::number(&c42_shared(&f,&i),"appid").unwrap();
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let mut root=steam::read_shortcuts(&path).unwrap();
    for(_,v) in vdf::ensure(&mut root,"shortcuts").unwrap(){if let Value::Object(e)=v{e.retain(|(k,_)|!["tags","wowRustRole","wowRustGame","wowRustEdition"].contains(&k.as_str()));}}
    fs::write(&path,vdf::binary(&root).unwrap()).unwrap();f.repair(&saved);assert_eq!(vdf::number(&c42_shared(&f,&i),"appid"),Some(id));assert_eq!(shortcuts(&f.ctx,&json!({"id":i.id})).unwrap()["games"].as_array().unwrap().len(),3);
}
#[test]
fn c42_owned_removal_covers_only_shared_and_launcher_tiles() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let sentinel=f.root.join("unrelated");fs::write(&sentinel,b"keep").unwrap();
    let q=review_clean(&f.ctx,&json!({"id":i.id,"keep_settings":true})).unwrap();apply(&f.ctx,&json!({"token":q["token"],"confirmation":"REMOVE THIS COPY"})).unwrap();
    assert_eq!(fs::read(&sentinel).unwrap(),b"keep");let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let root=steam::read_shortcuts(&path).unwrap();assert!(!vdf::object(&root,"shortcuts").unwrap().iter().any(|(_,v)|matches!(v,Value::Object(e)if steam::owned(e,&i.id))));
}

#[test]
fn c42_backup_detects_new_controller_record_before_commit() {
    let mut f=Fixture::new();let(i,ids)=c42_legacy(&mut f);
    let root=i.steam.join("steamapps/common/Steam Controller Configs").join(&i.user);fs::create_dir_all(&root).unwrap();fs::write(root.join("a.vdf"),b"A").unwrap();
    let q=review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":ids[0]})).unwrap();let p:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();backup_shared_migration(&f.ctx,&p).unwrap();
    fs::write(root.join("new.vdf"),b"new deliberate controls").unwrap();let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let old=fs::read(&path).unwrap();
    assert!(apply(&f.ctx,&json!({"token":q["token"]})).unwrap_err().contains("coverage changed"));assert_eq!(fs::read(path).unwrap(),old);
}
#[test]
fn c42_legacy_game_named_battle_net_keeps_selected_custom_fields() {
    let mut f=Fixture::new();let(i,ids)=c42_legacy(&mut f);let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let mut root=steam::read_shortcuts(&path).unwrap();
    for(_,v)in vdf::ensure(&mut root,"shortcuts").unwrap(){if let Value::Object(e)=v{if vdf::number(e,"appid")==Some(ids[0]){vdf::set(e,"appname",Value::Text("Battle.net".into()));vdf::set(e,"icon",Value::Text("custom.png".into()));}}}
    fs::write(&path,vdf::binary(&root).unwrap()).unwrap();let q=review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":ids[0]})).unwrap();apply(&f.ctx,&json!({"token":q["token"]})).unwrap();
    assert_eq!(vdf::number(&c42_shared(&f,&i),"appid"),Some(ids[0]));assert_eq!(vdf::string(&c42_shared(&f,&i),"icon"),Some("custom.png"));assert_eq!(vdf::object(&steam::read_shortcuts(&path).unwrap(),"shortcuts").unwrap().len(),1);
}
#[test]
fn c42_era_only_then_add_retail_keeps_one_shared_identity_and_game_bytes() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);
    for folder in ["_retail_","_forever_"] {fs::remove_dir_all(i.root.join("games/World of Warcraft").join(folder)).unwrap();}
    f.ctx.state.installations[0].editions=vec!["era".into()];f.ctx.save().unwrap();f.repair(&i);
    let id=vdf::number(&c42_shared(&f,&i),"appid").unwrap();let era=i.root.join("games/World of Warcraft/_classic_era_/Wow.exe");let before=fs::read(&era).unwrap();
    let retail=i.root.join("games/World of Warcraft/_retail_/Wow.exe");fs::create_dir_all(retail.parent().unwrap()).unwrap();fs::write(&retail,b"MZretail").unwrap();
    f.ctx.state.installations[0].editions.push("retail".into());f.ctx.save().unwrap();f.repair(&i);
    assert_eq!(vdf::number(&c42_shared(&f,&i),"appid"),Some(id));assert_eq!(fs::read(&era).unwrap(),before);assert_eq!(fs::read(&retail).unwrap(),b"MZretail");
    assert_eq!(shortcuts(&f.ctx,&json!({"id":i.id})).unwrap()["shortcuts"].as_array().unwrap().len(),1);
}


#[test]
fn c43_fresh_requires_reviewed_verified_exit_then_reuses_one_battle_net_id() {
    let mut f=Fixture::new();let i=f.install();
    let before=shortcuts(&f.ctx,&json!({"id":i.id})).unwrap();
    assert_eq!(before["shortcuts"].as_array().unwrap().len(),1);
    assert_eq!(before["shortcuts"][0]["name"],"Battle.net");
    f.games(&i);
    let path=exit_settings_path(&i);
    let original=br#"{"Client":{"GameLaunchWindowBehavior":"0","preserve":true},"opaque":"fixture-private"}"#;
    fs::write(&path,original).unwrap();
    f.ctx.state.installations[0].launcher_exit_reviewed=false;
    f.ctx.state.installations[0].launcher_exit_choice=None;
    fsutil::json(&f.ctx.data.join("state.json"),&f.ctx.state).unwrap();
    assert!(review_repair(&f.ctx,&json!({"id":i.id})).unwrap_err().contains("Automatic launcher exit"));
    assert!(review_exit(&f.ctx,&json!({"id":i.id,"choice":"close_after_play"})).is_err());
    assert_eq!(fs::read(&path).unwrap(),original);
    let q=review_exit(&f.ctx,&json!({"id":i.id,"choice":"close_after_play","confirm_installation_wide":true})).unwrap();
    let token=q["token"].as_str().unwrap().to_string();
    assert_eq!(fs::read(&path).unwrap(),original);
    apply(&f.ctx,&json!({"token":token})).unwrap();f.refresh();
    let saved=f.ctx.install(&i.id).unwrap();
    assert_eq!(exit_preference_status(&saved)["ready"],true);
    let backup=f.ctx.data.join("launcher-exit-backups").join(&token).join("Battle.net.config.original");
    assert_eq!(fs::read(backup).unwrap(),original);
    assert_eq!(fsutil::load::<Json>(&path).unwrap()["Client"]["preserve"],true);
    f.repair(&i);
    let ready=shortcuts(&f.ctx,&json!({"id":i.id})).unwrap();
    assert_eq!(ready["shortcuts"].as_array().unwrap().len(),1);
    assert_eq!(ready["shortcuts"][0]["name"],"Battle.net");
    assert_eq!(ready["shortcuts"][0]["appid"],i.bootstrap_id);
    assert_eq!(ready["single_tile_ready"],true);
    let bytes=fs::read(&path).unwrap();f.repair(&i);assert_eq!(fs::read(path).unwrap(),bytes);
}
#[test]
fn c43_current_preference_is_typed_fresh_and_never_silently_reapplied() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let saved=f.ctx.install(&i.id).unwrap();let path=exit_settings_path(&saved);
    for value in [json!(2),json!("0"),Json::Null,json!(false)] {
        fsutil::json(&path,&json!({"Client":{"GameLaunchWindowBehavior":value,"unchanged":true}})).unwrap();
        let bytes=fs::read(&path).unwrap();assert_eq!(exit_preference_status(&saved)["ready"],false);
        assert!(review_repair(&f.ctx,&json!({"id":i.id})).is_err());
        assert_eq!(fs::read(&path).unwrap(),bytes);
    }
    fs::write(&path,b"malformed").unwrap();assert_eq!(exit_preference_status(&saved)["state"],"unavailable");
    fs::remove_file(&path).unwrap();assert_eq!(exit_preference_status(&saved)["ready"],false);
}
#[test]
fn c43_current_enabled_can_be_reviewed_without_vendor_write_and_decline_stays_incomplete() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);let path=exit_settings_path(&i);
    for value in ["2","0"] {
        fsutil::json(&path,&json!({"Client":{"GameLaunchWindowBehavior":value,"keep":true}})).unwrap();
        let bytes=fs::read(&path).unwrap();
        let q=review_exit(&f.ctx,&json!({"id":i.id,"choice":"keep_current"})).unwrap();apply(&f.ctx,&json!({"token":q["token"]})).unwrap();f.refresh();
        assert_eq!(fs::read(&path).unwrap(),bytes);
        assert_eq!(exit_preference_status(&f.ctx.install(&i.id).unwrap())["ready"],value=="2");
    }
}
#[test]
fn c43_failed_exit_backup_refuses_before_any_vendor_write() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);let path=exit_settings_path(&i);
    fs::write(&path,br#"{"Client":{"GameLaunchWindowBehavior":"0"}}"#).unwrap();
    let q=review_exit(&f.ctx,&json!({"id":i.id,"confirm_installation_wide":true})).unwrap();
    fs::write(f.ctx.data.join("launcher-exit-backups"),b"blocked fixture").unwrap();
    let original=fs::read(&path).unwrap();assert!(apply(&f.ctx,&json!({"token":q["token"]})).is_err());assert_eq!(fs::read(path).unwrap(),original);
}
#[test]
fn c43_exit_review_refuses_concurrent_vendor_edits() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);let path=exit_settings_path(&i);
    let q=review_exit(&f.ctx,&json!({"id":i.id,"confirm_installation_wide":true})).unwrap();
    fs::write(&path,br#"{"Client":{"GameLaunchWindowBehavior":"0","new_user_setting":true}}"#).unwrap();
    let original=fs::read(&path).unwrap();assert!(apply(&f.ctx,&json!({"token":q["token"]})).is_err());assert_eq!(fs::read(path).unwrap(),original);
}
#[test]
fn c43_c42_two_tile_upgrade_preserves_selected_identity_and_controls() {
    for keep_launcher in [false,true] {
        let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
        let current=f.ctx.install(&i.id).unwrap();let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();
        let mut root=steam::read_shortcuts(&path).unwrap();
        for (_,v) in vdf::ensure(&mut root,"shortcuts").unwrap() {if let Value::Object(e)=v {if vdf::string(e,"wowRustRole")==Some("shared_game") {vdf::set(e,"appname",Value::Text("World of Warcraft".into()));}}}
        let mut bn=steam::entry("Battle.net",current.launcher.as_ref().unwrap(),&steam::launch_options(&i.prefix,None).unwrap(),&i.id,Some(3999999001)).unwrap();
        vdf::set(&mut bn,"wowRustRole",Value::Text("launcher".into()));vdf::ensure(&mut root,"shortcuts").unwrap().push(("1".into(),Value::Object(bn)));
        fs::write(&path,vdf::binary(&root).unwrap()).unwrap();
        f.ctx.state.installations[0].bindings=steam::bindings(&root,&i.id);fsutil::json(&f.ctx.data.join("state.json"),&f.ctx.state).unwrap();
        let control=i.steam.join("controller_base/sentinel.vdf");fs::create_dir_all(control.parent().unwrap()).unwrap();fs::write(&control,b"Era chosen custom controls").unwrap();
        let id=if keep_launcher {3999999001}else{i.bootstrap_id};
        assert!(review_repair(&f.ctx,&json!({"id":i.id})).is_err());
        let q=review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":id})).unwrap();
        let plan:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();assert!(plan.shared_migration);
        apply(&f.ctx,&json!({"token":q["token"]})).unwrap();f.refresh();
        let out=shortcuts(&f.ctx,&json!({"id":i.id})).unwrap();assert_eq!(out["shortcuts"].as_array().unwrap().len(),1);assert_eq!(out["shortcuts"][0]["name"],"Battle.net");assert_eq!(out["shortcuts"][0]["appid"],id);assert_eq!(out["single_tile_ready"],true);
        assert_eq!(fs::read(control).unwrap(),b"Era chosen custom controls");
    }
}
#[test]
fn c43_ambiguous_spaced_routing_refuses_without_guessing() {
    let p=Path::new("/fixture/prefix");
    assert!(!steam::ordinary_launcher("STEAM_COMPAT_DATA_PATH=/fixture/prefix %command% --exec launch"));
    assert!(steam::preserve_options("STEAM_COMPAT_DATA_PATH=/fixture/prefix %command% --exec launch",&steam::launch_options(p,None).unwrap()).is_err());
}

#[test]
fn c43_linked_exit_backup_refuses_before_vendor_or_state_write() {
    use std::os::unix::fs::symlink;
    for name in ["Battle.net.config.original","SHA256.json"] {
        let mut f=Fixture::new();let i=f.install();f.games(&i);let path=exit_settings_path(&i);
        let q=review_exit(&f.ctx,&json!({"id":i.id,"confirm_installation_wide":true})).unwrap();
        let dest=f.ctx.data.join("launcher-exit-backups").join(q["token"].as_str().unwrap());fsutil::private_dir(&dest).unwrap();
        symlink(&path,dest.join(name)).unwrap();
        let original=fs::read(&path).unwrap();let state=fs::read(f.ctx.data.join("state.json")).unwrap();
        assert!(apply(&f.ctx,&json!({"token":q["token"]})).unwrap_err().contains("Linked"));
        assert_eq!(fs::read(&path).unwrap(),original);assert_eq!(fs::read(f.ctx.data.join("state.json")).unwrap(),state);
    }
}

#[test]
fn c43_ordinary_chooser_requires_exactly_one_standalone_steam_command() {
    for options in ["STEAM_COMPAT_DATA_PATH=/fixture", "STEAM_COMPAT_DATA_PATH=/fixture %command% %command%", "STEAM_COMPAT_DATA_PATH=/fixture wrapper%command%"] {
        assert!(!steam::ordinary_launcher(options));
    }
    assert!(steam::ordinary_launcher("STEAM_COMPAT_DATA_PATH=/fixture CUSTOM=1 %command% --custom"));
}
