// Disposable production Rust capability/result tests; never host desktop actions.
fn c35_setup() -> (Fixture, Installation, Json, C12Path) {
    let mut f = Fixture::new();
    let i = f.install();
    c30_desktop(&f);
    let route = c12_flatpak(&f, true);
    let review = addon_lifecycle_review(&f.ctx, &json!({"id":i.id})).unwrap();
    (f, i, review, route)
}
fn c35_evidence(i: &Installation, r: &Json) -> Json {
    let identity = |pid: u64| json!({"pid":pid,"uid":unsafe{libc::geteuid()},"start":"100","executable_sha256":"a".repeat(64)});
    json!({"kind":"curseforge_lifecycle","installation":i.id,"review_token":r["token"],"source":crate::release::SOURCE,"ok":true,"normal_exit_verified":true,"reopened_verified":true,"initial_selection":"Classic","initial_selection_observed":true,"initial_selection_required":true,"initial_selection_restored":true,"exit_delivery":"acknowledged","before_identity":identity(111),"after_identity":identity(222),"registration_status":"not_tested","persistence_status":"not_tested","results":[],"diagnostic":{"phase":"restore_initial_selection"}})
}
#[test]
fn c35_review_needs_no_game_registration_proof() {
    let (f, i, r, _route) = c35_setup();
    assert_eq!(r["kind"], "curseforge_lifecycle");
    let plan =
        valid_addon_operation_plan(&f.ctx, &i.id, r["token"].as_str().unwrap(), true).unwrap();
    assert!(plan.targets.is_empty());
    assert!(registration_targets(&f.ctx, &i.id).is_err());
    assert!(!f.ctx.data.join("addon-registration-review.json").exists());
}
#[test]
fn c35_review_namespaces_and_scope_are_separate() {
    let (mut f, i, r, _route) = c35_setup();
    let token = r["token"].as_str().unwrap();
    assert!(valid_registration_plan(&f.ctx, &i.id, token).is_err());
    assert!(valid_addon_operation_plan(&f.ctx, "different", token, true).is_err());
    assert!(valid_addon_operation_plan(&f.ctx, &i.id, "old", true).is_err());
    f.ctx.state.installations[0].language = "deDE".into();
    assert!(valid_addon_operation_plan(&f.ctx, &i.id, token, true).is_err());
}
#[test]
fn c35_confirm_and_private_pause_refuse_before_job() {
    let (f, i, r, _route) = c35_setup();
    assert!(addon_lifecycle_apply(
        &f.ctx,
        &json!({"id":i.id,"token":r["token"],"confirm":false})
    )
    .is_err());
    let pause = f.ctx.data.join("addon-registration-private-pause");
    fs::write(&pause, b"private pause").unwrap();
    assert!(addon_lifecycle_apply(
        &f.ctx,
        &json!({"id":i.id,"token":r["token"],"confirm":true})
    )
    .unwrap_err()
    .contains("Private authentication"));
    assert_eq!(fs::read(&pause).unwrap(), b"private pause");
    assert!(!f.ctx.data.join("addon-lifecycle-job.json").exists());
    assert!(!f.ctx.data.join("addon-lifecycle-attempt.json").exists());
}
#[test]
fn c35_consumed_attempt_never_replays_exit() {
    let (f, i, r, _route) = c35_setup();
    fsutil::json(
        &f.ctx.data.join("addon-lifecycle-attempt.json"),
        &json!({"token":r["token"]}),
    )
    .unwrap();
    assert!(addon_lifecycle_apply(
        &f.ctx,
        &json!({"id":i.id,"token":r["token"],"confirm":true})
    )
    .unwrap_err()
    .contains("already consumed"));
    assert!(!f.ctx.data.join("addon-lifecycle-job.json").exists());
}
#[test]
fn c35_changed_route_expired_review_and_unknown_owner_refuse() {
    let (mut f, i, r, _route) = c35_setup();
    let token = r["token"].as_str().unwrap();
    let path = f.ctx.data.join("addon-lifecycle-review.json");
    let mut plan: AddonRegistrationPlan = fsutil::load(&path).unwrap();
    plan.created = 0;
    fsutil::json(&path, &plan).unwrap();
    assert!(valid_addon_operation_plan(&f.ctx, &i.id, token, true).is_err());
    let r = addon_lifecycle_review(&f.ctx, &json!({"id":i.id})).unwrap();
    fs::write(
        f.ctx
            .home
            .join(".local/share/applications/curseforge.desktop"),
        b"changed route",
    )
    .unwrap();
    assert!(valid_addon_operation_plan(&f.ctx, &i.id, r["token"].as_str().unwrap(), true).is_err());
    f.ctx.state.installations[0].owned = false;
    assert!(addon_lifecycle_review(&f.ctx, &json!({"id":i.id})).is_err());
}
#[test]
fn c35_lifecycle_success_never_claims_registration() {
    let (f, i, r, _route) = c35_setup();
    let plan =
        valid_addon_operation_plan(&f.ctx, &i.id, r["token"].as_str().unwrap(), true).unwrap();
    let mut value = c35_evidence(&i, &r);
    let result = checked_lifecycle_result(&value, &plan).unwrap();
    assert_eq!(result["lifecycle_ok"], true);
    assert_eq!(result["registration_status"], "not_tested");
    assert_eq!(result["persistence_status"], "not_tested");
    assert_eq!(result["results"], json!([]));
    assert!(result.get("operation_ok").is_none());
    for key in ["registration_status", "persistence_status"] {
        value[key] = json!("passed");
        assert!(checked_lifecycle_result(&value, &plan).is_err());
        value[key] = json!("not_tested");
    }
    value["results"] = json!([{"path_verified":true}]);
    assert!(checked_lifecycle_result(&value, &plan).is_err());
}
#[test]
fn c35_stages_and_fresh_same_app_identity_required() {
    let (f, i, r, _route) = c35_setup();
    let plan =
        valid_addon_operation_plan(&f.ctx, &i.id, r["token"].as_str().unwrap(), true).unwrap();
    let good = c35_evidence(&i, &r);
    for key in [
        "normal_exit_verified",
        "reopened_verified",
        "initial_selection_observed",
        "initial_selection_restored",
    ] {
        let mut v = good.clone();
        v[key] = json!(false);
        assert!(checked_lifecycle_result(&v, &plan).is_err(), "{key}");
    }
    for key in ["review_token", "source", "installation", "kind"] {
        let mut v = good.clone();
        v[key] = json!("wrong");
        assert!(checked_lifecycle_result(&v, &plan).is_err(), "{key}");
    }
    let mut v = good.clone();
    v["after_identity"] = v["before_identity"].clone();
    assert!(checked_lifecycle_result(&v, &plan).is_err());
    for key in ["uid", "executable_sha256", "start", "pid"] {
        let mut v = good.clone();
        v["after_identity"][key] = json!("invalid");
        assert!(checked_lifecycle_result(&v, &plan).is_err(), "{key}");
    }
    let mut v = good;
    v["after_identity"]["executable_sha256"] = json!("b".repeat(64));
    assert!(checked_lifecycle_result(&v, &plan).is_err());
}
#[test]
fn c35_partial_exit_and_unknown_reply_are_not_overall_pass() {
    let (f, i, r, _route) = c35_setup();
    let plan =
        valid_addon_operation_plan(&f.ctx, &i.id, r["token"].as_str().unwrap(), true).unwrap();
    let mut v = c35_evidence(&i, &r);
    v["ok"] = json!(false);
    v["exit_delivery"] = json!("outcome_unknown");
    v["reopened_verified"] = json!(false);
    v["initial_selection_restored"] = json!(false);
    v["after_identity"] = Json::Null;
    let partial = checked_lifecycle_result(&v, &plan).unwrap();
    assert_eq!(partial["normal_exit_verified"], true);
    assert_eq!(partial["lifecycle_ok"], false);
    assert_eq!(partial["persistence_status"], "not_tested");
}
#[test]
fn c35_result_query_refuses_stale_evidence_and_changed_scope() {
    let (mut f, i, r, _route) = c35_setup();
    let req = json!({"id":i.id,"token":r["token"]});
    let file = f.ctx.data.join("addon-lifecycle-result.json");
    let mut v = c35_evidence(&i, &r);
    fsutil::json(&file, &v).unwrap();
    assert_eq!(
        addon_lifecycle_result(&f.ctx, &req).unwrap()["lifecycle_ok"],
        true
    );
    v["review_token"] = json!("older-job");
    fsutil::json(&file, &v).unwrap();
    assert!(addon_lifecycle_result(&f.ctx, &req).is_err());
    v = c35_evidence(&i, &r);
    fsutil::json(&file, &v).unwrap();
    f.ctx.state.installations[0].proton = "changed".into();
    assert!(addon_lifecycle_result(&f.ctx, &req).is_err());
}

#[test]
fn c35_cancellation_is_scoped_stop_only_even_with_owner_lock() {
    let (f, i, r, _route) = c35_setup();
    let req = json!({"id":i.id,"token":r["token"]});
    assert!(addon_lifecycle_cancel(&f.ctx, &req).is_err());
    let job = json!({"kind":"curseforge_lifecycle","installation":i.id,"token":r["token"],"source":crate::release::SOURCE});
    fsutil::json(&f.ctx.data.join("addon-lifecycle-job.json"), &job).unwrap();
    assert!(addon_lifecycle_cancel(&f.ctx, &json!({"id":i.id,"token":"stale"})).is_err());
    assert!(!f.ctx.data.join("addon-lifecycle-cancel").exists());
    let _lock = fsutil::Lock::acquire(&f.ctx.data.join("operation.lock")).unwrap();
    let stopped = addon_lifecycle_cancel(&f.ctx, &req).unwrap();
    assert_eq!(stopped["cancel_requested"], true);
    assert!(stopped.get("lifecycle_ok").is_none());
    assert_eq!(
        fs::read(f.ctx.data.join("addon-lifecycle-cancel")).unwrap(),
        b"cancel"
    );
}
#[test]
fn c35_changed_source_or_root_identity_invalidates_review() {
    let (f, i, r, _route) = c35_setup();
    let path = f.ctx.data.join("addon-lifecycle-review.json");
    let mut plan: AddonRegistrationPlan = fsutil::load(&path).unwrap();
    plan.source = "old-source".into();
    fsutil::json(&path, &plan).unwrap();
    assert!(valid_addon_operation_plan(&f.ctx, &i.id, r["token"].as_str().unwrap(), true).is_err());
    plan.source = crate::release::SOURCE.into();
    plan.root_identity = json!({"dev":0,"ino":0});
    fsutil::json(&path, &plan).unwrap();
    assert!(valid_addon_operation_plan(&f.ctx, &i.id, r["token"].as_str().unwrap(), true).is_err());
}
