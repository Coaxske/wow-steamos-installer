//! Domain invariants for persisted state and UI requests. No filesystem effects.
use crate::{catalog, fsutil, model::*, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    LauncherInstall,
    Games,
    TestGames,
    Addons,
    Complete,
    Removed,
}
impl Stage {
    pub fn advance(self, next: Self) -> Result<Self> {
        if matches!(
            (self, next),
            (Self::LauncherInstall, Self::Games)
                | (Self::Games, Self::TestGames)
                | (Self::TestGames, Self::Addons)
                | (Self::Addons, Self::Complete)
        ) {
            Ok(next)
        } else {
            Err("Finish the current setup step before continuing.".into())
        }
    }
}
pub fn token(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|c| c.is_ascii_hexdigit())
}
pub fn path(p: &Path) -> Result<()> {
    fsutil::safe_path(p)?;
    if !p.is_absolute()
        || p == Path::new("/")
        || p.as_os_str().len() > 4096
        || p.components().any(|c| matches!(c, Component::ParentDir))
    {
        return Err("Saved location is invalid.".into());
    }
    Ok(())
}
pub const LANGUAGES: &[&str] = &[
    "enUS", "enGB", "deDE", "esES", "esMX", "frFR", "itIT", "koKR", "plPL", "ptBR", "ruRU", "zhTW",
    "zhCN",
];
pub fn installation(i: &Installation) -> Result<()> {
    if i.launcher_exit_choice
        .as_deref()
        .is_some_and(|v| !["close_after_play", "keep_current"].contains(&v))
        || (i.launcher_exit_reviewed && i.launcher_exit_choice.is_none())
    {
        return Err("Saved Battle.net behavior choice is invalid.".into());
    }
    if !token(&i.id)
        || i.user.is_empty()
        || i.user == "0"
        || !i.user.bytes().all(|c| c.is_ascii_digit())
        || i.proton.is_empty()
        || i.proton.len() > 100
        || !i
            .proton
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_- .".contains(c))
    {
        return Err("Saved installation identity is invalid.".into());
    }
    for p in [&i.root, &i.prefix, &i.steam] {
        path(p)?;
    }
    if i.owned && i.prefix != i.root.join("compatdata") {
        return Err("Owned environment location is invalid.".into());
    }
    if let Some(p) = &i.launcher {
        path(p)?;
        if !p.starts_with(&i.prefix) {
            return Err("Launcher is outside its environment.".into());
        }
    }
    if !["", "none", "curseforge", "wowup_cf", "wowup_wago"].contains(&i.addon_manager.as_str()) {
        return Err("Unknown addon-manager selection".into());
    }
    if i.game_associations.len() > 100
        || i.bindings
            .iter()
            .any(|b| b.shared && (b.launcher || b.edition.is_some() || b.game.is_some()))
    {
        return Err("Saved shared tile associations are invalid.".into());
    }
    for game in &i.game_associations {
        path(&game.path)?;
        path(&game.executable)?;
        if game.executable.parent() != Some(game.path.as_path())
            || (catalog::by_id(&game.id).is_none() && game.id != "other")
        {
            return Err("Saved game association is invalid.".into());
        }
    }
    let editions: BTreeSet<_> = i.editions.iter().collect();
    if editions.len() != i.editions.len()
        || editions.len() > 100
        || editions
            .iter()
            .any(|e| catalog::by_id(e).is_none() && e.as_str() != "other")
    {
        return Err("Saved editions are invalid.".into());
    }
    if !LANGUAGES.contains(&i.language.as_str())
        || i.restore_backup.as_deref().is_some_and(|s| !token(s))
    {
        return Err("Saved setup options are invalid.".into());
    }
    Ok(())
}
pub fn state(s: &State) -> Result<()> {
    if s.schema != 1
        || s.installations.len() > 100
        || s.extra_roots.len() > 100
        || s.tests.len() > 10000
    {
        return Err("Saved state is unsupported or damaged; it was not replaced.".into());
    }
    let mut ids = BTreeSet::new();
    let mut prefixes = BTreeSet::new();
    for i in &s.installations {
        installation(i)?;
        if !ids.insert(&i.id) || !prefixes.insert(&i.prefix) {
            return Err("Saved installations have duplicate identities.".into());
        }
    }
    if s.current.as_ref().is_some_and(|id| !ids.contains(id)) {
        return Err("Selected installation is missing from saved state.".into());
    }
    for p in &s.extra_roots {
        path(p)?;
    }
    Ok(())
}
pub fn request(v: &Value) -> Result<()> {
    let o = v.as_object().ok_or("Expected a request object")?;
    if v.to_string().len() > 65536 {
        return Err("Request is too large".into());
    }
    for (k, v) in o {
        if let Some(s) = v.as_str() {
            if s.len() > 4096 || s.chars().any(|c| c.is_control()) {
                return Err(format!("Invalid {k}"));
            }
        }
    }
    for k in ["keep_settings", "allow_missing", "confirm_forever_beta"] {
        if o.contains_key(k) && !v[k].is_boolean() {
            return Err(format!("{k} must be true or false"));
        }
    }
    for k in ["editions", "selected"] {
        if let Some(a) = o.get(k) {
            let a = a.as_array().ok_or("Selections must be lists")?;
            let mut seen = BTreeSet::new();
            if a.len() > 100 {
                return Err("Too many selections".into());
            }
            for item in a {
                if (k == "editions" && !item.is_string())
                    || (k == "selected"
                        && item.as_u64().and_then(|n| u32::try_from(n).ok()).is_none())
                    || !seen.insert(item.to_string())
                {
                    return Err("Invalid or duplicate selection".into());
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_stages() {
        assert!(Stage::LauncherInstall.advance(Stage::Complete).is_err());
        assert_eq!(
            Stage::Games.advance(Stage::TestGames).unwrap(),
            Stage::TestGames
        );
        assert!(Stage::Complete.advance(Stage::Games).is_err());
    }
    #[test]
    fn invalid_stage_rejected_by_deserialization() {
        assert!(serde_json::from_str::<Stage>("\"delete\"").is_err());
    }
    #[test]
    fn answers_are_not_coerced() {
        for v in [
            serde_json::json!({"keep_settings":"yes"}),
            serde_json::json!({"editions":["retail","retail"]}),
            serde_json::json!({"selected":[-1]}),
            serde_json::json!({"editions":[{}]}),
        ] {
            assert!(request(&v).is_err());
        }
    }
    #[test]
    fn traversing_path_rejected() {
        assert!(path(Path::new("/home/x/../other")).is_err());
    }
}
