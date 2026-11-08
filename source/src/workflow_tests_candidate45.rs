
fn c45_missing_fixture() -> (Fixture, Installation, u32) {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let i=f.ctx.install(&i.id).unwrap();let id=i.bindings.iter().find(|b|b.shared).unwrap().appid;
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let mut root=steam::read_shortcuts(&path).unwrap();
    vdf::ensure(&mut root,"shortcuts").unwrap().retain(|(_,v)|!matches!(v,Value::Object(e) if vdf::number(e,"appid")==Some(id)&&steam::owned(e,&i.id)));
    fs::write(path,vdf::binary(&root).unwrap()).unwrap();(f,i,id)
}
#[test]
fn c45_missing_tile_recovers_original_identity_and_repeat_is_single() {
    let (mut f,i,id)=c45_missing_fixture();let vendor=fs::read(exit_settings_path(&i)).unwrap();
    f.repair(&i);let current=f.ctx.install(&i.id).unwrap();assert_eq!(current.bindings.iter().find(|b|b.shared).unwrap().appid,id);
    assert_eq!(current.prefix,i.prefix);assert_eq!(current.launcher,i.launcher);
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let first=fs::read(&path).unwrap();f.repair(&current);
    assert_eq!(fs::read(path).unwrap(),first);assert_eq!(fs::read(exit_settings_path(&i)).unwrap(),vendor);
    assert_eq!(shortcuts(&f.ctx,&json!({"id":i.id})).unwrap()["shortcuts"].as_array().unwrap().len(),1);
}
#[test]
fn c45_missing_tile_uses_remaining_saved_id_proton_mapping() {
    let (mut f,i,id)=c45_missing_fixture();let path=i.steam.join("config/config.vdf");
    let edited=steam::set_proton(Some(&fs::read(&path).unwrap()),&[id],"proton_deliberate_existing").unwrap();fs::write(&path,&edited).unwrap();
    let q=review_repair(&f.ctx,&json!({"id":i.id})).unwrap();let plan:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    assert_eq!(plan.installation.proton,"proton_deliberate_existing");assert!(!plan.download);assert!(plan.remove.is_empty());
    apply(&f.ctx,&json!({"token":q["token"]})).unwrap();f.refresh();assert_eq!(fs::read(path).unwrap(),edited);
}
#[test]
fn c45_missing_tile_collision_refuses_unrelated_shortcut() {
    let (f,i,id)=c45_missing_fixture();let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let mut root=steam::read_shortcuts(&path).unwrap();
    let foreign=steam::entry("Unrelated game",Path::new("/fixture/unrelated"),"","other-owner",Some(id)).unwrap();
    vdf::ensure(&mut root,"shortcuts").unwrap().push(("0".into(),Value::Object(foreign)));fs::write(&path,vdf::binary(&root).unwrap()).unwrap();let before=fs::read(&path).unwrap();
    assert!(review_repair(&f.ctx,&json!({"id":i.id})).is_err());assert_eq!(fs::read(path).unwrap(),before);
}
#[test]
fn c45_ambiguous_saved_shared_identity_refuses_without_writes() {
    let (mut f,mut i,id)=c45_missing_fixture();let mut second=i.bindings.iter().find(|b|b.shared).unwrap().clone();second.appid=id.wrapping_add(1);i.bindings.push(second);
    f.ctx.state.installations[0]=i.clone();fsutil::json(&f.ctx.data.join("state.json"),&f.ctx.state).unwrap();
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let before=fs::read(&path).unwrap();let error=review_repair(&f.ctx,&json!({"id":i.id})).unwrap_err();assert!(error.contains("ambiguous"));assert_eq!(fs::read(path).unwrap(),before);
}
