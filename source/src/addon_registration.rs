// Product-owned supported-UI registration. No manager database access.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AddonRegistrationPlan {
    token: String,
    installation: String,
    targets: Vec<Json>,
    desktop: PathBuf,
    desktop_hash: String,
    created: u64,
    #[serde(default)]
    purpose: String,
    #[serde(default)]
    scope: Json,
    #[serde(default)]
    source: String,
    #[serde(default)]
    root_identity: Json,
}
fn registration_targets(ctx: &Context, id: &str) -> Result<Vec<Json>> {
    let i = ctx.install(id)?;
    let prefix = fsutil::canonical(&i.prefix)?;
    let scan = discovery::scan(&ctx.home, &ctx.state)?;
    if !scan.warnings.is_empty() {
        return Err(
            "Game discovery is incomplete. Resolve the scan warning before connecting CurseForge."
                .into(),
        );
    }
    let mut targets = vec![];
    let mut editions = BTreeSet::new();
    for game in installation_games(ctx, &i, &scan)? {
        if !["retail", "era"].contains(&game.id.as_str()) {
            continue;
        }
        if !editions.insert(game.id.clone()) {
            return Err("Multiple paths for the same edition are ambiguous. Choose the intended installation before connecting.".into());
        }
        let client = fsutil::canonical(&game.path)?;
        let exe = fsutil::canonical(&game.executable)?;
        if !client.starts_with(&prefix) || exe.parent() != Some(client.as_path()) || !exe.is_file()
        {
            return Err(
                "Detected game path is unavailable or outside the selected environment.".into(),
            );
        }
        if products::installed(&client).as_deref() != products::product_for(&game.id) {
            return Err("Installed product metadata does not verify the selected edition. Refresh discovery before connecting.".into());
        }
        let parent = crate::addons::game_handoff(&game)
            .ok_or("The detected folder cannot be handed to CurseForge safely")?;
        let addon = client.join("Interface/AddOns");
        if fs::symlink_metadata(&addon).is_ok() && !fsutil::canonical(&addon)?.starts_with(&client)
        {
            return Err("The addon folder links outside the reviewed client. Existing setup retained; inspect it before connecting.".into());
        }
        let identity = fsutil::identity(&client)?;
        let exe_hash = fsutil::hash(&fsutil::read(&exe)?);
        targets.push(json!({"edition":game.id,"label":if game.id=="era"{"Classic"}else{"Retail"},"client":client,"folder":parent,"addon_folder":client.join("Interface/AddOns"),"identity":identity,"executable":exe,"exe_hash":exe_hash}));
    }
    if targets.is_empty() {
        return Err("No verified installed Retail or Classic Era folder is available. Finish its download and refresh.".into());
    }
    targets.sort_by_key(|x| x["edition"].as_str().unwrap_or("").to_string());
    Ok(targets)
}
fn addon_registration_review(ctx: &Context, req: &Json) -> Result<Json> {
    addon_operation_review(ctx, req, false)
}
fn addon_lifecycle_review(ctx: &Context, req: &Json) -> Result<Json> {
    addon_operation_review(ctx, req, true)
}
fn addon_operation_name(lifecycle: bool) -> &'static str {
    if lifecycle {
        "addon-lifecycle"
    } else {
        "addon-registration"
    }
}
fn addon_operation_kind(lifecycle: bool) -> &'static str {
    if lifecycle {
        "curseforge_lifecycle"
    } else {
        "curseforge_registration"
    }
}
fn addon_operation_review(ctx: &Context, req: &Json, lifecycle: bool) -> Result<Json> {
    let id = field(req, "id")?;
    let installation = ctx.install(id)?;
    let targets = if lifecycle {
        vec![]
    } else {
        registration_targets(ctx, id)?
    };
    if lifecycle && !installation.owned {
        return Err("Select an installer-owned installation for the lifecycle check.".into());
    }
    let desktop = gear_desktop(ctx)?;
    let plan = AddonRegistrationPlan {
        token: fsutil::unique(),
        installation: id.into(),
        purpose: addon_operation_kind(lifecycle).into(),
        source: crate::release::SOURCE.into(),
        root_identity: serde_json::to_value(fsutil::identity(&installation.root)?)
            .map_err(|e| e.to_string())?,
        scope: serde_json::to_value(&installation).map_err(|e| e.to_string())?,
        targets,
        desktop_hash: fsutil::hash(&fsutil::read(&desktop)?),
        desktop,
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs(),
    };
    fsutil::json(
        &ctx.data
            .join(format!("{}-review.json", addon_operation_name(lifecycle))),
        &plan,
    )?;
    if lifecycle {
        return Ok(
            json!({"id":plan.installation,"token":plan.token,"kind":plan.purpose,"message":"Check normal CurseForge Exit, a fresh reopened app and restoration of the observed edition. Screen/input approval is required. No folder registration or persistence is checked; no clipboard access is requested.","registration_status":"not_tested","persistence_status":"not_tested"}),
        );
    }
    Ok(
        json!({"token":plan.token,"id":plan.installation,"targets":plan.targets,"message":"Connect these detected games through CurseForge's normal interface. Keep existing registrations; stop on ambiguity. Temporary screen, input and clipboard approval is required; only freshly copied public folder locations are read. CurseForge will close and reopen to verify saved paths. Only the reviewed folder is pasted; freshly copied public locations are read to verify it. Clipboard text may be replaced."}),
    )
}
fn valid_registration_plan(ctx: &Context, id: &str, token: &str) -> Result<AddonRegistrationPlan> {
    valid_addon_operation_plan(ctx, id, token, false)
}
fn valid_addon_operation_plan(
    ctx: &Context,
    id: &str,
    token: &str,
    lifecycle: bool,
) -> Result<AddonRegistrationPlan> {
    let plan: AddonRegistrationPlan = fsutil::load(
        &ctx.data
            .join(format!("{}-review.json", addon_operation_name(lifecycle))),
    )?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    if plan.source != crate::release::SOURCE
        || plan.purpose != addon_operation_kind(lifecycle)
        || plan.installation != id
        || plan.token != token
        || now < plan.created
        || now - plan.created > 900
    {
        return Err("CurseForge connection review expired or changed. Review the intended installation again.".into());
    }
    let installation = ctx.install(id)?;
    if lifecycle
        && (!installation.owned
            || serde_json::to_value(&installation).map_err(|e| e.to_string())? != plan.scope
            || serde_json::to_value(fsutil::identity(&installation.root)?)
                .map_err(|e| e.to_string())?
                != plan.root_identity
            || !plan.targets.is_empty())
    {
        return Err(
            "The reviewed lifecycle installation changed. Review again; no app action sent.".into(),
        );
    }
    let prefix = fsutil::canonical(&installation.prefix)?;
    if gear_desktop_path(
        &ctx.home,
        plan.desktop
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid managed desktop entry")?,
    )? != plan.desktop
        || fsutil::hash(&fsutil::read(&plan.desktop)?) != plan.desktop_hash
    {
        return Err(
            "Managed CurseForge launch route changed. Review again; no registration changed."
                .into(),
        );
    }
    for t in &plan.targets {
        let client = PathBuf::from(t["client"].as_str().ok_or("Missing reviewed client")?);
        let exe = PathBuf::from(
            t["executable"]
                .as_str()
                .ok_or("Missing reviewed executable")?,
        );
        let addon = client.join("Interface/AddOns");
        if fs::symlink_metadata(&addon).is_ok() && !fsutil::canonical(&addon)?.starts_with(&client)
        {
            return Err(
                "The reviewed addon folder changed or links outside this client. Review again."
                    .into(),
            );
        }
        if fsutil::canonical(&client)? != client
            || !client.starts_with(&prefix)
            || fsutil::canonical(&exe)?.parent() != Some(client.as_path())
            || products::installed(&client).as_deref()
                != t["edition"].as_str().and_then(products::product_for)
            || serde_json::to_value(fsutil::identity(&client)?).map_err(|e| e.to_string())?
                != t["identity"]
            || fsutil::hash(&fsutil::read(&exe)?) != t["exe_hash"]
        {
            return Err("A reviewed game folder changed. Refresh and review again.".into());
        }
    }
    Ok(plan)
}
fn valid_registration_job(ctx: &Context, req: &Json) -> Result<()> {
    valid_addon_operation_job(ctx, req, false)
}
fn valid_addon_operation_job(ctx: &Context, req: &Json, lifecycle: bool) -> Result<()> {
    let plan = valid_addon_operation_plan(ctx, field(req, "id")?, field(req, "token")?, lifecycle)?;
    if req["targets"] != serde_json::to_value(&plan.targets).map_err(|e| e.to_string())?
        || req["desktop"] != serde_json::to_value(&plan.desktop).map_err(|e| e.to_string())?
    {
        return Err(
            "Registration job does not match the exact reviewed targets and managed route.".into(),
        );
    }
    Ok(())
}
pub fn validate_addon_lifecycle(req: Json) -> Result<()> {
    valid_addon_operation_job(&Context::open()?, &req, true)
}
pub fn validate_addon_registration(req: Json) -> Result<()> {
    valid_registration_job(&Context::open()?, &req)
}

fn addon_registration_apply(ctx: &Context, req: &Json) -> Result<Json> {
    addon_operation_apply(ctx, req, false)
}
fn addon_lifecycle_apply(ctx: &Context, req: &Json) -> Result<Json> {
    addon_operation_apply(ctx, req, true)
}
fn addon_operation_apply(ctx: &Context, req: &Json, lifecycle: bool) -> Result<Json> {
    if req["confirm"] != true {
        return Err(if lifecycle {
            "Confirm the separate CurseForge lifecycle check before starting."
        } else {
            "Confirm automatic CurseForge connection before starting."
        }
        .into());
    }
    let plan = valid_addon_operation_plan(ctx, field(req, "id")?, field(req, "token")?, lifecycle)?;
    if !lifecycle && registration_targets(ctx, &plan.installation)? != plan.targets {
        return Err("Detected editions changed since review. Review again.".into());
    }
    let attempt = ctx.data.join("addon-lifecycle-attempt.json");
    if lifecycle && attempt.exists() && fsutil::load::<Json>(&attempt)?["token"] == plan.token {
        return Err("This lifecycle attempt is already consumed. Inspect its result before explicitly reviewing a new attempt; Exit was not repeated.".into());
    }
    let pause = ctx.data.join("addon-registration-private-pause");
    if pause.exists() {
        if req["auth_complete"] != true {
            return Err("Private authentication pause is active. Finish privately, then explicitly confirm completion before retrying.".into());
        }
        fs::remove_file(pause).map_err(|e| e.to_string())?;
    }
    crate::cancelled()?;
    let name = addon_operation_name(lifecycle);
    let job = ctx.data.join(format!("{name}-job.json"));
    let cancel = ctx.data.join(format!("{name}-cancel"));
    if lifecycle {
        fsutil::json(
            &attempt,
            &json!({"token":plan.token,"source":crate::release::SOURCE}),
        )?;
    }
    if cancel.exists() {
        fs::remove_file(&cancel).map_err(|e| e.to_string())?;
    }
    fsutil::json(
        &job,
        &json!({"schema":1,"kind":addon_operation_kind(lifecycle),"parent_pid":std::process::id(),"installation":plan.installation,"token":plan.token,"source":crate::release::SOURCE,"targets":plan.targets,"desktop":plan.desktop,"cancel_file":cancel,"permission_file":ctx.data.join("controller-permission")}),
    )?;
    let mut child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg("--controller-driver")
        .arg(&job)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let output = child
        .stdout
        .take()
        .ok_or("Registration driver output unavailable")?;
    let reader = thread::spawn(move || {
        use std::io::Read;
        let mut b = String::new();
        output.take(65536).read_to_string(&mut b).map(|_| b)
    });
    let started = std::time::Instant::now();
    let mut stopped = None;
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if crate::cancelled().is_err() || started.elapsed() > Duration::from_secs(600) {
            if stopped.is_none() {
                fsutil::atomic(&cancel, b"cancel")?;
                stopped = Some(std::time::Instant::now());
            }
            if stopped.is_some_and(|t| t.elapsed() > Duration::from_secs(12)) {
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGTERM);
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    };
    let text = reader
        .join()
        .map_err(|_| "Registration output failed")?
        .map_err(|e| e.to_string())?;
    let value: Json = serde_json::from_str(&text).map_err(|_| {
        "Connection interrupted. Its checkpoint remains; inspect CurseForge before retrying."
    })?;
    fsutil::json(&ctx.data.join(format!("{name}-result.json")), &value)?;
    if lifecycle {
        valid_addon_operation_plan(ctx, &plan.installation, &plan.token, true)?;
        let result = checked_lifecycle_result(&value, &plan)?;
        if status.success() && result["lifecycle_ok"] == true {
            fs::remove_file(&job).map_err(|e| e.to_string())?;
            return Ok(result);
        }
        return Err(value["error"].as_str().unwrap_or("Lifecycle check interrupted; inspect its separate stage results before a new review. No registration verified.").into());
    }
    if status.success() && value["ok"] == true {
        valid_registration_plan(ctx, &plan.installation, &plan.token)?;
        fs::remove_file(&job).map_err(|e| e.to_string())?;
        fs::remove_file(ctx.data.join("addon-registration-review.json"))
            .map_err(|e| e.to_string())?;
        Ok(
            json!({"operation_ok":true,"message":"CurseForge paths independently verified and retained after reopening; original selection restored where present.","results":value["results"],"initial_selection":value["initial_selection"],"initial_selection_observed":value["initial_selection_observed"],"initial_selection_required":value["initial_selection_required"],"initial_selection_restored":value["initial_selection_restored"],"normal_exit_verified":value["normal_exit_verified"],"reopened_verified":value["reopened_verified"],"exit_delivery":value["exit_delivery"]}),
        )
    } else {
        Err(value["error"].as_str().unwrap_or("CurseForge connection stopped safely; inspect current registrations before retrying.").into())
    }
}

fn addon_registration_result(ctx: &Context, req: &Json) -> Result<Json> {
    let plan = valid_registration_plan(ctx, field(req, "id")?, field(req, "token")?)?;
    let value: Json = fsutil::load(&ctx.data.join("addon-registration-result.json"))?;
    if value["installation"] != plan.installation
        || value["review_token"] != plan.token
        || value["source"] != crate::release::SOURCE
    {
        return Err("No terminal evidence matches this reviewed registration attempt.".into());
    }
    Ok(
        json!({"operation_ok":value["ok"],"phase":value["diagnostic"]["phase"],"results":value["results"],"initial_selection":value["initial_selection"],"initial_selection_observed":value["initial_selection_observed"],"initial_selection_required":value["initial_selection_required"],"initial_selection_restored":value["initial_selection_restored"],"normal_exit_verified":value["normal_exit_verified"],"reopened_verified":value["reopened_verified"],"exit_delivery":value["exit_delivery"],"message":"Partial connection evidence. Path verification, reopen persistence and original-selection restoration are separate checks; an incomplete operation is not a pass."}),
    )
}

// Lifecycle evidence is deliberately a separate result namespace. Never feed it
// to registration success/persistence. Query remains possible after partial exit.
fn checked_lifecycle_result(value: &Json, plan: &AddonRegistrationPlan) -> Result<Json> {
    if value["installation"] != plan.installation
        || value["review_token"] != plan.token
        || value["source"] != crate::release::SOURCE
        || value["kind"] != "curseforge_lifecycle"
        || value["registration_status"] != "not_tested"
        || value["persistence_status"] != "not_tested"
        || value["results"] != json!([])
    {
        return Err("No lifecycle-only evidence matches this reviewed attempt.".into());
    }
    for flag in [
        "ok",
        "normal_exit_verified",
        "reopened_verified",
        "initial_selection_observed",
        "initial_selection_required",
        "initial_selection_restored",
    ] {
        if !value[flag].is_boolean() {
            return Err("Incomplete lifecycle stage evidence.".into());
        }
    }
    let exit = value["normal_exit_verified"] == true;
    let reopened = value["reopened_verified"] == true;
    let known = value["initial_selection_observed"] == true;
    let required = value["initial_selection_required"] == true;
    let restored = value["initial_selection_restored"] == true;
    if required
        && (!known
            || !["Classic", "Classic Era", "Retail"]
                .contains(&value["initial_selection"].as_str().unwrap_or("")))
    {
        return Err("Required edition restoration lacks an observed initial public label.".into());
    }
    let delivery = value["exit_delivery"]
        .as_str()
        .ok_or("Missing Exit delivery evidence")?;
    if ![
        "not_requested",
        "requested",
        "acknowledged",
        "outcome_unknown",
    ]
    .contains(&delivery)
        || (exit && !["acknowledged", "outcome_unknown"].contains(&delivery))
        || (reopened && !exit)
        || (restored && (!reopened || !known || !required))
    {
        return Err("Inconsistent lifecycle stages; no success inferred.".into());
    }
    if reopened {
        let a = &value["before_identity"];
        let b = &value["after_identity"];
        let valid = |i: &Json| {
            i["pid"].as_u64().is_some_and(|p| p > 0)
                && i["uid"].as_u64() == Some(unsafe { libc::geteuid() } as u64)
                && i["start"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
                && i["executable_sha256"]
                    .as_str()
                    .is_some_and(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        };
        if !valid(a)
            || !valid(b)
            || (a["pid"] == b["pid"] && a["start"] == b["start"])
            || a["executable_sha256"] != b["executable_sha256"]
        {
            return Err("A fresh same-owned-app identity was not verified after reopening.".into());
        }
    }
    let complete = exit && reopened && known && (!required || restored);
    if value["ok"] == true && !complete {
        return Err("Lifecycle success lacks independent stage evidence.".into());
    }
    Ok(
        json!({"lifecycle_ok":value["ok"],"normal_exit_verified":exit,"reopened_verified":reopened,
        "initial_selection":value["initial_selection"],"initial_selection_observed":known,"initial_selection_required":required,"initial_selection_restored":restored,
        "exit_delivery":delivery,"before_identity":value["before_identity"],"after_identity":value["after_identity"],
        "registration_status":"not_tested","persistence_status":"not_tested","results":[],
        "phase":value["diagnostic"]["phase"],"message":"Lifecycle-only evidence. Exit, reopening and edition restoration are separate; no registration or path persistence was verified."}),
    )
}
fn addon_lifecycle_result(ctx: &Context, req: &Json) -> Result<Json> {
    let plan = valid_addon_operation_plan(ctx, field(req, "id")?, field(req, "token")?, true)?;
    let value = fsutil::load(&ctx.data.join("addon-lifecycle-result.json"))?;
    checked_lifecycle_result(&value, &plan)
}

// Cancellation is stop-only and may run while the parent's operation lock is held.
// It never clears authentication or reports completion of an app stage.
fn addon_lifecycle_cancel(ctx: &Context, req: &Json) -> Result<Json> {
    let job: Json = fsutil::load(&ctx.data.join("addon-lifecycle-job.json"))?;
    if job["kind"] != "curseforge_lifecycle"
        || job["installation"] != field(req, "id")?
        || job["token"] != field(req, "token")?
        || job["source"] != crate::release::SOURCE
    {
        return Err("No current lifecycle job matches this cancellation request.".into());
    }
    fsutil::atomic(&ctx.data.join("addon-lifecycle-cancel"), b"cancel")?;
    Ok(
        json!({"cancel_requested":true,"message":"Cancellation requested. Inspect the separate lifecycle result; no Exit, reopen or restoration success is implied."}),
    )
}
