//! Read-only help for an optional, manually installed addon manager.
use crate::model::Game;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize)]
pub struct Folder {
    pub edition: String,
    pub game_folder: Option<PathBuf>,
    pub addons_folder: Option<PathBuf>,
    pub available: bool,
    pub message: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Guide {
    pub selected_by_default: bool,
    pub folders: Vec<Folder>,
    pub message: String,
}
fn edition(id: &str) -> &'static str {
    match id {
        "retail" => "Retail",
        "era" => "Classic Era",
        "forever" => "Forever",
        "progression" => "Classic Progression",
        "classic_beta" => "Classic Beta",
        "ptr" => "Retail PTR",
        _ => "Other detected edition",
    }
}
/// Describe a fresh detector result. No folder or addon content is created.
pub fn describe(games: &[Game]) -> Guide {
    let mut folders = Vec::new();
    let mut seen = BTreeSet::new();
    for game in games {
        let label = edition(&game.id).to_string();
        let verified = game.path.canonicalize().ok().and_then(|folder| {
            let executable = game.executable.canonicalize().ok()?;
            if !folder.is_dir() || !executable.is_file() || executable.parent()? != folder.as_path()
            {
                return None;
            }
            Some((folder, executable))
        });
        let Some((folder, executable)) = verified else {
            if seen.insert((game.path.clone(), game.executable.clone())) {
                folders.push(Folder { edition:label, game_folder:None, addons_folder:None, available:false,
                    message:"This installation is unavailable. Reconnect its drive and refresh detected folders.".into() });
            }
            continue;
        };
        if !seen.insert((folder.clone(), executable)) {
            continue;
        }
        let addons =
            crate::folder_links::child_directory(&folder, "Interface").and_then(|p| match p {
                Some(p) => crate::folder_links::child_directory(&p, "AddOns"),
                None => Ok(None),
            });
        let (addons_folder,message) = match addons {
            Ok(Some(path))=>(Some(path),"Use this edition's AddOns folder when an addon app asks for it.".into()),
            Ok(None)=>(None,"No existing AddOns folder was found. Open this edition once, quit normally, then refresh detected folders. Deckhand has not created a folder or guessed its path.".into()),
            Err(_)=>(None,"The AddOns folder could not be identified safely. Reconnect the drive or check for conflicting folder names, then refresh detected folders. No path was guessed.".into()),
        };
        folders.push(Folder {
            edition: label,
            game_folder: Some(folder),
            available: addons_folder.is_some(),
            addons_folder,
            message,
        });
    }
    folders.sort_by(|a, b| {
        a.edition
            .cmp(&b.edition)
            .then(a.game_folder.cmp(&b.game_folder))
    });
    Guide { selected_by_default:false, message:if folders.is_empty() {
        "No installed WoW edition was detected. Finish installing a game or reconnect its drive, then refresh detected folders."
    } else { "These paths were checked when this guide opened. Refresh detected folders if a drive or installation changes." }.into(), folders }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "deckhand-guide-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn game(&self, id: &str, name: &str, addons: bool) -> Game {
            let folder = self.0.join(name);
            fs::create_dir_all(&folder).unwrap();
            let executable = folder.join("Wow.exe");
            fs::write(&executable, b"synthetic detector fixture").unwrap();
            if addons {
                fs::create_dir_all(folder.join("Interface/AddOns")).unwrap();
            }
            Game {
                id: id.into(),
                name: "Synthetic detected game".into(),
                path: folder,
                executable,
                prefix: Some(self.0.join("separate-prefix")),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn optional_default_does_not_select_a_manager() {
        let g = describe(&[]);
        assert!(!g.selected_by_default);
        assert!(g.folders.is_empty());
        assert!(!describe(&[]).selected_by_default);
    }
    #[test]
    fn each_edition_and_separate_copy_use_actual_paths() {
        let f = Fixture::new();
        let games = [
            f.game("retail", "drive-a/retail", true),
            f.game("era", "drive-b/era", true),
            f.game("forever", "drive-b/forever", true),
            f.game("retail", "drive-c/retail", true),
        ];
        let g = describe(&games);
        assert_eq!(g.folders.len(), 4);
        for row in &g.folders {
            assert!(row.available);
            let path = row.addons_folder.as_ref().unwrap();
            assert!(path.is_dir());
            assert_eq!(
                *path,
                row.game_folder.as_ref().unwrap().join("Interface/AddOns")
            );
        }
        assert_eq!(
            g.folders.iter().filter(|r| r.edition == "Retail").count(),
            2
        );
        assert!(g.folders.iter().any(|r| r.edition == "Classic Era"));
        assert!(g.folders.iter().any(|r| r.edition == "Forever"));
    }
    #[test]
    fn missing_addons_are_not_created_or_guessed() {
        let f = Fixture::new();
        let game = f.game("era", "client", false);
        let g = describe(std::slice::from_ref(&game));
        assert!(!g.folders[0].available);
        assert!(g.folders[0].addons_folder.is_none());
        assert!(!game.path.join("Interface").exists());
        assert!(game.executable.is_file());
    }
    #[test]
    fn unavailable_drive_reports_no_path() {
        let f = Fixture::new();
        let game = f.game("forever", "client", true);
        fs::remove_file(&game.executable).unwrap();
        let g = describe(&[game]);
        assert!(!g.folders[0].available);
        assert!(g.folders[0].game_folder.is_none());
        assert!(g.folders[0].addons_folder.is_none());
    }
    #[test]
    fn ambiguous_case_names_refuse_a_guess() {
        let f = Fixture::new();
        let game = f.game("retail", "client", true);
        fs::create_dir(game.path.join("Interface/addons")).unwrap();
        let g = describe(&[game]);
        assert!(!g.folders[0].available);
        assert!(g.folders[0].addons_folder.is_none());
    }
    #[test]
    fn actual_symlink_addon_destination_and_sample_are_preserved() {
        let f = Fixture::new();
        let game = f.game("era", "client", false);
        let target = f.0.join("connected-drive/addons");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("ExampleAddon.txt"), b"keep").unwrap();
        fs::create_dir(game.path.join("Interface")).unwrap();
        symlink(&target, game.path.join("Interface/AddOns")).unwrap();
        let g = describe(&[game]);
        assert_eq!(g.folders[0].addons_folder, Some(target));
        assert_eq!(
            fs::read(
                g.folders[0]
                    .addons_folder
                    .as_ref()
                    .unwrap()
                    .join("ExampleAddon.txt")
            )
            .unwrap(),
            b"keep"
        );
    }
    #[test]
    fn duplicate_alias_reused_and_escaped_executable_unavailable() {
        let f = Fixture::new();
        let game = f.game("retail", "client", true);
        let mut alias = game.clone();
        alias.path = game.path.join(".");
        assert_eq!(describe(&[game.clone(), alias]).folders.len(), 1);
        let other = f.game("retail", "other", true);
        let mut mismatch = game;
        mismatch.executable = other.executable;
        assert!(!describe(&[mismatch]).folders[0].available);
    }
    #[test]
    fn case_insensitive_addons_used_without_renaming() {
        let f = Fixture::new();
        let game = f.game("era", "client", false);
        fs::create_dir_all(game.path.join("interface/addons")).unwrap();
        let g = describe(std::slice::from_ref(&game));
        assert_eq!(
            g.folders[0].addons_folder,
            Some(game.path.join("interface/addons"))
        );
        assert!(!game.path.join("Interface").exists());
    }
}
