#[test]
fn c8_gear_accepts_home_alias_but_rejects_file_links_and_escapes() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let dir=f.ctx.home.join(".local/share/applications");fs::create_dir_all(&dir).unwrap();
    let desktop=dir.join("curseforge.desktop");fs::write(&desktop,b"[Desktop Entry]\nType=Application\n").unwrap();
    let alias=f.root.join("home-alias");symlink(&f.ctx.home,&alias).unwrap();
    assert_eq!(gear_desktop_path(&alias,"curseforge.desktop").unwrap(),desktop);
    for id in ["../escape.desktop","/tmp/escape.desktop","a\\b.desktop","a\n.desktop",".desktop","missing.desktop"] {assert!(gear_desktop_path(&alias,id).is_err(),"{id}");}
    let outside=f.root.join("outside.desktop");fs::write(&outside,b"outside").unwrap();
    for target in [&outside,&desktop] {
        symlink(target,dir.join("linked.desktop")).unwrap();assert!(gear_desktop_path(&alias,"linked.desktop").is_err());fs::remove_file(dir.join("linked.desktop")).unwrap();
    }
    fs::hard_link(&outside,dir.join("hard.desktop")).unwrap();assert!(gear_desktop_path(&alias,"hard.desktop").is_err());
    fs::create_dir(dir.join("directory.desktop")).unwrap();assert!(gear_desktop_path(&alias,"directory.desktop").is_err());
    fs::rename(&dir,f.root.join("real-applications")).unwrap();symlink(f.root.join("real-applications"),&dir).unwrap();assert!(gear_desktop_path(&alias,"curseforge.desktop").is_err());
}
#[test]
fn c8_vanished_record_or_directory_never_falls_back_and_stale_plan_refuses() {
    let mut f=Fixture::new();let i=f.install();f.games(&i);
    let response=review_repair(&f.ctx,&json!({"id":i.id})).unwrap();
    let request=json!({"token":response["token"],"id":i.id});
    let mut other=i.clone();other.id="11111111111111111111111111111111".into();
    f.ctx.state.installations=vec![other.clone()];f.ctx.state.current=Some(other.id.clone());
    assert!(f.ctx.install(&i.id).is_err());assert!(load_plan(&f.ctx,&request).is_err());
    assert!(f.ctx.install(&other.id).is_ok());
    f.ctx.state.installations.push(i.clone());
    assert!(load_plan(&f.ctx,&json!({"token":response["token"],"id":other.id})).unwrap_err().contains("differs"));
    f.ctx.state.installations.iter_mut().find(|x|x.id==i.id).unwrap().prefix=f.root.join("other-prefix");
    assert!(load_plan(&f.ctx,&request).unwrap_err().contains("target changed"));
    fs::rename(&i.root,f.root.join("retained-copy")).unwrap();
    assert!(f.ctx.install(&i.id).unwrap_err().contains("no longer available"));
    assert!(state_info(&f.ctx)["unavailable_installations"].as_array().unwrap().contains(&json!(i.id)));
}
#[test]
fn c8_operation_refusal_names_reviewed_operation_without_force_close() {
    for (kind,words) in [("launcher","updating"),("repair","updating"),("clean_owned","removing"),("reinstall","reinstalling"),("restore","restoring"),("exit","changing")] {
        let message=discovery::games_closed_message(discovery::operation_description(kind));assert!(message.contains(words));assert!(message.contains("Nothing was force-closed"));
    }
}
#[test]
fn c8_active_agent_refuses_guided_shortcut_apply_before_steam_shutdown() {
    use std::io::BufRead;
    let mut f=Fixture::new();let i=f.install();f.games(&i);let response=review_repair(&f.ctx,&json!({"id":i.id})).unwrap();
    let mut child=std::process::Command::new("python3").args(["-c","import ctypes,time; ctypes.CDLL(None).prctl(15,b'Agent.exe',0,0,0); print('ready',flush=True); time.sleep(15)"]).stdout(std::process::Stdio::piped()).spawn().unwrap();
    let mut line=String::new();std::io::BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();assert_eq!(line.trim(),"ready");
    let result=guided_apply(&f.ctx,&json!({"token":response["token"],"id":i.id,"manage_steam":true}));
    unsafe {libc::kill(child.id() as i32,libc::SIGTERM);};let _=child.wait();
    assert!(result.unwrap_err().contains("before updating the reviewed Steam shortcuts"));assert!(!f.ctx.data.join("steam-restart.json").exists());assert!(!f.ctx.data.join("transaction.json").exists());
}

#[test]
fn c8_stale_gear_setup_target_refuses_before_any_tool_install() {
 let f=Fixture::new();
 assert!(prepare_gear(&f.ctx,&json!({"id":"missing","confirm":true})).unwrap_err().contains("Select an installation"));
}
