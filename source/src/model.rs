use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub schema: u32,
    pub installations: Vec<Installation>,
    pub extra_roots: Vec<PathBuf>,
    pub current: Option<String>,
    pub addon_pending: bool,
    pub tests: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installation {
    #[serde(default)]
    pub launcher_exit_choice: Option<String>,
    #[serde(default)]
    pub launcher_exit_reviewed: bool,
    pub id: String,
    pub root: PathBuf,
    pub prefix: PathBuf,
    pub steam: PathBuf,
    pub user: String,
    pub proton: String,
    pub editions: Vec<String>,
    #[serde(default = "default_launcher_tile")]
    pub launcher_tile: bool,
    #[serde(default)]
    pub addon_manager: String,
    pub stage: crate::workflow::Stage,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub restore_backup: Option<String>,
    #[serde(default)]
    pub addon_pending: bool,
    pub launcher: Option<PathBuf>,
    pub bootstrap_id: u32,
    pub owned: bool,
    pub shortcuts: Vec<u32>,
    #[serde(default)]
    pub bindings: Vec<ShortcutBinding>,
    #[serde(default)]
    pub game_associations: Vec<Game>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub executable: PathBuf,
    pub prefix: Option<PathBuf>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Launcher {
    pub path: PathBuf,
    pub prefix: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Proton {
    pub name: String,
    pub label: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Steam {
    pub path: PathBuf,
    pub users: Vec<String>,
    pub libraries: Vec<PathBuf>,
    pub protons: Vec<Proton>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scan {
    pub steam: Vec<Steam>,
    pub games: Vec<Game>,
    pub launchers: Vec<Launcher>,
    pub warnings: Vec<String>,
    pub steam_running: bool,
}

fn default_language() -> String {
    "enUS".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShortcutBinding {
    #[serde(default)]
    pub shared: bool,
    pub appid: u32,
    pub edition: Option<String>,
    pub game: Option<String>,
    pub launcher: bool,
}

fn default_launcher_tile() -> bool {
    true
}
