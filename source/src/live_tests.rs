//! Live tests use the production operations, never a second installer implementation.
//! Only an installation allocated by prepare() can be changed. Launches are
//! checkpointed BEFORE dispatch, so resuming cannot silently launch twice.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Review,
    Setup,
    Installer,
    Installed,
    Handoff,
    Launcher,
    Downloaded,
    Repair,
    Repeat,
    ExitSetting,
    UndoSetting,
    Game,
    Observed,
    PostLaunchRepair,
    Finish,
    Reinstall,
    Restore,
    Clean,
    Done,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Row {
    label: String,
    status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Run {
    version: String,
    source: String,
    token: String,
    installation: Installation,
    phase: Phase,
    cleanup: bool,
    reinstall: bool,
    second_pass: bool,
    index: usize,
    games: Vec<(u32, String)>,
    rows: Vec<Row>,
    pending: Option<Plan>,
    after: Option<Phase>,
    undo: Option<Journal>,
    approved: bool,
    restore_started: bool,
}
fn path(ctx: &Context) -> PathBuf {
    ctx.data.join("live-test.json")
}
fn save(ctx: &Context, r: &Run) -> Result<()> {
    fsutil::json(&path(ctx), r)
}
fn load(ctx: &Context) -> Result<Run> {
    let r: Run = fsutil::load(&path(ctx))?;
    if r.version != VERSION || r.source != env!("WOW_SOURCE_SHA256") {
        return Err("This run belongs to another build. Keep its test copy and start testing from the normal setup screens.".into());
    }
    workflow::installation(&r.installation)?;
    if !r.installation.owned
        || !workflow::token(&r.token)
        || r.rows.len() > 100
        || r.index > r.games.len()
    {
        return Err("Invalid test checkpoint. No changes made.".into());
    }
    if r.installation.root.file_name().and_then(|x| x.to_str())
        != Some(&format!("WoW-{}", &r.installation.id[..12]))
    {
        return Err("Test scope is invalid.".into());
    }
    Ok(r)
}
fn scope(ctx: &Context, r: &Run) -> Result<()> {
    if let Ok(i) = ctx.install(&r.installation.id) {
        if i.root != r.installation.root
            || i.prefix != r.installation.prefix
            || i.steam != r.installation.steam
            || i.user != r.installation.user
            || !i.owned
        {
            return Err("Test installation changed. No changes made.".into());
        }
    } else if !matches!(
        r.phase,
        Phase::Review | Phase::Setup | Phase::Reinstall | Phase::Clean | Phase::Done
    ) {
        return Err("The test copy is no longer registered. No changes made.".into());
    }
    if r.installation.root.exists()
        && fsutil::canonical(&r.installation.root)? != r.installation.root
    {
        return Err("Test folder was replaced with a link.".into());
    }
    Ok(())
}
fn record(r: &mut Run, label: &str, status: &str) {
    if let Some(row) = r.rows.iter_mut().find(|x| x.label == label) {
        row.status = status.into()
    } else {
        r.rows.push(Row {
            label: label.into(),
            status: status.into(),
        })
    }
}
fn public(r: &Run) -> Json {
    let (title,message)=match r.phase {
        Phase::Review=>("Try a full installation","This creates a separate test copy, downloads Battle.net, adds and repairs Steam tiles, changes then restores its launch setting, and requests each selected game launch. Sign in only in Battle.net. Use its default game folder. Its Steam tiles start with Test. Other installations are kept. Games require disk space and downloads. You can pause at any point."),
        Phase::Installed=>("Finish installing Battle.net","Complete the installer and sign in, then close Battle.net and choose Steam → Exit. Return here and continue. KDE Connect can help you type. If no window opened, use Retry launch."),
        Phase::Downloaded=>("Download your games","In Battle.net, download every selected edition using its default folder. Then close Battle.net and Steam and continue. This checks files; it cannot prove a download or login succeeded just from opening a window."),
        Phase::Observed=>("Did the right game open?","Enter the game and play briefly. Close the game and Battle.net before continuing. Worked records your observation, not an automatic gameplay result. Use Retry launch if no window opened."),
        Phase::PostLaunchRepair=>("Check shortcuts after playing","Close WoW, Battle.net and Steam. Continue to check repair after Steam has saved its own changes."),
        Phase::Finish=>("Launch checks finished","Close WoW, Battle.net and Steam before continuing. The next step removes only this test copy if you selected removal, otherwise it keeps it. Settings backups and the small test environment may remain. If reinstall is selected, it runs next and requires downloading the games a second time. Addon updates and Gaming Mode still need separate checks."),
        Phase::Restore=>("Restore saved game settings","This restores the test copy’s saved Interface and WTF folders after its second download. Close games first. Existing folders are never overwritten. If there were no settings to save, skip this check; it will not count as a pass."),
        Phase::Done=>("Test run finished","Take a photo of the results below. REQUESTED only means a launch request was sent. CHECK and NOT TESTED are not passes. Use the other test screen for controller, addon and Gaming Mode observations."),
        _=>("Running the next check","The runner uses the normal installer operations. If Steam needs to close, choose Steam → Exit and retry. Never close Steam while a game is running."),
    };
    let automatic = !matches!(
        r.phase,
        Phase::Review
            | Phase::Installed
            | Phase::Downloaded
            | Phase::Observed
            | Phase::PostLaunchRepair
            | Phase::Finish
            | Phase::Done
    );
    let report = json!({"version":r.version,"build":&r.source[..12],"phase":r.phase,"rows":r.rows,"coverage_note":"Live run: new setup, launcher handoff, repair, repeat repair, launch setting/undo, game launch requests, optional reinstall/restore and removal. Other branches require fixture or manual checks; this is not an all-functionality pass."});
    json!({"run":{"token":r.token,"phase":r.phase,"title":title,"message":message,"automatic":automatic,"cleanup":r.cleanup,"reinstall":r.reinstall,"second_pass":r.second_pass,"rows":r.rows,"game":r.games.get(r.index).map(|x|x.1.clone()),"version":r.version,"build":&r.source[..12]},"text":serde_json::to_string_pretty(&report).unwrap(),"location":if r.phase==Phase::Review{Some(&r.installation.root)}else{None}})
}
// Keep test tiles distinguishable in the user's Steam library, without
// changing their IDs (and therefore their controller/artwork associations).
fn mark_titles(ctx: &Context, p: &mut Plan) -> Result<()> {
    let path = steam::shortcuts_path(&p.installation.steam, &p.installation.user)?;
    for change in &mut p.journal.changes {
        if change.path != path {
            continue;
        }
        let mut root = vdf::parse_binary(&change.after)?;
        for (_, value) in vdf::ensure(&mut root, "shortcuts")? {
            if let Value::Object(entry) = value {
                if steam::owned(entry, &p.installation.id)
                    || p.replacement
                        .as_ref()
                        .is_some_and(|i| steam::owned(entry, &i.id))
                {
                    let name = vdf::string(entry, "appname").unwrap_or("World of Warcraft");
                    if !name.starts_with("Test — ") {
                        let title = format!("Test — {name}");
                        vdf::set(entry, "appname", Value::Text(title));
                    }
                }
            }
        }
        change.after = vdf::binary(&root)?;
    }
    save_plan(ctx, p)?;
    Ok(())
}
fn prepare(ctx: &Context, req: &Json) -> Result<Json> {
    if path(ctx).exists() {
        let old: Run = fsutil::load(&path(ctx))?;
        if old.version == VERSION
            && old.source == env!("WOW_SOURCE_SHA256")
            && old.phase != Phase::Done
            && old.phase != Phase::Review
        {
            return Err("Resume or end the saved test first. Its test copy has been kept.".into());
        }
        let archive = ctx.data.join("previous-live-tests");
        fsutil::private_dir(&archive)?;
        fsutil::json(&archive.join(format!("{}.json", fsutil::unique())), &old)?;
    }
    review_setup(ctx, req)?;
    let mut p: Plan = fsutil::load(&ctx.data.join("review.json"))?;
    mark_titles(ctx, &mut p)?;
    if ctx
        .state
        .installations
        .iter()
        .any(|i| i.id == p.installation.id || i.prefix == p.installation.prefix)
        || p.installation.root.exists()
    {
        return Err("A test must use a new empty location.".into());
    }
    let r = Run {
        version: VERSION.into(),
        source: env!("WOW_SOURCE_SHA256").into(),
        token: fsutil::unique(),
        installation: p.installation.clone(),
        phase: Phase::Review,
        cleanup: req["cleanup"] == true,
        reinstall: req["reinstall"] == true,
        second_pass: false,
        index: 0,
        games: vec![],
        rows: [
            "Fresh setup",
            "Installer launch",
            "Launcher handoff",
            "Battle.net launch",
            "Shortcut repair",
            "Repair adds no duplicates",
            "Launch setting change",
            "Restore launch setting",
            "Removal and fresh setup",
            "Settings restore",
            "Test copy removal",
            "Addon manager / updates",
            "Existing-copy adoption",
            "Legacy tile cleanup",
            "External folder search",
            "Controller + mouse",
            "Gaming Mode",
        ]
        .iter()
        .map(|label| Row {
            label: (*label).into(),
            status: "NOT TESTED".into(),
        })
        .collect(),
        pending: Some(p),
        after: Some(Phase::Installer),
        undo: None,
        approved: false,
        restore_started: false,
    };
    save(ctx, &r)?;
    Ok(public(&r))
}
// A plan is written to the test checkpoint before applying. After a crash,
// transaction recovery must finish first; exact postconditions prevent replay.
fn queue(ctx: &Context, r: &mut Run, p: Json, next: Phase) -> Result<()> {
    let mut plan = load_plan(ctx, &p)?;
    mark_titles(ctx, &mut plan)?;
    if plan.installation.id != r.installation.id
        || (plan.replacement.is_some()
            && (!r.reinstall || r.second_pass || plan.kind != "reinstall"))
        || !["setup", "launcher", "repair", "exit", "clean", "reinstall"]
            .contains(&plan.kind.as_str())
    {
        return Err("Operation is outside this test run.".into());
    }
    validate_plan(ctx, r, &plan)?;
    r.pending = Some(plan);
    r.after = Some(next);
    save(ctx, r)
}
fn validate_plan(ctx: &Context, r: &Run, p: &Plan) -> Result<()> {
    let i = &r.installation;
    if p.installation.id != i.id
        || p.installation.root != i.root
        || p.installation.prefix != i.prefix
        || p.installation.steam != i.steam
        || p.installation.user != i.user
    {
        return Err("Test plan no longer matches its approved copy.".into());
    }
    if let Some(new) = &p.replacement {
        workflow::installation(new)?;
        if !r.reinstall
            || r.second_pass
            || p.kind != "reinstall"
            || !new.owned
            || new.root.parent() != i.root.parent()
            || new.root.file_name().and_then(|x| x.to_str())
                != Some(&format!("WoW-{}", &new.id[..12]))
            || new.steam != i.steam
            || new.user != i.user
            || new.id == i.id
        {
            return Err("Replacement is outside the approved test.".into());
        }
    }
    let shortcuts = steam::shortcuts_path(&i.steam, &i.user)?;
    let setting = i
        .prefix
        .join("pfx/drive_c/users/steamuser/AppData/Roaming/Battle.net/Battle.net.config");
    for c in &p.journal.changes {
        if ![
            shortcuts.clone(),
            i.steam.join("config/config.vdf"),
            ctx.data.join("state.json"),
            setting.clone(),
        ]
        .contains(&c.path)
        {
            return Err("Test plan contains an unrelated file.".into());
        }
        if c.path == shortcuts {
            if let Some(before) = &c.before {
                let before = vdf::parse_binary(before)?;
                let after = vdf::parse_binary(&c.after)?;
                let unrelated = |root: &vdf::Object| -> Result<Vec<Value>> {
                    Ok(vdf::object(root, "shortcuts")
                        .ok_or("Missing shortcuts root")?
                        .iter()
                        .filter_map(|(_, v)| match v {
                            Value::Object(e)
                                if !steam::owned(e, &i.id)
                                    && !p
                                        .replacement
                                        .as_ref()
                                        .is_some_and(|n| steam::owned(e, &n.id)) =>
                            {
                                Some(v.clone())
                            }
                            _ => None,
                        })
                        .collect())
                };
                if unrelated(&before)? != unrelated(&after)? {
                    return Err("Test plan would change an unrelated Steam tile.".into());
                }
            }
        }
    }
    if p.remove
        .iter()
        .any(|x| !x.path.starts_with(&i.root) || !x.quarantine.starts_with(&i.root))
    {
        return Err("Removal escaped the test folder.".into());
    }
    Ok(())
}
fn finish_plan(ctx: &mut Context, r: &mut Run) -> Result<()> {
    let mut p = r.pending.clone().ok_or("Missing test operation")?;
    let next = r.after.ok_or("Missing next test step")?;
    validate_plan(ctx, r, &p)?;
    let completed = p
        .journal
        .changes
        .iter()
        .all(|c| fsutil::fingerprint(&c.path).ok().flatten() == Some(fsutil::hash(&c.after)))
        && (!["clean", "reinstall"].contains(&p.kind.as_str())
            || p.remove
                .iter()
                .all(|x| !x.path.exists() && !x.quarantine.exists()));
    if !completed {
        save_plan(ctx, &p)?;
        apply(
            ctx,
            &json!({"token":p.token,"confirmation":if (p.kind=="clean"&&r.cleanup)||(p.kind=="reinstall"&&r.reinstall&&!r.second_pass){"REMOVE THIS COPY"}else{""}}),
        )?;
    }
    transaction::verify(&p.journal.changes, true)?;
    for c in &p.journal.changes {
        if fsutil::fingerprint(&c.path)? != Some(fsutil::hash(&c.after)) {
            return Err("Test operation did not reach its expected result.".into());
        }
    }
    ctx.state = fsutil::load(&ctx.data.join("state.json"))?;
    workflow::state(&ctx.state)?;
    if p.kind == "exit" {
        p.journal.done = true;
        r.undo = Some(p.journal.clone())
    }
    if let Some(replacement) = p.replacement.as_ref() {
        r.installation = replacement.clone();
        r.second_pass = true;
        r.index = 0;
        r.games.clear();
        r.undo = None;
    }
    record(
        r,
        match p.kind.as_str() {
            "setup" => "Fresh setup",
            "launcher" => "Launcher handoff",
            "repair" if r.phase == Phase::PostLaunchRepair => "Repair after game launch",
            "repair" => "Shortcut repair",
            "exit" => "Launch setting change",
            "clean" => "Test copy removal",
            "reinstall" => "Removal and fresh setup",
            _ => "Operation",
        },
        "PASS",
    );
    r.pending = None;
    r.after = None;
    r.phase = next;
    save(ctx, r)
}
fn request_launch(ctx: &Context, r: &mut Run, appid: u32, next: Phase, label: &str) -> Result<()> {
    // Never repeat this side effect implicitly, even if the opener crashes.
    r.phase = next;
    record(r, label, "NOT CONFIRMED");
    save(ctx, r)?;
    launch(ctx, &json!({"id":r.installation.id,"appid":appid}))?;
    record(r, label, "REQUESTED");
    save(ctx, r)
}
fn step(ctx: &mut Context, req: &Json) -> Result<Json> {
    let mut r = load(ctx)?;
    if field(req, "token")? != r.token {
        return Err("Test run changed. Reopen the test screen.".into());
    }
    scope(ctx, &r)?;
    if r.phase == Phase::Review {
        if req["approved"] != true {
            return Err("Review the test run before starting it.".into());
        }
        r.approved = true;
        r.phase = Phase::Setup;
        save(ctx, &r)?;
    }
    if !r.approved {
        return Err("This test run has not been approved.".into());
    }
    crate::cancelled()?;
    if r.pending.is_some() {
        finish_plan(ctx, &mut r)?;
        return Ok(public(&r));
    }
    let id = r.installation.id.clone();
    let args = json!({"id":id});
    match r.phase {
        Phase::Installer => {
            let app = r.installation.bootstrap_id;
            request_launch(ctx, &mut r, app, Phase::Installed, "Installer launch")?;
        }
        Phase::Installed => {
            discovery::require_stopped()?;
            discovery::require_games_closed()?;
            r.phase = Phase::Handoff;
            save(ctx, &r)?;
        }
        Phase::Handoff => queue(ctx, &mut r, review_launcher(ctx, &args)?, Phase::Launcher)?,
        Phase::Launcher => {
            let app = ctx.install(&id)?.bootstrap_id;
            request_launch(ctx, &mut r, app, Phase::Downloaded, "Battle.net launch")?;
        }
        Phase::Downloaded => {
            discovery::require_stopped()?;
            discovery::require_games_closed()?;
            r.phase = Phase::Repair;
            save(ctx, &r)?;
        }
        Phase::Repair => queue(
            ctx,
            &mut r,
            review_repair(
                ctx,
                &json!({"id":id,"confirm_forever_beta":req["confirm_forever_beta"]==true}),
            )?,
            Phase::Repeat,
        )?,
        Phase::Repeat => {
            discovery::require_stopped()?;
            let p = review_repair(ctx, &args)?;
            let mut p = load_plan(ctx, &p)?;
            mark_titles(ctx, &mut p)?;
            save_plan(ctx, &p)?;
            let stable = p
                .journal
                .changes
                .iter()
                .filter(|c| c.path != ctx.data.join("state.json"))
                .all(|c| c.before.as_ref() == Some(&c.after));
            record(
                &mut r,
                "Repair adds no duplicates",
                if stable { "PASS" } else { "FAIL" },
            );
            if !stable {
                save(ctx, &r)?;
                return Err("Repeating repair would change Steam again. Test stopped; inspect the result before continuing.".into());
            }
            // Actually apply the second repair through the same guarded transaction.
            let next = if r.second_pass {
                Phase::Restore
            } else {
                Phase::ExitSetting
            };
            queue(ctx, &mut r, json!({"token":p.token}), next)?;
        }
        Phase::ExitSetting => {
            discovery::require_stopped()?;
            discovery::require_games_closed()?;
            // The separately consented installation test temporarily changes and restores this setting.
            let mut exit_args = args.clone();
            exit_args["confirm_installation_wide"] = json!(true);
            match review_exit(ctx, &exit_args) {
                Ok(p) => queue(ctx, &mut r, p, Phase::UndoSetting)?,
                Err(_) => {
                    record(&mut r, "Launch setting change", "CHECK");
                    record(&mut r, "Restore launch setting", "NOT TESTED");
                    r.phase = Phase::Game;
                    save(ctx, &r)?;
                }
            }
        }
        Phase::UndoSetting => {
            discovery::require_stopped()?;
            discovery::require_games_closed()?;
            let old = r.undo.as_ref().ok_or("Missing setting backup")?;
            let mut changes = vec![];
            for c in &old.changes {
                let before = c.before.clone().ok_or("Cannot restore a missing setting")?;
                let now = fsutil::fingerprint(&c.path)?;
                if now != Some(fsutil::hash(&before)) && now != Some(fsutil::hash(&c.after)) {
                    return Err(
                        "Battle.net settings changed after the test; refusing to overwrite them."
                            .into(),
                    );
                }
                changes.push(Change::new(c.path.clone(), before)?);
            }
            let mut journal = Journal {
                id: fsutil::unique(),
                label: "Restore live test launch setting".into(),
                changes,
                done: false,
            };
            transaction::commit(&ctx.data, &mut journal, true)?;
            record(&mut r, "Restore launch setting", "PASS");
            r.phase = Phase::Game;
            save(ctx, &r)?;
        }
        Phase::Game => {
            discovery::require_games_closed()?;
            if r.games.is_empty() {
                let root = steam::read_shortcuts(&steam::shortcuts_path(
                    &r.installation.steam,
                    &r.installation.user,
                )?)?;
                for (_, v) in vdf::object(&root, "shortcuts").unwrap() {
                    if let Value::Object(e) = v {
                        if steam::owned(e, &id) && vdf::string(e, "wowRustGame").is_some() {
                            // Only allowlisted edition labels reach the photograph/report.
                            let edition = vdf::string(e, "wowRustEdition").unwrap_or("other");
                            let label = catalog::by_id(edition)
                                .map(|x| x.name)
                                .unwrap_or("Other edition");
                            r.games.push((
                                vdf::number(e, "appid").ok_or("Game tile lacks an ID")?,
                                label.into(),
                            ));
                        }
                    }
                }
                save(ctx, &r)?;
            }
            if let Some((app, label)) = r.games.get(r.index).cloned() {
                request_launch(
                    ctx,
                    &mut r,
                    app,
                    Phase::Observed,
                    &format!("Launch: {label}"),
                )?;
            } else {
                r.phase = Phase::PostLaunchRepair;
                save(ctx, &r)?;
            }
        }
        Phase::Observed => {
            let outcome = field(req, "outcome")?;
            if !["PASS", "FAIL", "SKIPPED"].contains(&outcome) {
                return Err("Choose Worked, Problem or Skip.".into());
            }
            discovery::require_games_closed()?;
            let label = r
                .games
                .get(r.index)
                .ok_or("Missing game observation")?
                .1
                .clone();
            let label = format!(
                "You{}: {label}",
                if r.second_pass {
                    " after reinstall"
                } else {
                    ""
                }
            );
            record(&mut r, &label, outcome);
            r.index += 1;
            r.phase = Phase::Game;
            save(ctx, &r)?;
        }
        Phase::PostLaunchRepair => {
            discovery::require_stopped()?;
            discovery::require_games_closed()?;
            let p = review_repair(ctx, &args)?;
            queue(ctx, &mut r, p, Phase::Finish)?;
        }
        Phase::Finish => {
            r.phase = if r.reinstall && !r.second_pass {
                Phase::Reinstall
            } else if r.cleanup {
                Phase::Clean
            } else {
                Phase::Done
            };
            if !r.cleanup {
                record(&mut r, "Test copy removal", "SKIPPED")
            };
            save(ctx, &r)?;
        }
        Phase::Reinstall => {
            discovery::require_stopped()?;
            discovery::require_games_closed()?;
            let i = ctx.install(&id)?;
            queue(
                ctx,
                &mut r,
                review_reinstall(
                    ctx,
                    &json!({"id":id,"keep_settings":true,"location":i.root.parent(),"editions":i.editions,"language":i.language}),
                )?,
                Phase::Installer,
            )?;
        }
        Phase::Restore => {
            discovery::require_games_closed()?;
            let i = ctx.install(&id)?;
            if req["skip"] == true {
                record(&mut r, "Settings restore", "SKIPPED");
            } else if let Some(backup) = i.restore_backup {
                r.restore_started = true;
                save(ctx, &r)?;
                restore_settings(ctx, &json!({"id":id,"backup":backup}))?;
                record(&mut r, "Settings restore", "PASS");
            } else if r.restore_started {
                // Restore may have completed just before a restart. Do not invent
                // a pass without retained postcondition evidence.
                record(&mut r, "Settings restore", "CHECK");
            } else {
                return Err("No settings backup is linked to this test copy. Skip this check if no settings were saved.".into());
            }
            r.phase = Phase::ExitSetting;
            save(ctx, &r)?;
        }
        Phase::Clean => {
            discovery::require_stopped()?;
            discovery::require_games_closed()?;
            queue(
                ctx,
                &mut r,
                review_clean(ctx, &json!({"id":id,"keep_settings":true}))?,
                Phase::Done,
            )?;
        }
        Phase::Done => {}
        _ => return Err("Invalid test step".into()),
    }
    Ok(public(&r))
}
pub(super) fn dispatch(ctx: &mut Context, req: &Json) -> Result<Json> {
    match field(req, "action")? {
        "live_prepare" => prepare(ctx, req),
        "live_result" => {
            if path(ctx).exists() {
                let saved: Run = fsutil::load(&path(ctx))?;
                if saved.version != VERSION || saved.source != env!("WOW_SOURCE_SHA256") {
                    Ok(
                        json!({"run":null,"message":"The saved test belongs to a different build. Its files are kept. Start a new test copy for this build; the earlier record will be archived locally."}),
                    )
                } else {
                    Ok(public(&load(ctx)?))
                }
            } else {
                Ok(json!({"run":null}))
            }
        }
        "live_step" => step(ctx, req),
        "live_stop" => {
            let mut r = load(ctx)?;
            if field(req, "token")? != r.token {
                return Err("Reopen the current test before ending it.".into());
            }
            r.pending = None;
            r.after = None;
            r.phase = Phase::Done;
            record(&mut r, "Run ended early; files kept", "SKIPPED");
            save(ctx, &r)?;
            Ok(public(&r))
        }
        "live_retry_launch" => {
            let mut r = load(ctx)?;
            scope(ctx, &r)?;
            if field(req, "token")? != r.token || !r.approved {
                return Err("Review the active test run first.".into());
            }
            discovery::require_games_closed()?;
            let (app, label) = match r.phase {
                Phase::Installed => (r.installation.bootstrap_id, "Installer launch".into()),
                Phase::Downloaded => (
                    ctx.install(&r.installation.id)?.bootstrap_id,
                    "Battle.net launch".into(),
                ),
                Phase::Observed => {
                    let (app, label) = r.games.get(r.index).ok_or("Missing game")?;
                    (*app, format!("Launch: {label}"))
                }
                _ => return Err("No launch is waiting for confirmation.".into()),
            };
            let phase = r.phase;
            request_launch(ctx, &mut r, app, phase, &label)?;
            Ok(public(&r))
        }
        _ => Err("Unknown live test action".into()),
    }
}
