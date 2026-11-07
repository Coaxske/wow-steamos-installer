// These tests exercise production filesystem mutations in disposable fixtures.
fn live_prepare_fixture(f: &mut Fixture, cleanup: bool) -> (String, Installation) {
    let mut request = f.setup_request();
    request["action"] = json!("live_prepare");
    request["cleanup"] = json!(cleanup);
    let result = live_tests::dispatch(&mut f.ctx, &request).unwrap();
    let p: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let i = p.installation;
    assert!(!i.root.exists());
    let bytes = test_installer();
    fs::create_dir_all(&i.root).unwrap();
    fs::write(i.root.join("Battle.net-Setup.exe"), bytes).unwrap();
    (result["run"]["token"].as_str().unwrap().into(), i)
}
fn live_advance(f: &mut Fixture, token: &str) -> Json {
    live_tests::dispatch(
        &mut f.ctx,
        &json!({"action":"live_step","token":token,"approved":true}),
    )
    .unwrap()
}
fn live_phase(f: &Fixture, phase: &str) {
    let path = f.ctx.data.join("live-test.json");
    let mut value: Json = fsutil::load(&path).unwrap();
    value["phase"] = json!(phase);
    fsutil::json(&path, &value).unwrap();
}
#[test]
fn live_run_requires_review_and_rejects_stale_token() {
    let mut f = Fixture::new();
    let (token, _) = live_prepare_fixture(&mut f, false);
    assert!(
        live_tests::dispatch(&mut f.ctx, &json!({"action":"live_step","token":token})).is_err()
    );
    assert!(live_tests::dispatch(
        &mut f.ctx,
        &json!({"action":"live_step","token":"wrong","approved":true})
    )
    .is_err());
    assert!(f.ctx.state.installations.is_empty());
}
#[test]
fn live_run_real_mutations_are_scoped_and_repeatable() {
    let mut f = Fixture::new();
    let existing = f.install();
    f.games(&existing);
    f.repair(&existing);
    let (token, i) = live_prepare_fixture(&mut f, true);
    assert_eq!(live_advance(&mut f, &token)["run"]["phase"], "installer");
    f.games(&i);
    // No actual external windows in unit tests; only the launch boundary is skipped.
    live_phase(&f, "handoff");
    live_advance(&mut f, &token);
    live_advance(&mut f, &token);
    live_phase(&f, "repair");
    live_advance(&mut f, &token);
    live_advance(&mut f, &token);
    live_advance(&mut f, &token);
    let r = live_advance(&mut f, &token);
    assert_eq!(r["run"]["phase"], "exit_setting");
    let settings = i
        .prefix
        .join("pfx/drive_c/users/steamuser/AppData/Roaming/Battle.net/Battle.net.config");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let original = b"{\"Client\":{\"GameLaunchWindowBehavior\":\"0\",\"Other\":\"keep\"}}";
    fs::write(&settings, original).unwrap();
    live_advance(&mut f, &token);
    live_advance(&mut f, &token);
    assert_ne!(fs::read(&settings).unwrap(), original);
    live_advance(&mut f, &token);
    assert_eq!(fs::read(&settings).unwrap(), original);
    live_phase(&f, "finish");
    live_advance(&mut f, &token);
    live_advance(&mut f, &token);
    let result = live_advance(&mut f, &token);
    assert_eq!(result["run"]["phase"], "done");
    assert!(existing
        .root
        .join("games/World of Warcraft/_retail_/Wow.exe")
        .exists());
    assert!(!i.root.join("games/World of Warcraft").exists());
    assert_eq!(f.ctx.state.installations.len(), 1);
    assert!(!result["text"]
        .as_str()
        .unwrap()
        .contains(f.ctx.home.to_str().unwrap()));
    assert!(!result["text"].as_str().unwrap().contains("\"user\""));
}
#[test]
fn live_run_reconciles_applied_plan_without_reapplying() {
    let mut f = Fixture::new();
    let (token, _) = live_prepare_fixture(&mut f, false);
    let path = f.ctx.data.join("live-test.json");
    let mut checkpoint: Json = fsutil::load(&path).unwrap();
    checkpoint["approved"] = json!(true);
    checkpoint["phase"] = json!("setup");
    fsutil::json(&path, &checkpoint).unwrap();
    apply(&f.ctx, &json!({"token":checkpoint["pending"]["token"]})).unwrap();
    f.refresh();
    let before = fs::read(f.ctx.data.join("last-transaction.json")).unwrap();
    assert_eq!(live_advance(&mut f, &token)["run"]["phase"], "installer");
    assert_eq!(
        fs::read(f.ctx.data.join("last-transaction.json")).unwrap(),
        before
    );
}
#[test]
fn live_run_refuses_foreign_scope_and_changed_settings() {
    let mut f = Fixture::new();
    let (token, i) = live_prepare_fixture(&mut f, false);
    live_advance(&mut f, &token);
    f.ctx.state.installations[0].prefix = f.ctx.home.join("foreign");
    assert!(
        live_tests::dispatch(&mut f.ctx, &json!({"action":"live_step","token":token})).is_err()
    );
    f.ctx.state.installations[0] = i;
    let p = f.ctx.data.join("live-test.json");
    let mut r: Json = fsutil::load(&p).unwrap();
    r["source"] = json!("other-build");
    fsutil::json(&p, &r).unwrap();
    assert!(
        live_tests::dispatch(&mut f.ctx, &json!({"action":"live_result"})).unwrap()["run"]
            .is_null()
    );
}
#[test]
fn live_run_does_not_treat_launch_request_as_observation() {
    let mut f = Fixture::new();
    let (token, _) = live_prepare_fixture(&mut f, false);
    live_advance(&mut f, &token);
    live_phase(&f, "observed");
    assert!(
        live_tests::dispatch(&mut f.ctx, &json!({"action":"live_step","token":token})).is_err()
    );
    let result = live_tests::dispatch(&mut f.ctx, &json!({"action":"live_result"})).unwrap();
    assert_eq!(result["run"]["phase"], "observed");
}
#[test]
fn live_reinstall_preserves_and_restores_test_settings() {
    let mut f = Fixture::new();
    let (token, i) = live_prepare_fixture(&mut f, false);
    let path = f.ctx.data.join("live-test.json");
    let mut r: Json = fsutil::load(&path).unwrap();
    r["reinstall"] = json!(true);
    fsutil::json(&path, &r).unwrap();
    live_advance(&mut f, &token);
    f.games(&i);
    f.repair(&i);
    let settings = i.root.join("games/World of Warcraft/_retail_/WTF");
    fs::create_dir_all(&settings).unwrap();
    fs::write(settings.join("Config.wtf"), "test setting").unwrap();
    live_phase(&f, "reinstall");
    live_advance(&mut f, &token);
    let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let new = plan.replacement.unwrap();
    fs::create_dir_all(&new.root).unwrap();
    let bytes = test_installer();
    fs::write(new.root.join("Battle.net-Setup.exe"), bytes).unwrap();
    assert_eq!(live_advance(&mut f, &token)["run"]["phase"], "installer");
    assert!(!settings.exists());
    f.games(&new);
    f.confirm_fixture_exit(&new);f.repair(&new);
    live_phase(&f, "restore");
    let result = live_advance(&mut f, &token);
    assert_eq!(result["run"]["phase"], "exit_setting");
    assert_eq!(
        fs::read_to_string(
            new.root
                .join("games/World of Warcraft/_retail_/WTF/Config.wtf")
        )
        .unwrap(),
        "test setting"
    );
}
#[test]
fn live_plan_cannot_write_an_unrelated_file() {
    let mut f = Fixture::new();
    let (token, _) = live_prepare_fixture(&mut f, false);
    let path = f.ctx.data.join("live-test.json");
    let mut r: Json = fsutil::load(&path).unwrap();
    let sentinel = f.ctx.home.join("unrelated");
    fs::write(&sentinel, "keep").unwrap();
    r["pending"]["journal"]["changes"][0]["path"] = json!(sentinel);
    fsutil::json(&path, &r).unwrap();
    assert!(live_tests::dispatch(
        &mut f.ctx,
        &json!({"action":"live_step","token":token,"approved":true})
    )
    .is_err());
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "keep");
}
#[test]
fn download_log_refuses_symlink_without_truncating_target() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let response = review_setup(&f.ctx, &f.setup_request()).unwrap();
    let sentinel = f.root.join("private-file");
    fs::write(&sentinel, "unchanged").unwrap();
    symlink(&sentinel, f.ctx.data.join("download-error.txt")).unwrap();
    assert!(apply(&f.ctx, &json!({"token":response["token"]})).is_err());
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "unchanged");
    assert!(!f.ctx.data.join("state.json").exists());
}

#[test]
fn live_stop_keeps_files_and_allows_another_review() {
    let mut f = Fixture::new();
    let (token, i) = live_prepare_fixture(&mut f, false);
    live_advance(&mut f, &token);
    let result =
        live_tests::dispatch(&mut f.ctx, &json!({"action":"live_stop","token":token})).unwrap();
    assert_eq!(result["run"]["phase"], "done");
    assert!(i.root.exists());
    assert!(f.ctx.install(&i.id).is_ok());
    let mut request = f.setup_request();
    request["action"] = json!("live_prepare");
    let next = live_tests::dispatch(&mut f.ctx, &request).unwrap();
    assert_ne!(next["run"]["token"], token);
    assert_eq!(
        fs::read_dir(f.ctx.data.join("previous-live-tests"))
            .unwrap()
            .count(),
        1
    );
}
