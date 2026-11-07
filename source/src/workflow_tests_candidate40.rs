#[test]
fn c40_pointer_actions_unavailable_without_creating_review_or_backup() {
    let f = Fixture::new();
    let before = fs::read_dir(&f.ctx.data).unwrap().count();
    for request in [
        json!({"action":"controller_review","selected":[123],"replace_custom":true}),
        json!({"action":"controller_apply","token":"stale"}),
    ] {
        assert_eq!(controller_review(&f.ctx, &request).unwrap_err(), CONTROLLER_AUTOMATION_UNAVAILABLE);
        assert_eq!(controller_apply(&f.ctx, &request).unwrap_err(), CONTROLLER_AUTOMATION_UNAVAILABLE);
        assert_eq!(dispatch_inner(request).unwrap_err(), CONTROLLER_AUTOMATION_UNAVAILABLE);
    }
    assert_eq!(fs::read_dir(&f.ctx.data).unwrap().count(), before);
    assert!(!f.ctx.data.join("controller-review.json").exists());
    assert!(!f.ctx.data.join("controller-backups").exists());
}

#[test]
fn c40_stale_controller_material_and_custom_layout_are_untouched() {
    let mut f = Fixture::new();
    let i = f.install();
    let layout = i.steam.join("custom-controller-layout.vdf");
    fs::write(&layout, b"deliberate custom shared controls").unwrap();
    let names = ["controller-job.json", "controller-review.json", "controller-result.json", "controller-permission"];
    for name in names {
        fs::write(f.ctx.data.join(name), b"preserved historical bytes, not a new job").unwrap();
    }
    let backup = f.ctx.data.join("controller-backups/preserved");
    fs::create_dir_all(&backup).unwrap();
    fs::write(backup.join("original.bin"), b"verified old backup").unwrap();
    assert_eq!(controller_apply(&f.ctx, &json!({"id":i.id,"token":"stale","replace_custom":true})).unwrap_err(), CONTROLLER_AUTOMATION_UNAVAILABLE);
    assert_eq!(validate_controller_target(json!({"installation":i.id,"appid":i.bootstrap_id})).unwrap_err(), CONTROLLER_AUTOMATION_UNAVAILABLE);
    for name in names {
        assert_eq!(fs::read(f.ctx.data.join(name)).unwrap(), b"preserved historical bytes, not a new job");
    }
    assert_eq!(fs::read(&layout).unwrap(), b"deliberate custom shared controls");
    assert_eq!(fs::read(backup.join("original.bin")).unwrap(), b"verified old backup");
}

#[test]
fn c40_retained_pointer_job_does_not_block_normal_shortcut_repair() {
    let mut f = Fixture::new();
    let i = f.install();
    f.games(&i);
    let job = f.ctx.data.join("controller-job.json");
    fs::write(&job, b"preserved legacy checkpoint").unwrap();
    f.repair(&i);
    assert_eq!(fs::read(&job).unwrap(), b"preserved legacy checkpoint");
    assert!(steam::read_shortcuts(&steam::shortcuts_path(&i.steam, &i.user).unwrap()).is_ok());
}
