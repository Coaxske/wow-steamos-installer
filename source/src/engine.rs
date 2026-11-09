use crate::{
    catalog, discovery, fsutil,
    model::*,
    steam,
    transaction::{self, Change, Journal},
    vdf::{self, Value},
    Result,
};
use crate::{
    products,
    workflow::{self, Stage},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
struct Context {
    home: PathBuf,
    data: PathBuf,
    state: State,
    state_error: bool,
}
impl Context {
    fn open() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is not set")?;
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"))
            .join("wow-steamos-rust");
        fsutil::private_dir(&data)?;
        let loaded = if data.join("state.json").exists() {
            fsutil::load::<State>(&data.join("state.json")).and_then(|s| {
                if s.schema != 1 {
                    return Err("Unsupported saved schema".into());
                }
                workflow::state(&s)?;
                Ok(s)
            })
        } else {
            Ok(State {
                schema: 1,
                ..State::default()
            })
        };
        let state_error = loaded.is_err();
        let state = loaded.unwrap_or_else(|_| State {
            schema: 1,
            ..State::default()
        });
        Ok(Self {
            home,
            data,
            state,
            state_error,
        })
    }
    fn save(&self) -> Result<()> {
        workflow::state(&self.state)?;
        fsutil::json(&self.data.join("state.json"), &self.state)
    }
    fn persist(&mut self, state: State) -> Result<()> {
        workflow::state(&state)?;
        fsutil::json(&self.data.join("state.json"), &state)?;
        self.state = state;
        Ok(())
    }
    fn install(&self, id: &str) -> Result<Installation> {
        let installation = self
            .state
            .installations
            .iter()
            .find(|x| x.id == id)
            .cloned()
            .ok_or_else(|| "Select an installation first".to_string())?;
        if !installation.root.is_dir() {
            return Err("The selected installation is no longer available. Scan again and explicitly choose an installation; nothing was switched to another copy.".into());
        }
        Ok(installation)
    }
}
fn field<'a>(req: &'a Json, key: &str) -> Result<&'a str> {
    req[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing {key}"))
}
fn strings(req: &Json, key: &str) -> Vec<String> {
    req[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
fn state_info(ctx: &Context) -> Json {
    json!({"unavailable_installations":ctx.state.installations.iter().filter(|i|!i.root.is_dir()).map(|i|i.id.clone()).collect::<Vec<_>>(),"state_error":ctx.state_error,"version":VERSION,"state":ctx.state,"catalog":catalog::EDITIONS,"pending_transaction":ctx.data.join("transaction.json").exists(),"pending_cleanup":ctx.data.join("cleanup.json").exists(),"pending_restore":ctx.data.join("restore.json").exists(),"home":ctx.home,"data":ctx.data,"pending_controller":ctx.data.join("controller-job.json").exists(),"steam_restart_pending":ctx.data.join("steam-restart.json").exists()})
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Plan {
    token: String,
    kind: String,
    summary: Vec<String>,
    installation: Installation,
    journal: Journal,
    remove: Vec<Removal>,
    keep_settings: bool,
    download: bool,
    #[serde(default)]
    replacement: Option<Installation>,
    #[serde(default)]
    client_removals: Vec<ClientRemoval>,
    #[serde(default)]
    shared_migration: bool,
    #[serde(default)]
    installed_editions: Vec<String>,
}
include!("obsolete_clients.rs");
include!("shared_tile.rs");
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Removal {
    path: PathBuf,
    identity: fsutil::Identity,
    quarantine: PathBuf,
    removed: bool,
}
fn save_plan(ctx: &Context, plan: &Plan) -> Result<Json> {
    workflow::installation(&plan.installation)?;
    fsutil::json(&ctx.data.join("review.json"), plan)?;
    let proposed = plan.replacement.as_ref().unwrap_or(&plan.installation);
    let displayed_editions: Vec<String> = if plan.kind == "repair" {
        plan.installed_editions.clone()
    } else {
        proposed.editions.clone()
    };
    Ok(
        json!({"token":plan.token,"kind":plan.kind,"summary":plan.summary,"removal":plan.remove.iter().map(|x|x.path.clone()).collect::<Vec<_>>(),"destructive":!plan.remove.is_empty(),"download":plan.download,"installation":{"id":plan.installation.id,"root":plan.installation.root,"proton":plan.installation.proton},"shortcut_removals":plan.client_removals,"choices":{"editions":displayed_editions,"requested_editions":proposed.editions,"addon_manager":plan.replacement.as_ref().unwrap_or(&plan.installation).addon_manager,"launcher_tile":plan.replacement.as_ref().unwrap_or(&plan.installation).launcher_tile,"launcher_exit_choice":proposed.launcher_exit_choice}}),
    )
}
fn load_plan(ctx: &Context, req: &Json) -> Result<Plan> {
    let p: Plan = fsutil::load(&ctx.data.join("review.json"))?;
    workflow::installation(&p.installation)?;
    if field(req, "token")? != p.token {
        return Err("Review expired. Prepare the plan again.".into());
    }
    if !["setup", "adopt"].contains(&p.kind.as_str()) {
        let current = ctx.install(&p.installation.id)?;
        if current.root != p.installation.root
            || current.prefix != p.installation.prefix
            || current.steam != p.installation.steam
            || current.user != p.installation.user
        {
            return Err("The reviewed installation target changed. Scan and explicitly review it again; no other copy was selected.".into());
        }
        if let Some(id) = req.get("id").and_then(Json::as_str) {
            if id != p.installation.id {
                return Err("The selected installation differs from the reviewed target. Review again before applying.".into());
            }
        }
    }
    for r in &p.client_removals {
        discovery::require_games_closed_for("confirming an uninstalled client's shortcut cleanup")?;
        if uninstall_evidence(&p.installation, &r.edition, &r.game)? != r.metadata_hash {
            return Err("Client uninstall evidence changed since review. Review again before shortcut cleanup.".into());
        }
    }
    Ok(p)
}
fn validate_proton(steam: &Steam, requested: &str) -> Result<String> {
    if steam.protons.iter().any(|p| p.name == requested) {
        Ok(requested.into())
    } else {
        Err("The selected Proton version is not installed. Install it from Steam → Library → Tools, then scan again.".into())
    }
}
fn review_setup(ctx: &Context, req: &Json) -> Result<Json> {
    workflow::request(req)?;
    if !workflow::LANGUAGES.contains(&req["language"].as_str().unwrap_or("enUS")) {
        return Err("Choose a listed installer language".into());
    }
    let root = fsutil::canonical(Path::new(field(req, "steam")?))?;
    if !discovery::steam_roots(&ctx.home).contains(&root) {
        return Err("Select a detected native Steam installation".into());
    }
    let info = discovery::steam_info(&root)?;
    let user = field(req, "user")?;
    if !info.users.iter().any(|x| x == user) {
        return Err("Steam profile not found".into());
    }
    let proton = validate_proton(&info, field(req, "proton")?)?;
    let editions = strings(req, "editions");
    if editions.is_empty() || editions.iter().any(|e| catalog::by_id(e).is_none()) {
        return Err("Choose at least one listed WoW client".into());
    }
    let location = fsutil::canonical(Path::new(field(req, "location")?))?;
    fsutil::safe_path(&location)?;
    if location == Path::new("/") {
        return Err("Choose a folder on your game drive".into());
    }
    let free = fsutil::free_bytes(&location)?;
    if free < 2 * 1024 * 1024 * 1024 {
        return Err("At least 2 GiB free is needed to prepare Battle.net. WoW needs additional space shown by Blizzard.".into());
    }
    let id = fsutil::unique();
    let base = location.join(format!("WoW-{}", &id[..12]));
    let prefix = base.join("compatdata");
    let installer = base.join("Battle.net-Setup.exe");
    let opts = format!(
        "{} --lang={}",
        steam::launch_options(&prefix, None)?,
        req["language"].as_str().unwrap_or("enUS")
    );
    let mut entry = steam::entry("Battle.net", &installer, &opts, &id, None)?;
    vdf::set(&mut entry, "wowRustRole", Value::Text("launcher".into()));
    let bootstrap_id = vdf::number(&entry, "appid").unwrap();
    let path = steam::shortcuts_path(&root, user)?;
    let mut shortcuts = steam::read_shortcuts(&path)?;
    steam::upsert(&mut shortcuts, entry, &id)?;
    let cfg = root.join("config/config.vdf");
    let config = steam::set_proton(Some(&fsutil::read(&cfg)?), &[bootstrap_id], &proton)?;
    let i = Installation {
        launcher_exit_choice: None,
        launcher_exit_reviewed: false,
        id: id.clone(),
        root: base.clone(),
        prefix,
        steam: root,
        user: user.into(),
        proton: proton.clone(),
        editions,
        stage: Stage::LauncherInstall,
        language: req["language"].as_str().unwrap_or("enUS").into(),
        restore_backup: None,
        addon_pending: false,
        launcher: None,
        bootstrap_id,
        owned: true,
        shortcuts: vec![bootstrap_id],
        launcher_tile: true,
        addon_manager: req["addon_manager"].as_str().unwrap_or("none").to_string(),
        bindings: vec![],
        game_associations: vec![],
    };
    let mut s = ctx.state.clone();
    s.current = Some(id);
    s.installations.push(i.clone());
    let changes = vec![
        Change::new(path, vdf::binary(&shortcuts)?)?,
        Change::new(cfg, config)?,
        Change::new(
            ctx.data.join("state.json"),
            serde_json::to_vec_pretty(&s).unwrap(),
        )?,
    ];
    let mut p=Plan{token:fsutil::unique(),kind:"setup".into(),summary:vec![format!("Create a separate installation in {}",base.display()),format!("Use installed Proton tool {proton}"),format!("Available space: {:.1} GiB. Battle.net will show the actual game download sizes.",free as f64/1073741824.0),"Download Blizzard's Windows Battle.net installer over HTTPS.".into(),"Add one Battle.net tile to the selected Steam profile. Existing installations stay in place.".into(),"Sign in and download games inside Battle.net. Return here afterward.".into()],installation:i,journal:Journal{id:fsutil::unique(),label:"New Battle.net setup".into(),changes,done:false},remove:vec![],keep_settings:false,download:true,replacement:None,client_removals:vec![],shared_migration:false,installed_editions:vec![]};
    p.summary.push(format!(
        "Selected games: {}. Addon manager: {}. Separate Battle.net tile: {}.",
        p.installation.editions.join(", "),
        p.installation.addon_manager,
        p.installation.launcher_tile
    ));
    save_plan(ctx, &p)
}
fn associated_games(scan: &Scan, i: &Installation) -> Vec<Game> {
    scan.games
        .iter()
        .filter(|g| {
            g.prefix.as_ref() == Some(&i.prefix) || g.path.starts_with(i.root.join("games"))
        })
        .cloned()
        .collect()
}
fn review_repair(ctx: &Context, req: &Json) -> Result<Json> {
    let mut i = ctx.install(field(req, "id")?)?;
    require_exit_preference(&i)?;
    let scan = discovery::scan(&ctx.home, &ctx.state)?;
    let launchers: Vec<_> = scan
        .launchers
        .iter()
        .filter(|l| l.prefix == i.prefix)
        .collect();
    if launchers.len() != 1 {
        return Err("A single Battle.net launcher was not found in this environment. Complete installation, close Battle.net, then scan again.".into());
    }
    let launcher = launchers[0].path.clone();
    i.launcher = Some(launcher.clone());
    let mut games = installation_games(ctx, &i, &scan)?;
    if req["confirm_forever_beta"] == true {
        if games.iter().any(|g| g.id == "forever") {
            return Err(
                "A Forever client is already associated. Leave the beta confirmation unchecked."
                    .into(),
            );
        }
        let matches: Vec<_> = games
            .iter_mut()
            .filter(|g| g.id == "classic_beta")
            .collect();
        if matches.len() != 1 {
            return Err("Confirming Forever requires exactly one detected Classic / Forever beta client in this copy.".into());
        }
        for game in matches {
            game.id = "forever".into();
            game.name = catalog::by_id("forever").unwrap().name.into();
        }
    }
    if games.is_empty() {
        return Err("No installed WoW client was found for this copy. Finish downloading in Battle.net, then check again. Use Search another folder for an external game location.".into());
    }
    // A recovered association also updates the remembered selection; otherwise
    // an earlier beta selection would be reported as a missing download.
    if games
        .iter()
        .any(|g| g.id == "forever" && g.path.file_name().is_some_and(|n| n == "_classic_beta_"))
        && !games.iter().any(|g| g.id == "classic_beta")
    {
        for id in &mut i.editions {
            if id == "classic_beta" {
                *id = "forever".into();
            }
        }
        let mut selected = BTreeSet::new();
        i.editions.retain(|id| selected.insert(id.clone()));
    }
    let missing: Vec<_> = i
        .editions
        .iter()
        .filter(|id| id.as_str() != "other" && !games.iter().any(|g| &g.id == *id))
        .cloned()
        .collect();
    if let Some(scope) = req.get("installed_only_scope") {
        let mut selected = i.editions.clone();
        selected.sort();
        let mut absent = missing.clone();
        absent.sort();
        if scope["id"] != i.id
            || scope["selected"] != json!(selected)
            || scope["missing"] != json!(absent)
        {
            return Err("Installed or missing editions changed. Check the installation and explicitly choose again before reviewing repair.".into());
        }
    }
    if !missing.is_empty() && req["allow_missing"] != true {
        return Err(format!("Selected clients are still missing: {}. Finish downloads, or explicitly choose Use installed editions only.",missing.join(", ")));
    }
    let path = steam::shortcuts_path(&i.steam, &i.user)?;
    let mut root = steam::read_shortcuts(&path)?;
    steam::restore_identity(&mut root, &i)?;
    associate_legacy_forever(&mut root, &i, &games)?;
    let client_removals = obsolete_clients(&i, &games, &mut root)?;
    let entries = vdf::object(&root, "shortcuts").unwrap();
    let previous_tiles: Vec<_> = entries
        .iter()
        .filter_map(|(_, v)| match v {
            Value::Object(e) if steam::owned(e, &i.id) => Some((
                vdf::number(e, "appid")?,
                vdf::string(e, "appname")
                    .unwrap_or("Steam shortcut")
                    .to_string(),
            )),
            _ => None,
        })
        .collect();
    let candidates = shared_tile_candidates(&root, &i, &games)?;
    let restored_identity = if candidates.is_empty() {
        missing_shared_tile_identity(&root, &i)?
    } else {
        None
    };
    let keeper = shared_tile_keeper(&candidates, req)?.or(restored_identity);
    let cfgbytes = fsutil::read(&i.steam.join("config/config.vdf"))?;
    let mut actual = BTreeSet::new();
    if let Some(id) = restored_identity {
        if let Some(tool) = steam::proton_for(&cfgbytes, id)? {
            actual.insert(tool);
        }
    }
    for (_, v) in entries {
        if let Value::Object(e) = v {
            if steam::owned(e, &i.id) && vdf::string(e, "exe") != Some("\"/usr/bin/env\"") {
                if let Some(id) = vdf::number(e, "appid") {
                    if let Some(tool) = steam::proton_for(&cfgbytes, id)? {
                        actual.insert(tool);
                    }
                }
            }
        }
    }
    if actual.len() > 1 {
        return Err("This installation's tiles have conflicting Proton choices. Resolve them in Steam before repair.".into());
    }
    if let Some(tool) = actual.into_iter().next() {
        i.proton = tool;
    }
    let mut summary = vec![
        format!("Repair this Battle.net environment: {}", i.prefix.display()),
        format!(
            "Keep Proton {} and open the ordinary Battle.net edition chooser from its sole Steam tile.",
            i.proton
        ),
    ];
    if req["confirm_forever_beta"] == true {
        summary.push("Use the single detected Classic / Forever beta folder as Forever, as you confirmed in Battle.net. Preserve this association on future repairs.".into());
    }
    for r in &client_removals {
        summary.push(format!("Remove confirmed uninstalled {} shortcut {} and its stale binding; preserve all game files, settings, controller layouts and shared Proton.",r.edition,r.appid));
    }
    let mut ids = vec![];
    if let Some(id) = restored_identity {
        summary.push(format!("Restore missing Battle.net tile: keep its saved ID {id}, controller association, original launcher and Proton environment. Games, settings and access links stay in place; no download is needed."));
    }
    i.launcher_tile = true;
    let mut shared = steam::entry(
        "Battle.net",
        &launcher,
        &steam::launch_options(&i.prefix, None)?,
        &i.id,
        keeper,
    )?;
    vdf::set(
        &mut shared,
        "wowRustRole",
        Value::Text("shared_game".into()),
    );
    let shared_id = steam::upsert(&mut root, shared, &i.id)?;
    ids.push(shared_id);
    let retired: Vec<u32> = candidates
        .iter()
        .filter_map(|e| vdf::number(e, "appid"))
        .filter(|id| *id != shared_id)
        .collect();
    vdf::ensure(&mut root,"shortcuts")?.retain(|(_,v)| !matches!(v,Value::Object(e) if steam::owned(e,&i.id) && vdf::number(e,"appid").is_some_and(|id|retired.contains(&id))));
    i.game_associations
        .retain(|hint| !games.iter().any(|g| g.executable == hint.executable));
    i.game_associations.extend(games.clone());
    summary.push(format!("Single Battle.net tile: retain ID {shared_id}. Choose Retail, Classic Era or Forever in Battle.net, then press Play. The tile never chooses or starts an edition automatically."));
    if !candidates.is_empty()
        && candidates.iter().any(|e| {
            vdf::string(e, "wowRustRole") != Some("shared_game")
                || vdf::string(e, "appname") != Some("Battle.net")
        })
    {
        summary.push(format!("Single tile upgrade: keep the selected tile's existing controller association; retire only the reviewed installer-owned alternatives {:?}. Other layouts and artwork files are not merged, changed or deleted.",retired));
        summary.push("Before applying, create and verify a private outside-installation backup of the Steam shortcut/configuration/setup metadata and available controller records. The backup remains on this device.".into());
    }
    for (id, name) in previous_tiles {
        let remains = vdf::object(&root, "shortcuts")
            .unwrap()
            .iter()
            .any(|(_, v)| matches!(v,Value::Object(e) if vdf::number(e,"appid")==Some(id)));
        if !remains {
            summary.push(format!("Remove duplicate shortcut: {name}. Its controller/artwork files remain preserved; the shared tile retains the explicitly selected identity."));
        }
    }
    // Remove only our bootstrap entry, never another installer's similarly named tile.
    let entries = vdf::ensure(&mut root, "shortcuts")?;
    entries.retain(|(_,v)|!matches!(v,Value::Object(e)if steam::owned(e,&i.id)&&vdf::number(e,"appid")==Some(i.bootstrap_id)&&vdf::string(e,"appname")==Some("Install Battle.net")));
    for (n, (k, _)) in entries.iter_mut().enumerate() {
        *k = n.to_string();
    }
    let cfg = i.steam.join("config/config.vdf");
    let config = steam::set_proton(Some(&fsutil::read(&cfg)?), &ids, &i.proton)?;
    if i.stage == Stage::LauncherInstall {
        i.stage = i.stage.advance(Stage::Games)?;
    }
    if i.stage == Stage::Games {
        i.stage = i.stage.advance(Stage::TestGames)?;
    }
    i.addon_pending |= matches!(
        ctx.install(&i.id)?.stage,
        Stage::LauncherInstall | Stage::Games
    );
    i.shortcuts = ids;
    i.bindings = steam::bindings(&root, &i.id);
    let mut state = ctx.state.clone();
    if let Some(n) = state.installations.iter().position(|x| x.id == i.id) {
        state.installations[n] = i.clone();
    }
    state.addon_pending = state.installations.iter().any(|i| i.addon_pending);
    let p = Plan {
        token: fsutil::unique(),
        kind: "repair".into(),
        summary,
        installation: i,
        journal: Journal {
            id: fsutil::unique(),
            label: "Repair Steam shortcuts".into(),
            changes: vec![
                Change::new(path, vdf::binary(&root)?)?,
                Change::new(cfg, config)?,
                Change::new(
                    ctx.data.join("state.json"),
                    serde_json::to_vec_pretty(&state).unwrap(),
                )?,
            ],
            done: false,
        },
        remove: vec![],
        keep_settings: false,
        download: false,
        replacement: None,
        client_removals,
        shared_migration: candidates.len() > 1
            || candidates.iter().any(|e| {
                vdf::string(e, "wowRustRole") != Some("shared_game")
                    || vdf::string(e, "appname") != Some("Battle.net")
            }),
        installed_editions: games.iter().map(|g| g.id.clone()).collect(),
    };
    save_plan(ctx, &p)
}
fn download_host(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    // The server name ends at the first '/', '?' or '#'. Refuse user names,
    // ports, escapes and spaces so a redirect cannot disguise another server.
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty()
        || host.contains(['@', ':', '%', '\\'])
        || host.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return false;
    }
    [
        "downloader.battle.net",
        "battle.net",
        "blizzard.com",
        "blizzard.net",
    ]
    .iter()
    .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}
fn valid_installer(b: &[u8]) -> bool {
    if b.len() < 1024 || b.len() > 104857600 || !b.starts_with(b"MZ") {
        return false;
    }
    let offset = u32::from_le_bytes(b[0x3c..0x40].try_into().unwrap()) as usize;
    offset >= 0x40
        && offset.checked_add(26).is_some_and(|end| end <= b.len())
        && &b[offset..offset + 4] == b"PE\0\0"
        && matches!(
            u16::from_le_bytes([b[offset + 4], b[offset + 5]]),
            0x14c | 0x8664
        )
        && matches!(
            u16::from_le_bytes([b[offset + 24], b[offset + 25]]),
            0x10b | 0x20b
        )
}
#[cfg(test)]
fn test_installer() -> Vec<u8> {
    let mut b = vec![0; 2048];
    b[..2].copy_from_slice(b"MZ");
    b[0x3c] = 0x80;
    b[0x80..0x84].copy_from_slice(b"PE\0\0");
    b[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
    b[0x98..0x9a].copy_from_slice(&0x20bu16.to_le_bytes());
    b
}
fn download(ctx: &Context, i: &Installation) -> Result<()> {
    fsutil::private_dir(&i.root)?;
    fsutil::private_dir(&i.prefix)?;
    fsutil::private_dir(&i.root.join("games"))?;
    fsutil::json(&i.root.join(".wow-rust-owner.json"), i)?;
    let destination = i.root.join("Battle.net-Setup.exe");
    if destination.is_file() {
        let b = fsutil::read(&destination)?;
        if valid_installer(&b) {
            return Ok(());
        }
    }
    let part = i.root.join("Battle.net-Setup.part");
    use std::os::unix::fs::OpenOptionsExt;
    let log = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(ctx.data.join("download-error.txt"))
        .map_err(|e| e.to_string())?;
    let response_path = ctx.data.join("download-response.txt");
    let mut url =
        "https://downloader.battle.net/download/getInstaller?os=win&installer=Battle.net-Setup.exe"
            .to_string();
    let mut complete = false;
    for _ in 0..6 {
        if !download_host(&url) {
            return Err(
                "The installer download redirected to an unrecognized host. No installer was run."
                    .into(),
            );
        }
        let response = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&response_path)
            .map_err(|e| e.to_string())?;
        let mut child = host_command("curl")
            .args([
                "--fail",
                "--proto",
                "=https",
                "--connect-timeout",
                "20",
                "--max-time",
                "600",
                "--max-filesize",
                "104857600",
                "--retry",
                "2",
                "--silent",
                "--show-error",
                "--write-out",
                "%{http_code}\n%{redirect_url}",
                "--output",
            ])
            .arg(&part)
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(response)
            .stderr(log.try_clone().map_err(|e| e.to_string())?)
            .spawn()
            .map_err(|e| format!("Could not run curl: {e}"))?;
        loop {
            if crate::cancelled().is_err() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&part);
                return Err("Download cancelled. Steam settings were not changed.".into());
            }
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => {
                    if !status.success() {
                        let _ = fs::remove_file(&part);
                        return Err("Battle.net download failed. Check your connection and retry. Details are in local download-error.txt.".into());
                    }
                    break;
                }
                None => thread::sleep(Duration::from_millis(100)),
            }
        }
        let response = fs::read_to_string(&response_path).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&response_path);
        let (status, next) = response.split_once('\n').ok_or("Missing download status")?;
        if status == "200" {
            complete = true;
            break;
        }
        if !["301", "302", "303", "307", "308"].contains(&status) {
            return Err("Unexpected installer download response".into());
        }
        url = next.trim().to_string();
    }
    let b = fsutil::read(&part)?;
    if !complete || !valid_installer(&b) {
        let _ = fs::remove_file(&part);
        return Err("Download was not a complete Windows installer. Nothing was run.".into());
    }
    fsutil::atomic(&destination, &b)?;
    let _ = fs::remove_file(part);
    Ok(())
}
fn adoption_candidates(ctx: &Context, req: &Json) -> Result<Json> {
    let prefix = fsutil::canonical(Path::new(field(req, "prefix")?))?;
    let scan = discovery::scan(&ctx.home, &ctx.state)?;
    let launcher = scan
        .launchers
        .iter()
        .find(|x| x.prefix == prefix)
        .ok_or("Launcher is no longer present")?;
    let mut out = vec![];
    for s in scan.steam {
        for user in &s.users {
            let path = steam::shortcuts_path(&s.path, user)?;
            let root = steam::read_shortcuts(&path)?;
            let config = fsutil::read(&s.path.join("config/config.vdf"))?;
            for (_, v) in vdf::object(&root, "shortcuts").unwrap() {
                if let Value::Object(e) = v {
                    let exe = vdf::string(e, "exe").unwrap_or("").trim_matches('"');
                    let Ok(target) = Path::new(exe).canonicalize() else {
                        continue;
                    };
                    let options = vdf::string(e, "LaunchOptions").unwrap_or("");
                    let explicit_prefix =
                        steam::compat_prefix(options).and_then(|p| p.canonicalize().ok());
                    if (options.contains("STEAM_COMPAT_DATA_PATH") && explicit_prefix.is_none())
                        || explicit_prefix.as_ref().is_some_and(|p| p != &prefix)
                    {
                        continue;
                    }
                    let direct = scan.games.iter().find(|g| {
                        g.executable == target
                            && (g.prefix.as_ref() == Some(&prefix)
                                || explicit_prefix.as_ref() == Some(&prefix))
                    });
                    let route = vdf::string(e, "LaunchOptions").and_then(steam::product);
                    let options = vdf::string(e, "LaunchOptions").unwrap_or("");
                    let route_edition = steam::edition(options).and_then(catalog::by_id);
                    let matching: Vec<_> = scan
                        .games
                        .iter()
                        .filter(|g| {
                            g.prefix.as_ref() == Some(&prefix)
                                && route_edition.is_some_and(|e| g.id == e.id)
                        })
                        .collect();
                    let beta: Vec<_> = scan
                        .games
                        .iter()
                        .filter(|g| g.prefix.as_ref() == Some(&prefix) && g.id == "classic_beta")
                        .collect();
                    let routed = if steam::launcher_alias(&target, &launcher.path) {
                        if matching.len() == 1 {
                            Some(matching[0])
                        } else if matching.is_empty()
                            && route.as_deref() == Some("WoWF")
                            && beta.len() == 1
                        {
                            Some(beta[0])
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let associated = direct.or(routed);
                    if steam::launcher_alias(&target, &launcher.path) || direct.is_some() {
                        if let Some(id) = vdf::number(e, "appid") {
                            let p = steam::proton_for(&config, id)?;
                            out.push(json!({"steam":s.path,"user":user,"appid":id,"name":vdf::string(e,"appname"),"proton":p,"product":route.or_else(||steam::selected_product(options)),"game":associated.map(|g|g.executable.clone()),"edition":associated.map(|g|if routed.is_some(){route_edition.unwrap().id.to_string()}else{g.id.clone()})}));
                        }
                    }
                }
            }
        }
    }
    Ok(json!({"candidates":out,"prefix":prefix,"launcher":launcher.path}))
}
fn review_adopt(ctx: &Context, req: &Json) -> Result<Json> {
    let data = adoption_candidates(ctx, req)?;
    let candidates = data["candidates"].as_array().unwrap();
    let chosen = candidates
        .iter()
        .find(|x| {
            x["appid"] == req["appid"] && x["steam"] == req["steam"] && x["user"] == req["user"]
        })
        .ok_or("Choose an existing Battle.net Steam shortcut")?;
    let steam_root = PathBuf::from(field(chosen, "steam")?);
    let user = field(chosen, "user")?.to_string();
    let prefix = PathBuf::from(field(&data, "prefix")?);
    if ctx.state.installations.iter().any(|x| x.prefix == prefix) {
        return Err("This environment is already listed. Use its repair action.".into());
    }
    let proton=field(chosen,"proton").map_err(|_|"This shortcut has no explicit Proton selection. Set a working compatibility tool in Steam, launch it once, then scan again.")?.to_string();
    let id = fsutil::unique();
    let launcher = PathBuf::from(field(&data, "launcher")?);
    let path = steam::shortcuts_path(&steam_root, &user)?;
    let mut root = steam::read_shortcuts(&path)?;
    let mut adopted = vec![];
    let mut summary = vec![
        format!("Use existing environment: {}", prefix.display()),
        format!("Preserve Proton {proton}. Game files stay in place."),
    ];
    let entries = vdf::ensure(&mut root, "shortcuts")?;
    let selected_ids: Vec<u32> = req["selected"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_u64().and_then(|x| u32::try_from(x).ok()))
                .collect()
        })
        .unwrap_or_else(|| vec![chosen["appid"].as_u64().unwrap() as u32]);
    if selected_ids.iter().any(|app| {
        !candidates.iter().any(|x| {
            x["appid"] == *app
                && x["steam"] == chosen["steam"]
                && x["user"] == chosen["user"]
                && x["proton"] == chosen["proton"]
        })
    }) {
        return Err("Select shortcuts from the same Steam profile and Proton tool.".into());
    }
    for (_, v) in entries.iter_mut() {
        if let Value::Object(e) = v {
            if let Some(app) = vdf::number(e, "appid") {
                if selected_ids.contains(&app) {
                    let valid = candidates.iter().any(|x| {
                        x["appid"] == app
                            && x["steam"] == chosen["steam"]
                            && x["user"] == chosen["user"]
                            && x["proton"] == chosen["proton"]
                    });
                    if !valid {
                        return Err(
                            "Selected shortcuts do not share one launcher and Proton choice".into(),
                        );
                    }
                    let name =
                        steam::canonical_title(vdf::string(e, "appname").unwrap_or("Battle.net"));
                    let candidate = candidates
                        .iter()
                        .find(|x| {
                            x["appid"] == app
                                && x["steam"] == chosen["steam"]
                                && x["user"] == chosen["user"]
                        })
                        .unwrap();
                    if let (Some(game), Some(edition)) =
                        (candidate["game"].as_str(), candidate["edition"].as_str())
                    {
                        summary.push(format!(
                            "Associate {} with {} using the existing shortcut route.",
                            name, game
                        ));
                        vdf::set(e, "wowRustGame", Value::Text(game.into()));
                        vdf::set(e, "wowRustEdition", Value::Text(edition.into()));
                        vdf::set(e, "exe", Value::Text(fsutil::steam_quote(&launcher)?));
                        vdf::set(
                            e,
                            "StartDir",
                            Value::Text(fsutil::steam_quote(launcher.parent().unwrap())?),
                        );
                        vdf::set(
                            e,
                            "LaunchOptions",
                            Value::Text(steam::preserve_options(
                                vdf::string(e, "LaunchOptions").unwrap_or(""),
                                &products::options(
                                    &prefix,
                                    edition,
                                    products::installed(Path::new(game).parent().unwrap())
                                        .as_deref(),
                                )?,
                            )?),
                        );
                    }
                    if candidate["game"].is_null() {
                        match candidate["product"].as_str() {
                            Some("WoW") => {
                                vdf::set(e, "wowRustEdition", Value::Text("retail".into()))
                            }
                            Some("WoWF") => {
                                vdf::set(e, "wowRustEdition", Value::Text("forever".into()))
                            }
                            Some("WoWC") => {
                                vdf::set(e, "wowRustEdition", Value::Text("era".into()))
                            }
                            None if !vdf::string(e, "LaunchOptions")
                                .unwrap_or("")
                                .contains("--game")
                                && !vdf::string(e, "LaunchOptions")
                                    .unwrap_or("")
                                    .contains("--exec") =>
                            {
                                vdf::set(e, "wowRustRole", Value::Text("launcher".into()))
                            }
                            _ => {}
                        }
                    }
                    vdf::set(e, "appname", Value::Text(name.clone()));
                    let tags = vdf::ensure(e, "tags")?;
                    let key = tags.len().to_string();
                    tags.push((key, Value::Text(format!("wow-rust:{id}"))));
                    summary.push(format!("Adopt {name}; preserve shortcut ID {app}."));
                    adopted.push(app);
                }
            }
        }
    }
    if adopted.is_empty() {
        return Err("Select at least one matching shortcut".into());
    }
    let i = Installation {
        launcher_exit_choice: None,
        launcher_exit_reviewed: false,
        id: id.clone(),
        root: prefix.clone(),
        prefix,
        steam: steam_root,
        user,
        proton,
        editions: vec![],
        stage: Stage::Games,
        language: "enUS".into(),
        restore_backup: None,
        addon_pending: false,
        launcher: Some(launcher),
        bootstrap_id: chosen["appid"].as_u64().unwrap() as u32,
        owned: false,
        launcher_tile: true,
        addon_manager: "none".into(),
        shortcuts: adopted,
        bindings: steam::bindings(&root, &id),
        game_associations: vec![],
    };
    let mut state = ctx.state.clone();
    state.current = Some(id);
    state.installations.push(i.clone());
    let p = Plan {
        token: fsutil::unique(),
        kind: "adopt".into(),
        summary,
        installation: i,
        journal: Journal {
            id: fsutil::unique(),
            label: "Adopt existing Battle.net shortcuts".into(),
            changes: vec![
                Change::new(path, vdf::binary(&root)?)?,
                Change::new(
                    ctx.data.join("state.json"),
                    serde_json::to_vec_pretty(&state).unwrap(),
                )?,
            ],
            done: false,
        },
        remove: vec![],
        keep_settings: false,
        download: false,
        replacement: None,
        client_removals: vec![],
        shared_migration: false,
        installed_editions: vec![],
    };
    save_plan(ctx, &p)
}

fn review_clean(ctx: &Context, req: &Json) -> Result<Json> {
    workflow::request(req)?;
    let mut i = ctx.install(field(req, "id")?)?;
    if i.owned {
        let marker: Installation = fsutil::load(&i.root.join(".wow-rust-owner.json"))?;
        if marker.id != i.id
            || marker.prefix != i.prefix
            || fsutil::canonical(&i.root)? != i.root
            || i.prefix != i.root.join("compatdata")
        {
            return Err("Installation ownership changed; removal blocked".into());
        }
    } else {
        let info = discovery::steam_info(&i.steam)?;
        if !i
            .prefix
            .file_name()
            .is_some_and(|n| n.to_string_lossy().bytes().all(|c| c.is_ascii_digit()))
            || !info
                .libraries
                .iter()
                .any(|l| i.prefix.parent() == Some(l.join("steamapps/compatdata").as_path()))
        {
            return Err("Only conventional native Steam environments can be removed. Keep this copy and install another.".into());
        }
    }
    require_exclusive_apps(&i)?;
    let token = fsutil::unique();
    let mut remove = vec![];
    for p in [
        i.root.join("games/World of Warcraft"),
        i.prefix
            .join("pfx/drive_c/Program Files (x86)/World of Warcraft"),
        i.prefix.join("pfx/drive_c/Program Files/World of Warcraft"),
        i.prefix.join("pfx/drive_c/Program Files (x86)/Battle.net"),
        i.prefix.join("pfx/drive_c/Program Files/Battle.net"),
    ] {
        if p.exists() {
            if fsutil::canonical(&p)? != p {
                return Err("A linked parent blocks automatic removal".into());
            }
            fsutil::check_tree(&p)?;
            remove.push(Removal {
                identity: fsutil::identity(&p)?,
                quarantine: p.with_file_name(format!(".wow-removing-{token}-{}", remove.len())),
                path: p,
                removed: false,
            });
        }
    }
    let complete_owned = req["complete_owned"] == true;
    if complete_owned {
        exclusive_owned_root(ctx, &i)?;
        remove = vec![Removal {
            identity: fsutil::identity(&i.root)?,
            quarantine: i.root.with_file_name(format!(".wow-removing-{token}-0")),
            path: i.root.clone(),
            removed: false,
        }];
    }
    if remove.is_empty() {
        return Err("No conventional game or Battle.net folders found for removal.".into());
    }
    if ctx.state.installations.iter().any(|x| {
        x.id != i.id
            && (x.prefix == i.prefix || x.root.starts_with(&i.root) || i.root.starts_with(&x.root))
    }) {
        return Err("Another installation shares this location; removal blocked".into());
    }
    let path = steam::shortcuts_path(&i.steam, &i.user)?;
    let mut root = steam::read_shortcuts(&path)?;
    removal_references(ctx, &i, false)?;
    let entries = vdf::ensure(&mut root, "shortcuts")?;
    entries.retain(
        |(_, v)| !matches!(v,Value::Object(e)if steam::owned(e,&i.id) && !steam::native_addon(e)),
    );
    for (n, (k, _)) in entries.iter_mut().enumerate() {
        *k = n.to_string();
    }
    i.stage = Stage::Removed;
    // Keep reviewed IDs in the cleanup journal for reference/mapping safety.
    i.launcher = None;
    let mut state = ctx.state.clone();
    state.installations.retain(|x| x.id != i.id);
    state.current = None;
    state.addon_pending = true;
    let mut removable_ids = i.shortcuts.clone();
    removable_ids.push(i.bootstrap_id);
    for user in discovery::steam_info(&i.steam)?.users {
        let remaining = if user == i.user {
            root.clone()
        } else {
            steam::read_shortcuts(&steam::shortcuts_path(&i.steam, &user)?)?
        };
        for (_, v) in vdf::object(&remaining, "shortcuts").unwrap() {
            if let Value::Object(e) = v {
                if let Some(id) = vdf::number(e, "appid") {
                    removable_ids.retain(|x| *x != id);
                }
            }
        }
    }
    let keep = req["keep_settings"].as_bool().unwrap_or(false);
    let mut summary=vec!["Remove only the exact folders listed below and this copy's managed Steam tiles.".into(),"The Proton environment itself and Windows support files stay in place.".into(),"Deleted game files require a new download. Steam configuration backups cannot restore them.".into(),"After removal, choose Set up a new copy to create a fresh environment.".into()];
    if complete_owned {
        summary[0]="Remove this dedicated installer-owned environment, including its cached installer, local Windows support and owner marker. Shared addon managers and Steam/Proton installations remain.".into();
        summary[1]="The exact dedicated root below is deleted; symlink targets outside it are never followed.".into();
    } else {
        summary.push("Retained: surrounding Proton environment, cached installer and owner marker; these may be shared or adopted. Complete removal is available only for proven dedicated app-owned roots.".into());
    }
    summary.push(if keep{"Copy Interface and WTF folders into a local settings backup before removal. Restore them explicitly after the new download."}else{"Remove addons and settings too. No game/settings backup will be made."}.into());
    if keep {
        summary.push(format!("Settings backup location: {}. Keep this folder outside the removed installation. After downloading the same editions, use Help → Restore preserved settings before opening WoW. Only Interface and WTF are preserved; back up the full game separately outside the removal root if you need to retain downloaded game files.", ctx.data.join("settings-backups").join(&token).display()));
    }
    let mut p = Plan {
        token,
        kind: if complete_owned {
            "clean_owned"
        } else {
            "clean"
        }
        .into(),
        summary,
        installation: i,
        journal: Journal {
            id: fsutil::unique(),
            label: "reinstall: remove managed copy".into(),
            changes: vec![
                Change::new(path, vdf::binary(&root)?)?,
                Change::new(
                    ctx.data.join("state.json"),
                    serde_json::to_vec_pretty(&state).unwrap(),
                )?,
            ],
            done: false,
        },
        remove,
        keep_settings: keep,
        download: false,
        replacement: None,
        client_removals: vec![],
        shared_migration: false,
        installed_editions: vec![],
    };
    if complete_owned {
        let cfg = p.installation.steam.join("config/config.vdf");
        p.journal.changes.push(Change::new(
            cfg.clone(),
            steam::remove_proton(&fsutil::read(&cfg)?, &removable_ids)?,
        )?);
        p.summary.push("Remove compatibility selections only for this copy's IDs that no remaining Steam profile uses. Shared tool installations remain.".into());
    }
    save_plan(ctx, &p)
}
#[derive(Serialize, Deserialize)]
struct Cleanup {
    plan: Plan,
    settings_saved: bool,
}
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fsutil::check_tree(from)?;
    let mut stack = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((src, dst)) = stack.pop() {
        fsutil::private_dir(&dst)?;
        for e in fs::read_dir(src).map_err(|e| e.to_string())? {
            crate::cancelled()?;
            let e = e.map_err(|e| e.to_string())?;
            let kind = e.file_type().map_err(|e| e.to_string())?;
            let target = dst.join(e.file_name());
            if kind.is_dir() {
                stack.push((e.path(), target));
            } else if kind.is_file() {
                if target.exists() {
                    return Err(
                        "Settings destination already exists; nothing will be overwritten".into(),
                    );
                }
                let b = fsutil::read(&e.path())?;
                fsutil::atomic(&target, &b)?;
            } else {
                return Err("Linked settings cannot be copied automatically".into());
            }
        }
    }
    Ok(())
}
fn run_cleanup(ctx: &Context, job: &mut Cleanup) -> Result<()> {
    discovery::require_stopped()?;
    discovery::require_games_closed_for(discovery::operation_description(&job.plan.kind))?;
    validate_cleanup(ctx, job)?;
    let jobpath = ctx.data.join("cleanup.json");
    preserve_cleanup_settings(ctx, job)?;
    let report = crate::folder_links::remove(&ctx.data, &ctx.home, &job.plan.installation.id)
        .unwrap_or_else(
            |e| json!({"message":e,"targets":"untouched; changed access links preserved"}),
        );
    fsutil::json(&ctx.data.join("last-folder-unlink.json"), &report)?;

    if !job.plan.journal.done {
        if let Some(i) = &job.plan.replacement {
            download(ctx, i)?;
        }
        if ctx.data.join("transaction.json").exists() {
            transaction::recover(&ctx.data, true)?;
        }
        transaction::rebase(&mut job.plan.journal)?;
        fsutil::json(&jobpath, job)?;
        transaction::verify(&job.plan.journal.changes, true)?;
        transaction::finish(&ctx.data, &mut job.plan.journal, true)?;
        fsutil::json(&jobpath, job)?;
    }
    for n in 0..job.plan.remove.len() {
        discovery::require_stopped()?;
        discovery::require_games_closed_for(discovery::operation_description(&job.plan.kind))?;
        removal_references(ctx, &job.plan.installation, job.plan.kind == "clean_owned")?;
        if job.plan.keep_settings {
            verify_cleanup_backup(ctx, job)?;
        }
        let r = &job.plan.remove[n];
        if r.removed {
            continue;
        }
        if r.quarantine.exists() {
            if r.path.exists() {
                return Err("A new folder appeared at the old location. Removal paused.".into());
            }
            if fsutil::identity(&r.quarantine)? != r.identity {
                return Err("Removal folder identity changed; stop and inspect".into());
            }
        } else if r.path.exists() {
            if fsutil::identity(&r.path)? != r.identity || fsutil::canonical(&r.path)? != r.path {
                return Err("Reviewed folder changed; removal blocked".into());
            }
            if job.plan.kind == "clean_owned" {
                owned_tree(&r.path)?;
            } else {
                fsutil::check_tree(&r.path)?;
            }
            fs::rename(&r.path, &r.quarantine).map_err(|e| e.to_string())?;
        }
        if r.quarantine.exists() {
            removal_references(ctx, &job.plan.installation, job.plan.kind == "clean_owned")?;
            if job.plan.keep_settings {
                verify_cleanup_backup(ctx, job)?;
            }
            if job.plan.kind == "clean_owned" {
                owned_tree(&r.quarantine)?;
            } else {
                fsutil::check_tree(&r.quarantine)?;
            }
            fs::remove_dir_all(&r.quarantine).map_err(|e| e.to_string())?;
        }
        job.plan.remove[n].removed = true;
        fsutil::json(&jobpath, job)?;
    }
    fsutil::json(&ctx.data.join("last-cleanup.json"), job)?;
    fs::remove_file(jobpath).map_err(|e| e.to_string())?;
    Ok(())
}
fn apply(ctx: &Context, req: &Json) -> Result<Json> {
    let mut p = load_plan(ctx, req)?;
    discovery::require_stopped()?;
    if p.kind == "repair" && !p.installation.bindings.iter().any(|b| b.shared) {
        return Err("This shortcut review predates the shared tile. Check the games and create a new review before applying.".into());
    }
    if p.download {
        validate_proton(
            &discovery::steam_info(&p.installation.steam)?,
            &p.installation.proton,
        )?;
    }
    transaction::rebase(&mut p.journal)?;
    transaction::verify(&p.journal.changes, false)?;
    crate::cancelled()?;
    if p.kind == "clean" || p.kind == "clean_owned" || p.kind == "reinstall" {
        if field(req, "confirmation")? != "REMOVE THIS COPY" {
            return Err("Type REMOVE THIS COPY to approve the listed deletion.".into());
        }
        for r in &p.remove {
            if fsutil::identity(&r.path)? != r.identity {
                return Err("A removal folder changed after review".into());
            }
            if p.kind == "clean_owned" {
                owned_tree(&r.path)?;
            } else {
                fsutil::check_tree(&r.path)?;
            }
        }
        let mut job = Cleanup {
            plan: p,
            settings_saved: false,
        };
        fsutil::json(&ctx.data.join("cleanup.json"), &job)?;
        run_cleanup(ctx, &mut job)?;
        return Ok(
            json!({"message":if job.plan.replacement.is_some(){"Fresh setup prepared. Open Battle.net in Steam, then return here."}else{"Selected copy removed. Set up a new copy for the fresh installation."},"kind":job.plan.kind,"id":job.plan.replacement.as_ref().map(|i|i.id.clone())}),
        );
    }
    if p.kind == "repair" {
        require_exit_preference(&ctx.install(&p.installation.id)?)?;
    }
    if p.kind == "exit" {
        discovery::require_games_closed_for(discovery::operation_description("exit"))?;
    }
    if p.download {
        download(ctx, &p.installation)?;
    }
    transaction::rebase(&mut p.journal)?;
    if shared_migration_needed(&p)? {
        backup_shared_migration(ctx, &p)?;
    }
    if p.kind == "exit" {
        backup_exit_preference(ctx, &p)?;
    }
    transaction::commit(&ctx.data, &mut p.journal, true)?;
    if p.kind == "exit"
        && p.installation.launcher_exit_choice.as_deref() == Some("close_after_play")
    {
        require_exit_preference(&p.installation)?;
    }
    let _ = fs::remove_file(ctx.data.join("review.json"));
    if ["launcher", "repair"].contains(&p.kind.as_str()) {
        let _report = folder_access(ctx, &p.installation, true);
    }
    Ok(
        json!({"message":if p.kind=="setup"{"Setup saved. Start Steam and open Battle.net."}else if p.kind=="adopt"{"Existing setup adopted. Check games, then review shortcut repair."}else if p.kind=="exit"{"Battle.net behavior choice saved for this installation. Continue setup; existing game files and shortcuts are unchanged."}else{"Shortcuts ready. Test your games."},"kind":p.kind,"id":p.installation.id}),
    )
}
#[cfg(any(test, feature = "developer-tools"))]
fn review_addon(ctx: &Context, req: &Json) -> Result<Json> {
    let mut i = ctx.install(field(req, "id")?)?;
    if req["manager"] == "curseforge" {
        i.addon_manager = "curseforge".into();
        let desktop = gear_desktop(ctx)?;
        let mut entry = steam::native_entry(
            "CurseForge",
            &format!("gio launch {}", fsutil::shell_quote(&desktop)?),
            &i.id,
        )?;
        vdf::set(
            &mut entry,
            "wowRustNativeAddon",
            Value::Text("gearlever".into()),
        );
        let path = steam::shortcuts_path(&i.steam, &i.user)?;
        let mut root = steam::read_shortcuts(&path)?;
        let desired_route = vdf::string(&entry, "LaunchOptions")
            .unwrap_or("")
            .to_string();
        for (_, v) in vdf::ensure(&mut root, "shortcuts")? {
            if let Value::Object(e) = v {
                let same =
                    vdf::string(e, "LaunchOptions").is_some_and(|o| o.starts_with(&desired_route));
                if steam::native_addon(e) && same && !steam::owned(e, &i.id) {
                    let tags = vdf::ensure(e, "tags")?;
                    tags.push((
                        tags.len().to_string(),
                        Value::Text(format!("wow-rust:{}", i.id)),
                    ));
                }
            }
        }
        // Preserve identity/name and any custom suffix on an already stable desktop route.
        if let Some(old) =
            vdf::object(&root, "shortcuts")
                .unwrap()
                .iter()
                .find_map(|(_, v)| match v {
                    Value::Object(e)
                        if steam::owned(e, &i.id)
                            && steam::native_addon(e)
                            && (vdf::string(e, "appname") == Some("CurseForge")
                                || vdf::string(e, "wowRustNativeAddon") == Some("gearlever")
                                // Steam may drop the marker after a user rename.
                                // The owned, already validated desktop route remains identity.
                                || vdf::string(e, "LaunchOptions").is_some_and(|o| {
                                    o == desired_route
                                        || o.strip_prefix(&desired_route)
                                            .is_some_and(|suffix| suffix.starts_with(' '))
                                })) =>
                    {
                        Some(e)
                    }
                    _ => None,
                })
        {
            if let Some(id) = vdf::number(old, "appid") {
                vdf::set(&mut entry, "appid", Value::I32(id));
            }
            if let Some(name) = vdf::string(old, "appname") {
                vdf::set(&mut entry, "appname", Value::Text(name.into()));
            }
            let options = vdf::string(old, "LaunchOptions").unwrap_or("");
            let desired = vdf::string(&entry, "LaunchOptions").unwrap_or("");
            if options.starts_with(desired) {
                vdf::set(&mut entry, "LaunchOptions", Value::Text(options.into()));
            } else if options.starts_with("APPIMAGE_EXTRACT_AND_RUN=1 ") {
                // Known old installer format has one shell-quoted AppImage path.
                let raw = options.trim_start_matches("APPIMAGE_EXTRACT_AND_RUN=1 ");
                if !(raw.starts_with('\'')
                    && raw.ends_with('\'')
                    && !raw[1..raw.len() - 1].contains('\''))
                {
                    return Err("The old addon tile has custom launch options. Keep it and review the migration manually; nothing was replaced.".into());
                }
            } else {
                return Err("The addon tile has a custom route. Keep it and review manually; nothing changed.".into());
            }
        }
        steam::upsert(&mut root, entry, &i.id)?;
        return simple_plan(ctx,i,"addon","Use Gear Lever's desktop entry for CurseForge; replacement updates keep this tile independent of AppImage filenames. No store-packaged CurseForge, no Proton, no promised automatic updates.",vec![Change::new(path,vdf::binary(&root)?)?]);
    }
    let app = fsutil::canonical(Path::new(field(req, "path")?))?;
    let b = fsutil::read(&app)?;
    if !crate::addons::is_appimage(&b) {
        return Err("Select the Linux AppImage downloaded from the publisher".into());
    }
    use std::os::unix::fs::PermissionsExt;
    if fs::metadata(&app)
        .map_err(|e| e.to_string())?
        .permissions()
        .mode()
        & 0o111
        == 0
    {
        return Err(
            "Enable Is executable in the AppImage's file permissions, then select it again.".into(),
        );
    }
    i.addon_manager = field(req, "manager")?.to_string();
    let name = match field(req, "manager")? {
        "curseforge" => "CurseForge",
        "wowup_cf" => "WowUp with CurseForge",
        "wowup_wago" => "WowUp with Wago",
        _ => "WoW Addon Manager",
    };
    let options = format!("APPIMAGE_EXTRACT_AND_RUN=1 {}", fsutil::shell_quote(&app)?);
    let entry = steam::native_entry(name, &options, &i.id)?;
    let path = steam::shortcuts_path(&i.steam, &i.user)?;
    let mut root = steam::read_shortcuts(&path)?;
    let entries = vdf::ensure(&mut root, "shortcuts")?;
    for (_, value) in entries.iter_mut() {
        if let Value::Object(e) = value {
            if steam::native_addon(e)
                && vdf::string(e, "appname") == Some(name)
                && vdf::string(e, "LaunchOptions") == Some(options.as_str())
                && !steam::owned(e, &i.id)
            {
                let tags = vdf::ensure(e, "tags")?;
                tags.push((
                    tags.len().to_string(),
                    Value::Text(format!("wow-rust:{}", i.id)),
                ));
            }
        }
    }
    steam::upsert(&mut root, entry, &i.id)?;
    let mut state = ctx.state.clone();
    *state
        .installations
        .iter_mut()
        .find(|x| x.id == i.id)
        .ok_or("Select an installation first")? = i.clone();
    let state_change = Change::new(
        ctx.data.join("state.json"),
        serde_json::to_vec_pretty(&state).unwrap(),
    )?;
    let p=Plan{token:fsutil::unique(),kind:"addon".into(),summary:vec![format!("Add a native Linux Steam shortcut for {name}"),format!("Use {}",app.display()),"Use extraction-and-run; FUSE is not required. Keep the AppImage at this location.".into(),"No Proton and no manager database edits. Reconnect each game folder inside the manager.".into()],installation:i,journal:Journal{id:fsutil::unique(),label:"Add native addon manager".into(),changes:vec![Change::new(path,vdf::binary(&root)?)?,state_change],done:false},remove:vec![],keep_settings:false,download:false,replacement:None,client_removals:vec![],shared_migration:false,installed_editions:vec![]};
    save_plan(ctx, &p)
}
fn link_game(ctx: &mut Context, req: &Json) -> Result<Json> {
    let i = ctx.install(field(req, "id")?)?;
    let root = fsutil::canonical(Path::new(field(req, "path")?))?;
    let mut scan = Scan::default();
    discovery::walk(
        &root,
        &mut scan,
        &mut std::collections::HashSet::new(),
        &mut 10_000,
    )?;
    if scan.games.is_empty() {
        return Err(
            "This folder contains no recognized WoW clients. Select the World of Warcraft folder."
                .into(),
        );
    }
    if scan
        .games
        .iter()
        .any(|g| g.prefix.as_ref().map(|p| p != &i.prefix).unwrap_or(false))
    {
        return Err("This folder belongs to a different Proton environment".into());
    }
    // Explicit game links are saved separately; they do not imply ownership for deletion.
    let file = ctx.data.join(format!("games-{}.json", i.id));
    let mut links: Vec<PathBuf> = if file.exists() {
        fsutil::load(&file)?
    } else {
        vec![]
    };
    if !links.contains(&root) {
        links.push(root.clone());
    }
    fsutil::json(&file, &links)?;
    if !ctx.state.extra_roots.contains(&root) {
        ctx.state.extra_roots.push(root);
    }
    ctx.save()?;
    Ok(json!({"message":"Folder linked to the selected setup. Return to Check downloaded games."}))
}
#[cfg(any(test, feature = "developer-tools"))]
pub(crate) fn device_games(
    home: &Path,
    data: &Path,
    state: &State,
    i: &Installation,
    scan: &Scan,
) -> Result<Vec<Game>> {
    installation_games(
        &Context {
            home: home.into(),
            data: data.into(),
            state: state.clone(),
            state_error: false,
        },
        i,
        scan,
    )
}
// Recover the same constrained association used during adoption for older saved
// installations. Never use titles, touch unowned tiles, or guess among copies.
fn associate_legacy_forever(
    root: &mut vdf::Object,
    i: &Installation,
    games: &[Game],
) -> Result<()> {
    let candidates: Vec<_> = games
        .iter()
        .filter(|g| g.path.file_name().is_some_and(|n| n == "_classic_beta_"))
        .collect();
    if candidates.len() != 1
        || games
            .iter()
            .any(|g| g.id == "forever" && g.path != candidates[0].path)
    {
        return Ok(());
    }
    let game = candidates[0];
    let entries = vdf::ensure(root, "shortcuts")?;
    let legacy = entries.iter().any(|(_, v)| matches!(v, Value::Object(e)
        if steam::owned(e, &i.id)
        && vdf::string(e, "wowRustEdition") == Some("forever")
        && vdf::string(e, "LaunchOptions").and_then(steam::product).as_deref() == Some("WoWF")
        && (vdf::string(e, "wowRustGame").is_none()
            || vdf::string(e, "wowRustGame") == game.executable.to_str())
        && steam::compat_prefix(vdf::string(e, "LaunchOptions").unwrap_or("")).as_ref() == Some(&i.prefix)
        && i.launcher.as_ref().is_some_and(|launcher| steam::launcher_alias(
            Path::new(vdf::string(e, "exe").unwrap_or("").trim_matches('"')), launcher))
    ));
    if !legacy {
        return Ok(());
    }
    for (_, v) in entries {
        let Value::Object(e) = v else { continue };
        if !steam::owned(e, &i.id) {
            continue;
        }
        let route_matches = steam::compat_prefix(vdf::string(e, "LaunchOptions").unwrap_or(""))
            .as_ref()
            == Some(&i.prefix)
            && i.launcher.as_ref().is_some_and(|launcher| {
                steam::launcher_alias(
                    Path::new(vdf::string(e, "exe").unwrap_or("").trim_matches('"')),
                    launcher,
                )
            });
        if !route_matches {
            continue;
        }
        let old_forever = vdf::string(e, "wowRustEdition") == Some("forever")
            && vdf::string(e, "LaunchOptions")
                .and_then(steam::product)
                .as_deref()
                == Some("WoWF")
            && vdf::string(e, "wowRustGame").is_none();
        let same_game = vdf::string(e, "wowRustGame") == game.executable.to_str()
            && matches!(
                vdf::string(e, "wowRustEdition"),
                Some("classic_beta" | "forever")
            );
        if old_forever || same_game {
            vdf::set(
                e,
                "wowRustGame",
                Value::Text(game.executable.to_string_lossy().into()),
            );
            vdf::set(e, "wowRustEdition", Value::Text("forever".into()));
        }
    }
    Ok(())
}
fn installation_games(ctx: &Context, i: &Installation, scan: &Scan) -> Result<Vec<Game>> {
    let mut games = associated_games(scan, i);
    games.extend(linked_games(ctx, i)?);
    games.retain(|g| g.prefix.as_ref().map(|p| p == &i.prefix).unwrap_or(true));
    games.sort_by(|a, b| a.path.cmp(&b.path));
    games.dedup_by(|a, b| a.path == b.path);
    let mut root = steam::read_shortcuts(&steam::shortcuts_path(&i.steam, &i.user)?)?;
    steam::restore_identity(&mut root, i)?;
    associate_legacy_forever(&mut root, i, &games)?;
    for game in &mut games {
        let mut saved = BTreeSet::new();
        for (_, value) in vdf::object(&root, "shortcuts").unwrap() {
            if let Value::Object(e) = value {
                if steam::owned(e, &i.id)
                    && vdf::string(e, "wowRustGame") == game.executable.to_str()
                {
                    if let Some(id) =
                        vdf::string(e, "wowRustEdition").filter(|id| catalog::by_id(id).is_some())
                    {
                        saved.insert(id);
                    }
                }
            }
        }
        for known in &i.game_associations {
            if known.executable == game.executable {
                saved.insert(known.id.as_str());
            }
        }
        if saved.len() > 1 {
            return Err("Conflicting edition associations exist for one game folder. Review the old tiles before repair.".into());
        }
        if let Some(id) = saved.into_iter().next() {
            game.id = id.into();
            game.name = catalog::by_id(id).unwrap().name.into();
        }
    }
    Ok(games)
}

fn linked_games(ctx: &Context, i: &Installation) -> Result<Vec<Game>> {
    let p = ctx.data.join(format!("games-{}.json", i.id));
    let mut scan = Scan::default();
    if p.exists() {
        let paths: Vec<PathBuf> = fsutil::load(&p)?;
        let mut seen = std::collections::HashSet::new();
        let mut budget = 20_000;
        for p in paths {
            discovery::walk(&p, &mut scan, &mut seen, &mut budget)?;
        }
    }
    for known in &i.game_associations {
        if let Some(parent) = known.executable.parent() {
            discovery::walk(
                parent,
                &mut scan,
                &mut std::collections::HashSet::new(),
                &mut 2000,
            )?;
        }
    }
    let mut root = steam::read_shortcuts(&steam::shortcuts_path(&i.steam, &i.user)?)?;
    steam::restore_identity(&mut root, i)?;
    for (_, v) in vdf::object(&root, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            if steam::owned(e, &i.id) {
                if let Some(exe) = vdf::string(e, "wowRustGame") {
                    if let Some(parent) = Path::new(exe).parent() {
                        discovery::walk(
                            parent,
                            &mut scan,
                            &mut std::collections::HashSet::new(),
                            &mut 2000,
                        )?;
                    }
                }
            }
        }
    }
    Ok(scan.games)
}
pub(crate) fn host_command(program: &str) -> Command {
    let mut opener = Command::new(program);
    for key in [
        "LD_LIBRARY_PATH",
        "QT_PLUGIN_PATH",
        "QML2_IMPORT_PATH",
        "QML_IMPORT_PATH",
    ] {
        if let Ok(value) = std::env::var(format!("WOW_ORIGINAL_{key}")) {
            if value.is_empty() {
                opener.env_remove(key);
            } else {
                opener.env(key, value);
            }
        } else if std::env::var_os("WOW_BUNDLED").is_some() {
            opener.env_remove(key);
        }
    }
    opener.env_remove("WOW_BUNDLED");
    for (key, _) in std::env::vars_os() {
        if key
            .to_str()
            .map(|k| k.starts_with("WOW_UPDATE_") || k.starts_with("WOW_PORTABLE_"))
            .unwrap_or(false)
        {
            opener.env_remove(key);
        }
    }
    opener
}
fn launch(ctx: &Context, req: &Json) -> Result<Json> {
    let i = ctx.install(field(req, "id")?)?;
    let app = req["appid"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or("Select a shortcut")?;
    let mut root = steam::read_shortcuts(&steam::shortcuts_path(&i.steam, &i.user)?)?;
    steam::restore_identity(&mut root, &i)?;
    if !vdf::object(&root,"shortcuts").unwrap().iter().any(|(_,v)|matches!(v,Value::Object(e)if steam::owned(e,&i.id)&&vdf::number(e,"appid")==Some(app))){return Err("Shortcut is not owned by this setup. Rescan first.".into());}
    let mut opener = host_command("xdg-open");
    let mut child = opener
        .arg(format!("steam://rungameid/{}", steam::game_id(app)))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot open Steam: {e}"))?;
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(
        json!({"message":"Launch requested. Confirm the correct edition opens; this is not a verified gameplay result."}),
    )
}
fn folder_targets(
    ctx: &Context,
    i: &Installation,
) -> Result<(Vec<crate::folder_links::Target>, Vec<String>)> {
    let scan = discovery::scan(&ctx.home, &ctx.state)?;
    let mut targets = vec![];
    let mut messages = vec![];
    if let Some(path) = i
        .launcher
        .as_ref()
        .filter(|p| p.is_file())
        .and_then(|p| p.parent())
    {
        targets.push(crate::folder_links::Target {
            label: "Battle.net".into(),
            path: path.into(),
        });
    }
    let mut games = installation_games(ctx, i, &scan)?;
    for (n, g) in games.iter().enumerate() {
        if games
            .iter()
            .skip(n + 1)
            .any(|other| other.executable == g.executable && other.id != g.id)
        {
            return Err(
                "Conflicting explicit game associations; no folder labels were guessed.".into(),
            );
        }
    }
    let extra: Vec<_> = scan
        .games
        .iter()
        .filter(|g| {
            !games
                .iter()
                .any(|current| current.executable == g.executable)
        })
        .cloned()
        .collect();
    games.extend(extra);
    games.sort_by(|a, b| a.executable.cmp(&b.executable));
    games.dedup_by(|a, b| a.executable == b.executable);
    for g in games {
        if !g.executable.is_file() {
            continue;
        }
        let label = match g.id.as_str() {
            "retail" => "Retail",
            "era" => "Classic Era",
            "forever" => "Forever",
            "progression" => "Classic Progression",
            "classic_beta" => "Classic Beta",
            "ptr" => "Retail PTR",
            _ => g.name.as_str(),
        }
        .to_string();
        targets.push(crate::folder_links::Target {
            label: format!("{label} installation"),
            path: g.path.clone(),
        });
        match crate::folder_links::child_directory(&g.path,"Interface").and_then(|p|match p{Some(p)=>crate::folder_links::child_directory(&p,"AddOns"),None=>Ok(None)}) {
            Ok(Some(path))=>targets.push(crate::folder_links::Target{label:format!("{label} AddOns"),path}),
            Ok(None)=>messages.push(format!("{label} AddOns directory was not found in the detected client; no path was guessed. Finish the official installation or reconnect its drive, then refresh.")),
            Err(e)=>messages.push(format!("{label} AddOns: {e}")),
        }
    }
    Ok((targets, messages))
}
fn folder_access(ctx: &Context, i: &Installation, refresh: bool) -> Json {
    let read = || -> Result<Json> {
        let (targets, mut messages) = folder_targets(ctx, i)?;
        let mut result = if refresh {
            crate::folder_links::sync(&ctx.data, &ctx.home, &i.id, &targets)?
        } else {
            crate::folder_links::status(&ctx.data, &ctx.home)
        };
        if let Some(old) = result["messages"].as_array() {
            messages.extend(old.iter().filter_map(|v| v.as_str().map(str::to_string)));
        }
        messages.extend(crate::folder_links::missing_targets(
            &ctx.data, &ctx.home, &targets,
        ));
        messages.sort();
        messages.dedup();
        result["messages"] = json!(messages);
        result["id"] = json!(i.id);
        Ok(result)
    };
    read().unwrap_or_else(|e|json!({"id":i.id,"path":ctx.home.join("World of Warcraft"),"available":false,"state":"conflict","message":e,"links":[],"messages":[]}))
}
fn shortcuts(ctx: &Context, req: &Json) -> Result<Json> {
    let i = ctx.install(field(req, "id")?)?;
    let mut root = steam::read_shortcuts(&steam::shortcuts_path(&i.steam, &i.user)?)?;
    steam::restore_identity(&mut root, &i)?;
    let mut items = vec![];
    for (_, v) in vdf::object(&root, "shortcuts").unwrap() {
        if let Value::Object(e) = v {
            if steam::owned(e, &i.id) {
                items.push(json!({"appid":vdf::number(e,"appid"),"name":vdf::string(e,"appname"),"game":vdf::string(e,"wowRustGame").is_some()||vdf::string(e,"wowRustRole")==Some("shared_game"),"shared":vdf::string(e,"wowRustRole")==Some("shared_game"),"addon":steam::native_addon(e)}));
            }
        }
    }
    let scan = discovery::scan(&ctx.home, &ctx.state)?;
    let games = installation_games(ctx, &i, &scan)?;
    let handoff: Vec<_> = games
        .iter()
        .filter_map(|g| {
            crate::addons::game_handoff(g)
                .map(|path| json!({"name":g.name,"path":path,"client_path":g.path,"mode":"guided"}))
        })
        .collect();
    let migration = shared_tile_candidates(&root, &i, &games)?.iter().map(|e|json!({"appid":vdf::number(e,"appid"),"name":format!("{} · ID {}",vdf::string(e,"appname").unwrap_or("Steam tile"),vdf::number(e,"appid").unwrap_or(0)),"shared":vdf::string(e,"wowRustRole")==Some("shared_game")})).collect::<Vec<_>>();
    Ok(
        json!({"shortcuts":items,"games":games,"addon_handoff":handoff,"installation":i,"shared_tile_choices":migration,"launcher_exit_status":exit_preference_status(&i),"single_tile_ready":require_single_tile(ctx,&i).is_ok(),"folders":folder_access(ctx,&i,false)}),
    )
}

fn dispatch_inner(req: Json) -> Result<Json> {
    workflow::request(&req)?;
    let action = field(&req, "action")?;
    if action.starts_with("live_") && !["live_result", "live_stop"].contains(&action) {
        return Err("The installation test runner is deferred for this one-Battle.net-tile release. Existing test records are preserved; use normal setup and reviewed removal.".into());
    }
    if [
        "review_addon",
        "prepare_gear",
        "download_addon",
        "addon_registration_review",
        "addon_registration_apply",
    ]
    .contains(&action)
    {
        return Err("Addon-manager integration is deferred for this one-Battle.net-tile release. Existing addons and managers are preserved.".into());
    }
    if ["controller_review", "controller_apply"].contains(&action) {
        return Err(CONTROLLER_AUTOMATION_UNAVAILABLE.into());
    }
    if action == "catalog" {
        return Ok(json!({"version":VERSION,"catalog":catalog::EDITIONS}));
    }
    if action == "demo" {
        return Ok(json!({"version":VERSION,"catalog":catalog::EDITIONS,"demo":true}));
    }
    #[cfg(any(test, feature = "developer-tools"))]
    if ["device_run", "device_result", "device_mark"].contains(&action) {
        return crate::device_tests::dispatch(&req);
    }
    if action.starts_with("updater_") {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is not set")?;
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"))
            .join("wow-steamos-rust");
        return crate::updater::dispatch(&data, &req);
    }
    let mut ctx = Context::open()?;
    #[cfg(any(test, feature = "developer-tools"))]
    if action == "addon_lifecycle_cancel" {
        return addon_lifecycle_cancel(&ctx, &req);
    }
    let _lock = fsutil::Lock::acquire(&ctx.data.join("operation.lock"))?;
    if ctx.state_error
        && !["init", "scan", "diagnostics", "reset_state", "reopen_steam"].contains(&action)
    {
        return Err("Saved setup records could not be read. Use Reset setup records on Home. Game files and Steam shortcuts are unchanged.".into());
    }
    if action == "reset_state" {
        if !ctx.state_error {
            return Err("Setup records are readable; reset is not needed.".into());
        }
        if req["confirm"] != true {
            return Err("Confirm resetting setup records first.".into());
        }
        let archive = ctx.data.join("stopped-operations").join(fsutil::unique());
        fsutil::private_dir(&archive)?;
        for name in [
            "state.json",
            "review.json",
            "transaction.json",
            "cleanup.json",
            "restore.json",
        ] {
            let file = ctx.data.join(name);
            if file.exists() {
                fs::rename(&file, archive.join(name)).map_err(|e| e.to_string())?;
            }
        }
        return Ok(
            json!({"message":"Setup records archived. Game files and Steam shortcuts were kept. Scan again and use the existing installation. Any partly removed folders need manual inspection."}),
        );
    }

    if (ctx.data.join("transaction.json").exists()
        || ctx.data.join("cleanup.json").exists()
        || ctx.data.join("restore.json").exists())
        && ![
            "init",
            "scan",
            "recover",
            "guided_existing",
            "reopen_steam",
            "diagnostics",
            "cancel_cleanup",
            "abandon_cleanup",
        ]
        .contains(&action)
    {
        return Err("An interrupted change needs recovery first. Choose Continue unfinished change; setup can close and reopen Steam for recovery.".into());
    }
    match action {
        #[cfg(any(test, feature = "developer-tools"))]
        "live_prepare" | "live_result" | "live_step" | "live_retry_launch" | "live_stop" => {
            live_tests::dispatch(&mut ctx, &req)
        }
        #[cfg(any(test, feature = "developer-tools"))]
        "addon_lifecycle_review" => addon_lifecycle_review(&ctx, &req),
        #[cfg(any(test, feature = "developer-tools"))]
        "addon_lifecycle_apply" => addon_lifecycle_apply(&ctx, &req),
        #[cfg(any(test, feature = "developer-tools"))]
        "addon_lifecycle_result" => addon_lifecycle_result(&ctx, &req),
        #[cfg(any(test, feature = "developer-tools"))]
        "addon_registration_result" => addon_registration_result(&ctx, &req),
        #[cfg(any(test, feature = "developer-tools"))]
        "addon_registration_review" => addon_registration_review(&ctx, &req),
        #[cfg(any(test, feature = "developer-tools"))]
        "addon_registration_apply" => addon_registration_apply(&ctx, &req),
        #[cfg(any(test, feature = "developer-tools"))]
        "controller_review" => controller_review(&ctx, &req),
        #[cfg(any(test, feature = "developer-tools"))]
        "controller_apply" => controller_apply(&ctx, &req),
        "controller_status" => Ok(
            json!({"available":false,"message":CONTROLLER_AUTOMATION_UNAVAILABLE,"pending":ctx.data.join("controller-job.json").exists(),"result":if ctx.data.join("controller-result.json").exists(){fsutil::load::<Json>(&ctx.data.join("controller-result.json"))?}else{Json::Null},"historical_only":true}),
        ),
        "init" => Ok(state_info(&ctx)),
        "select_installation" => {
            let i = ctx.install(field(&req, "id")?)?;
            let mut state = ctx.state.clone();
            state.current = Some(i.id);
            ctx.persist(state)?;
            Ok(json!({"message":"Selected setup saved."}))
        }
        "scan" => {
            Ok(json!({"scan":discovery::scan(&ctx.home,&ctx.state)?,"session":state_info(&ctx)}))
        }
        "add_root" => {
            let p = fsutil::canonical(Path::new(field(&req, "path")?))?;
            if !p.is_dir() {
                return Err("Choose a folder".into());
            }
            if !ctx.state.extra_roots.contains(&p) {
                ctx.state.extra_roots.push(p.clone());
            }
            ctx.save()?;
            Ok(json!({"path":p,"message":"Search location saved. Scan again to see results."}))
        }
        "review_setup" => review_setup(&ctx, &req),
        "review_repair" => review_repair(&ctx, &req),
        "review_clean" => review_clean(&ctx, &req),
        "review_reinstall" => review_reinstall(&ctx, &req),
        "review_launcher" => review_launcher(&ctx, &req),
        "review_exit" => review_exit(&ctx, &req),
        "legacy_candidates" => legacy_candidates(&ctx, &req),
        "review_legacy" => review_legacy(&ctx, &req),
        #[cfg(any(test, feature = "developer-tools"))]
        "review_addon" => review_addon(&ctx, &req),
        #[cfg(any(test, feature = "developer-tools"))]
        "download_addon" => {
            let p =
                crate::addons::download(&ctx.data, field(&req, "manager")?, req["url"].as_str())?;
            Ok(
                json!({"path":p,"message":"Download validated. Review the native Steam tile next; no application was executed."}),
            )
        }
        "adoption_candidates" => adoption_candidates(&ctx, &req),
        "review_adopt" => review_adopt(&ctx, &req),
        "apply" => apply(&ctx, &req),
        "guided_apply" => guided_apply(&ctx, &req),
        "reopen_steam" => reopen_steam(&ctx),
        #[cfg(any(test, feature = "developer-tools"))]
        "prepare_gear" => prepare_gear(&ctx, &req),
        "proton_install" => proton_install(&ctx, &req),
        "proton_status" => proton_status(&ctx, &req),
        "cancel_cleanup" => cancel_cleanup(&ctx),
        "abandon_cleanup" => abandon_cleanup(&ctx),
        "recover" => recover_pending(&mut ctx),
        "guided_existing" => guided_existing(&mut ctx, &req),
        "undo" => {
            transaction::undo(&ctx.data, true)?;
            Ok(json!({"message":"Previous configuration restored. Game files were not changed."}))
        }
        "link_game" => link_game(&mut ctx, &req),
        "shortcuts" => shortcuts(&ctx, &req),
        "launch" => launch(&ctx, &req),
        "set_product" => {
            let i = ctx.install(field(&req, "id")?)?;
            let app = req["appid"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("Select a game shortcut")?;
            let code = field(&req, "product")?;
            let options = steam::launch_options(&i.prefix, Some(code))?;
            let path = steam::shortcuts_path(&i.steam, &i.user)?;
            let mut root = steam::read_shortcuts(&path)?;
            let e = vdf::ensure(&mut root, "shortcuts")?
                .iter_mut()
                .find_map(|(_, v)| match v {
                    Value::Object(e)
                        if steam::owned(e, &i.id) && vdf::number(e, "appid") == Some(app) =>
                    {
                        Some(e)
                    }
                    _ => None,
                })
                .ok_or("Shortcut not found")?;
            if matches!(
                vdf::string(e, "wowRustRole"),
                Some("shared_game" | "launcher")
            ) || i
                .bindings
                .iter()
                .any(|b| b.appid == app && (b.shared || b.launcher))
            {
                return Err("The single Battle.net tile does not select an edition. Choose the edition and press Play inside Battle.net.".into());
            }
            vdf::set(e, "LaunchOptions", Value::Text(options));
            let p=Plan{token:fsutil::unique(),kind:"product".into(),summary:vec![format!("Set explicitly confirmed Battle.net product command: {code}"),"The app cannot independently verify this product association. Test the resulting tile.".into()],installation:i,journal:Journal{id:fsutil::unique(),label:"Set confirmed product route".into(),changes:vec![Change::new(path,vdf::binary(&root)?)?],done:false},remove:vec![],keep_settings:false,download:false,replacement:None,client_removals:vec![],shared_migration:false,installed_editions:vec![]};
            save_plan(&ctx, &p)
        }
        "refresh_folders" => {
            let i = ctx.install(field(&req, "id")?)?;
            Ok(folder_access(&ctx, &i, true))
        }
        "open_folders" => {
            let i = ctx.install(field(&req, "id")?)?;
            let mut report = folder_access(&ctx, &i, false);
            if report["available"] != true {
                return Err(report["message"]
                    .as_str()
                    .unwrap_or("Folder access is unavailable")
                    .into());
            }
            report["open_path"] = report["path"].clone();
            Ok(report)
        }
        "remove_folder_links" => {
            if req["confirmed"] != true {
                return Err("Confirm removal of this installation's access links only; targets stay in place.".into());
            }
            let i = ctx.install(field(&req, "id")?)?;
            let removal = crate::folder_links::remove(&ctx.data, &ctx.home, &i.id)?;
            let mut report = folder_access(&ctx, &i, false);
            let conflicts = removal["messages"]
                .as_array()
                .is_some_and(|v| !v.is_empty());
            report["removal"] = removal;
            report["message"] = json!(if conflicts {
                "Changed access entries were preserved. Games and addons stayed in place."
            } else {
                "Only verified installer-owned access links were removed. Games and addons stayed in place."
            });
            Ok(report)
        }
        "addon_done" => {
            let id = field(&req, "id")?;
            let mut state = ctx.state.clone();
            let i = state
                .installations
                .iter_mut()
                .find(|i| i.id == id)
                .ok_or("Select an installation")?;
            require_exit_preference(i)?;
            require_single_tile(&ctx, i)?;
            if i.restore_backup.is_some() {
                return Err(
                    "Restore preserved settings or explicitly skip their restoration first.".into(),
                );
            }
            if i.stage == Stage::TestGames {
                i.stage = i.stage.advance(Stage::Addons)?;
            }
            if i.stage == Stage::Addons {
                i.stage = i.stage.advance(Stage::Complete)?;
            }
            i.addon_pending = false;
            state.addon_pending = state.installations.iter().any(|i| i.addon_pending);
            ctx.persist(state)?;
            Ok(json!({"message":"Addon follow-up resolved."}))
        }
        "skip_restore" => {
            if req["confirmed"] != true {
                return Err("Confirm that you want to skip restoring saved settings. The backup is retained.".into());
            }
            let id = field(&req, "id")?;
            let mut state = ctx.state.clone();
            state
                .installations
                .iter_mut()
                .find(|i| i.id == id)
                .ok_or("Select an installation")?
                .restore_backup = None;
            ctx.persist(state)?;
            Ok(json!({"message":"Settings restoration skipped; the backup remains available."}))
        }
        #[cfg(any(test, feature = "developer-tools"))]
        "mark_tested" => {
            let i = ctx.install(field(&req, "id")?)?;
            let name = field(&req, "name")?;
            let listed = shortcuts(&ctx, &req)?;
            if !listed["shortcuts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == name)
            {
                return Err("Select a current shortcut before recording a test.".into());
            }
            let key = format!("{}:{}", i.id, name);
            if !ctx.state.tests.contains(&key) {
                ctx.state.tests.push(key);
            }
            ctx.save()?;
            Ok(json!({"message":"Your successful test was recorded."}))
        }
        "settings_backups" => {
            let mut items = vec![];
            if let Ok(entries) = fs::read_dir(ctx.data.join("settings-backups")) {
                for e in entries.flatten() {
                    if e.path().is_dir() {
                        items.push(e.file_name().to_string_lossy().to_string());
                    }
                }
            }
            Ok(json!({"backups":items,"location":ctx.data.join("settings-backups")}))
        }
        "restore_settings" => restore_settings(&mut ctx, &req),
        "diagnostics" => {
            let scan = discovery::scan(&ctx.home, &ctx.state)?;
            let v = json!({"version":VERSION,"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"native_steam_count":scan.steam.len(),"launcher_count":scan.launchers.len(),"game_editions":scan.games.iter().map(|x|x.id.clone()).collect::<Vec<_>>(),"pending_transaction":ctx.data.join("transaction.json").exists(),"pending_cleanup":ctx.data.join("cleanup.json").exists(),"events":fsutil::load::<Vec<Json>>(&ctx.data.join("events.json")).unwrap_or_default(),"note":"No account IDs, usernames, paths, raw Steam files, or game settings included."});
            Ok(json!({"text":serde_json::to_string_pretty(&v).unwrap()}))
        }
        _ => Err("Unknown operation".into()),
    }
}

include!("operations.rs");
include!("guided.rs");
#[cfg(any(test, feature = "developer-tools"))]
include!("controller_setup.rs");
#[cfg(not(any(test, feature = "developer-tools")))]
const CONTROLLER_AUTOMATION_UNAVAILABLE: &str =
    "Automatic pointer configuration is unavailable in this version. Use Gaming Mode pointer setup for manual instructions.";
#[cfg(any(test, feature = "developer-tools"))]
include!("addon_registration.rs");
include!("preservation.rs");
#[cfg(any(test, feature = "developer-tools"))]
#[path = "live_tests.rs"]
mod live_tests;

/// Diagnostic events contain no request values, paths, account IDs or raw errors.
fn event(action: &str, outcome: &str) {
    let Ok(ctx) = Context::open() else { return };
    let Ok(_lock) = fsutil::Lock::acquire(&ctx.data.join("events.lock")) else {
        return;
    };
    let path = ctx.data.join("events.json");
    let mut events: Vec<Json> = fsutil::load(&path).unwrap_or_default();
    if events.len() > 119 {
        events.drain(..events.len() - 119);
    }
    events.push(json!({"version":VERSION,"operation":action,"result":outcome}));
    let _ = fsutil::json(&path, &events);
}
pub fn dispatch(req: Json) -> Result<Json> {
    crate::developer_tools::check_action(req["action"].as_str().unwrap_or(""))?;
    let action = req["action"].as_str().unwrap_or("");
    let action = if [
        "apply",
        "guided_apply",
        "guided_existing",
        "recover",
        "undo",
        "review_setup",
        "review_repair",
        "review_adopt",
        "review_reinstall",
        "review_clean",
        "launch",
        "cancel_cleanup",
        "abandon_cleanup",
        "reset_state",
        "live_step",
    ]
    .contains(&action)
    {
        action.to_string()
    } else {
        String::new()
    };
    if !action.is_empty() {
        event(&action, "started");
    }
    let result = dispatch_inner(req);
    if !action.is_empty() {
        let status = match &result {
            Ok(_) => "completed",
            Err(_) => "failed",
        };
        event(&action, status);
    }
    result
}

#[cfg(test)]
mod workflow_tests {
    use super::*;
    struct Fixture {
        ctx: Context,
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("wow-workflow-{}", fsutil::unique()));
            let home = root.join("home");
            let data = home.join(".local/share/wow-steamos-rust");
            let steam = home.join(".local/share/Steam");
            for p in [
                data.clone(),
                home.join("Games"),
                steam.join("userdata/123/config"),
                steam.join("config"),
                steam.join("steamapps/common/Proton 11"),
            ] {
                fs::create_dir_all(p).unwrap();
            }
            fs::write(
                steam.join("config/config.vdf"),
                b"\"InstallConfigStore\" { \"Software\" { \"Valve\" { \"Steam\" { } } } }",
            )
            .unwrap();
            fs::write(steam.join("steamapps/common/Proton 11/proton"), b"test").unwrap();
            fs::write(
                steam.join("steamapps/common/Proton 11/toolmanifest.vdf"),
                b"\"manifest\" { \"commandline\" \"/proton %verb%\" }",
            )
            .unwrap();
            fs::write(
                steam.join("steamapps/common/Proton 11/version"),
                b"1752000000 proton-11.0-1",
            )
            .unwrap();
            fs::create_dir_all(steam.join("steamapps/common/Proton 11/files/bin")).unwrap();
            fs::write(
                steam.join("steamapps/common/Proton 11/files/bin/wine"),
                b"fixture",
            )
            .unwrap();
            fs::write(
                steam.join("steamapps/common/Proton 11/files/bin/wineserver"),
                b"fixture",
            )
            .unwrap();
            fs::write(steam.join("steamapps/appmanifest_4628710.acf"),b"\"AppState\" { \"appid\" \"4628710\" \"StateFlags\" \"4\" \"SizeOnDisk\" \"1\" \"installdir\" \"Proton 11\" }").unwrap();
            Self {
                ctx: Context {
                    home,
                    data,
                    state_error: false,
                    state: State {
                        schema: 1,
                        ..State::default()
                    },
                },
                root,
            }
        }
        fn setup_request(&self) -> Json {
            json!({"steam":self.ctx.home.join(".local/share/Steam"),"user":"123","proton":"proton_11","location":self.ctx.home.join("Games"),"editions":["retail","forever","era"]})
        }
        fn refresh(&mut self) {
            self.ctx.state = fsutil::load(&self.ctx.data.join("state.json")).unwrap();
        }
        fn install(&mut self) -> Installation {
            let response = review_setup(&self.ctx, &self.setup_request()).unwrap();
            let p: Plan = fsutil::load(&self.ctx.data.join("review.json")).unwrap();
            fs::create_dir_all(&p.installation.root).unwrap();
            let bytes = test_installer();
            fs::write(p.installation.root.join("Battle.net-Setup.exe"), bytes).unwrap();
            apply(&self.ctx, &json!({"token":response["token"]})).unwrap();
            self.refresh();
            self.ctx.state.installations[0].clone()
        }
        fn games(&mut self, i: &Installation) {
            fs::create_dir_all(i.prefix.join("pfx/drive_c/Program Files (x86)/Steam")).unwrap();
            let bn = i.prefix.join("pfx/drive_c/Program Files (x86)/Battle.net");
            fs::create_dir_all(&bn).unwrap();
            fs::write(bn.join("Battle.net.exe"), b"MZ").unwrap();
            // Explicit simulated fixture precondition, not a real guided setup result.
            let settings = exit_settings_path(i);
            fs::create_dir_all(settings.parent().unwrap()).unwrap();
            if !settings.exists() {
                fs::write(
                    &settings,
                    br#"{"Client":{"GameLaunchWindowBehavior":"2","fixture_other":true}}"#,
                )
                .unwrap();
            }
            let saved = self
                .ctx
                .state
                .installations
                .iter_mut()
                .find(|x| x.id == i.id)
                .unwrap();
            if saved.launcher_exit_choice.is_none() {
                saved.launcher_exit_choice = Some("close_after_play".into());
                saved.launcher_exit_reviewed = true;
            }
            fsutil::json(&self.ctx.data.join("state.json"), &self.ctx.state).unwrap();

            for folder in ["_retail_", "_forever_", "_classic_era_"] {
                let path = i.root.join("games/World of Warcraft").join(folder);
                fs::create_dir_all(&path).unwrap();
                fs::write(path.join("Wow.exe"), b"MZ").unwrap();
            }
        }
        fn confirm_fixture_exit(&mut self, i: &Installation) {
            // Synthetic existing-enabled client, explicitly confirmed through production review.
            let path = exit_settings_path(i);
            if !path.exists() {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, br#"{"Client":{"GameLaunchWindowBehavior":"2"}}"#).unwrap();
            }
            let q = review_exit(&self.ctx, &json!({"id":i.id,"choice":"keep_current"})).unwrap();
            apply(&self.ctx, &json!({"token":q["token"]})).unwrap();
            self.refresh();
        }
        fn repair(&mut self, i: &Installation) {
            let p = review_repair(&self.ctx, &json!({"id":i.id})).unwrap();
            apply(&self.ctx, &json!({"token":p["token"]})).unwrap();
            self.refresh();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn fresh_setup_is_a_reviewed_checkpoint() {
        let mut f = Fixture::new();
        let i = f.install();
        assert_eq!(i.stage, Stage::LauncherInstall);
        assert!(i.root.join(".wow-rust-owner.json").exists());
        assert!(i.prefix.is_dir());
        assert_eq!(f.ctx.state.installations.len(), 1);
    }
    #[test]
    fn setup_review_does_not_create_game_folders() {
        let f = Fixture::new();
        let _ = review_setup(&f.ctx, &f.setup_request()).unwrap();
        let p: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
        assert!(!p.installation.root.exists());
    }
    #[test]
    fn setup_rejects_missing_proton() {
        let f = Fixture::new();
        let mut req = f.setup_request();
        req["proton"] = json!("missing");
        assert!(review_setup(&f.ctx, &req).is_err());
    }
    #[test]
    fn setup_rejects_unknown_editions() {
        let f = Fixture::new();
        let mut req = f.setup_request();
        req["editions"] = json!(["fictional"]);
        assert!(review_setup(&f.ctx, &req).is_err());
    }
    #[test]
    fn applying_wrong_review_token_is_blocked() {
        let f = Fixture::new();
        review_setup(&f.ctx, &f.setup_request()).unwrap();
        assert!(apply(&f.ctx, &json!({"token":"wrong"})).is_err());
    }
    #[test]
    fn incomplete_download_has_no_game_tiles() {
        let mut f = Fixture::new();
        let i = f.install();
        assert!(review_repair(&f.ctx, &json!({"id":i.id})).is_err());
    }
    #[test]
    fn repair_replaces_bootstrap_and_persists_addon_followup() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        f.repair(&i);
        let p = shortcuts(&f.ctx, &json!({"id":i.id})).unwrap();
        assert_eq!(p["shortcuts"].as_array().unwrap().len(), 1);
        assert_eq!(
            f.ctx.state.installations[0]
                .bindings
                .iter()
                .filter(|b| b.shared)
                .count(),
            1
        );
        assert_eq!(
            f.ctx.state.installations[0]
                .bindings
                .iter()
                .filter(|b| b.launcher)
                .count(),
            0
        );
        assert!(p["shortcuts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|x| x["name"] != "Install Battle.net"));
        assert!(f.ctx.state.addon_pending);
    }
    #[test]
    fn repeated_repair_preserves_steam_bytes() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        f.repair(&i);
        let path = steam::shortcuts_path(&i.steam, &i.user).unwrap();
        let before = fs::read(&path).unwrap();
        f.repair(&i);
        assert_eq!(before, fs::read(path).unwrap());
    }
    #[test]
    fn separate_copy_keeps_old_files() {
        let mut f = Fixture::new();
        let first = f.install();
        f.games(&first);
        let p = review_setup(&f.ctx, &f.setup_request()).unwrap();
        let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
        assert_ne!(plan.installation.root, first.root);
        assert_eq!(p["destructive"], false);
        assert!(first
            .root
            .join("games/World of Warcraft/_retail_/Wow.exe")
            .exists());
    }
    #[test]
    fn external_game_links_are_explicit() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let external = f.ctx.home.join("external/World of Warcraft/_classic_");
        fs::create_dir_all(&external).unwrap();
        fs::write(external.join("WowClassic.exe"), b"MZ").unwrap();
        link_game(
            &mut f.ctx,
            &json!({"id":i.id,"path":external.parent().unwrap()}),
        )
        .unwrap();
        f.repair(&i);
        let p = shortcuts(&f.ctx, &json!({"id":i.id})).unwrap();
        assert_eq!(p["games"].as_array().unwrap().len(), 4);
    }
    #[test]
    fn deletion_requires_exact_confirmation() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let p = review_clean(&f.ctx, &json!({"id":i.id})).unwrap();
        assert!(apply(&f.ctx, &json!({"token":p["token"],"confirmation":"yes"})).is_err());
        assert!(i.root.join("games/World of Warcraft").exists());
    }
    #[test]
    fn clean_removes_only_reviewed_application_folders() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let keep = i.prefix.join("pfx/drive_c/windows");
        fs::create_dir_all(&keep).unwrap();
        fs::write(keep.join("support.dll"), b"keep").unwrap();
        let p = review_clean(&f.ctx, &json!({"id":i.id})).unwrap();
        apply(
            &f.ctx,
            &json!({"token":p["token"],"confirmation":"REMOVE THIS COPY"}),
        )
        .unwrap();
        assert!(keep.join("support.dll").exists());
        assert!(i.prefix.exists());
        assert!(!i.root.join("games/World of Warcraft").exists());
        f.refresh();
        assert!(f.ctx.state.installations.is_empty());
        assert!(f.ctx.state.addon_pending);
    }
    #[test]
    fn unowned_copy_cannot_be_deleted() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        f.ctx.state.installations[0].owned = false;
        assert!(review_clean(&f.ctx, &json!({"id":i.id})).is_err());
    }
    #[test]
    fn removal_detects_folder_swap_after_review() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let p = review_clean(&f.ctx, &json!({"id":i.id})).unwrap();
        let game = i.root.join("games/World of Warcraft");
        fs::rename(&game, i.root.join("old-game")).unwrap();
        fs::create_dir_all(&game).unwrap();
        assert!(apply(
            &f.ctx,
            &json!({"token":p["token"],"confirmation":"REMOVE THIS COPY"})
        )
        .is_err());
    }
    #[test]
    fn settings_are_preserved_before_deletion() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        let settings = i.root.join("games/World of Warcraft/_retail_/WTF");
        fs::create_dir_all(&settings).unwrap();
        fs::write(settings.join("Config.wtf"), b"test settings").unwrap();
        let p = review_clean(&f.ctx, &json!({"id":i.id,"keep_settings":true})).unwrap();
        apply(
            &f.ctx,
            &json!({"token":p["token"],"confirmation":"REMOVE THIS COPY"}),
        )
        .unwrap();
        let saved = f
            .ctx
            .data
            .join("settings-backups")
            .join(p["token"].as_str().unwrap())
            .join("retail/WTF/Config.wtf");
        assert_eq!(fs::read(saved).unwrap(), b"test settings");
    }
    #[test]
    fn interrupted_cleanup_resumes_after_rename() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        review_clean(&f.ctx, &json!({"id":i.id})).unwrap();
        let mut p: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
        transaction::finish(&f.ctx.data, &mut p.journal, false).unwrap();
        fs::rename(&p.remove[0].path, &p.remove[0].quarantine).unwrap();
        let mut job = Cleanup {
            plan: p,
            settings_saved: true,
        };
        fsutil::json(&f.ctx.data.join("cleanup.json"), &job).unwrap();
        run_cleanup(&f.ctx, &mut job).unwrap();
        assert!(!f.ctx.data.join("cleanup.json").exists());
        assert!(job.plan.remove.iter().all(|r| r.removed));
    }
    #[test]
    fn adoption_preserves_existing_id_and_proton() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        f.repair(&i);
        f.ctx.state.installations.clear();
        f.ctx.state.current = None;
        f.ctx.save().unwrap();
        let data = adoption_candidates(&f.ctx, &json!({"prefix":i.prefix})).unwrap();
        let c = &data["candidates"][0];
        assert!(!c.is_null());
        let req = json!({"prefix":i.prefix,"steam":c["steam"],"user":c["user"],"appid":c["appid"]});
        let p = review_adopt(&f.ctx, &req).unwrap();
        let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
        assert_eq!(plan.installation.proton, "proton_11");
        assert!(!plan.installation.owned);
        assert!(plan
            .installation
            .shortcuts
            .contains(&(c["appid"].as_u64().unwrap() as u32)));
        assert_eq!(p["kind"], "adopt");
    }
    #[test]
    fn cleanup_recovers_backup_committed_before_checkpoint() {
        let mut f = Fixture::new();
        let i = f.install();
        f.games(&i);
        review_clean(&f.ctx, &json!({"id":i.id,"keep_settings":true})).unwrap();
        let plan: Plan = fsutil::load(&f.ctx.data.join("review.json")).unwrap();
        let backup = f.ctx.data.join("settings-backups").join(&plan.token);
        fs::create_dir_all(&backup).unwrap();
        fsutil::json(
            &backup.join(".inventory.json"),
            &SettingsInventory {
                token: plan.token.clone(),
                items: vec![],
            },
        )
        .unwrap();
        fsutil::json(&backup.join(".complete.json"), &plan.token).unwrap();
        let mut job = Cleanup {
            plan,
            settings_saved: false,
        };
        fsutil::json(&f.ctx.data.join("cleanup.json"), &job).unwrap();
        run_cleanup(&f.ctx, &mut job).unwrap();
        assert!(job.settings_saved);
        assert!(backup.exists());
    }
    include!("workflow_tests_alpha2.rs");
    include!("workflow_tests_alpha3.rs");
    include!("workflow_tests_live.rs");
    include!("workflow_tests_alpha8.rs");
    include!("review_repro_a8.rs");
    include!("workflow_tests_alpha9.rs");
    include!("workflow_tests_alpha10.rs");
    include!("workflow_tests_candidate4.rs");
    include!("workflow_tests_candidate6.rs");
    include!("workflow_tests_candidate8.rs");
    include!("workflow_tests_candidate9.rs");
    include!("workflow_tests_candidate10.rs");
    include!("workflow_tests_candidate12.rs");
    include!("workflow_tests_candidate13.rs");
    include!("workflow_tests_candidate14.rs");
    include!("workflow_tests_candidate17.rs");
    include!("workflow_tests_candidate23.rs");
    include!("workflow_tests_candidate40.rs");
    include!("workflow_tests_candidate42.rs");
    include!("workflow_tests_candidate44.rs");
    include!("workflow_tests_candidate45.rs");
    include!("workflow_tests_candidate47.rs");
    include!("workflow_tests_candidate30.rs");
    include!("workflow_tests_candidate35.rs");
}
