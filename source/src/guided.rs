// Guided operations. Steam is never signalled or force-terminated.
fn no_running_games(ctx: &Context, operation: &str) -> Result<()> {
    discovery::require_games_closed_for(operation)?;
    let mut roots = vec![];
    for steam in discovery::steam_roots(&ctx.home) {
        for lib in discovery::steam_info(&steam)?.libraries {
            roots.push(lib.join("steamapps/common"));
        }
    }
    for entry in fs::read_dir("/proc").map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        for name in ["exe", "cwd"] {
            if let Ok(path) = fs::read_link(entry.path().join(name)) {
                for root in &roots {
                    if let Ok(relative) = path.strip_prefix(root) {
                        let first = relative
                            .components()
                            .next()
                            .map(|c| c.as_os_str().to_string_lossy())
                            .unwrap_or_default();
                        if !first.starts_with("Proton") && !first.starts_with("SteamLinuxRuntime") {
                            return Err("A game or application is using a Steam library. Close it yourself before setup closes Steam.".into());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn reopen_steam(ctx: &Context) -> Result<Json> {
    let mut child = host_command("xdg-open")
        .arg("steam://open/main")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            "Steam could not be reopened. Open Steam yourself; saved setup is preserved."
        })?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if !status.success() => return Err(
                "Steam did not accept the reopen request. Open Steam yourself; setup is preserved."
                    .into(),
            ),
            Some(_) => break,
            None if std::time::Instant::now() >= deadline => {
                return Err(
                    "Steam reopen is waiting for a desktop prompt. Open Steam yourself if needed."
                        .into(),
                )
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    }
    let ready_deadline=std::time::Instant::now()+Duration::from_secs(8);
    loop {
        if discovery::steam_running()? {break;}
        if std::time::Instant::now()>=ready_deadline {
            return Err("Steam reopen was requested but its processes have not appeared. Saved setup and the reopen checkpoint are kept; setup will retry when idle.".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = fs::remove_file(ctx.data.join("steam-restart.json"));
    Ok(
        json!({"message":"Asked Steam to reopen. If its window does not appear, open Steam yourself."}),
    )
}
fn exit_steam_if_running(flatpak: bool) -> Result<()> {
    if discovery::steam_running()? {
        let mut command = if flatpak
        {
            let mut c = host_command("flatpak");
            c.args(["run", "com.valvesoftware.Steam", "-shutdown"]);
            c
        } else {
            let mut c = host_command("steam");
            c.arg("-shutdown");
            c
        };
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| {
                "Could not request Steam exit. Choose Steam → Exit, then retry. Steam configuration changes were not committed. Downloaded installer and bootstrap files may remain in the reviewed copy."
            })?;
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            // Reap our shutdown helper before process detection. An exited child
            // remains visible as steam until its parent collects its exit status.
            child.try_wait().map_err(|e| format!("Could not collect the Steam shutdown helper status: {e}. Steam configuration changes were not committed; downloaded installer/bootstrap files may remain. Close Steam normally, then retry."))?;
            if !discovery::steam_running()? { break; }
            if std::time::Instant::now() >= deadline || crate::cancelled().is_err() {
                let _ = child.try_wait();
                return Err("Steam shutdown did not complete or was cancelled. Steam configuration changes were not committed. Downloaded installer and bootstrap files may remain in the reviewed copy. Close any game or Steam dialog yourself, then retry. Steam was not force-closed.".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = child.try_wait();
    }
    Ok(())
}
fn complete_steam_interval(result: Result<Json>, restore: impl FnOnce() -> Result<Json>) -> Result<Json> {
    let restart=restore();
    match result {
        Ok(mut value) => {
            value["steam_message"] = json!(match restart {
                Ok(v) => v["message"].as_str().unwrap_or("").to_string(),
                Err(e) => e,
            });
            Ok(value)
        }
        Err(error) => Err(match restart {
            Ok(_) => format!("{error} Steam reopen requested."),
            Err(reopen_error) => format!("{error} Steam restore failed: {reopen_error} Open Steam yourself if automatic retries cannot restore desktop controls; saved checkpoints remain."),
        }),
    }
}
fn prepare_reinstall_download(ctx: &Context, job: &Cleanup) -> Result<()> {
    if !job.plan.journal.done {
        if let Some(i)=&job.plan.replacement {
            validate_cleanup(ctx,job)?;
            validate_proton(&discovery::steam_info(&i.steam)?,&i.proton)?;
            download(ctx,i)?;
        }
    }
    Ok(())
}
fn guided_apply(ctx: &Context, req: &Json) -> Result<Json> {
    let mut _plan = load_plan(ctx, req)?;
    if req["manage_steam"] != true {
        return Err("Confirm the Steam shutdown/restart summary first.".into());
    }
    if !_plan.remove.is_empty() && req["confirmation"] != "REMOVE THIS COPY" {
        return Err("Confirm the listed removal first; Steam was not closed.".into());
    }
    no_running_games(ctx, discovery::operation_description(&_plan.kind))?;
    transaction::rebase(&mut _plan.journal)?;
    if _plan.download {
        validate_proton(
            &discovery::steam_info(&_plan.installation.steam)?,
            &_plan.installation.proton,
        )?;
        download(ctx, &_plan.installation)?;
    }
    prepare_reinstall_download(ctx,&Cleanup{plan:_plan.clone(),settings_saved:false})?;
    // Downloads can take time; a game may have started since the initial review.
    no_running_games(ctx, discovery::operation_description(&_plan.kind))?;
    fsutil::json(&ctx.data.join("steam-restart.json"), &json!({"requested":true}))?;
    let flatpak=_plan.installation.steam.starts_with(ctx.home.join(".var/app/com.valvesoftware.Steam"));
    let result=(|| {exit_steam_if_running(flatpak)?; apply(ctx,req)})();
    complete_steam_interval(result, || reopen_steam(ctx))
}

fn proton_status(ctx:&Context,req:&Json)->Result<Json>{
    let root=fsutil::canonical(Path::new(field(req,"steam")?))?;
    if !discovery::steam_roots(&ctx.home).contains(&root){return Err("Choose a detected Steam installation before obtaining Proton.".into());}
    let requested=req["proton"].as_str().unwrap_or("proton_11");
    let info=discovery::steam_info(&root)?;
    let ready=info.protons.iter().any(|p|p.name==requested || (requested=="proton_110" && p.name=="proton_11"));
    let missing_runtime = if ["proton_11", "proton_110"].contains(&requested) && !ready {
        discovery::missing_valve11_runtime(&info)
    } else { None };
    let message = if ready {
        "Proton and its required runtime are installed and ready. Your setup choices are unchanged.".to_string()
    } else if let Some(app) = &missing_runtime {
        format!("Proton 11 is installed. Its required Steam runtime (app {app}) is missing or incomplete. Keep Proton installed. Choose Get required Steam runtime, confirm Install in Steam, and let it finish; then Check again. Your setup choices are kept.")
    } else {
        "Proton 11 or its required files are not ready. In Steam, confirm Install if asked and let the download finish, then Check again. Your setup choices are kept.".to_string()
    };
    Ok(json!({"steam":root,"proton":requested,"ready":ready,"protons":info.protons,
        "missing_runtime":missing_runtime,"message":message}))
}
fn proton_install(ctx: &Context,req:&Json) -> Result<Json> {
    let status=proton_status(ctx,req)?;
    if status["ready"]==true {return Ok(status);}
    let requested=req["proton"].as_str().unwrap_or("proton_11");
    if !["proton_11","proton_110"].contains(&requested){return Err("Automatic Steam handoff is available for the configured Valve Proton 11 only. Open Steam Library → Tools for another version; no alternative distribution was installed.".into());}
    let app = proton_install_target(&status, req["install_runtime"] == true)?;
    let root=Path::new(status["steam"].as_str().ok_or("Steam location unavailable")?);
    let mut command=if root.starts_with(ctx.home.join(".var/app/com.valvesoftware.Steam")) {
        let mut c=host_command("flatpak");c.args(["run","com.valvesoftware.Steam"]);c
    } else {host_command("steam")};
    let uri=format!("steam://install/{app}");
    command.arg(uri);
    send_proton_request(command)?;
    Ok(json!({"steam":root,"ready":false,"requested":true,"message":"Installation requested in Steam—not yet complete. If Steam was closed, let it open and sign in privately if needed. Confirm Install in Steam; if no install window appears, choose Get Proton in Steam again. Your choices are kept while we check completion."}))
}
fn proton_install_target(status: &Json, install_runtime: bool) -> Result<&str> {
    if install_runtime {
        return status["missing_runtime"].as_str().filter(|app| *app == "4183110")
            .ok_or_else(|| "No supported missing Proton 11 runtime was detected. Check again; for another runtime use Steam Library → Tools.".into());
    }
    if status["missing_runtime"].is_string() {
        return Err("Proton 11 is already installed. Choose Get required Steam runtime instead; no duplicate Proton request was sent.".into());
    }
    Ok(discovery::VALVE11_APP)
}
fn send_proton_request(mut command:Command)->Result<()> {
    let mut child=command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(|_|"Steam could not be opened. Open Steam yourself, sign in if needed, and retry Get Proton in Steam. Your setup choices are preserved.")?;
    let deadline=std::time::Instant::now()+Duration::from_secs(5);
    loop {
        match child.try_wait().map_err(|e|e.to_string())? {
            Some(s) if !s.success()=>return Err("Steam rejected the install request. Open Steam, sign in if needed and retry. Setup choices are preserved.".into()),
            Some(_)=>break,
            None if std::time::Instant::now()<deadline=>std::thread::sleep(Duration::from_millis(50)),
            None=>break,
        }
    }
    Ok(())
}

fn native_gear_available() -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p).any(|dir| {
                fs::metadata(dir.join("gearlever"))
                    .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}
// Only Gear Lever/Flatpak helpers use this deadline; never Steam or a game.
fn gear_command(ctx: &Context, args: &[&std::ffi::OsStr], seconds: u64) -> Result<Vec<u8>> {
    use std::os::unix::fs::OpenOptionsExt;
    let output = ctx.data.join(format!(".gear-output-{}", fsutil::unique()));
    let file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&output)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let native = native_gear_available()
            && args
                .first()
                .is_some_and(|a| *a == std::ffi::OsStr::new("run"));
        let mut command = host_command(if native { "gearlever" } else { "flatpak" });
        let mut child = command
            .args(if native { &args[2..] } else { args })
            .stdin(Stdio::null())
            .stdout(file)
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Flatpak is unavailable; install Gear Lever yourself and retry.")?;
        let deadline = std::time::Instant::now() + Duration::from_secs(seconds);
        loop {
            if crate::cancelled().is_err()
                || std::time::Instant::now() >= deadline
                || fs::metadata(&output)
                    .map(|m| m.len() > 2 * 1024 * 1024)
                    .unwrap_or(false)
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Gear Lever needs attention or was cancelled. Open it yourself, then retry; no Steam tile changed.".into());
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err("Gear Lever requires an interactive step or newer CLI. Open it and retry; no Steam tile changed.".into());
                }
                return fs::read(&output).map_err(|e| e.to_string());
            }
            thread::sleep(Duration::from_millis(50));
        }
    })();
    let _ = fs::remove_file(output);
    result
}
fn gear_list(ctx: &Context) -> Result<Json> {
    let args: Vec<_> = ["run", "it.mijorus.gearlever", "--list-installed", "--json"]
        .iter()
        .map(std::ffi::OsStr::new)
        .collect();
    let output = gear_command(ctx, &args, 15)?;
    let value:Json=serde_json::from_slice(&output).map_err(|_|"This Gear Lever version lacks machine-readable app discovery. Update Gear Lever or finish integration manually; no duplicate tile was added.")?;
    if value["schema_version"] != 1 || !value["installed"].is_array() {
        return Err("Unsupported Gear Lever app-list format; no tile changed.".into());
    }
    Ok(value)
}
fn gear_is_curseforge(row: &Json) -> bool {
    let name = row["name"].as_str().unwrap_or("").to_ascii_lowercase();
    name == "curseforge" || (name.starts_with("curseforge (") && name.ends_with(')'))
}
fn gear_desktop(ctx: &Context) -> Result<PathBuf> {
    let list = gear_list(ctx)?;
    let apps: Vec<_> = list["installed"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| gear_is_curseforge(a))
        .collect();
    if apps.len() != 1 {
        return Err("Gear Lever must list exactly one CurseForge AppImage. Open Gear Lever to choose your existing copy or integrate the official download, then retry.".into());
    }
    let id = apps[0]["desktop_id"]
        .as_str()
        .ok_or("Gear Lever did not provide a desktop ID")?;
    // Discovery of a registration alone is insufficient if its payload disappeared.
    crate::addons::validate_appimage_input(apps[0]["path"].as_str().unwrap_or(""))?;
    gear_desktop_path(&ctx.home, id)
}
// Permit aliases of the home directory, never links below it or linked desktop files.
fn gear_desktop_path(home: &Path, id: &str) -> Result<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    if !id.ends_with(".desktop") || id == ".desktop" || id.contains('/') || id.contains('\\') || id.chars().any(char::is_control) {
        return Err("Invalid Gear Lever desktop ID".into());
    }
    let home = fsutil::canonical(home)?;
    let mut directory = home.clone();
    for component in [".local", "share", "applications"] {
        directory.push(component);
        let metadata = fs::symlink_metadata(&directory).map_err(|_| "Gear Lever desktop directory is missing")?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("Gear Lever desktop directory is linked or unsafe; inspect it before continuing.".into());
        }
    }
    let path = directory.join(id);
    let metadata = fs::symlink_metadata(&path).map_err(|_| "Gear Lever desktop entry is missing; inspect it before continuing.")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1
        || metadata.uid() != fs::metadata(&home).map_err(|e| e.to_string())?.uid()
        || fsutil::canonical(&path)? != path {
        return Err("Gear Lever desktop entry is linked or unsafe; inspect it before continuing.".into());
    }
    Ok(path)
}

fn prepare_gear(ctx: &Context, req: &Json) -> Result<Json> {
    if let Some(id) = req.get("id").and_then(Json::as_str) { ctx.install(id)?; }
    if req["confirm"] != true {
        return Err(
            "Confirm Gear Lever installation and official AppImage integration first.".into(),
        );
    }
    if let Ok(path) = gear_desktop(ctx) {
        return Ok(json!({"desktop":path,"message":"Reusing the existing Gear Lever-managed CurseForge. Updates remain managed in Gear Lever."}));
    }
    let selected=req["path"].as_str().unwrap_or("").trim();
    // A deliberately supplied invalid file still refuses before installing tools.
    let selected_app=if selected.is_empty(){None}else{Some(crate::addons::validate_appimage_input(selected)?)};
    let present = if native_gear_available() {
        true
    } else {
        host_command("flatpak")
            .args(["info", "it.mijorus.gearlever"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| "Install Gear Lever from your software store, then retry.")?
            .success()
    };
    if present {
        let known=gear_list(ctx)?;
        if known["installed"].as_array().unwrap().iter().any(gear_is_curseforge){return Err("An existing CurseForge registration needs attention in Gear Lever. No download or second copy was made. Repair/select that copy in Gear Lever, then retry.".into());}
    }
    let app=match selected_app {Some(path)=>path,None=>crate::addons::download(&ctx.data,"curseforge",None)?};
    crate::cancelled()?;
    if !present {
        let args: Vec<_> = [
            "install",
            "--user",
            "--assumeyes",
            "--noninteractive",
            "https://flathub.org/repo/appstream/it.mijorus.gearlever.flatpakref",
        ]
        .iter()
        .map(std::ffi::OsStr::new)
        .collect();
        gear_command(ctx,&args,600).map_err(|_|"Install Gear Lever from Discover or your Bazzite software store, then retry. Do not install the store-packaged CurseForge app.")?;
    }
    let known = gear_list(ctx)?;
    if known["installed"]
        .as_array()
        .unwrap()
        .iter()
        .any(gear_is_curseforge)
    {
        return Err("An existing CurseForge entry needs attention in Gear Lever. No second copy was integrated.".into());
    }
    let args = [
        std::ffi::OsStr::new("run"),
        std::ffi::OsStr::new("it.mijorus.gearlever"),
        std::ffi::OsStr::new("--integrate"),
        app.as_os_str(),
        std::ffi::OsStr::new("--yes"),
    ];
    gear_command(ctx, &args, 90)?;
    Ok(
        json!({"desktop":gear_desktop(ctx)?,"message":"AppImage integrated. Gear Lever can check supported update sources; updates may require your action."}),
    )
}

fn owned_tree(root: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let dev = fs::symlink_metadata(root).map_err(|e| e.to_string())?.dev();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let m = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if m.file_type().is_symlink() {
            continue;
        } // deletion unlinks; never follow outside targets
        if m.dev() != dev || (!m.is_dir() && (!m.is_file() || m.nlink() > 1)) {
            return Err("Mounted, shared or special files block complete removal. Use scoped removal instead.".into());
        }
        if m.is_dir() {
            for e in fs::read_dir(path).map_err(|e| e.to_string())? {
                stack.push(e.map_err(|e| e.to_string())?.path());
            }
        }
    }
    Ok(())
}
fn exclusive_owned_root(ctx: &Context, i: &Installation) -> Result<()> {
    if !i.owned
        || i.root.file_name().and_then(|x| x.to_str())
            != Some(format!("WoW-{}", &i.id[..12]).as_str())
        || i.root.starts_with(&ctx.data)
        || ctx.data.starts_with(&i.root)
    {
        return Err(
            "Only an installer-created dedicated environment can be completely removed.".into(),
        );
    }
    let marker: Installation = fsutil::load(&i.root.join(".wow-rust-owner.json"))?;
    if marker.id != i.id || marker.prefix != i.prefix || marker.root != i.root {
        return Err("Ownership changed; complete removal blocked.".into());
    }
    for e in fs::read_dir(&i.root).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        if ![
            "compatdata",
            "games",
            "Battle.net-Setup.exe",
            ".wow-rust-owner.json",
        ]
        .contains(&e.file_name().to_string_lossy().as_ref())
        {
            return Err("Unrelated files in the installation root will be preserved. Uncheck complete removal to remove only game/launcher folders.".into());
        }
    }
    let games = i.root.join("games");
    if games.is_dir() {
        for e in fs::read_dir(games).map_err(|e| e.to_string())? {
            if e.map_err(|e| e.to_string())?.file_name() != "World of Warcraft" {
                return Err(
                    "Other game folders block complete removal; choose scoped removal.".into(),
                );
            }
        }
    }
    owned_references(ctx, i)?;
    owned_tree(&i.root)
}

fn owned_references(ctx: &Context, i: &Installation) -> Result<()> {
    if ctx.data.join("state.json").exists() {
        let state: State = fsutil::load(&ctx.data.join("state.json"))?;
        workflow::state(&state)?;
        if state.installations.iter().any(|other| {
            other.id != i.id
                && (other.root.starts_with(&i.root)
                    || i.root.starts_with(&other.root)
                    || other.prefix == i.prefix)
        }) {
            return Err(
                "Another saved installation now shares this root. Complete removal paused.".into(),
            );
        }
    }
    removal_references(ctx, i, true)
}

// Strict destructive-operation scope: no silently skipped root/profile errors.
fn removal_steam_roots(ctx: &Context, i: &Installation) -> Result<Vec<PathBuf>> {
    let mut roots = BTreeSet::from([i.steam.clone()]);
    for relative in [".steam/steam", ".local/share/Steam", ".steam/root", ".var/app/com.valvesoftware.Steam/.local/share/Steam"] {
        let path=ctx.home.join(relative);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let path=fsutil::canonical(&path).map_err(|_|"A Steam root cannot be inspected; removal paused.")?;
                match fs::metadata(path.join("steamapps")) {
                    Ok(m) if m.is_dir()=>{roots.insert(path);},
                    Ok(_)=>return Err("Invalid Steam library root; removal paused.".into()),
                    Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},
                    Err(_)=>return Err("A Steam root cannot be inspected; removal paused.".into()),
                }
            },
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},
            Err(_)=>return Err("A Steam root cannot be inspected; removal paused.".into()),
        }
    }
    Ok(roots.into_iter().collect())
}
fn removal_profiles(steam: &Path) -> Result<Vec<String>> {
    let entries=match fs::read_dir(steam.join("userdata")) {
        Ok(entries)=>entries,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(vec![]),
        Err(_)=>return Err("Steam profiles cannot be fully inspected; removal paused.".into()),
    };
    let mut users=vec![];
    for e in entries {
        let e=e.map_err(|_|"Steam profiles cannot be fully inspected; removal paused.")?;
        let name=e.file_name().to_string_lossy().to_string();
        if name!="0"&&!name.is_empty()&&name.bytes().all(|b|b.is_ascii_digit()) {
            if !fs::metadata(e.path()).map_err(|_|"Steam profile cannot be inspected; removal paused.")?.is_dir() {return Err("Invalid Steam profile; removal paused.".into());}
            users.push(name);
        }
    }
    Ok(users)
}
fn removal_references(ctx: &Context, i: &Installation, complete: bool) -> Result<()> {
    for steam_root in removal_steam_roots(ctx,i)? {
        for user in removal_profiles(&steam_root)? {
            let shortcuts=steam::read_shortcuts(&steam::shortcuts_path(&steam_root,&user)?)?;
            for (_,v) in vdf::object(&shortcuts,"shortcuts").unwrap() {
                if let Value::Object(e)=v {
                    if complete && steam_root==i.steam && user!=i.user
                        && vdf::number(e,"appid").is_some_and(|id|i.shortcuts.contains(&id)||i.bootstrap_id==id) {
                        return Err("Another profile shares a shortcut ID. Use scoped removal to retain its compatibility selection.".into());
                    }
                    let target=Path::new(vdf::string(e,"exe").unwrap_or("").trim_matches('"'));
                    let options=vdf::string(e,"LaunchOptions").unwrap_or("");
                    let resolved_target=target.canonicalize().unwrap_or_else(|_|target.to_path_buf());
                    let parsed_prefix=steam::compat_prefix(options).map(|p|p.canonicalize().unwrap_or(p));
                    let references=resolved_target.starts_with(&i.root)||resolved_target.starts_with(&i.prefix)
                        ||parsed_prefix.is_some_and(|p|p==i.prefix||p.starts_with(&i.root))
                        ||options.contains(i.root.to_str().unwrap_or("\0"))||options.contains(i.prefix.to_str().unwrap_or("\0"));
                    if references && (steam_root!=i.steam||user!=i.user||!steam::owned(e,&i.id)||steam::native_addon(e)) {
                        return Err("Another Steam root/profile or shared component references this installation. Removal paused; shared files and shortcuts are kept.".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn recover_pending(ctx: &mut Context) -> Result<Json> {
    discovery::require_stopped()?;
    if ctx.data.join("restore.json").exists(){let mut job:RestoreJob=fsutil::load(&ctx.data.join("restore.json"))?;run_restore(ctx,&mut job)?;}
    else if ctx.data.join("cleanup.json").exists(){let mut job:Cleanup=fsutil::load(&ctx.data.join("cleanup.json"))?;run_cleanup(ctx,&mut job)?;}
    else if ctx.data.join("transaction.json").exists(){transaction::recover(&ctx.data,true)?;}
    Ok(json!({"message":"Saved operation completed. Scan again to continue."}))
}
fn guided_existing(ctx:&mut Context,req:&Json)->Result<Json>{
    if req["manage_steam"]!=true{return Err("Confirm closing and reopening Steam first.".into());}
    let op=field(req,"operation")?.to_string();
    if !["recover","undo","abandon_cleanup"].contains(&op.as_str()){return Err("Choose a supported saved operation.".into());}
    if op=="undo" && ["transaction.json","cleanup.json","restore.json"].iter().any(|name|ctx.data.join(name).exists()) {
        return Err("Continue the saved operation before Undo; Steam was not closed.".into());
    }
    no_running_games(ctx,"continuing a saved operation")?;
    if op=="recover" && ctx.data.join("cleanup.json").exists(){let job:Cleanup=fsutil::load(&ctx.data.join("cleanup.json"))?;prepare_reinstall_download(ctx,&job)?;no_running_games(ctx,"continuing a saved operation")?;}
    let flatpak=ctx.state.installations.iter().find(|i|Some(&i.id)==ctx.state.current.as_ref()).is_some_and(|i|i.steam.starts_with(ctx.home.join(".var/app/com.valvesoftware.Steam")));
    fsutil::json(&ctx.data.join("steam-restart.json"),&json!({"requested":true}))?;
    let result=(||{exit_steam_if_running(flatpak)?;match op.as_str(){"recover"=>recover_pending(ctx),"undo"=>{transaction::undo(&ctx.data,true)?;Ok(json!({"message":"Previous configuration restored. Game files were not changed."}))},_=>abandon_cleanup(ctx)}})();
    complete_steam_interval(result,||reopen_steam(ctx))
}
