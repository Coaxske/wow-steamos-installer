// Real shared production engine; disposable files/mocked Gear Lever, not live Steam.
#[test]
fn c13_new_native_ids_are_scoped_and_stable_without_order_change() {
    let a = steam::native_entry("CurseForge", "gio launch '/home/deck/curseforge.desktop'", "owner-a").unwrap();
    let b = steam::native_entry("CurseForge", "gio launch '/home/deck/curseforge.desktop'", "owner-b").unwrap();
    let updated = steam::native_entry("CurseForge", "gio launch '/new path/curseforge.desktop'", "owner-a").unwrap();
    let id = vdf::number(&a, "appid").unwrap();
    assert_ne!(id, 4011762609);
    assert_ne!(id, vdf::number(&b,"appid").unwrap());
    assert_eq!(id, vdf::number(&updated,"appid").unwrap());
    assert_ne!(id & 0x80000000,0);
    assert!(matches!(vdf::get(&a,"appid"),Some(Value::I32(_))));
    assert_eq!(a[0].0,"appname"); // Deliberately NOT the unsupported order-only fix.
    let legacy=steam::entry("CurseForge",std::path::Path::new("/usr/bin/env"),"gio launch '/home/deck/curseforge.desktop'","owner-a",None).unwrap();
    let strip_id=|e:crate::vdf::Object|e.into_iter().filter(|(k,_)|k!="appid").collect::<crate::vdf::Object>();
    assert_eq!(strip_id(a.clone()),strip_id(legacy));
    assert_eq!(vdf::parse_binary(&vdf::binary(&a).unwrap()).unwrap(),a);
    for title in ["Install Battle.net","Battle.net","WoW Classic Era","World of Warcraft","World of Warcraft Forever"] {
        let target=std::path::Path::new("/home/deck/WoW-fixture/Battle.net.exe");
        let a=steam::entry(title,target,"%command%","owner-a",None).unwrap();
        let b=steam::entry(title,target,"%command%","owner-b",None).unwrap();
        assert_eq!(vdf::number(&a,"appid"),vdf::number(&b,"appid"));
        assert_eq!(vdf::number(&a,"appid"),Some(steam::app_id("\"/home/deck/WoW-fixture/Battle.net.exe\"",title)));
    }
}
#[test]
fn c13_native_review_preserves_accepted_identity_options_and_unrelated_records_twice() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let desktop=f.ctx.home.join(".local/share/applications/curseforge.desktop");
    fs::create_dir_all(desktop.parent().unwrap()).unwrap();fs::write(&desktop,b"[Desktop Entry]\nType=Application\n").unwrap();
    let _path=c12_flatpak(&f,true);
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();
    let before=steam::read_shortcuts(&path).unwrap();
    let original_entries=vdf::object(&before,"shortcuts").unwrap().clone();
    let response=review_addon(&f.ctx,&json!({"id":i.id,"manager":"curseforge"})).unwrap();
    let plan:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    let proposed=plan.journal.changes.iter().find(|c|c.path==path).unwrap();
    let proposed=vdf::parse_binary(&proposed.after).unwrap();
    let new=vdf::object(&proposed,"shortcuts").unwrap().iter().find_map(|(_,v)| match v{Value::Object(e) if steam::native_addon(e)=>Some(e),_=>None}).unwrap();
    let generated=vdf::number(new,"appid").unwrap();assert_ne!(generated,4011762609);assert_eq!(new[0].0,"appname");
    apply(&f.ctx,&json!({"token":response["token"]})).unwrap();f.refresh();
    // Simulated Steam rewrite: arbitrary accepted ID/name/options, NOT a live acceptance claim.
    let accepted=2570244444;let mut root=steam::read_shortcuts(&path).unwrap();let mut expected=None;
    for (_,v) in vdf::ensure(&mut root,"shortcuts").unwrap(){if let Value::Object(e)=v {if steam::native_addon(e){vdf::set(e,"appid",Value::I32(accepted));vdf::set(e,"appname",Value::Text("My native addon".into()));e.retain(|(k,_)|k!="wowRustNativeAddon");let opts=format!("{} --kept-user-option",vdf::string(e,"LaunchOptions").unwrap());vdf::set(e,"LaunchOptions",Value::Text(opts));let mut wanted=e.clone();vdf::set(&mut wanted,"wowRustNativeAddon",Value::Text("gearlever".into()));expected=Some(wanted);}}}
    fs::write(&path,vdf::binary(&root).unwrap()).unwrap();
    let layout=f.root.join("custom-layout.vdf");fs::write(&layout,b"fixture association sentinel").unwrap();
    for _ in 0..2 {
        let response=review_addon(&f.ctx,&json!({"id":i.id,"manager":"curseforge"})).unwrap();
        apply(&f.ctx,&json!({"token":response["token"]})).unwrap();f.refresh();
        let root=steam::read_shortcuts(&path).unwrap();let entries=vdf::object(&root,"shortcuts").unwrap();
        let natives:Vec<_>=entries.iter().filter_map(|(_,v)|match v {Value::Object(e) if steam::native_addon(e)=>Some(e),_=>None}).collect();
        assert_eq!(natives.len(),1);assert_eq!(natives[0],expected.as_ref().unwrap());
        for (key,value) in &original_entries {assert_eq!(entries.iter().find(|(k,_)|k==key).unwrap().1,*value);}
        let found=shortcuts(&f.ctx,&json!({"id":i.id})).unwrap();assert!(found["shortcuts"].as_array().unwrap().iter().any(|t|t["addon"]==true&&t["appid"]==accepted));
        assert_eq!(fs::read(&layout).unwrap(),b"fixture association sentinel");
    }
}
