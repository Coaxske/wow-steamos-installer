// Real engine / synthetic temporary fixture: Steam drops nonstandard VDF keys.
#[test]
fn c10_markerless_native_gear_tile_survives_scoped_and_owned_removal() {
    for complete in [false, true] {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        f.repair(&i);
        let shared = f.ctx.home.join("AppImages/curseforge.appimage");
        fs::create_dir_all(shared.parent().unwrap()).unwrap();
        fs::write(&shared, b"shared native app fixture").unwrap();
        let desktop = f
            .ctx
            .home
            .join(".local/share/applications/curseforge.desktop");
        fs::create_dir_all(desktop.parent().unwrap()).unwrap();
        fs::write(&desktop, b"[Desktop Entry]\nType=Application\n").unwrap();
        let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
        let mut root = steam::read_shortcuts(&path).unwrap();
        let mut native = steam::entry(
            "My CurseForge",
            std::path::Path::new("/usr/bin/env"),
            &format!("gio launch {}", fsutil::shell_quote(&desktop).unwrap()),
            &i.id,
            None,
        )
        .unwrap();
        vdf::set(
            &mut native,
            "wowRustNativeAddon",
            Value::Text("gearlever".into()),
        );
        let addon = steam::upsert(&mut root, native, &i.id).unwrap();
        let unrelated = steam::entry(
            "Unrelated app",
            std::path::Path::new("/unrelated/app"),
            "--custom-option",
            "unrelated",
            None,
        )
        .unwrap();
        let other = steam::upsert(&mut root, unrelated, "unrelated").unwrap();
        for (_, value) in vdf::ensure(&mut root, "shortcuts").unwrap() {
            if let Value::Object(e) = value {
                e.retain(|(key, _)| !key.to_ascii_lowercase().starts_with("wowrust"));
            }
        }
        let native_before = vdf::object(&root, "shortcuts")
            .unwrap()
            .iter()
            .find_map(|(_, v)| match v {
                Value::Object(e) if vdf::number(e, "appid") == Some(addon) => {
                    Some(vdf::binary(e).unwrap())
                }
                _ => None,
            })
            .unwrap();
        fs::write(&path, vdf::binary(&root).unwrap()).unwrap();
        let layout = i.steam.join("controller_base/c10-layout.vdf");
        fs::create_dir_all(layout.parent().unwrap()).unwrap();
        fs::write(&layout, b"keep controller fixture").unwrap();
        let proton = i.steam.join("steamapps/common/Proton 11/proton");
        let proton_before = fs::read(&proton).unwrap();
        let response = review_clean(
            &f.ctx,
            &json!({"id":i.id,"complete_owned":complete,"keep_settings":true}),
        )
        .unwrap();
        let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
        let proposed = &plan
            .journal
            .changes
            .iter()
            .find(|c| c.path == path)
            .unwrap()
            .after;
        let proposed = vdf::parse_binary(proposed).unwrap();
        assert!(
            vdf::object(&proposed, "shortcuts")
                .unwrap()
                .iter()
                .any(|(_, v)| matches!(v,Value::Object(e) if vdf::number(e,"appid")==Some(addon))),
            "Native tile must be retained in review after Steam metadata loss"
        );
        apply(
            &f.ctx,
            &json!({"token":response["token"],"confirmation":"REMOVE THIS COPY"}),
        )
        .unwrap();
        let after = steam::read_shortcuts(&path).unwrap();
        let entries = vdf::object(&after, "shortcuts").unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|(_, v)| matches!(v,Value::Object(e) if vdf::number(e,"appid")==Some(other))));
        let native_after = entries
            .iter()
            .find_map(|(_, v)| match v {
                Value::Object(e) if vdf::number(e, "appid") == Some(addon) => {
                    Some(vdf::binary(e).unwrap())
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(native_before, native_after);
        assert!(shared.is_file() && desktop.is_file());
        assert_eq!(fs::read(layout).unwrap(), b"keep controller fixture");
        assert_eq!(fs::read(proton).unwrap(), proton_before);
    }
}
#[test]
fn c10_native_route_classification_never_depends_on_custom_marker() {
    let mut e = steam::entry(
        "Renamed",
        std::path::Path::new("/usr/bin/env"),
        "gio launch '/home/test/.local/share/applications/curseforge.desktop'",
        "fixture",
        None,
    )
    .unwrap();
    assert!(steam::native_addon(&e));
    let root = vec![(
        "shortcuts".into(),
        Value::Object(vec![("0".into(), Value::Object(e.clone()))]),
    )];
    assert!(steam::bindings(&root, "fixture").is_empty());
    vdf::set(&mut e, "exe", Value::Text("\"/wine/launcher.exe\"".into()));
    assert!(!steam::native_addon(&e));
    vdf::set(&mut e, "exe", Value::Text("\"/usr/bin/env\"".into()));
    for options in [
        "gio launch ",
        "gio launch",
        "STEAM_COMPAT_DATA_PATH='/prefix' %command%",
        "gio open file:///tmp/example",
    ] {
        vdf::set(&mut e, "LaunchOptions", Value::Text(options.into()));
        assert!(!steam::native_addon(&e));
    }
}
