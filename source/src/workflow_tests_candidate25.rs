#[test]
fn c25_review_targets_and_replacement_scope() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let before=fs::read(steam::shortcuts_path(&i.steam,&i.user).unwrap()).unwrap();
    let a=controller_review(&f.ctx,&json!({"id":i.id,"selected":[i.bootstrap_id]})).unwrap();
    assert_eq!(a["replace_custom"],false);
    assert!(controller_review(&f.ctx,&json!({"id":i.id,"selected":[1]})).is_err());
    assert!(controller_review(&f.ctx,&json!({"id":i.id,"selected":[i.bootstrap_id,i.bootstrap_id]})).is_err());
    assert!(controller_review(&f.ctx,&json!({"id":i.id,"selected":[i.bootstrap_id],"replace_custom":"yes"})).is_err());
    let b=controller_review(&f.ctx,&json!({"id":i.id,"selected":[i.bootstrap_id],"replace_custom":true})).unwrap();
    assert_eq!(b["replace_custom"],true);assert_ne!(a["token"],b["token"]);
    assert!(controller_apply(&f.ctx,&json!({"id":i.id,"token":a["token"]})).unwrap_err().contains("expired"));
    assert_eq!(fs::read(steam::shortcuts_path(&i.steam,&i.user).unwrap()).unwrap(),before);
}
#[test]
fn c25_changed_or_unavailable_target_refuses_before_driver() {
    let mut f=Fixture::new();let i=f.install();let q=controller_review(&f.ctx,&json!({"id":i.id,"selected":[i.bootstrap_id]})).unwrap();
    let path=steam::shortcuts_path(&i.steam,&i.user).unwrap();let mut root=steam::read_shortcuts(&path).unwrap();
    for (_,v) in vdf::ensure(&mut root,"shortcuts").unwrap(){if let Value::Object(e)=v {vdf::set(e,"appname",Value::Text("Changed after review".into()));}}
    fs::write(&path,vdf::binary(&root).unwrap()).unwrap();
    assert!(controller_apply(&f.ctx,&json!({"id":i.id,"token":q["token"]})).unwrap_err().contains("changed"));
    assert!(!f.ctx.data.join("controller-job.json").exists());
    fs::rename(&i.root,i.root.with_extension("moved")).unwrap();
    assert!(controller_review(&f.ctx,&json!({"id":i.id,"selected":[i.bootstrap_id]})).unwrap_err().contains("no longer available"));
}
#[test]
fn c25_backup_verified_and_links_refused() {
    use std::os::unix::fs::symlink;
    let mut f=Fixture::new();let i=f.install();
    let root=i.steam.join("steamapps/common/Steam Controller Configs").join(&i.user);
    fs::create_dir_all(&root).unwrap();fs::write(root.join("layout.vdf"),b"deliberate customized controls").unwrap();
    let path=controller_backup(&f.ctx,&i,&fsutil::unique()).unwrap();
    let files:Vec<Json>=fsutil::load(&path.join("inventory.json")).unwrap();
    assert!(files.iter().any(|v|v["sha256"]==fsutil::hash(b"deliberate customized controls")));
    fs::write(root.join("layout.vdf"),b"changed after first backup").unwrap();
    let token=path.file_name().unwrap().to_str().unwrap();assert_eq!(controller_backup(&f.ctx,&i,token).unwrap(),path);
    for v in files {assert_eq!(fsutil::hash(&fs::read(v["file"].as_str().unwrap()).unwrap()),v["sha256"]);}
    symlink(f.root.join("unrelated"),root.join("linked-layout")).unwrap();
    assert!(controller_backup(&f.ctx,&i,&fsutil::unique()).is_err());
}
