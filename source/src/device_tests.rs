//! Device checks read installation data; only their separate report is written.
//! Exported values are enums, counts and build IDs, never paths or raw errors.
use crate::{catalog, discovery, engine::VERSION, fsutil, model::*, steam, vdf, workflow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Attention,
    NotTested,
    Fail,
    Skipped,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoId {
    State,
    Recovery,
    Scan,
    Steam,
    Proton,
    Files,
    Shortcuts,
    Duplicates,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub id: AutoId,
    pub outcome: Outcome,
    pub count: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub id: String,
    pub outcome: Outcome,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum System {
    Bazzite,
    SteamOs,
    OtherLinux,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Session {
    Wayland,
    X11,
    Other,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Valve11,
    Valve10,
    Experimental,
    Cachy,
    Ge,
    Other,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub version: String,
    pub source: String,
    pub run: u64,
    pub system: System,
    pub session: Session,
    pub scoped: bool,
    pub installation_count: usize,
    pub game_count: usize,
    pub tools: Vec<Tool>,
    pub automatic: Vec<Check>,
    pub manual: Vec<Observation>,
    pub stale: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    report: Report,
    snapshot: String,
    scope: Option<String>,
}
pub const MANUAL: &[&str] = &[
    "startup",
    "controls",
    "navigation",
    "battle_net",
    "addons",
    "gaming_mode",
    "repair",
    "fresh_install",
    "clean_install",
    "restore",
];
fn source() -> String {
    env!("WOW_SOURCE_SHA256").into()
}
fn tool(name: &str) -> Tool {
    match name {
        "proton_11" | "proton_110" => Tool::Valve11,
        "proton_10" | "proton_100" => Tool::Valve10,
        "proton_experimental" => Tool::Experimental,
        _ if name.to_ascii_lowercase().contains("cachy") => Tool::Cachy,
        _ if name.to_ascii_lowercase().starts_with("ge-proton") => Tool::Ge,
        _ => Tool::Other,
    }
}
fn system(text: &str) -> System {
    let id = text
        .lines()
        .find_map(|l| l.strip_prefix("ID="))
        .unwrap_or("")
        .trim_matches('"');
    match id {
        "bazzite" => System::Bazzite,
        "steamos" => System::SteamOs,
        _ => System::OtherLinux,
    }
}
fn session(text: &str) -> Session {
    match text {
        "wayland" => Session::Wayland,
        "x11" => Session::X11,
        _ => Session::Other,
    }
}
fn safe_edition(id: &str) -> &str {
    if catalog::by_id(id).is_some() {
        id
    } else {
        "other"
    }
}
fn valid_manual(id: &str) -> bool {
    MANUAL.contains(&id)
        || id
            .strip_prefix("game_")
            .is_some_and(|e| e == "other" || catalog::by_id(e).is_some())
}
fn load_state(data: &Path) -> Result<State> {
    if !data.join("state.json").exists() {
        return Ok(State {
            schema: 1,
            ..State::default()
        });
    }
    let s: State = fsutil::load(&data.join("state.json"))?;
    workflow::state(&s)?;
    Ok(s)
}
// Freshness hashes stay in the private record, not in the exported report.
fn snapshot(home: &Path, data: &Path, state: &State) -> String {
    let mut paths = BTreeSet::new();
    for n in [
        "state.json",
        "transaction.json",
        "cleanup.json",
        "restore.json",
    ] {
        paths.insert(data.join(n));
    }
    for root in discovery::steam_roots(home) {
        paths.insert(root.join("config/config.vdf"));
        paths.insert(root.join("steamapps/libraryfolders.vdf"));
        if let Ok(info) = discovery::steam_info(&root) {
            for user in info.users {
                if let Ok(p) = steam::shortcuts_path(&root, &user) {
                    paths.insert(p);
                }
            }
        }
    }
    for i in &state.installations {
        paths.insert(data.join(format!("games-{}.json", i.id)));
        if let Some(p) = &i.launcher {
            paths.insert(p.clone());
        }
    }
    let mut bytes = Vec::new();
    for p in paths {
        bytes.extend_from_slice(p.as_os_str().as_encoded_bytes());
        bytes.push(0);
        match fs::metadata(&p) {
            Ok(m) => {
                bytes.extend_from_slice(&m.len().to_le_bytes());
                if let Ok(t) = m
                    .modified()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).map_err(std::io::Error::other))
                {
                    bytes.extend_from_slice(&t.as_nanos().to_le_bytes());
                }
            }
            Err(_) => bytes.push(255),
        }
    }
    fsutil::hash(&bytes)
}
fn tile_checks(
    scan: &Scan,
    state: &State,
    selected: Option<&Installation>,
) -> (usize, usize, usize, usize) {
    let mut bad = 0;
    let mut examined = 0;
    let mut duplicate = 0;
    let mut unreadable = 0;
    for info in &scan.steam {
        for user in &info.users {
            if selected.is_some_and(|i| i.steam != info.path || i.user != *user) {
                continue;
            }
            let parsed =
                steam::shortcuts_path(&info.path, user).and_then(|p| steam::read_shortcuts(&p));
            let Ok(root) = parsed else {
                bad += 1;
                unreadable += 1;
                continue;
            };
            let entries = vdf::object(&root, "shortcuts").unwrap();
            let relevant: Vec<_> = state
                .installations
                .iter()
                .filter(|i| {
                    i.steam == info.path
                        && i.user == *user
                        && selected.map_or(true, |s| s.id == i.id)
                })
                .collect();
            let cfg = fsutil::read(&info.path.join("config/config.vdf")).ok();
            let mut signatures = BTreeSet::new();
            let mut ids = BTreeSet::new();
            for (_, value) in entries {
                let vdf::Value::Object(e) = value else {
                    continue;
                };
                let target = vdf::string(e, "exe").unwrap_or("").trim_matches('"');
                let options = vdf::string(e, "LaunchOptions").unwrap_or("");
                let game = Path::new(target)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(catalog::is_game_exe);
                let routed = steam::product(options).is_some()
                    || vdf::string(e, "wowRustRole") == Some("shared_game");
                let in_scope = selected.map_or(true, |i| {
                    steam::owned(e, &i.id)
                        || steam::compat_prefix(options).as_ref() == Some(&i.prefix)
                        || Path::new(target).starts_with(&i.prefix)
                        || Path::new(target).starts_with(i.root.join("games"))
                });
                if in_scope
                    && (game || routed)
                    && !signatures.insert((target.to_string(), options.to_string()))
                {
                    duplicate += 1;
                }
                if let Some(i) = relevant.iter().find(|i| steam::owned(e, &i.id)) {
                    examined += 1;
                    let id = vdf::number(e, "appid");
                    if let Some(id) = id {
                        ids.insert(id);
                    }
                    let available = id
                        .and_then(|id| {
                            cfg.as_ref()
                                .and_then(|c| steam::proton_for(c, id).ok().flatten())
                        })
                        .is_some_and(|name| info.protons.iter().any(|p| p.name == name));
                    let route_ok = steam::native_addon(e)
                        || (steam::compat_prefix(options).as_ref() == Some(&i.prefix)
                            && available
                            && (vdf::string(e, "wowRustGame").is_none()
                                || i.launcher.as_ref().is_some_and(|p| p == Path::new(target))));
                    if id.is_none() || !Path::new(target).is_file() || !route_ok {
                        bad += 1;
                    }
                }
            }
            for i in relevant {
                for id in &i.shortcuts {
                    if !ids.contains(id) {
                        bad += 1;
                    }
                }
            }
        }
    }
    (bad, examined, duplicate, unreadable)
}
pub fn run(home: &Path, data: &Path, selected: Option<&str>) -> Result<Report> {
    let state_result = load_state(data);
    let state_ok = state_result.is_ok();
    let state = state_result.unwrap_or(State {
        schema: 1,
        ..State::default()
    });
    let scope = selected.and_then(|id| state.installations.iter().find(|i| i.id == id));
    if selected.is_some() && scope.is_none() && state_ok {
        return Err("That copy is no longer available. Return Home and scan again.".into());
    }
    let scan_result = discovery::scan(home, &state);
    crate::cancelled()?;
    let scan_ok = scan_result.is_ok();
    let scan = scan_result.unwrap_or_default();
    let pending = ["transaction.json", "cleanup.json", "restore.json"]
        .iter()
        .filter(|n| data.join(n).exists())
        .count();
    let games_result = scope.map_or_else(
        || Ok(scan.games.clone()),
        |i| crate::engine::device_games(home, data, &state, i, &scan),
    );
    let games_ok = games_result.is_ok();
    let games = games_result.unwrap_or_default();
    let mut editions: BTreeSet<String> = games
        .iter()
        .map(|g| safe_edition(&g.id).to_string())
        .collect();
    // Selected editions must remain visible even when their downloads are absent.
    if let Some(i) = scope {
        editions.extend(i.editions.iter().map(|e| safe_edition(e).to_string()));
    }
    let tools: BTreeSet<_> = if let Some(i) = scope {
        BTreeSet::from([tool(&i.proton)])
    } else {
        scan.steam
            .iter()
            .flat_map(|s| s.protons.iter().map(|p| tool(&p.name)))
            .collect()
    };
    let proton_ok = scope.map_or(!tools.is_empty(), |i| {
        scan.steam
            .iter()
            .any(|s| s.path == i.steam && s.protons.iter().any(|p| p.name == i.proton))
    });
    let (bad, examined, duplicate, unreadable) = tile_checks(&scan, &state, scope);
    let check = |id, yes, count| Check {
        id,
        outcome: if yes {
            Outcome::Pass
        } else {
            Outcome::Attention
        },
        count,
    };
    let mut automatic = vec![
        check(AutoId::State, state_ok, state.installations.len()),
        check(AutoId::Recovery, pending == 0, pending),
        check(
            AutoId::Scan,
            scan_ok && scan.warnings.is_empty(),
            scan.warnings.len(),
        ),
        check(AutoId::Steam, !scan.steam.is_empty(), scan.steam.len()),
        check(AutoId::Proton, proton_ok, tools.len()),
        Check {
            id: AutoId::Files,
            outcome: if !scan_ok || !games_ok {
                Outcome::Attention
            } else if games.is_empty() {
                Outcome::NotTested
            } else {
                Outcome::Pass
            },
            count: games.len(),
        },
        Check {
            id: AutoId::Shortcuts,
            outcome: if bad > 0 {
                Outcome::Attention
            } else if examined == 0 {
                Outcome::NotTested
            } else {
                Outcome::Pass
            },
            count: bad,
        },
        Check {
            id: AutoId::Duplicates,
            outcome: if !scan_ok || unreadable > 0 || !scan.warnings.is_empty() {
                Outcome::NotTested
            } else if duplicate > 0 {
                Outcome::Attention
            } else {
                Outcome::Pass
            },
            count: duplicate,
        },
    ];
    if !scan_ok {
        for c in automatic.iter_mut().skip(3) {
            c.outcome = Outcome::NotTested;
        }
    }
    let mut manual: Vec<_> = MANUAL
        .iter()
        .map(|id| Observation {
            id: (*id).into(),
            outcome: Outcome::NotTested,
        })
        .collect();
    manual.extend(editions.into_iter().map(|e| Observation {
        id: format!("game_{e}"),
        outcome: Outcome::NotTested,
    }));
    let report = Report {
        version: VERSION.into(),
        source: source(),
        run: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        system: system(&fs::read_to_string("/etc/os-release").unwrap_or_default()),
        session: session(&std::env::var("XDG_SESSION_TYPE").unwrap_or_default()),
        scoped: scope.is_some(),
        installation_count: scan.launchers.len(),
        game_count: games.len(),
        tools: tools.into_iter().collect(),
        automatic,
        manual,
        stale: false,
    };
    crate::cancelled()?;
    fsutil::json(
        &data.join("device-test.json"),
        &Saved {
            report: report.clone(),
            snapshot: snapshot(home, data, &state),
            scope: scope.map(|i| i.id.clone()),
        },
    )?;
    Ok(report)
}
fn load(data: &Path) -> Result<Saved> {
    let s: Saved = fsutil::load(&data.join("device-test.json"))
        .map_err(|_| "Saved test results could not be read. Start a new check.")?;
    if s.report.version != VERSION || s.report.source != source() {
        return Err("These results belong to another build. Start a new check.".into());
    }
    if s.report.manual.len() > MANUAL.len() + catalog::EDITIONS.len() + 1
        || s.report.manual.iter().any(|m| !valid_manual(&m.id))
        || s.report.automatic.len() != 8
    {
        return Err("Saved test results are invalid. Start a new check.".into());
    }
    Ok(s)
}
fn response(home: &Path, data: &Path, mut saved: Saved) -> Value {
    saved.report.stale = load_state(data)
        .map(|s| snapshot(home, data, &s) != saved.snapshot)
        .unwrap_or(true);
    let text = serde_json::to_string_pretty(&saved.report).unwrap();
    json!({"report":saved.report,"text":text})
}
pub fn dispatch(req: &Value) -> Result<Value> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("Home folder unavailable")?;
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"))
        .join("wow-steamos-rust");
    fsutil::private_dir(&data)
        .map_err(|_| "Test results cannot be saved in the app data folder.")?;
    let _lock = fsutil::Lock::acquire(&data.join("operation.lock"))?;
    dispatch_at(req, &home, &data)
}
fn dispatch_at(req: &Value, home: &Path, data: &Path) -> Result<Value> {
    match req["action"].as_str().unwrap_or("") {
        "device_run" => {
            run(home, data, req["id"].as_str().filter(|s| !s.is_empty()))?;
        }
        "device_result" => {
            if !data.join("device-test.json").exists() {
                return Ok(json!({"report":null,"text":""}));
            }
        }
        "device_mark" => {
            let mut saved = load(data)?;
            let id = req["test"].as_str().ok_or("Choose a test")?;
            let outcome: Outcome = serde_json::from_value(req["outcome"].clone())
                .map_err(|_| "Choose a listed result")?;
            if outcome == Outcome::Attention {
                return Err("Choose Worked, Problem, Not tested, or Skipped".into());
            }
            saved
                .report
                .manual
                .iter_mut()
                .find(|m| m.id == id)
                .ok_or("Choose a listed test")?
                .outcome = outcome;
            fsutil::json(&data.join("device-test.json"), &saved)
                .map_err(|_| "Test result could not be saved.")?;
        }
        _ => return Err("Unknown test action".into()),
    }
    Ok(response(home, data, load(data)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf) {
        let home = std::env::temp_dir().join(format!("wow-device-{}", fsutil::unique()));
        let data = home.join("data");
        fs::create_dir_all(&data).unwrap();
        (home, data)
    }
    #[test]
    fn report_uses_allowlisted_system_and_tool_names() {
        assert!(matches!(
            system("ID=bazzite\nPRETTY_NAME=private@example.test"),
            System::Bazzite
        ));
        assert!(matches!(
            system("ID=private@example.test"),
            System::OtherLinux
        ));
        assert!(matches!(session("private@example.test"), Session::Other));
        assert_eq!(tool("private@example.test"), Tool::Other);
        assert_eq!(safe_edition("private"), "other");
    }
    #[test]
    fn check_does_not_create_installation_or_pass_manual_tests() {
        let (h, d) = fixture();
        let r = run(&h, &d, None).unwrap();
        assert!(r.manual.iter().all(|m| m.outcome == Outcome::NotTested));
        assert!(!d.join("state.json").exists());
        assert!(!h.join("Games").exists());
        let exported = serde_json::to_string(&r).unwrap();
        assert!(!exported.contains(h.to_str().unwrap()));
        fs::remove_dir_all(h).unwrap();
    }
    #[test]
    fn damaged_state_is_reported_without_being_replaced() {
        let (h, d) = fixture();
        fs::write(d.join("state.json"), b"damaged private@example.test").unwrap();
        let r = run(&h, &d, None).unwrap();
        assert_eq!(r.automatic[0].outcome, Outcome::Attention);
        assert!(!serde_json::to_string(&r).unwrap().contains("private@"));
        assert_eq!(
            fs::read(d.join("state.json")).unwrap(),
            b"damaged private@example.test"
        );
        fs::remove_dir_all(h).unwrap();
    }
    #[test]
    fn pending_removal_is_only_observed() {
        let (h, d) = fixture();
        fs::write(d.join("cleanup.json"), b"untouched").unwrap();
        let r = run(&h, &d, None).unwrap();
        assert_eq!(r.automatic[1].outcome, Outcome::Attention);
        assert_eq!(fs::read(d.join("cleanup.json")).unwrap(), b"untouched");
        fs::remove_dir_all(h).unwrap();
    }
    #[test]
    fn manual_result_validation_and_build_freshness() {
        let (h, d) = fixture();
        run(&h, &d, None).unwrap();
        assert!(dispatch_at(
            &json!({"action":"device_mark","test":"private@example.test","outcome":"pass"}),
            &h,
            &d
        )
        .is_err());
        assert!(dispatch_at(
            &json!({"action":"device_mark","test":"controls","outcome":"attention"}),
            &h,
            &d
        )
        .is_err());
        let r = dispatch_at(
            &json!({"action":"device_mark","test":"controls","outcome":"pass"}),
            &h,
            &d,
        )
        .unwrap();
        assert_eq!(r["report"]["manual"][1]["outcome"], "pass");
        fs::write(d.join("transaction.json"), b"pending").unwrap();
        assert_eq!(
            dispatch_at(&json!({"action":"device_result"}), &h, &d).unwrap()["report"]["stale"],
            true
        );
        let mut s = load(&d).unwrap();
        s.report.source = "old".into();
        fsutil::json(&d.join("device-test.json"), &s).unwrap();
        assert!(load(&d).is_err());
        fs::remove_dir_all(h).unwrap();
    }
}
