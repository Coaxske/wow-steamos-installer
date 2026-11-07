#[test]
fn c14_saved_stages_require_explicit_scoped_repair_consent() {
    for stage in [Stage::TestGames, Stage::Complete] {
        let mut f=Fixture::new(); let i=f.install(); f.games(&i);
        f.ctx.state.installations[0].stage=stage;
        f.ctx.state.installations[0].editions=vec!["era".into(),"retail".into(),"forever".into()];
        f.ctx.persist(f.ctx.state.clone()).unwrap();
        fs::remove_dir_all(i.root.join("games/World of Warcraft/_forever_")).unwrap();
        let state=fs::read(f.ctx.data.join("state.json")).unwrap();
        assert!(review_repair(&f.ctx,&json!({"id":i.id})).unwrap_err().contains("forever"));
        let scope=json!({"id":i.id,"selected":["era","forever","retail"],"missing":["forever"]});
        let q=review_repair(&f.ctx,&json!({"id":i.id,"allow_missing":true,"installed_only_scope":scope})).unwrap();
        assert_eq!(q["choices"]["editions"],json!(["era","retail"]));
        assert_eq!(q["choices"]["requested_editions"],json!(["era","retail","forever"]));
        assert_eq!(state,fs::read(f.ctx.data.join("state.json")).unwrap());
        fs::remove_dir_all(i.root.join("games/World of Warcraft/_retail_")).unwrap();
        assert!(review_repair(&f.ctx,&json!({"id":i.id,"allow_missing":true,"installed_only_scope":scope})).unwrap_err().contains("changed"));
        let wrong=json!({"id":"different","selected":["era","forever","retail"],"missing":["forever","retail"]});
        assert!(review_repair(&f.ctx,&json!({"id":i.id,"allow_missing":true,"installed_only_scope":wrong})).is_err());
        f.games(&i);
        assert!(review_repair(&f.ctx,&json!({"id":i.id})).is_ok());
    }
}
