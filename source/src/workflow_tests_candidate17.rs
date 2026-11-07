#[test]
fn c17_restore_attempted_on_every_managed_result(){
 for failure in [false,true]{let calls=std::cell::Cell::new(0);let result=if failure{Err("cancelled or shutdown failed".into())}else{Ok(json!({"kind":"repair"}))};let out=complete_steam_interval(result,||{calls.set(calls.get()+1);Ok(json!({"message":"Steam reopen requested"}))});assert_eq!(calls.get(),1);assert_eq!(out.is_err(),failure);}
 let out=complete_steam_interval(Err("original conflict".into()),||Err("reopen failed".into())).unwrap_err();assert!(out.contains("original conflict"));assert!(out.contains("Open Steam"));
}
#[test]
fn c17_guided_saved_operations_require_explicit_consent(){let mut f=Fixture::new();f.install();assert!(guided_existing(&mut f.ctx,&json!({"operation":"recover"})).unwrap_err().contains("Confirm"));assert!(guided_existing(&mut f.ctx,&json!({"operation":"invalid","manage_steam":true})).is_err());assert!(!f.ctx.data.join("steam-restart.json").exists());}

#[test]
fn c17_undo_cannot_bypass_pending_recovery(){let mut f=Fixture::new();f.install();fs::write(f.ctx.data.join("transaction.json"),b"{}").unwrap();assert!(guided_existing(&mut f.ctx,&json!({"operation":"undo","manage_steam":true})).unwrap_err().contains("before Undo"));assert!(!f.ctx.data.join("steam-restart.json").exists());}
