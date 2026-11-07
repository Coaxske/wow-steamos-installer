// Candidate 6 regressions: real engine/private filesystem fixtures, never live clients.
fn c6_job(f: &Fixture, i: &Installation, keep: bool, complete: bool) -> Cleanup {
    review_clean(
        &f.ctx,
        &json!({"id":i.id,"keep_settings":keep,"complete_owned":complete}),
    )
    .unwrap();
    Cleanup {
        plan: fsutil::load(&f.ctx.data.join("review.json")).unwrap(),
        settings_saved: false,
    }
}
fn c6_settings(i: &Installation) -> PathBuf {
    let game = i.root.join("games/World of Warcraft/_classic_era_");
    fs::create_dir_all(game.join("WTF")).unwrap();
    fs::write(game.join("WTF/Config.wtf"), b"settings").unwrap();
    fs::create_dir_all(game.join("Interface/AddOns/Fixture")).unwrap();
    fs::write(game.join("Interface/AddOns/Fixture/data.lua"), b"addon").unwrap();
    game
}
fn c6_other_reference(f: &Fixture, i: &Installation) -> PathBuf {
    let root = f.ctx.home.join(".steam/steam");
    fs::create_dir_all(root.join("steamapps")).unwrap();
    let path = root.join("userdata/456/config/shortcuts.vdf");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let entry = steam::entry(
        "Unrelated shared launcher",
        &i.root.join("anything.exe"),
        &steam::launch_options(&i.prefix, None).unwrap(),
        "foreign",
        None,
    )
    .unwrap();
    fs::write(
        &path,
        vdf::binary(&vec![(
            "shortcuts".into(),
            Value::Object(vec![("0".into(), Value::Object(entry))]),
        )])
        .unwrap(),
    )
    .unwrap();
    path
}
#[test]
fn c6_missing_or_empty_executable_preserves_settings_for_scoped_and_owned_removal() {
    for complete in [false, true] {
        for missing in [false, true] {
            let mut f = Fixture::new();
            let i = f.install();
            f.games(&i);
            f.repair(&i);
            let game = c6_settings(&i);
            if missing {
                fs::remove_file(game.join("Wow.exe")).unwrap();
            } else {
                fs::write(game.join("Wow.exe"), b"").unwrap();
            }
            let mut job = c6_job(&f, &i, true, complete);
            run_cleanup(&f.ctx, &mut job).unwrap();
            let backup = f.ctx.data.join("settings-backups").join(&job.plan.token);
            assert_eq!(
                fs::read(backup.join("era/WTF/Config.wtf")).unwrap(),
                b"settings"
            );
            assert_eq!(
                fs::read(backup.join("era/Interface/AddOns/Fixture/data.lua")).unwrap(),
                b"addon"
            );
            assert!(backup.join(".inventory.json").is_file());
            assert!(!game.exists());
        }
    }
}
#[test]
fn c6_settings_discovery_limit_depth_and_unreadable_reader_are_errors() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    let game = c6_settings(&i);
    let job = c6_job(&f, &i, true, false);
    assert!(
        settings_inventory_with(&f.ctx, &job, 0, 24, &settings_read_dir)
            .unwrap_err()
            .contains("uncertain")
    );
    assert!(
        settings_inventory_with(&f.ctx, &job, 50_000, 0, &settings_read_dir)
            .unwrap_err()
            .contains("uncertain")
    );
    let err = settings_inventory_with(&f.ctx, &job, 50_000, 24, &|_| {
        Err("Permission denied (simulated reader)".into())
    })
    .unwrap_err();
    assert!(err.contains("Permission denied"));
    assert_eq!(fs::read(game.join("WTF/Config.wtf")).unwrap(), b"settings");
    assert!(!f
        .ctx
        .data
        .join("settings-backups")
        .join(&job.plan.token)
        .exists());
}
#[test]
fn c6_unrecognized_settings_client_refuses_removal_before_steam_changes() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    let odd = i.root.join("games/World of Warcraft/unknown-client/WTF");
    fs::create_dir_all(&odd).unwrap();
    fs::write(odd.join("keep"), b"keep").unwrap();
    let mut job = c6_job(&f, &i, true, false);
    let shortcuts = steam::shortcuts_path(&i.steam, &i.user).unwrap();
    let before = fs::read(&shortcuts).unwrap();
    assert!(run_cleanup(&f.ctx, &mut job)
        .unwrap_err()
        .contains("unrecognized"));
    assert_eq!(fs::read(shortcuts).unwrap(), before);
    assert!(odd.join("keep").exists());
}
#[test]
fn c6_unverifiable_legacy_backup_marker_refuses_deletion() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    let game = c6_settings(&i);
    let mut job = c6_job(&f, &i, true, false);
    let backup = f.ctx.data.join("settings-backups").join(&job.plan.token);
    fs::create_dir_all(&backup).unwrap();
    fsutil::json(&backup.join(".complete.json"), &job.plan.token).unwrap();
    assert!(run_cleanup(&f.ctx, &mut job)
        .unwrap_err()
        .contains("inventory"));
    assert!(game.join("WTF/Config.wtf").exists());
}
#[test]
fn c6_backup_corruption_and_new_settings_on_resume_refuse_deletion() {
    for corrupt_backup in [false, true] {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let game = c6_settings(&i);
        let mut job = c6_job(&f, &i, true, false);
        preserve_cleanup_settings(&f.ctx, &mut job).unwrap();
        if corrupt_backup {
            fs::write(
                f.ctx
                    .data
                    .join("settings-backups")
                    .join(&job.plan.token)
                    .join("era/WTF/Config.wtf"),
                b"corrupt",
            )
            .unwrap();
        } else {
            fs::write(game.join("WTF/new-settings"), b"new").unwrap();
        }
        assert!(run_cleanup(&f.ctx, &mut job).is_err());
        assert!(game.join("WTF/Config.wtf").exists());
        assert!(!job.plan.journal.done);
    }
}
#[test]
fn c6_verified_backup_committed_before_saved_flag_resumes() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    c6_settings(&i);
    let mut job = c6_job(&f, &i, true, false);
    preserve_cleanup_settings(&f.ctx, &mut job).unwrap();
    job.settings_saved = false;
    run_cleanup(&f.ctx, &mut job).unwrap();
    assert!(job.settings_saved);
    assert_eq!(
        fs::read(
            f.ctx
                .data
                .join("settings-backups")
                .join(&job.plan.token)
                .join("era/WTF/Config.wtf")
        )
        .unwrap(),
        b"settings"
    );
}
#[test]
fn c6_partial_quarantine_delete_resumes_only_with_verified_backup() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    c6_settings(&i);
    let mut job = c6_job(&f, &i, true, false);
    preserve_cleanup_settings(&f.ctx, &mut job).unwrap();
    transaction::finish(&f.ctx.data, &mut job.plan.journal, false).unwrap();
    let removal = &job.plan.remove[0];
    fs::rename(&removal.path, &removal.quarantine).unwrap();
    fs::remove_file(removal.quarantine.join("_classic_era_/WTF/Config.wtf")).unwrap();
    run_cleanup(&f.ctx, &mut job).unwrap();
    assert!(!job.plan.remove[0].quarantine.exists());
    assert_eq!(
        fs::read(
            f.ctx
                .data
                .join("settings-backups")
                .join(&job.plan.token)
                .join("era/WTF/Config.wtf")
        )
        .unwrap(),
        b"settings"
    );
}
#[test]
fn c6_second_steam_root_reference_blocks_scoped_preview() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    let foreign = c6_other_reference(&f, &i);
    let before = fs::read(&foreign).unwrap();
    assert!(review_clean(&f.ctx, &json!({"id":i.id}))
        .unwrap_err()
        .contains("Another Steam root"));
    assert_eq!(fs::read(foreign).unwrap(), before);
    assert!(i.root.join("games/World of Warcraft").exists());
}
#[test]
fn c6_reference_introduced_before_resume_blocks_before_and_after_quarantine() {
    for staged in [false, true] {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let mut job = c6_job(&f, &i, false, false);
        if staged {
            transaction::finish(&f.ctx.data, &mut job.plan.journal, false).unwrap();
            let r = &job.plan.remove[0];
            fs::rename(&r.path, &r.quarantine).unwrap();
        }
        let foreign = c6_other_reference(&f, &i);
        let before = fs::read(&foreign).unwrap();
        assert!(run_cleanup(&f.ctx, &mut job)
            .unwrap_err()
            .contains("Another Steam root"));
        assert_eq!(fs::read(foreign).unwrap(), before);
        if staged {
            assert!(job.plan.remove[0].quarantine.exists());
        } else {
            assert!(job.plan.remove[0].path.exists());
        }
    }
}
#[test]
fn c6_reinstall_changed_and_omitted_options_match_review_and_execution() {
    for explicit in [false, true] {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        f.ctx.state.installations[0].addon_manager = "wowup_cf".into();
        f.ctx.state.installations[0].launcher_tile = false;
        f.ctx.save().unwrap();
        let mut req = json!({"id":i.id,"location":f.ctx.home.join("Games"),"editions":["era"],"keep_settings":false});
        if explicit {
            req["addon_manager"] = json!("curseforge");
            req["launcher_tile"] = json!(true);
        }
        let manager = if explicit { "curseforge" } else { "wowup_cf" };
        let tile = true;
        let response = review_reinstall(&f.ctx, &req).unwrap();
        let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
        let new = plan.replacement.as_ref().unwrap();
        assert_eq!(new.addon_manager, manager);
        assert_eq!(new.launcher_tile, tile);
        assert_eq!(response["choices"]["addon_manager"], manager);
        assert_eq!(response["choices"]["launcher_tile"], tile);
        fs::create_dir_all(&new.root).unwrap();
        fs::write(new.root.join("Battle.net-Setup.exe"), test_installer()).unwrap();
        apply(
            &f.ctx,
            &json!({"token":response["token"],"confirmation":"REMOVE THIS COPY"}),
        )
        .unwrap();
        f.refresh();
        assert_eq!(f.ctx.state.installations[0].addon_manager, manager);
        assert_eq!(f.ctx.state.installations[0].launcher_tile, tile);
    }
}
#[test]
fn c6_original_reinstall_choice_reproduction_and_explicit_none() {
    for manager in ["curseforge", "none"] {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let q=review_reinstall(&f.ctx,&json!({"id":i.id,"location":f.ctx.home.join("Games"),"editions":["era"],"addon_manager":manager,"launcher_tile":false})).unwrap();
        let p: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
        assert_eq!(p.replacement.unwrap().addon_manager, manager);
        assert_eq!(q["choices"]["launcher_tile"], true);
    }
}
