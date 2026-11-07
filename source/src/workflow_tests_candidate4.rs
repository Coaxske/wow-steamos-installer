#[test]
fn c4_apply_merges_unrelated_steam_edits_after_review() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    let q = review_repair(&f.ctx, &json!({"id":i.id})).unwrap();
    let cfg = i.steam.join("config/config.vdf");
    fs::write(
        &cfg,
        format!(
            "{}\n\"OtherApplication\" \"preserved\"",
            fs::read_to_string(&cfg).unwrap()
        ),
    )
    .unwrap();
    apply(&f.ctx, &json!({"token":q["token"]})).unwrap();
    assert!(fs::read_to_string(&cfg).unwrap().contains("preserved"));
}
#[test]
fn c4_apply_conflict_refuses_without_download_or_overwrite() {
    let f = Fixture::new();
    let q = review_setup(&f.ctx, &f.setup_request()).unwrap();
    let p: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let cfg = p.installation.steam.join("config/config.vdf");
    let conflicting = steam::set_proton(
        Some(&fs::read(&cfg).unwrap()),
        &[p.installation.bootstrap_id],
        "my_working_tool",
    )
    .unwrap();
    fs::write(&cfg, &conflicting).unwrap();
    assert!(apply(&f.ctx, &json!({"token":q["token"]})).is_err());
    assert_eq!(fs::read(&cfg).unwrap(), conflicting);
    assert!(!p.installation.root.exists());
}
#[test]
fn c4_complete_owned_cleanup_unlinks_external_links_and_keeps_backups() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    let game = i.root.join("games/World of Warcraft/_classic_era_");
    fs::create_dir_all(game.join("WTF")).unwrap();
    fs::write(game.join("WTF/config"), b"settings").unwrap();
    let outside = f.root.join("unrelated");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("keep"), b"keep").unwrap();
    symlink(&outside, i.prefix.join("outside-link")).unwrap();
    let owned_ids = f.ctx.state.installations[0].shortcuts.clone();
    let cfg = i.steam.join("config/config.vdf");
    let before =
        steam::set_proton(Some(&fs::read(&cfg).unwrap()), &[3212345678], "shared_tool").unwrap();
    fs::write(&cfg, before).unwrap();
    let q = review_clean(
        &f.ctx,
        &json!({"id":i.id,"complete_owned":true,"keep_settings":true}),
    )
    .unwrap();
    assert_eq!(q["kind"], "clean_owned");
    apply(
        &f.ctx,
        &json!({"token":q["token"],"confirmation":"REMOVE THIS COPY"}),
    )
    .unwrap();
    assert!(!i.root.exists());
    assert_eq!(fs::read(outside.join("keep")).unwrap(), b"keep");
    let backups = f.ctx.data.join("settings-backups");
    assert!(backups.is_dir());
    assert!(fs::read_dir(backups).unwrap().any(|e| fs::read(
        e.unwrap().path().join("era/WTF/config")
    )
    .map(|b| b == b"settings")
    .unwrap_or(false)));
    let after = fs::read(&cfg).unwrap();
    assert_eq!(
        steam::proton_for(&after, 3212345678).unwrap(),
        Some("shared_tool".into())
    );
    for id in owned_ids {
        assert_eq!(steam::proton_for(&after, id).unwrap(), None);
    }

    assert!(i.steam.join("steamapps/common/Proton 11/proton").exists());
}
#[test]
fn c4_complete_cleanup_refuses_unrelated_root_content() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    fs::write(i.root.join("unrelated.txt"), b"keep").unwrap();
    assert!(review_clean(&f.ctx, &json!({"id":i.id,"complete_owned":true})).is_err());
    assert!(i.root.join("unrelated.txt").exists());
    assert!(review_clean(&f.ctx, &json!({"id":i.id})).is_ok());
}
#[test]
fn c4_complete_cleanup_resumes_owned_root_quarantine() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    review_clean(&f.ctx, &json!({"id":i.id,"complete_owned":true})).unwrap();
    let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let mut job = Cleanup {
        plan,
        settings_saved: true,
    };
    transaction::finish(&f.ctx.data, &mut job.plan.journal, false).unwrap();
    let r = &job.plan.remove[0];
    fs::rename(&r.path, &r.quarantine).unwrap();
    fsutil::json(&f.ctx.data.join("cleanup.json"), &job).unwrap();
    run_cleanup(&f.ctx, &mut job).unwrap();
    assert!(!i.root.exists());
    assert!(!job.plan.remove[0].quarantine.exists());
}
#[test]
fn c4_setup_keeps_addon_choice_and_normalizes_required_launcher_tile() {
    let mut f = Fixture::new();
    let mut req = f.setup_request();
    req["launcher_tile"] = json!(false);
    req["addon_manager"] = json!("curseforge");
    let q = review_setup(&f.ctx, &req).unwrap();
    let p: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    fs::create_dir_all(&p.installation.root).unwrap();
    fs::write(
        p.installation.root.join("Battle.net-Setup.exe"),
        test_installer(),
    )
    .unwrap();
    apply(&f.ctx, &json!({"token":q["token"]})).unwrap();
    f.refresh();
    let i = f.ctx.state.installations[0].clone();
    assert!(i.launcher_tile);
    assert_eq!(i.addon_manager, "curseforge");
    f.games(&i);
    f.repair(&i);
    let root = steam::read_shortcuts(&steam::shortcuts_path(&i.steam, &i.user).unwrap()).unwrap();
    assert!(vdf::object(&root, "shortcuts").unwrap().iter().any(
        |(_, v)| matches!(v,Value::Object(e)if vdf::string(e,"wowRustRole")==Some("shared_game")&&vdf::string(e,"appname")==Some("Battle.net"))
    ));
}

#[test]
fn c4_complete_quarantine_stops_if_another_profile_appears() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    f.repair(&i);
    review_clean(&f.ctx, &json!({"id":i.id,"complete_owned":true})).unwrap();
    let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let mut job = Cleanup {
        plan,
        settings_saved: true,
    };
    transaction::finish(&f.ctx.data, &mut job.plan.journal, false).unwrap();
    let r = &job.plan.remove[0];
    fs::rename(&r.path, &r.quarantine).unwrap();
    fs::create_dir_all(i.steam.join("userdata/456/config")).unwrap();
    let other = steam::shortcuts_path(&i.steam, "456").unwrap();
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    let entry = steam::entry(
        "Other profile",
        &i.root.join("another.exe"),
        "",
        "external",
        None,
    )
    .unwrap();
    fs::write(
        other,
        vdf::binary(&vec![(
            "shortcuts".into(),
            Value::Object(vec![("0".into(), Value::Object(entry))]),
        )])
        .unwrap(),
    )
    .unwrap();
    assert!(run_cleanup(&f.ctx, &mut job).is_err());
    assert!(job.plan.remove[0].quarantine.exists());
}

#[test]
fn c4_gear_versioned_names_do_not_confuse_other_managers() {
    assert!(gear_is_curseforge(&json!({"name":"CurseForge"})));
    assert!(gear_is_curseforge(&json!({"name":"CurseForge (0.1.2)"})));
    assert!(!gear_is_curseforge(
        &json!({"name":"WowUp with CurseForge"})
    ));
    assert!(!gear_is_curseforge(&json!({"name":"CurseForge helper"})));
}
