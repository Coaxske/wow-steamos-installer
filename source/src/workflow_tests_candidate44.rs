
#[test]
fn c44_normal_repair_creates_correct_edition_addon_links_and_rerun_reuses() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);
    for client in ["_retail_","_classic_era_","_forever_"] {fs::create_dir_all(i.root.join("games/World of Warcraft").join(client).join("Interface/AddOns")).unwrap();}
    let vendor=fs::read(exit_settings_path(&i)).unwrap();f.repair(&i);
    let root=f.ctx.home.join("World of Warcraft");
    for (label,client) in [("Retail","_retail_"),("Classic Era","_classic_era_"),("Forever","_forever_")] {
        let link=root.join(format!("{label} AddOns"));fs::write(link.join("ExampleAddon.lua"),b"addon routing").unwrap();
        assert_eq!(fs::read(i.root.join("games/World of Warcraft").join(client).join("Interface/AddOns/ExampleAddon.lua")).unwrap(),b"addon routing");
    }
    let before=fs::read(f.ctx.data.join("folder-links.json")).unwrap();f.repair(&i);assert_eq!(fs::read(f.ctx.data.join("folder-links.json")).unwrap(),before);
    assert_eq!(fs::read_dir(&root).unwrap().count(),7);
    assert_eq!(shortcuts(&f.ctx,&json!({"id":i.id})).unwrap()["shortcuts"].as_array().unwrap().len(),1);
    assert_eq!(fs::read(exit_settings_path(&i)).unwrap(),vendor);
    crate::folder_links::remove(&f.ctx.data,&f.ctx.home,&i.id).unwrap();
    for client in ["_retail_","_classic_era_","_forever_"] {assert!(i.root.join("games/World of Warcraft").join(client).join("Interface/AddOns/ExampleAddon.lua").is_file());}
}
#[test]
fn c44_explicit_forever_beta_association_keeps_forever_folder_names() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);
    let old=i.root.join("games/World of Warcraft/_forever_");let beta=old.with_file_name("_classic_beta_");fs::rename(old,&beta).unwrap();fs::create_dir_all(beta.join("interface/addons")).unwrap();
    let q=review_repair(&f.ctx,&json!({"id":i.id,"confirm_forever_beta":true})).unwrap();apply(&f.ctx,&json!({"token":q["token"]})).unwrap();f.refresh();
    let current=f.ctx.install(&i.id).unwrap();let (targets,_)=folder_targets(&f.ctx,&current).unwrap();
    assert!(targets.iter().any(|v|v.label=="Forever AddOns"&&v.path==beta.join("interface/addons")));
    assert!(!targets.iter().any(|v|v.label=="Classic Beta AddOns"));
    assert!(f.ctx.home.join("World of Warcraft/Forever AddOns").is_symlink());
}
#[test]
fn c44_actual_missing_addon_directory_is_reported_without_guess_or_creation() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);f.repair(&i);
    let current=f.ctx.install(&i.id).unwrap();let(_,warnings)=folder_targets(&f.ctx,&current).unwrap();assert!(warnings.iter().any(|s|s.contains("no path was guessed")));
    assert!(!f.ctx.home.join("World of Warcraft/Retail AddOns").is_symlink());
    assert!(!i.root.join("games/World of Warcraft/_retail_/Interface").exists());
}
