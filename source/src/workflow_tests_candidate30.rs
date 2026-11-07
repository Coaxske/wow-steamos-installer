// Disposable production-engine fixtures. No desktop, authentication or manager DB.
fn c30_games(_f: &Fixture, i: &Installation) {
    let base = i
        .prefix
        .join("pfx/drive_c/Program Files (x86)/World of Warcraft");
    for (folder, exe, product) in [
        ("_retail_", "Wow.exe", "wow"),
        ("_classic_era_", "WowClassic.exe", "wow_classic_era"),
    ] {
        let p = base.join(folder);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join(exe), b"MZ-client").unwrap();
        fs::write(
            p.join(".build.info"),
            format!("Product!STRING:0|Active!DEC:1\n{product}|1\n"),
        )
        .unwrap();
    }
}
fn c30_desktop(f: &Fixture) {
    let p = f
        .ctx
        .home
        .join(".local/share/applications/curseforge.desktop");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, b"[Desktop Entry]\nType=Application\n").unwrap();
}
#[test]
fn c30_verified_targets_and_shared_parent() {
    let mut f = Fixture::new();
    let i = f.install();
    c30_games(&f, &i);
    let t = registration_targets(&f.ctx, &i.id).unwrap();
    assert_eq!(t.len(), 2);
    assert_eq!(t[0]["folder"], t[1]["folder"]);
    assert_ne!(t[0]["client"], t[1]["client"]);
    assert_eq!(t[0]["edition"], "era");
}
#[test]
fn c30_unknown_metadata_and_no_games_refuse() {
    let mut f = Fixture::new();
    let i = f.install();
    assert!(registration_targets(&f.ctx, &i.id).is_err());
    c30_games(&f, &i);
    fs::remove_file(
        i.prefix
            .join("pfx/drive_c/Program Files (x86)/World of Warcraft/_retail_/.build.info"),
    )
    .unwrap();
    assert!(registration_targets(&f.ctx, &i.id)
        .unwrap_err()
        .contains("metadata"));
}
#[test]
fn c30_review_consent_and_scope_refuse_before_helper() {
    let mut f = Fixture::new();
    let i = f.install();
    c30_games(&f, &i);
    c30_desktop(&f);
    let _path = c12_flatpak(&f, true);
    let r = addon_registration_review(&f.ctx, &json!({"id":i.id})).unwrap();
    assert!(valid_registration_plan(&f.ctx, &i.id, r["token"].as_str().unwrap()).is_ok());
    assert!(valid_registration_plan(&f.ctx, "another", r["token"].as_str().unwrap()).is_err());
    assert!(valid_registration_plan(&f.ctx, &i.id, "stale").is_err());
    assert!(addon_registration_apply(
        &f.ctx,
        &json!({"id":i.id,"token":r["token"],"confirm":false})
    )
    .is_err());
    assert!(!f.ctx.data.join("addon-registration-job.json").exists());
}
#[test]
fn c30_route_or_client_change_invalidates_review() {
    let mut f = Fixture::new();
    let i = f.install();
    c30_games(&f, &i);
    c30_desktop(&f);
    let _path = c12_flatpak(&f, true);
    let r = addon_registration_review(&f.ctx, &json!({"id":i.id})).unwrap();
    fs::write(
        i.prefix
            .join("pfx/drive_c/Program Files (x86)/World of Warcraft/_retail_/Wow.exe"),
        b"changed",
    )
    .unwrap();
    assert!(valid_registration_plan(&f.ctx, &i.id, r["token"].as_str().unwrap()).is_err());
    c30_games(&f, &i);
    let r = addon_registration_review(&f.ctx, &json!({"id":i.id})).unwrap();
    fs::write(
        f.ctx
            .home
            .join(".local/share/applications/curseforge.desktop"),
        b"changed route",
    )
    .unwrap();
    assert!(valid_registration_plan(&f.ctx, &i.id, r["token"].as_str().unwrap()).is_err());
}
#[test]
fn c30_expired_review_refuses() {
    let mut f = Fixture::new();
    let i = f.install();
    c30_games(&f, &i);
    c30_desktop(&f);
    let _path = c12_flatpak(&f, true);
    let r = addon_registration_review(&f.ctx, &json!({"id":i.id})).unwrap();
    let mut p: AddonRegistrationPlan =
        fsutil::load(&f.ctx.data.join("addon-registration-review.json")).unwrap();
    p.created -= 901;
    fsutil::json(&f.ctx.data.join("addon-registration-review.json"), &p).unwrap();
    assert!(valid_registration_plan(&f.ctx, &i.id, r["token"].as_str().unwrap()).is_err());
}

#[test]
fn c30_outside_addon_link_refuses() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new();
    let i = f.install();
    c30_games(&f, &i);
    let client = i
        .prefix
        .join("pfx/drive_c/Program Files (x86)/World of Warcraft/_retail_");
    fs::create_dir_all(client.join("Interface")).unwrap();
    let outside = f.root.join("unrelated-addons");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep"), "keep").unwrap();
    symlink(&outside, client.join("Interface/AddOns")).unwrap();
    assert!(registration_targets(&f.ctx, &i.id)
        .unwrap_err()
        .contains("outside"));
    assert_eq!(fs::read(outside.join("keep")).unwrap(), b"keep");
}

#[test]
fn c30_job_targets_are_bound_to_review() {
    let mut f = Fixture::new();
    let i = f.install();
    c30_games(&f, &i);
    c30_desktop(&f);
    let _path = c12_flatpak(&f, true);
    let r = addon_registration_review(&f.ctx, &json!({"id":i.id})).unwrap();
    let p: AddonRegistrationPlan =
        fsutil::load(&f.ctx.data.join("addon-registration-review.json")).unwrap();
    let mut req = json!({"id":i.id,"token":r["token"],"targets":p.targets,"desktop":p.desktop});
    assert!(valid_registration_job(&f.ctx, &req).is_ok());
    req["targets"][0]["folder"] = json!("/unreviewed");
    assert!(valid_registration_job(&f.ctx, &req).is_err());
}
#[test]
fn c30_reference_changed_after_review_refuses() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new();
    let i = f.install();
    c30_games(&f, &i);
    c30_desktop(&f);
    let _path = c12_flatpak(&f, true);
    let r = addon_registration_review(&f.ctx, &json!({"id":i.id})).unwrap();
    let client = i
        .prefix
        .join("pfx/drive_c/Program Files (x86)/World of Warcraft/_retail_");
    fs::create_dir_all(client.join("Interface")).unwrap();
    let outside = f.root.join("outside-on-resume");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, client.join("Interface/AddOns")).unwrap();
    assert!(valid_registration_plan(&f.ctx, &i.id, r["token"].as_str().unwrap()).is_err());
    assert!(!f.ctx.data.join("addon-registration-job.json").exists());
}

#[test]
fn c33_partial_result_requires_current_scope_token_and_source() {
 let mut f=Fixture::new();let i=f.install();c30_games(&f,&i);c30_desktop(&f);let _path=c12_flatpak(&f,true);
 let plan=addon_registration_review(&f.ctx,&json!({"id":i.id})).unwrap();
 let req=json!({"id":i.id,"token":plan["token"]});
 let mut partial=json!({"installation":i.id,"review_token":plan["token"],"source":crate::release::SOURCE,"ok":false,"results":[{"edition":"era","path_verified":true,"persistence_verified":false,"verified_path":"/fixture/observed"}],"initial_selection_required":true,"initial_selection_restored":false,"diagnostic":{"phase":"normal_close_and_reopen"}});
 fsutil::json(&f.ctx.data.join("addon-registration-result.json"),&partial).unwrap();let result=addon_registration_result(&f.ctx,&req).unwrap();assert_eq!(result["operation_ok"],false);assert_eq!(result["results"][0]["path_verified"],true);assert_eq!(result["results"][0]["persistence_verified"],false);assert!(result.get("review_token").is_none());
 partial["review_token"]=json!("old");fsutil::json(&f.ctx.data.join("addon-registration-result.json"),&partial).unwrap();assert!(addon_registration_result(&f.ctx,&req).is_err());
 partial["review_token"]=plan["token"].clone();partial["source"]=json!("old-source");fsutil::json(&f.ctx.data.join("addon-registration-result.json"),&partial).unwrap();assert!(addon_registration_result(&f.ctx,&req).is_err());
 partial["source"]=json!(crate::release::SOURCE);partial["installation"]=json!("another");fsutil::json(&f.ctx.data.join("addon-registration-result.json"),&partial).unwrap();assert!(addon_registration_result(&f.ctx,&req).is_err());
}
