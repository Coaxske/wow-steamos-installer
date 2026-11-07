// Production engine, disposable fixtures and mocked read-only Gear Lever boundary.
struct C12Path(Option<std::ffi::OsString>);
impl Drop for C12Path { fn drop(&mut self){if let Some(v)=&self.0{std::env::set_var("PATH",v)}else{std::env::remove_var("PATH")}} }
fn c12_flatpak(f:&Fixture, installed:bool)->C12Path {
 use std::os::unix::fs::PermissionsExt;
 let bin=f.root.join("mock-bin");fs::create_dir(&bin).unwrap();
 let payload=f.root.join("managed.AppImage");let mut bytes=vec![0;2048];bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");bytes[8..11].copy_from_slice(b"AI\x02");bytes[18]=62;fs::write(&payload,bytes).unwrap();
 let list=if installed {json!({"schema_version":1,"installed":[{"name":"CurseForge","desktop_id":"curseforge.desktop","path":payload}]}).to_string()}else{json!({"schema_version":1,"installed":[]}).to_string()};
 let script=format!("#!/bin/sh\nif [ \"$1\" = install ]; then echo INSTALL >> '{}'; fi\nif [ \"$1\" = run ]; then printf '%s' '{}'; exit 0; fi\nexit 1\n",f.root.join("calls").display(),list);
 fs::write(bin.join("flatpak"),script).unwrap();fs::set_permissions(bin.join("flatpak"),fs::Permissions::from_mode(0o755)).unwrap();
 let old=std::env::var_os("PATH");std::env::set_var("PATH",&bin);C12Path(old)
}
#[test]
fn c12_explicit_bad_input_refuses_before_installing_tools(){
 let f=Fixture::new();let _path=c12_flatpak(&f,false);
 let e=prepare_gear(&f.ctx,&json!({"confirm":true,"path":"/not/a/selected/AppImage"})).unwrap_err();assert!(e.contains("Browse"),"{e}");assert!(!f.root.join("calls").exists());
}
#[test]
fn c12_curseforge_review_persists_choice_on_apply_not_review(){
 let mut f=Fixture::new();let i=f.install();let desktop=f.ctx.home.join(".local/share/applications/curseforge.desktop");fs::create_dir_all(desktop.parent().unwrap()).unwrap();fs::write(&desktop,b"[Desktop Entry]\nType=Application\n").unwrap();
 let _path=c12_flatpak(&f,true);let before=f.ctx.state.installations[0].addon_manager.clone();let response=review_addon(&f.ctx,&json!({"id":i.id,"manager":"curseforge"})).unwrap();assert_eq!(response["choices"]["addon_manager"],"curseforge");assert_eq!(f.ctx.state.installations[0].addon_manager,before);
 apply(&f.ctx,&json!({"token":response["token"]})).unwrap();f.refresh();assert_eq!(f.ctx.state.installations[0].addon_manager,"curseforge");
 let response=review_addon(&f.ctx,&json!({"id":i.id,"manager":"curseforge"})).unwrap();assert_eq!(response["choices"]["addon_manager"],"curseforge");let plan:Plan=fsutil::load(&f.ctx.data.join("review.json")).unwrap();assert!(plan.journal.changes.iter().all(|c|c.before.as_ref()==Some(&c.after)));
 let reused=prepare_gear(&f.ctx,&json!({"id":i.id,"confirm":true,"path":""})).unwrap();assert!(reused["message"].as_str().unwrap().contains("Reusing"));assert!(!f.root.join("calls").exists());
}
