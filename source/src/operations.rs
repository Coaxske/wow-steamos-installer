// Included in engine so operations share the private transaction context.
fn review_launcher(ctx: &Context, req: &Json) -> Result<Json> {
    let mut i = ctx.install(field(req, "id")?)?;
    if i.stage != Stage::LauncherInstall {
        return Err("Battle.net setup is already complete for this copy. Use Check downloaded games / repair instead.".into());
    }
    let scan = discovery::scan(&ctx.home, &ctx.state)?;
    let found: Vec<_> = scan
        .launchers
        .iter()
        .filter(|l| l.prefix == i.prefix)
        .collect();
    if found.len() != 1 {
        return Err("Finish installing Battle.net, close it, then check again. Reconnect a missing drive before continuing.".into());
    }
    i.launcher = Some(found[0].path.clone());
    if i.stage == Stage::LauncherInstall {
        i.stage = i.stage.advance(Stage::Games)?;
    }
    let cfg = fsutil::read(&i.steam.join("config/config.vdf"))?;
    if let Some(tool) = steam::proton_for(&cfg, i.bootstrap_id)? {
        i.proton = tool;
    }
    let path = steam::shortcuts_path(&i.steam, &i.user)?;
    let mut root = steam::read_shortcuts(&path)?;
    let entries = vdf::ensure(&mut root, "shortcuts")?;
    if let Some(e) = entries.iter_mut().find_map(|(_, v)| match v {
        Value::Object(e)
            if steam::owned(e, &i.id) && vdf::number(e, "appid") == Some(i.bootstrap_id) =>
        {
            Some(e)
        }
        _ => None,
    }) {
        vdf::set(e, "appname", Value::Text("Battle.net".into()));
        vdf::set(
            e,
            "exe",
            Value::Text(fsutil::steam_quote(i.launcher.as_ref().unwrap())?),
        );
        vdf::set(
            e,
            "StartDir",
            Value::Text(fsutil::steam_quote(
                i.launcher.as_ref().unwrap().parent().unwrap(),
            )?),
        );
        vdf::set(
            e,
            "LaunchOptions",
            Value::Text(steam::launch_options(&i.prefix, None)?),
        );
    } else {
        let entry = steam::entry(
            "Battle.net",
            i.launcher.as_ref().unwrap(),
            &steam::launch_options(&i.prefix, None)?,
            &i.id,
            None,
        )?;
        let id = steam::upsert(&mut root, entry, &i.id)?;
        if !i.shortcuts.contains(&id) {
            i.shortcuts.push(id);
        }
    }
    let mut state = ctx.state.clone();
    *state
        .installations
        .iter_mut()
        .find(|x| x.id == i.id)
        .unwrap() = i.clone();
    let config = i.steam.join("config/config.vdf");
    let changes = vec![
        Change::new(path, vdf::binary(&root)?)?,
        Change::new(
            config.clone(),
            steam::set_proton(Some(&fsutil::read(&config)?), &i.shortcuts, &i.proton)?,
        )?,
        Change::new(
            ctx.data.join("state.json"),
            serde_json::to_vec_pretty(&state).unwrap(),
        )?,
    ];
    simple_plan(ctx,i,"launcher","Open the installed Battle.net launcher using the same Steam tile and Proton tool. Install your selected games inside Battle.net.",changes)
}
fn simple_plan(
    ctx: &Context,
    i: Installation,
    kind: &str,
    description: &str,
    mut changes: Vec<Change>,
) -> Result<Json> {
    if kind == "addon" {
        let mut state=ctx.state.clone();
        *state.installations.iter_mut().find(|x|x.id==i.id).ok_or("Select an installation first")?=i.clone();
        changes.push(Change::new(ctx.data.join("state.json"),serde_json::to_vec_pretty(&state).unwrap())?);
    }
    save_plan(
        ctx,
        &Plan {
            token: fsutil::unique(),
            kind: kind.into(),
            summary: vec![description.into()],
            installation: i,
            journal: Journal {
                id: fsutil::unique(),
                label: description.into(),
                changes,
                done: false,
            },
            remove: vec![],
            keep_settings: false,
            download: false,
            replacement: None,
            client_removals: vec![],shared_migration:false,installed_editions:vec![],
        },
    )
}
fn exit_settings_path(i: &Installation) -> PathBuf {
    i.prefix.join("pfx/drive_c/users/steamuser/AppData/Roaming/Battle.net/Battle.net.config")
}
// Read only the required typed field. No private vendor contents enter the response.
fn exit_preference_status(i: &Installation) -> Json {
    let read = || -> Result<bool> {
        let path = exit_settings_path(i);
        if fsutil::canonical(&path)? != path || fs::metadata(&path).map_err(|e|e.to_string())?.len() > 1024*1024 {
            return Err("Battle.net settings are unavailable or unsafe to inspect".into());
        }
        let bytes=fsutil::read(&path)?;
        let text=std::str::from_utf8(&bytes).map_err(|_|"Unrecognized Battle.net settings encoding")?.trim_start_matches('\u{feff}');
        let value:Json=serde_json::from_str(text).map_err(|_|"Unrecognized Battle.net settings")?;
        Ok(value.get("Client").and_then(|c|c.get("GameLaunchWindowBehavior")).and_then(Json::as_str)==Some("2"))
    };
    match read() {
        Ok(configured)=>json!({"configured":configured,"ready":configured&&i.launcher_exit_reviewed,"state":if configured {"configured"} else {"needs_review"},"message":if configured {"Current Close after Play setting verified. Actual launcher exit must still be checked in game."}else{"Automatic launcher exit is not configured. Review the backed-up Close after Play change before completing setup."}}),
        Err(_)=>json!({"configured":false,"ready":false,"state":"unavailable","message":"Battle.net settings could not be verified. Open this installation's Battle.net once, sign in privately if needed, then close Battle.net and WoW normally and choose Check again. No preference was changed."}),
    }
}
fn require_exit_preference(i: &Installation) -> Result<()> {
    if exit_preference_status(i)["ready"]!=true {
        return Err("Automatic launcher exit is not verified for this installation. Continue the normal Battle.net after Play setup step; preserve current/shared preferences until you explicitly approve a reviewed change.".into());
    }
    Ok(())
}
fn backup_exit_preference(ctx:&Context,plan:&Plan)->Result<()> {
    if plan.installation.launcher_exit_choice.as_deref()!=Some("close_after_play") {return Ok(())}
    let path=exit_settings_path(&plan.installation);
    let change=plan.journal.changes.iter().find(|c|c.path==path).ok_or("Missing reviewed Battle.net preference change")?;
    let original=change.before.as_ref().ok_or("Original Battle.net settings are required for backup")?;
    if fsutil::read(&path)?!=*original {return Err("Battle.net settings changed before backup; review again. No preference changed.".into())}
    let dest=ctx.data.join("launcher-exit-backups").join(&plan.token);
    for directory in [ctx.data.join("launcher-exit-backups"),dest.clone()] {
        match fs::symlink_metadata(&directory) {
            Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink()=>return Err("Linked or non-directory launcher backup location refused; no preference changed.".into()),
            Ok(_)=>{},
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},
            Err(e)=>return Err(e.to_string()),
        }
        fsutil::private_dir(&directory)?;
    }
    for file in [dest.join("Battle.net.config.original"),dest.join("SHA256.json")] {
        match fs::symlink_metadata(file) {
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink()=>return Err("Linked or nonregular launcher backup refused; no preference changed.".into()),
            Ok(_)=>{},
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},
            Err(e)=>return Err(e.to_string()),
        }
    }
    let saved = dest.join("Battle.net.config.original");
    if saved.exists() {
        if fsutil::read(&saved)?!=*original {return Err("Existing launcher-settings backup differs; preserve it and review again.".into())}
    } else {fsutil::atomic(&saved,original)?;}
    let seal=dest.join("SHA256.json");
    let expected=json!({"sha256":fsutil::hash(original),"bytes":original.len(),"scope":"Original settings for this reviewed installation-wide Close after Play change"});
    if seal.exists() {
        if fsutil::load::<Json>(&seal)?!=expected {return Err("Launcher-settings backup seal differs; no preference changed.".into())}
    } else {fsutil::json(&seal,&expected)?;}
    if fsutil::hash(&fsutil::read(&saved)?)!=fsutil::hash(original) || fsutil::read(&path)?!=*original {
        return Err("Launcher-settings backup verification failed; no preference changed.".into());
    }
    Ok(())
}
fn review_exit(ctx: &Context, req: &Json) -> Result<Json> {
    let mut i = ctx.install(field(req, "id")?)?;
    let choice = req["choice"].as_str().unwrap_or("close_after_play");
    if !["close_after_play", "keep_current"].contains(&choice) {
        return Err("Choose Close after Play or Keep current behavior.".into());
    }
    if choice == "close_after_play" && req["confirm_installation_wide"] != true {
        return Err("Explicitly approve changing this installation's Battle.net behavior in BOTH Desktop and Gaming Mode, including other users of a shared environment.".into());
    }
    i.launcher_exit_choice = Some(choice.into());
    i.launcher_exit_reviewed = true;
    let mut state = ctx.state.clone();
    *state.installations.iter_mut().find(|x| x.id == i.id).ok_or("Select an installation")? = i.clone();
    let state_change = Change::new(ctx.data.join("state.json"), serde_json::to_vec_pretty(&state).unwrap())?;
    if choice == "keep_current" {
        return simple_plan(ctx, i, "exit", "Keep the current Battle.net setting unchanged and remember your reviewed choice. Setup completes only when the current Close after Play setting is verified; otherwise automatic-exit setup stays incomplete. No vendor setting is changed.", vec![state_change]);
    }
    let path = i
        .prefix
        .join("pfx/drive_c/users/steamuser/AppData/Roaming/Battle.net/Battle.net.config");
    if !path.exists() {
        return Err("Battle.net settings are not available yet. Open the installed Battle.net once, then close Battle.net and WoW normally and review this choice again. No preference was changed.".into());
    }
    if fsutil::canonical(&path)? != path
        || fs::metadata(&path).map_err(|e| e.to_string())?.len() > 1024 * 1024
    {
        return Err("Battle.net settings cannot be safely changed.".into());
    }
    let bytes = fsutil::read(&path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "Unrecognized Battle.net settings encoding")?
        .trim_start_matches('\u{feff}');
    let mut value: Json =
        serde_json::from_str(text).map_err(|_| "Unrecognized Battle.net settings")?;
    let client = value
        .get_mut("Client")
        .and_then(Json::as_object_mut)
        .ok_or(
            "Battle.net Client settings were not found; change the setting inside Battle.net.",
        )?;
    client.insert("GameLaunchWindowBehavior".into(), json!("2"));
    simple_plan(ctx,i,"exit","Back up this installation’s Battle.net settings locally, then set Exit Battle.net completely after Play. This affects BOTH Desktop and Gaming Mode and everyone using this installation. Other fields, games and shortcuts are preserved. Close Battle.net and WoW before applying. Save the reviewed choice for reruns; do not silently reapply it.",vec![Change::new(path,serde_json::to_vec_pretty(&value).unwrap())?,state_change])
}
fn review_reinstall(ctx: &Context, req: &Json) -> Result<Json> {
    workflow::request(req)?;
    let old = ctx.install(field(req, "id")?)?;
    review_clean(ctx, req)?;
    let mut clean: Plan = fsutil::load(&ctx.data.join("review.json"))?;
    let request = json!({"steam":old.steam,"user":old.user,"proton":old.proton,"location":field(req,"location")?,"editions":req["editions"],"language":req["language"].as_str().unwrap_or(&old.language),"addon_manager":req.get("addon_manager").cloned().unwrap_or_else(||json!(if old.addon_manager.is_empty(){"none"}else{old.addon_manager.as_str()})),"launcher_tile":req.get("launcher_tile").cloned().unwrap_or_else(||json!(old.launcher_tile))});
    // Build the new proposal independently; combine desired states into one transaction.
    review_setup(ctx, &request)?;
    let setup: Plan = fsutil::load(&ctx.data.join("review.json"))?;
    let mut replacement = setup.installation;
    replacement.launcher_exit_choice = old.launcher_exit_choice.clone();
    replacement.launcher_exit_reviewed = false; // Fresh prefix needs its own explicit review and backup.
    replacement.addon_pending = true;
    replacement.restore_backup = if clean.keep_settings {
        Some(clean.token.clone())
    } else {
        None
    };
    let mut state = ctx.state.clone();
    state.installations.retain(|i| i.id != old.id);
    state.installations.push(replacement.clone());
    state.current = Some(replacement.id.clone());
    state.addon_pending = true;
    let path = steam::shortcuts_path(&old.steam, &old.user)?;
    let change = clean
        .journal
        .changes
        .iter_mut()
        .find(|c| c.path == path)
        .ok_or("Missing shortcut removal plan")?;
    let mut shortcuts = vdf::parse_binary(&change.after)?;
    let opts = format!(
        "{} --lang={}",
        steam::launch_options(&replacement.prefix, None)?,
        replacement.language
    );
    steam::upsert(
        &mut shortcuts,
        steam::entry(
            "Battle.net",
            &replacement.root.join("Battle.net-Setup.exe"),
            &opts,
            &replacement.id,
            Some(replacement.bootstrap_id),
        )?,
        &replacement.id,
    )?;
    change.after = vdf::binary(&shortcuts)?;
    clean
        .journal
        .changes
        .iter_mut()
        .find(|c| c.path == ctx.data.join("state.json"))
        .unwrap()
        .after = serde_json::to_vec_pretty(&state).unwrap();
    for change in setup.journal.changes {
        if change.path == old.steam.join("config/config.vdf") {
            clean.journal.changes.push(change);
        }
    }
    clean.kind = "reinstall".into();
    clean.replacement = Some(replacement);
    clean.summary.retain(|s| !s.starts_with("After removal"));
    clean.summary.push("Prepare a fresh Battle.net installer and environment, remove only the reviewed old folders, then continue installation here. A failed download leaves old game folders intact.".into());
    clean.summary.push("After downloading games, restore the preserved settings from Continue setup, then reconnect the addon manager. Existing destination settings will never be overwritten.".into());
    clean.summary.extend(
        setup
            .summary
            .into_iter()
            .filter(|s| s.starts_with("Create") || s.starts_with("Use installed")),
    );
    save_plan(ctx, &clean)
}
fn validate_cleanup(ctx: &Context, job: &Cleanup) -> Result<()> {
    let p = &job.plan;
    workflow::installation(&p.installation)?;
    if !workflow::token(&p.token)
        || !["clean", "clean_owned", "reinstall"].contains(&p.kind.as_str())
        || p.remove.is_empty()
    {
        return Err("Cleanup checkpoint is invalid".into());
    }
    if let Some(i) = &p.replacement {
        workflow::installation(i)?;
        if i.prefix == p.installation.prefix {
            return Err("Replacement must have a fresh environment".into());
        }
    }
    if job.settings_saved && p.keep_settings {
        let marker: String = fsutil::load(
            &ctx.data
                .join("settings-backups")
                .join(&p.token)
                .join(".complete.json"),
        )?;
        if marker != p.token {
            return Err("Completed preservation checkpoint has no matching backup.".into());
        }
    }
    let i = &p.installation;
    let mut allowed = vec![i.root.join("games/World of Warcraft")];
    if p.kind=="clean_owned" {
        owned_references(ctx,i)?;
        if !i.owned || p.remove.len()!=1 || p.remove[0].path!=i.root {return Err("Invalid complete-removal checkpoint.".into());}
        if i.root.exists(){exclusive_owned_root(ctx,i)?;}
        allowed.push(i.root.clone());
    }

    for parent in ["Program Files", "Program Files (x86)"] {
        for child in ["World of Warcraft", "Battle.net"] {
            allowed.push(i.prefix.join("pfx/drive_c").join(parent).join(child));
        }
    }
    let mut seen = BTreeSet::new();
    for r in &p.remove {
        if !allowed.contains(&r.path)
            || !seen.insert(&r.path)
            || r.quarantine.parent() != r.path.parent()
            || !r
                .quarantine
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .starts_with(&format!(".wow-removing-{}-", p.token))
        {
            return Err("Cleanup checkpoint contains an unexpected removal path.".into());
        }
    }
    // Only this validated checkpoint's intact quarantine directories are ours.
    let quarantines: Vec<PathBuf> = p.remove.iter().filter(|r| {
        r.quarantine.exists() && !r.path.exists()
            && fsutil::identity(&r.quarantine).map(|id| id == r.identity).unwrap_or(false)
    }).map(|r| r.quarantine.clone()).collect();
    require_exclusive_apps_except(i, &quarantines)?;
    for c in &p.journal.changes {
        if c.path != ctx.data.join("state.json")
            && c.path != steam::shortcuts_path(&i.steam, &i.user)?
            && c.path != i.steam.join("config/config.vdf")
        {
            return Err("Cleanup checkpoint contains an unexpected configuration path.".into());
        }
    }
    // Recheck every discovered Steam root/profile, including scoped cleanup.
    removal_references(ctx, i, p.kind == "clean_owned")?;
    if p.journal.done {
        let root = steam::read_shortcuts(&steam::shortcuts_path(&i.steam,&i.user)?)?;
        if vdf::object(&root,"shortcuts").unwrap().iter().any(|(_,v)|matches!(v,Value::Object(e) if steam::owned(e,&i.id) && !steam::native_addon(e))) {
            return Err("This installation has Steam tiles again. Stop removal and review its shortcuts before continuing.".into());
        }
        let state: State = fsutil::load(&ctx.data.join("state.json"))?;
        if state.installations.iter().any(|x|x.id==i.id) {
            return Err("This installation is registered again. Stop removal and review it.".into());
        }
    }
    Ok(())
}
fn legacy_candidates(ctx: &Context, req: &Json) -> Result<Json> {
    let i = ctx.install(field(req, "id")?)?;
    let root = steam::read_shortcuts(&steam::shortcuts_path(&i.steam, &i.user)?)?;
    let mut items = vec![];
    for (_, v) in vdf::object(&root, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            let name = vdf::string(e, "appname").unwrap_or("");
            let lower = name.to_ascii_lowercase();
            if !steam::owned(e, &i.id)
                && (lower.contains("wow")
                    || lower.contains("world of warcraft")
                    || lower.contains("battle.net"))
            {
                items.push(json!({"appid":vdf::number(e,"appid"),"name":name,"target":vdf::string(e,"exe"),"options":vdf::string(e,"LaunchOptions")}));
            }
        }
    }
    Ok(json!({"items":items}))
}
fn review_legacy(ctx: &Context, req: &Json) -> Result<Json> {
    workflow::request(req)?;
    let i = ctx.install(field(req, "id")?)?;
    let candidates = legacy_candidates(ctx, req)?;
    let selected = req["selected"]
        .as_array()
        .ok_or("Select the old tiles to remove")?;
    if selected.is_empty() {
        return Err("Select at least one old tile".into());
    }
    for id in selected {
        if !candidates["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["appid"] == *id)
        {
            return Err("Old tile selection changed; review again".into());
        }
    }
    let path = steam::shortcuts_path(&i.steam, &i.user)?;
    let mut root = steam::read_shortcuts(&path)?;
    let entries = vdf::ensure(&mut root, "shortcuts")?;
    entries.retain(|(_,v)|!matches!(v,Value::Object(e)if selected.iter().any(|id|id.as_u64()==vdf::number(e,"appid").map(u64::from))));
    for (n, (k, _)) in entries.iter_mut().enumerate() {
        *k = n.to_string();
    }
    let names: Vec<_> = candidates["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| selected.contains(&e["appid"]))
        .map(|e| format!("{} (ID {})", e["name"].as_str().unwrap_or(""), e["appid"]))
        .collect();
    simple_plan(ctx,i,"legacy",&format!("Remove only these explicitly selected old Steam tiles: {}. Game files stay in place. These tiles may belong to another installation; check the targets before applying.",names.join(", ")),vec![Change::new(path,vdf::binary(&root)?)?])
}
#[derive(Serialize, Deserialize)]
struct RestoreItem {
    source: PathBuf,
    destination: PathBuf,
    digest: String,
    done: bool,
}
#[derive(Serialize, Deserialize)]
struct RestoreJob {
    installation: String,
    backup: String,
    items: Vec<RestoreItem>,
}
fn tree_digest(root: &Path) -> Result<String> {
    fsutil::check_tree(root)?;
    let mut paths = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in fs::read_dir(&p).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            paths.push(e.path());
            if e.file_type().map_err(|e| e.to_string())?.is_dir() {
                stack.push(e.path());
            }
        }
    }
    paths.sort();
    let mut bytes = vec![];
    for p in paths {
        bytes.extend_from_slice(p.strip_prefix(root).unwrap().as_os_str().as_encoded_bytes());
        bytes.push(0);
        if p.is_file() {
            bytes.extend_from_slice(fsutil::hash(&fsutil::read(&p)?).as_bytes());
        }
        bytes.push(0);
    }
    Ok(fsutil::hash(&bytes))
}
fn restore_settings(ctx: &mut Context, req: &Json) -> Result<Json> {
    discovery::require_games_closed_for(discovery::operation_description("restore"))?;
    let i = ctx.install(field(req, "id")?)?;
    let key = field(req, "backup")?;
    if !workflow::token(key) {
        return Err("Invalid backup selection".into());
    }
    let backup = ctx.data.join("settings-backups").join(key);
    let marker: String = fsutil::load(&backup.join(".complete.json"))?;
    if marker != key {
        return Err("Backup identity mismatch".into());
    }
    let scan = discovery::scan(&ctx.home, &ctx.state)?;
    let games = installation_games(ctx, &i, &scan)?;
    let mut items = vec![];
    let mut used = BTreeSet::new();
    for game in games {
        for folder in ["Interface", "WTF"] {
            let source = backup.join(&game.id).join(folder);
            let destination = game.path.join(folder);
            if source.is_dir() {
                if !used.insert(source.clone()) {
                    return Err("Multiple copies match the saved edition. Link only the intended copy before restoring.".into());
                }
                if destination.exists() {
                    return Err(format!("{} already exists. Move it aside before restoring; setup never overwrites it.",destination.display()));
                }
                items.push(RestoreItem {
                    digest: tree_digest(&source)?,
                    source,
                    destination,
                    done: false,
                });
            }
        }
    }
    if items.is_empty() {
        return Err("No installed edition matches this settings backup.".into());
    }
    for edition in fs::read_dir(&backup).map_err(|e| e.to_string())? {
        let edition = edition.map_err(|e| e.to_string())?;
        for folder in ["Interface", "WTF"] {
            let source = edition.path().join(folder);
            if source.is_dir() && !used.contains(&source) {
                return Err("This backup includes editions that are not installed or linked yet. Finish those downloads and check the edition associations before restoring. The backup is kept.".into());
            }
        }
    }

    let mut job = RestoreJob {
        installation: i.id,
        backup: key.into(),
        items,
    };
    fsutil::json(&ctx.data.join("restore.json"), &job)?;
    run_restore(ctx, &mut job)?;
    Ok(
        json!({"message":"Preserved settings restored. Check the game folders in your addon manager."}),
    )
}
fn run_restore(ctx: &mut Context, job: &mut RestoreJob) -> Result<()> {
    discovery::require_games_closed_for(discovery::operation_description("restore"))?;
    let i = ctx.install(&job.installation)?;
    if !workflow::token(&job.backup) || job.items.is_empty() {
        return Err("Invalid restore checkpoint".into());
    }
    let backup = ctx.data.join("settings-backups").join(&job.backup);
    let scan = discovery::scan(&ctx.home, &ctx.state)?;
    let games = installation_games(ctx, &i, &scan)?;
    for item in &job.items {
        if !item.source.starts_with(&backup)
            || !games.iter().any(|g| {
                ["Interface", "WTF"].iter().any(|f| {
                    item.destination == g.path.join(f) && item.source == backup.join(&g.id).join(f)
                })
            })
            || tree_digest(&item.source)? != item.digest
        {
            return Err("Restore paths or saved contents changed. Restore paused.".into());
        }
        if item.destination.exists() && tree_digest(&item.destination)? != item.digest {
            return Err(
                "A restore destination contains different settings. Nothing will be overwritten."
                    .into(),
            );
        }
    }
    for n in 0..job.items.len() {
        crate::cancelled()?;
        discovery::require_games_closed()?;
        let item = &job.items[n];
        if !item.destination.exists() {
            let stage = item
                .destination
                .with_file_name(format!(".restore-{}", fsutil::unique()));
            if let Err(e) = copy_tree(&item.source, &stage) {
                let _ = fs::remove_dir_all(stage);
                return Err(e);
            }
            if tree_digest(&stage)? != item.digest || item.destination.exists() {
                let _ = fs::remove_dir_all(stage);
                return Err("Settings changed during restoration; retry after inspection.".into());
            }
            fs::rename(stage, &item.destination).map_err(|e| e.to_string())?;
            fs::File::open(item.destination.parent().unwrap())
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
        job.items[n].done = true;
        fsutil::json(&ctx.data.join("restore.json"), job)?;
    }
    let mut state = ctx.state.clone();
    let i = state
        .installations
        .iter_mut()
        .find(|x| x.id == job.installation)
        .unwrap();
    i.restore_backup = None;
    i.addon_pending = true;
    state.addon_pending = true;
    ctx.persist(state)?;
    fs::remove_file(ctx.data.join("restore.json")).map_err(|e| e.to_string())?;
    Ok(())
}

fn require_exclusive_apps(i: &Installation) -> Result<()> {
    require_exclusive_apps_except(i, &[])
}
fn require_exclusive_apps_except(i: &Installation, quarantines: &[PathBuf]) -> Result<()> {
    for parent in ["Program Files", "Program Files (x86)"] {
        let path = i.prefix.join("pfx/drive_c").join(parent);
        if !path.exists() { continue; }
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if entry.file_type().map_err(|e| e.to_string())?.is_dir()
                && !["battle.net", "world of warcraft", "steam", "common files", "internet explorer", "windows media player", "windows nt"].contains(&name.as_str())
                && !quarantines.contains(&entry.path()) {
                return Err("Other application folders share this Windows environment. Keep this copy and install another; automatic removal is blocked.".into());
            }
        }
    }
    Ok(())
}
fn cancel_cleanup(ctx: &Context) -> Result<Json> {
    let job: Cleanup = fsutil::load(&ctx.data.join("cleanup.json"))?;
    if job.plan.journal.done || ctx.data.join("transaction.json").exists() {
        return Err("Steam changes have started. Resume the saved operation to finish safely.".into());
    }
    fs::remove_file(ctx.data.join("cleanup.json")).map_err(|e| e.to_string())?;
    fs::File::open(&ctx.data).and_then(|f| f.sync_all()).map_err(|e| e.to_string())?;
    Ok(json!({"message":"Reinstall/removal cancelled before Steam changes. Any preserved settings backup is kept; game folders were not removed."}))
}

/// Stop a stuck operation without undoing committed writes or deleting more files.
fn abandon_cleanup(ctx: &Context) -> Result<Json> {
    discovery::require_stopped()?;
    discovery::require_games_closed()?;
    let job: Cleanup=fsutil::load(&ctx.data.join("cleanup.json"))?;
    // Restore only an intact quarantined folder; never overwrite a replacement.
    for r in &job.plan.remove {
        if r.quarantine.exists() {return Err("A removal folder is already staged or partly deleted. Resume removal or inspect it manually; setup will not hide this unfinished deletion.".into());}
    }
    let archive=ctx.data.join("stopped-operations").join(fsutil::unique());
    fsutil::private_dir(&archive)?;
    for name in ["cleanup.json","transaction.json","review.json"] {
        let p=ctx.data.join(name);if p.exists(){fs::rename(&p,archive.join(name)).map_err(|e|e.to_string())?;}
    }
    Ok(json!({"message":"Stopped this operation. No more files will be removed. Changes already saved remain in place. Scan to find any remaining installation; local recovery records were kept."}))
}
