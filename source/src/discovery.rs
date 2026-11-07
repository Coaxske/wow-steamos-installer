use crate::{
    catalog, fsutil,
    model::*,
    vdf::{self, Value},
    Result,
};
use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
};
pub fn steam_running() -> Result<bool> {
    crate::steam_process::current_user_running()
}
pub fn require_stopped() -> Result<()> {
    if steam_running()? {
        Err("Steam is still running. Choose Steam → Exit, then retry. Keep a working input method available.".into())
    } else {
        Ok(())
    }
}
pub fn operation_description(kind: &str) -> &str {
    match kind {
        "setup" => "setting up this installation",
        "launcher" | "repair" | "addon" | "legacy" => "updating the reviewed Steam shortcuts",
        "clean" | "clean_owned" => "removing the reviewed installation files",
        "reinstall" => "reinstalling the reviewed copy",
        "restore" => "restoring preserved settings",
        "exit" => "changing Battle.net settings",
        _ => "continuing the reviewed operation",
    }
}
pub fn games_closed_message(operation: &str) -> String {
    format!(
        "Close Battle.net, its update agent, and WoW before {operation}. Nothing was force-closed."
    )
}
pub fn require_games_closed() -> Result<()> {
    require_games_closed_for("continuing the reviewed operation")
}
pub fn require_games_closed_for(operation: &str) -> Result<()> {
    for entry in fs::read_dir("/proc").map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if let Ok(name) = fs::read_to_string(entry.path().join("comm")) {
            let name = name.trim().to_ascii_lowercase();
            if name.starts_with("battle.net") || name == "agent.exe" || catalog::is_game_exe(&name)
            {
                return Err(games_closed_message(operation));
            }
        }
    }
    Ok(())
}
pub fn steam_roots(home: &Path) -> Vec<PathBuf> {
    let mut out = BTreeSet::new();
    for p in [
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ] {
        if p.join("steamapps").is_dir() {
            if let Ok(p) = p.canonicalize() {
                out.insert(p);
            }
        }
    }
    out.into_iter().collect()
}
pub fn steam_info(root: &Path) -> Result<Steam> {
    let mut libraries = BTreeSet::from([root.to_path_buf()]);
    let lf = root.join("steamapps/libraryfolders.vdf");
    if lf.is_file() {
        let o = vdf::parse_text(
            &String::from_utf8(fsutil::read(&lf)?).map_err(|_| "Invalid library list encoding")?,
        )?;
        if let Some(o) = vdf::object(&o, "libraryfolders") {
            for (_, v) in o {
                let p = match v {
                    Value::Object(x) => vdf::string(x, "path"),
                    Value::Text(x) => Some(x.as_str()),
                    _ => None,
                };
                if let Some(p) = p {
                    let p = PathBuf::from(p);
                    if p.join("steamapps").is_dir() {
                        libraries.insert(fsutil::canonical(&p)?);
                    }
                }
            }
        }
    }
    let users = fs::read_dir(root.join("userdata"))
        .map_err(|_| {
            "No Steam profiles found. Open native Steam and sign in once, then scan again."
        })?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_dir()
                && e.file_name()
                    .to_string_lossy()
                    .chars()
                    .all(|c| c.is_ascii_digit())
                && e.file_name() != "0"
        })
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let mut protons = Vec::new();
    for lib in &libraries {
        let common = lib.join("steamapps/common");
        if let Ok(entries) = fs::read_dir(&common) {
            for e in entries.flatten() {
                if !e.path().join("proton").is_file() {
                    continue;
                }
                let tool = e.path().join("compatibilitytool.vdf");
                if !tool.exists() {
                    if let Some(p) = valve_proton(&e.path(), &libraries) {
                        protons.push(p);
                    }
                }
                if let Ok(b) = fs::read_to_string(tool) {
                    if let Ok(o) = vdf::parse_text(&b) {
                        if let Some(o) = vdf::object(&o, "compatibilitytools")
                            .and_then(|o| vdf::object(o, "compat_tools"))
                        {
                            for (k, v) in o {
                                if let Value::Object(x) = v {
                                    protons.push(Proton {
                                        name: k.clone(),
                                        label: vdf::string(x, "display_name").unwrap_or(k).into(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if let Ok(entries) = fs::read_dir(root.join("compatibilitytools.d")) {
        for e in entries.flatten() {
            if let Ok(b) = fs::read_to_string(e.path().join("compatibilitytool.vdf")) {
                if let Ok(o) = vdf::parse_text(&b) {
                    if let Some(o) = vdf::object(&o, "compatibilitytools")
                        .and_then(|o| vdf::object(o, "compat_tools"))
                    {
                        for (k, v) in o {
                            if let Value::Object(x) = v {
                                protons.push(Proton {
                                    name: k.clone(),
                                    label: vdf::string(x, "display_name").unwrap_or(k).into(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    protons.sort_by(|a, b| a.name.cmp(&b.name));
    protons.dedup_by(|a, b| a.name == b.name);
    Ok(Steam {
        path: root.to_path_buf(),
        users,
        libraries: libraries.into_iter().collect(),
        protons,
    })
}
pub fn prefix_of(path: &Path) -> Option<PathBuf> {
    for a in path.ancestors() {
        if a.file_name().map(|x| x == "pfx").unwrap_or(false) && a.join("drive_c").is_dir() {
            return a.parent().map(Path::to_path_buf);
        }
    }
    None
}
pub fn walk(
    root: &Path,
    scan: &mut Scan,
    seen: &mut HashSet<PathBuf>,
    budget: &mut usize,
) -> Result<()> {
    let root = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            scan.warnings
                .push(format!("Location unavailable: {}", root.display()));
            return Ok(());
        }
    };
    let mut stack = vec![(root, 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        crate::cancelled()?;
        if *budget == 0 {
            scan.warnings.push(
                "Scan limit reached. Select the specific game folder to search more narrowly."
                    .into(),
            );
            break;
        }
        // A boundary folder has not had its children scanned. Let a later,
        // narrower explicit root revisit it with a fresh depth allowance.
        if seen.contains(&dir) {
            continue;
        }
        if depth < 11 {
            seen.insert(dir.clone());
        }
        *budget -= 1;
        let entries = match fs::read_dir(&dir) {
            Ok(x) => x,
            Err(_) => {
                scan.warnings
                    .push(format!("Could not read {}", dir.display()));
                continue;
            }
        };
        let folder = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut game = None;
        for e in entries.flatten() {
            let path = e.path();
            let kind = match e.file_type() {
                Ok(x) => x,
                Err(_) => continue,
            };
            let name = e.file_name().to_string_lossy().to_string();
            if kind.is_symlink() {
                continue;
            }
            if kind.is_file() {
                if name.eq_ignore_ascii_case("Battle.net.exe")
                    && dir.file_name().map(|x| x == "Battle.net").unwrap_or(false)
                {
                    if let Some(prefix) = prefix_of(&path) {
                        scan.launchers.push(Launcher {
                            path: path.clone(),
                            prefix,
                        });
                    }
                }
                if catalog::is_game_exe(&name) && e.metadata().map(|m| m.len() > 0).unwrap_or(false)
                {
                    game = Some(path.clone());
                }
            }
            if kind.is_dir() && depth < 11 && !skip_dir(&name) {
                stack.push((path, depth + 1));
            }
        }
        if let Some(executable) = game {
            if let Some(e) = catalog::by_folder(&folder) {
                scan.games.push(Game {
                    id: e.id.into(),
                    name: e.name.into(),
                    path: dir.clone(),
                    executable,
                    prefix: prefix_of(&dir),
                });
            } else if crate::products::installed(&dir).is_some() {
                scan.games.push(Game {
                    id: "other".into(),
                    name: format!("Other WoW client ({folder})"),
                    path: dir.clone(),
                    executable,
                    prefix: prefix_of(&dir),
                });
            }
        }
    }
    Ok(())
}
fn skip_dir(name: &str) -> bool {
    [
        "windows",
        "node_modules",
        ".git",
        "cache",
        "logs",
        "utils",
        "utilities",
        "data",
        "interface",
        "wtf",
        "screenshots",
        "shadercache",
        "depotcache",
        "appcache",
        "dosdevices",
    ]
    .contains(&name.to_ascii_lowercase().as_str())
}
pub fn scan(home: &Path, state: &State) -> Result<Scan> {
    let mut result = Scan {
        steam_running: steam_running()?,
        ..Scan::default()
    };
    let mut roots = vec![];
    for i in &state.installations {
        roots.push(i.prefix.clone());
        roots.push(i.root.clone());
    }
    roots.extend(state.extra_roots.iter().cloned());
    let mut broad = vec![home.join("Games")];
    for root in steam_roots(home) {
        match steam_info(&root) {
            Ok(steam) => {
                for lib in &steam.libraries {
                    roots.push(lib.join("steamapps/compatdata"));
                    broad.push(lib.join("steamapps/common"));
                }
                result.steam.push(steam);
            }
            Err(e) => result.warnings.push(e),
        }
    }
    if home.join(".var/app/com.valvesoftware.Steam").exists() {
        result
            .warnings
            .push("Flatpak Steam was found. This build supports native Steam only.".into());
    }
    roots.extend(state.extra_roots.iter().cloned());
    for i in &state.installations {
        roots.push(i.root.clone());
        if !i.owned {
            roots.push(i.prefix.clone());
        }
    }
    let mut seen = HashSet::new();
    roots.extend(broad);
    for root in roots {
        let mut budget = 50_000;
        if root.exists() {
            walk(&root, &mut result, &mut seen, &mut budget)?;
        }
    }
    result.games.sort_by(|a, b| a.path.cmp(&b.path));
    result.games.dedup_by(|a, b| a.path == b.path);
    result.launchers.sort_by(|a, b| a.path.cmp(&b.path));
    result.launchers.dedup_by(|a, b| a.path == b.path);
    result.warnings.truncate(30);
    Ok(result)
}

/// Valve-distributed x86_64 Proton 11 app ID, verified against the installed Steam
/// app manifest (Proton 11.0) and official client metadata; never a guessed depot ID.
pub const VALVE11_APP: &str = "4628710";
fn complete_app(library: &Path, app: &str) -> Option<PathBuf> {
    let bytes =
        fs::read_to_string(library.join(format!("steamapps/appmanifest_{app}.acf"))).ok()?;
    let parsed = vdf::parse_text(&bytes).ok()?;
    let state = vdf::object(&parsed, "AppState")?;
    if vdf::string(state, "appid")? != app
        || vdf::string(state, "StateFlags")? != "4"
        || vdf::string(state, "SizeOnDisk")?.parse::<u64>().ok()? == 0
    {
        return None;
    }
    if vdf::string(state, "UpdateResult").is_some_and(|v| v != "0") {
        return None;
    }
    if vdf::string(state, "StagingSize").is_some_and(|v| v != "0") {
        return None;
    }
    for (total, done) in [
        ("BytesToDownload", "BytesDownloaded"),
        ("BytesToStage", "BytesStaged"),
    ] {
        match (vdf::string(state, total), vdf::string(state, done)) {
            (None, None) => {}
            (Some(a), Some(b))
                if a.parse::<u64>()
                    .ok()
                    .zip(b.parse::<u64>().ok())
                    .is_some_and(|(a, b)| b >= a) => {}
            _ => return None,
        }
    }
    let dir = vdf::string(state, "installdir")?;
    if dir.is_empty()
        || Path::new(dir).components().count() != 1
        || !matches!(
            Path::new(dir).components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return None;
    }
    Some(library.join("steamapps/common").join(dir))
}
fn nonempty(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() > 0)
}
fn valve11_ready(path: &Path, libraries: &BTreeSet<PathBuf>) -> bool {
    let Some(library) = path.parent().and_then(Path::parent).and_then(Path::parent) else {
        return false;
    };
    if complete_app(library, VALVE11_APP).and_then(|p| p.canonicalize().ok())
        != path.canonicalize().ok()
    {
        return false;
    }
    if !["proton", "files/bin/wine", "files/bin/wineserver"]
        .iter()
        .all(|p| nonempty(&path.join(p)))
    {
        return false;
    }
    let Ok(text) = fs::read_to_string(path.join("toolmanifest.vdf")) else {
        return false;
    };
    let Ok(parsed) = vdf::parse_text(&text) else {
        return false;
    };
    let Some(m) = vdf::object(&parsed, "manifest") else {
        return false;
    };
    if let Some(app) = vdf::string(m, "require_tool_appid") {
        if !app.chars().all(|c| c.is_ascii_digit()) || app.is_empty() {
            return false;
        }
        if !libraries.iter().any(|lib| {
            complete_app(lib, app).is_some_and(|p| {
                nonempty(&p.join("toolmanifest.vdf")) && nonempty(&p.join("_v2-entry-point"))
            })
        }) {
            return false;
        }
    }
    true
}

/// Diagnose a completed Valve Proton whose declared runtime is not ready.
/// This does not relax the readiness predicate or install anything.
pub fn missing_valve11_runtime(info: &Steam) -> Option<String> {
    for library in &info.libraries {
        let Some(path) = complete_app(library, VALVE11_APP) else {
            continue;
        };
        if !path.file_name()?.to_str()?.starts_with("Proton 11")
            || !["proton", "files/bin/wine", "files/bin/wineserver"]
                .iter()
                .all(|p| nonempty(&path.join(p)))
        {
            continue;
        }
        let version = fs::read_to_string(path.join("version")).ok()?;
        if !version
            .split_whitespace()
            .any(|v| v.starts_with("proton-11."))
        {
            continue;
        }
        let text = fs::read_to_string(path.join("toolmanifest.vdf")).ok()?;
        let parsed = vdf::parse_text(&text).ok()?;
        let manifest = vdf::object(&parsed, "manifest")?;
        if !vdf::string(manifest, "commandline")?.contains("proton") {
            continue;
        }
        let Some(app) = vdf::string(manifest, "require_tool_appid") else {
            continue;
        };
        if app.is_empty() || !app.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let ready = info.libraries.iter().any(|lib| {
            complete_app(lib, app).is_some_and(|p| {
                nonempty(&p.join("toolmanifest.vdf")) && nonempty(&p.join("_v2-entry-point"))
            })
        });
        if !ready {
            return Some(app.into());
        }
    }
    None
}

/// Steam-distributed Proton has toolmanifest.vdf, not compatibilitytool.vdf.
/// Only accept Valve's release version format; custom builds keep their own manifest.
fn valve_proton(path: &Path, libraries: &BTreeSet<PathBuf>) -> Option<Proton> {
    let text = fs::read_to_string(path.join("toolmanifest.vdf")).ok()?;
    let manifest = vdf::parse_text(&text).ok()?;
    let m = vdf::object(&manifest, "manifest")?;
    if !vdf::string(m, "commandline")?.contains("proton") {
        return None;
    }
    let label = path.file_name()?.to_str()?;
    if label == "Proton - Experimental" {
        return Some(Proton {
            name: "proton_experimental".into(),
            label: label.into(),
        });
    }
    let version = fs::read_to_string(path.join("version")).ok()?;
    let release = version
        .split_whitespace()
        .find_map(|w| w.strip_prefix("proton-"))?;
    let major = release.split('.').next()?.parse::<u32>().ok()?;
    if major == 11 && !valve11_ready(path, libraries) {
        return None;
    }
    let name = if (8..=99).contains(&major) {
        format!("proton_{major}")
    } else if release.starts_with("7.0") {
        "proton_7".into()
    } else if release.starts_with("6.3") {
        "proton_63".into()
    } else {
        return None;
    };
    // Reject a custom version string pretending to be an unrelated installed tool.
    if !label.starts_with(&format!("Proton {major}")) {
        return None;
    }
    Some(Proton {
        name,
        label: label.into(),
    })
}

#[cfg(test)]
mod steam_scope_live_fixture {
    #[test]
    fn same_and_other_uid_guard_transactions() {
        if std::env::var("C5_STEAM_SCOPE_FIXTURE").as_deref() != Ok("1") {
            return;
        }
        assert_eq!(
            unsafe { libc::geteuid() },
            0,
            "disposable container root required for other-UID fixture"
        );
        assert!(!super::steam_running().unwrap());
        use std::io::BufRead;
        for (uid, expected) in [(65534, false), (0, true)] {
            let code=format!("import os,ctypes,time;os.setuid({uid});ctypes.CDLL(None).prctl(15,b'steam',0,0,0);print('ready',flush=True);time.sleep(10)");
            let mut child = std::process::Command::new("python3")
                .args(["-c", &code])
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let mut line = String::new();
            std::io::BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut line)
                .unwrap();
            assert_eq!(line.trim(), "ready");
            let detected = super::steam_running();
            let ffi = crate::wow_steam_state();
            let root =
                std::env::temp_dir().join(format!("candidate5-guard-{}-{uid}", std::process::id()));
            std::fs::create_dir(&root).unwrap();
            let target = root.join("test-shortcuts");
            std::fs::write(&target, b"original").unwrap();
            let mut journal = crate::transaction::Journal {
                id: "scope-check".into(),
                label: "disposable test".into(),
                done: false,
                changes: vec![
                    crate::transaction::Change::new(target.clone(), b"updated".to_vec()).unwrap(),
                ],
            };
            crate::wow_reset_cancel();
            let commit = crate::transaction::commit(&root, &mut journal, true);
            let bytes = std::fs::read(&target).unwrap();
            let _ = child.kill();
            let _ = child.wait();
            assert_eq!(detected.unwrap(), expected);
            assert_eq!(ffi, if expected { 1 } else { 0 });
            assert_eq!(commit.is_err(), expected);
            assert_eq!(
                bytes,
                if expected {
                    b"original".to_vec()
                } else {
                    b"updated".to_vec()
                }
            );
            std::fs::remove_dir_all(&root).unwrap();
            println!("SIMULATED process current_uid=0 fixture_uid={uid} steam_running={expected} ffi={ffi} file_write_blocked={expected}");
        }
        assert!(!super::steam_running().unwrap());
        assert_eq!(crate::wow_steam_state(), 0);
        println!("PASS post-exit detector and Qt FFI report stopped");
    }
}
