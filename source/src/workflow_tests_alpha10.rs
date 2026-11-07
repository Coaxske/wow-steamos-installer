// Reproduce a saved legacy Forever binding with no executable association,
// followed by alpha9 adding a selector tile for the same beta client.
#[test]
fn alpha10_legacy_forever_repair_reuses_original_id() {
    for duplicate in [false, true] {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let(i,_)=c42_seed_legacy(&mut f,&i);
        let old = i.root.join("games/World of Warcraft/_forever_");
        let beta = old.with_file_name("_classic_beta_");
        fs::rename(&old, &beta).unwrap();
        let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
        let mut root = steam::read_shortcuts(&path).unwrap();
        let mut original = 0;
        for (_, v) in vdf::ensure(&mut root,"shortcuts").unwrap() {
            if let Value::Object(e) = v {
                if vdf::string(e,"wowRustEdition") == Some("forever") {
                    original = vdf::number(e,"appid").unwrap();
                    e.retain(|(k,_)| k != "wowRustGame");
                    vdf::set(e,"appname",Value::Text("My original Forever".into()));
                    vdf::set(e,"IsHidden",Value::I32(1));
                }
            }
        }
        for b in &mut f.ctx.state.installations[0].bindings {
            if b.appid == original { b.game = None; }
        }
        // Adopted installations may have no requested-edition list.
        f.ctx.state.installations[0].editions.clear();
        if duplicate {
            f.ctx.state.installations[0].editions = vec!["retail".into(),"classic_beta".into(),"era".into()];
            let launcher = f.ctx.state.installations[0].launcher.as_ref().unwrap();
            let mut e = steam::entry("WoW Classic / Forever Beta",launcher,
                &products::options(&i.prefix,"classic_beta",None).unwrap(),&i.id,Some(0x81234567)).unwrap();
            vdf::set(&mut e,"wowRustGame",Value::Text(beta.join("Wow.exe").to_string_lossy().into()));
            vdf::set(&mut e,"wowRustEdition",Value::Text("classic_beta".into()));
            vdf::ensure(&mut root,"shortcuts").unwrap().insert(0,("new".into(),Value::Object(e)));
        }
        fs::write(&path,vdf::binary(&root).unwrap()).unwrap();
        f.ctx.save().unwrap();
        for _ in 0..2 {
            let q=review_repair(&f.ctx,&json!({"id":i.id,"shared_tile_appid":original})).unwrap();
            apply(&f.ctx,&json!({"token":q["token"]})).unwrap();f.refresh();
            let repaired = steam::read_shortcuts(&path).unwrap();
            let entries = vdf::object(&repaired,"shortcuts").unwrap();
            assert_eq!(entries.len(),1,"one Battle.net tile for all games");
            let forever:Vec<_> = entries.iter().filter_map(|(_,v)| match v {
                Value::Object(e) if vdf::string(e,"wowRustRole")==Some("shared_game") => Some(e),
                _ => None,
            }).collect();
            assert_eq!(forever.len(),1);
            assert_eq!(vdf::number(forever[0],"appid"),Some(original));
            assert_eq!(vdf::string(forever[0],"appname"),Some("Battle.net"));
            assert_eq!(vdf::number(forever[0],"IsHidden"),Some(1));
            assert!(f.ctx.state.installations[0].game_associations.iter().any(|g|g.id=="forever"&&g.executable==beta.join("Wow.exe")));
            assert!(steam::product(vdf::string(forever[0],"LaunchOptions").unwrap()).is_none());
        }
    }
}
#[test]
fn alpha10_legacy_association_does_not_guess_other_copies() {
    for scenario in ["unowned", "separate_forever", "two_beta", "wrong_prefix", "wrong_target"] {
        let mut f = Fixture::new();
        let i = f.install(); f.games(&i); f.repair(&i);
        let i = f.ctx.state.installations[0].clone();
        let beta = i.root.join("games/World of Warcraft/_classic_beta_");
        let game = Game { id:"classic_beta".into(), name:"Beta".into(),path:beta.clone(),executable:beta.join("Wow.exe"),prefix:Some(i.prefix.clone()) };
        let mut games = vec![game];
        if scenario=="separate_forever" {
            let mut other=games[0].clone(); other.id="forever".into();other.path=beta.with_file_name("_forever_");games.push(other);
        }
        if scenario=="two_beta" {
            let mut other=games[0].clone();other.path=i.root.join("another/_classic_beta_");games.push(other);
        }
        let target=if scenario=="wrong_target" { i.root.join("unrelated.exe") } else { i.launcher.clone().unwrap() };
        let prefix=if scenario=="wrong_prefix" { i.root.join("different-prefix") } else { i.prefix.clone() };
        let mut e=steam::entry("Forever",&target,&steam::launch_options(&prefix,Some("WoWF")).unwrap(),if scenario=="unowned" { "someone-else" } else { &i.id },Some(123)).unwrap();
        vdf::set(&mut e,"wowRustEdition",Value::Text("forever".into()));
        let mut root=vec![("shortcuts".into(),Value::Object(vec![("0".into(),Value::Object(e))]))];
        let before=vdf::binary(&root).unwrap();
        associate_legacy_forever(&mut root,&i,&games).unwrap();
        assert_eq!(vdf::binary(&root).unwrap(),before,"{scenario}");
    }
}

#[test]
fn alpha10_cleanup_recovers_quarantined_windows_game_directory() {
    for unrelated in [false, true] {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let games = i.prefix.join("pfx/drive_c/Program Files (x86)/World of Warcraft");
        fs::rename(i.root.join("games/World of Warcraft"), &games).unwrap();
        f.repair(&i);
        review_clean(&f.ctx, &json!({"id":i.id,"keep_settings":false})).unwrap();
        let mut plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
        transaction::finish(&f.ctx.data, &mut plan.journal, false).unwrap();
        let removal = plan.remove.iter().find(|r|r.path==games).unwrap();
        let quarantine = removal.quarantine.clone();
        fs::rename(&games, &quarantine).unwrap();
        let mut job = Cleanup { plan, settings_saved:true };
        fsutil::json(&f.ctx.data.join("cleanup.json"), &job).unwrap();
        if unrelated {
            let other = i.prefix.join("pfx/drive_c/Program Files (x86)/Another application");
            fs::create_dir_all(&other).unwrap();
            fs::write(other.join("keep"), b"unrelated").unwrap();
            assert!(run_cleanup(&f.ctx,&mut job).is_err());
            assert!(quarantine.exists());
            assert_eq!(fs::read(other.join("keep")).unwrap(), b"unrelated");
        } else {
            run_cleanup(&f.ctx,&mut job).unwrap();
            assert!(!quarantine.exists());
            assert!(!f.ctx.data.join("cleanup.json").exists());
        }
    }
}
