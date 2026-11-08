fn c47_external_copy(f: &Fixture, name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let prefix = f
        .ctx
        .home
        .join("Outside default locations")
        .join(name)
        .join("compatdata");
    let launcher = prefix.join("pfx/drive_c/Program Files (x86)/Battle.net/Battle.net.exe");
    let game = prefix
        .join("pfx/drive_c/Program Files (x86)/World of Warcraft/_classic_era_/WowClassic.exe");
    for p in [&launcher, &game] {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"MZ disposable detector fixture, not a runnable game").unwrap();
    }
    (prefix, launcher, game)
}
fn c47_write_shortcuts(f: &Fixture, user: &str, entries: Vec<vdf::Object>) -> PathBuf {
    let root = f.ctx.home.join(".local/share/Steam");
    fs::create_dir_all(root.join(format!("userdata/{user}/config"))).unwrap();
    let path = steam::shortcuts_path(&root, user).unwrap();
    let entries = entries
        .into_iter()
        .enumerate()
        .map(|(n, e)| (n.to_string(), Value::Object(e)))
        .collect();
    fs::write(
        &path,
        vdf::binary(&vec![("shortcuts".into(), Value::Object(entries))]).unwrap(),
    )
    .unwrap();
    path
}
fn c47_entry(target: &Path, prefix: &Path, app: u32) -> vdf::Object {
    steam::entry(
        "Existing copy",
        target,
        &steam::launch_options(prefix, None).unwrap(),
        "lost-record",
        Some(app),
    )
    .unwrap()
}
#[test]
fn c47_empty_records_use_valid_shortcut_hint_without_adopting() {
    let f = Fixture::new();
    let (prefix, launcher, game) = c47_external_copy(&f, "Current copy");
    let before = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert!(before.launchers.is_empty() && before.games.is_empty());
    assert!(before.warnings.is_empty());
    let path = c47_write_shortcuts(&f, "123", vec![c47_entry(&launcher, &prefix, 3100000011)]);
    let bytes = fs::read(&path).unwrap();
    let file = fs::read(&game).unwrap();
    let scan = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert_eq!(scan.launchers.len(), 1);
    assert_eq!(scan.games.len(), 1);
    assert_eq!(scan.launchers[0].prefix, prefix);
    assert_eq!(scan.games[0].executable, game);
    assert!(f.ctx.state.installations.is_empty() && f.ctx.state.extra_roots.is_empty());
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(fs::read(game).unwrap(), file);
}
#[test]
fn c47_hint_deduplicates_same_copy_and_keeps_other_copy_separate() {
    let f = Fixture::new();
    let (a, la, _) = c47_external_copy(&f, "A");
    let (b, lb, _) = c47_external_copy(&f, "B");
    c47_write_shortcuts(
        &f,
        "123",
        vec![
            c47_entry(&la, &a, 100),
            c47_entry(&la, &a, 101),
            c47_entry(&lb, &b, 102),
        ],
    );
    let scan = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert_eq!(scan.launchers.len(), 2);
    assert_eq!(scan.games.len(), 2);
    assert_ne!(scan.launchers[0].prefix, scan.launchers[1].prefix);
}
#[test]
fn c47_valid_detected_hint_still_requires_explicit_adoption_review() {
    let f = Fixture::new();
    let (prefix, launcher, _) = c47_external_copy(&f, "Review copy");
    let path = c47_write_shortcuts(&f, "123", vec![c47_entry(&launcher, &prefix, 4001)]);
    let config = f.ctx.home.join(".local/share/Steam/config/config.vdf");
    fs::write(
        &config,
        steam::set_proton(Some(&fs::read(&config).unwrap()), &[4001], "proton_11").unwrap(),
    )
    .unwrap();
    let bytes = fs::read(&path).unwrap();
    let candidates = adoption_candidates(&f.ctx, &json!({"prefix":prefix})).unwrap();
    assert_eq!(candidates["candidates"].as_array().unwrap().len(), 1);
    assert!(f.ctx.state.installations.is_empty());
    let chosen = &candidates["candidates"][0];
    let review=review_adopt(&f.ctx,&json!({"prefix":prefix,"appid":chosen["appid"],"steam":chosen["steam"],"user":chosen["user"]})).unwrap();
    assert_eq!(review["kind"], "adopt");
    assert_eq!(review["download"], false);
    let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
    assert!(!plan.installation.owned && !plan.download);
    assert_eq!(plan.installation.launcher, Some(launcher));
    assert_eq!(plan.installation.prefix, prefix);
    assert_eq!(plan.installation.proton, "proton_11");
    assert!(plan.remove.is_empty());
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert!(f.ctx.state.installations.is_empty());
}
#[test]
fn c47_setup_hint_finds_existing_games_without_inventing_launcher() {
    let f = Fixture::new();
    let (prefix, launcher, _) = c47_external_copy(&f, "Unfinished copy");
    fs::remove_file(launcher).unwrap();
    let setup = prefix.parent().unwrap().join("Battle.net-Setup.exe");
    fs::write(&setup, b"MZ setup fixture").unwrap();
    c47_write_shortcuts(&f, "123", vec![c47_entry(&setup, &prefix, 5001)]);
    let scan = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert!(scan.launchers.is_empty());
    assert_eq!(scan.games.len(), 1);
}
#[test]
fn c47_unrelated_wrappers_missing_targets_and_unsafe_options_are_not_hints() {
    let f = Fixture::new();
    let (prefix, launcher, _) = c47_external_copy(&f, "Ignored");
    let wrapper = f.root.join("env");
    fs::write(&wrapper, b"unrelated fixture").unwrap();
    let mut relative = c47_entry(&launcher, &prefix, 6002);
    vdf::set(
        &mut relative,
        "exe",
        Value::Text("relative/Battle.net.exe".into()),
    );
    let mut unsafe_option = c47_entry(&launcher, &prefix, 6003);
    vdf::set(
        &mut unsafe_option,
        "LaunchOptions",
        Value::Text(format!(
            "STEAM_COMPAT_DATA_PATH={} STEAM_COMPAT_DATA_PATH=/other %command%",
            fsutil::shell_quote(&prefix).unwrap()
        )),
    );
    c47_write_shortcuts(
        &f,
        "123",
        vec![
            c47_entry(&wrapper, &prefix, 6001),
            relative,
            unsafe_option,
            c47_entry(&prefix.join("missing/Battle.net.exe"), &prefix, 6004),
        ],
    );
    let scan = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert!(scan.launchers.is_empty() && scan.games.is_empty());
}
#[test]
fn c47_conflicting_prefix_hint_is_ignored_without_scanning_either_copy() {
    let f = Fixture::new();
    let (a, la, _) = c47_external_copy(&f, "A");
    let (b, _, _) = c47_external_copy(&f, "B");
    c47_write_shortcuts(&f, "123", vec![c47_entry(&la, &b, 7001)]);
    let scan = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert!(scan.launchers.is_empty() && scan.games.is_empty());
    assert!(scan.warnings.iter().any(|w| w.contains("conflicting")));
    assert!(a.exists());
}
#[test]
fn c47_bad_profile_metadata_does_not_hide_valid_other_profile_hint() {
    let f = Fixture::new();
    let (prefix, launcher, _) = c47_external_copy(&f, "Valid");
    let bad = c47_write_shortcuts(&f, "123", vec![]);
    fs::write(&bad, b"not a binary shortcut file").unwrap();
    c47_write_shortcuts(&f, "456", vec![c47_entry(&launcher, &prefix, 8001)]);
    let scan = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert_eq!(scan.launchers.len(), 1);
    assert_eq!(scan.games.len(), 1);
    assert!(scan.warnings.iter().any(|w| w.contains("search hints")));
    assert_eq!(fs::read(bad).unwrap(), b"not a binary shortcut file");
}
#[test]
fn c47_shortcut_hint_count_is_bounded() {
    let f = Fixture::new();
    let mut entries = vec![];
    for n in 0..34 {
        let (prefix, launcher, _) = c47_external_copy(&f, &format!("Copy {n}"));
        entries.push(c47_entry(&launcher, &prefix, 9000 + n));
    }
    c47_write_shortcuts(&f, "123", entries);
    let scan = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert_eq!(scan.launchers.len(), 32);
    assert!(scan.warnings.iter().any(|w| w.contains("hint limit")));
}

#[test]
fn c47_canonical_shortcut_alias_can_be_reviewed_without_rewriting_target() {
    let f = Fixture::new();
    let (prefix, launcher, _) = c47_external_copy(&f, "Alias copy");
    let alias = f.root.join("home-alias");
    std::os::unix::fs::symlink(&f.ctx.home, &alias).unwrap();
    let alias_prefix = alias.join(prefix.strip_prefix(&f.ctx.home).unwrap());
    let alias_launcher = alias.join(launcher.strip_prefix(&f.ctx.home).unwrap());
    let path = c47_write_shortcuts(
        &f,
        "123",
        vec![c47_entry(&alias_launcher, &alias_prefix, 11001)],
    );
    let config = f.ctx.home.join(".local/share/Steam/config/config.vdf");
    fs::write(
        &config,
        steam::set_proton(Some(&fs::read(&config).unwrap()), &[11001], "proton_11").unwrap(),
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    let scan = discovery::scan(&f.ctx.home, &f.ctx.state).unwrap();
    assert_eq!(scan.launchers.len(), 1);
    let found = adoption_candidates(&f.ctx, &json!({"prefix":prefix})).unwrap();
    assert_eq!(found["candidates"].as_array().unwrap().len(), 1);
    let c = &found["candidates"][0];
    review_adopt(
        &f.ctx,
        &json!({"prefix":prefix,"appid":c["appid"],"steam":c["steam"],"user":c["user"]}),
    )
    .unwrap();
    assert_eq!(fs::read(path).unwrap(), before);
}
#[test]
fn c47_adoption_does_not_accept_conflicting_shortcut_prefix() {
    let mut f = Fixture::new();
    let (a, la, _) = c47_external_copy(&f, "A");
    let (b, _, _) = c47_external_copy(&f, "B");
    f.ctx.state.extra_roots.push(a.clone());
    c47_write_shortcuts(&f, "123", vec![c47_entry(&la, &b, 12001)]);
    let found = adoption_candidates(&f.ctx, &json!({"prefix":a})).unwrap();
    assert!(found["candidates"].as_array().unwrap().is_empty());
}
