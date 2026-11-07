use crate::{discovery, fsutil, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub path: PathBuf,
    #[serde(with = "optional_bytes")]
    pub before: Option<Vec<u8>>,
    #[serde(with = "bytes")]
    pub after: Vec<u8>,
}
impl Change {
    pub fn new(path: PathBuf, after: Vec<u8>) -> Result<Self> {
        let before = match fs::read(&path) {
            Ok(x) => Some(x),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        Ok(Self {
            path,
            before,
            after,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Journal {
    pub id: String,
    pub label: String,
    pub changes: Vec<Change>,
    pub done: bool,
}
pub fn verify(changes: &[Change], allow_after: bool) -> Result<()> {
    for c in changes {
        let now = fsutil::fingerprint(&c.path)?;
        let before = c.before.as_ref().map(|b| fsutil::hash(b));
        if now != before && (!allow_after || now != Some(fsutil::hash(&c.after))) {
            return Err(format!(
                "{} changed since review. Scan again before applying changes.",
                c.path.display()
            ));
        }
    }
    Ok(())
}
pub fn commit(data: &Path, j: &mut Journal, live: bool) -> Result<()> {
    if live {
        discovery::require_stopped()?;
    }
    verify(&j.changes, false)?;
    crate::cancelled()?;
    let pending = data.join("transaction.json");
    if pending.exists() {
        return Err("A saved transaction must be recovered first".into());
    }
    fsutil::json(&pending, j)?;
    finish(data, j, live)
}
pub fn finish(data: &Path, j: &mut Journal, live: bool) -> Result<()> {
    if live {
        discovery::require_stopped()?;
    }
    verify(&j.changes, true)?;
    for c in &j.changes {
        if live {
            discovery::require_stopped()?;
        }
        let now = fsutil::fingerprint(&c.path)?;
        if now == Some(fsutil::hash(&c.after)) {
            continue;
        }
        if now != c.before.as_ref().map(|b| fsutil::hash(b)) {
            return Err("A file changed during the transaction. Close Steam and recover.".into());
        }
        fsutil::atomic(&c.path, &c.after)?;
    }
    j.done = true;
    let backups = data.join("backups");
    fsutil::private_dir(&backups)?;
    fsutil::json(&backups.join(format!("{}.json", j.id)), j)?;
    fsutil::json(&data.join("last-transaction.json"), j)?;
    if data.join("transaction.json").exists() {
        fs::remove_file(data.join("transaction.json")).map_err(|e| e.to_string())?;
    }
    prune(data, &j.id)?;
    Ok(())
}
pub fn recover(data: &Path, live: bool) -> Result<()> {
    let mut j: Journal = fsutil::load(&data.join("transaction.json"))?;
    finish(data, &mut j, live)
}
pub fn undo(data: &Path, live: bool) -> Result<()> {
    let old: Journal = fsutil::load(&data.join("last-transaction.json"))?;
    if old.label.contains("reinstall") || old.label.contains("Undo") {
        return Err("This operation cannot be undone by restoring Steam settings.".into());
    }
    let mut changes = vec![];
    for c in old.changes {
        let before = c.before.ok_or("This backup predates a newly created file. Keep the current file and use shortcut repair instead.")?;
        let current = fsutil::read(&c.path)?;
        let restored = if c.path == data.join("state.json") {
            crate::state_merge::undo(&c.after, &before, &current)?
        } else {
            crate::merge::apply(&c.path, &c.after, &before, &current)?
        };
        changes.push(Change::new(c.path, restored)?);
    }
    let mut j = Journal {
        id: fsutil::unique(),
        label: "Undo Steam configuration".into(),
        changes,
        done: false,
    };
    commit(data, &mut j, live)
}

/// Rebase a paused proposal before writing, keeping concurrent unrelated Steam edits.
pub fn rebase(j: &mut Journal) -> Result<()> {
    let mut changes = vec![];
    for c in &j.changes {
        let current = std::fs::read(&c.path).ok();
        let after = if current == c.before || current.as_deref() == Some(c.after.as_slice()) {
            c.after.clone()
        } else {
            crate::merge::apply(
                &c.path,
                c.before
                    .as_deref()
                    .ok_or("A new file appeared after review")?,
                &c.after,
                current.as_deref().ok_or("A reviewed file disappeared")?,
            )?
        };
        changes.push(Change {
            path: c.path.clone(),
            before: current,
            after,
        });
    }
    j.changes = changes;
    Ok(())
}
fn prune(data: &Path, keep: &str) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(data.join("backups"))
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_ok_and(|t| t.is_file())
                && e.path().extension().is_some_and(|x| x == "json")
        })
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
    let mut total = 0u64;
    for (n, e) in entries.into_iter().enumerate() {
        total = total.saturating_add(e.metadata().map_err(|e| e.to_string())?.len());
        if (n >= 20 || total > 64 * 1024 * 1024) && e.file_name() != format!("{keep}.json").as_str()
        {
            std::fs::remove_file(e.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
// Compact hex writes; old alpha JSON number arrays remain readable.
mod bytes {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        use std::fmt::Write;
        let mut hex = String::with_capacity(v.len() * 2);
        for b in v {
            write!(&mut hex, "{b:02x}").unwrap();
        }
        s.serialize_str(&hex)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Hex(String),
            Old(Vec<u8>),
        }
        match Repr::deserialize(d)? {
            Repr::Old(v) => Ok(v),
            Repr::Hex(h) => {
                if h.len() % 2 != 0 || !h.is_ascii() {
                    return Err(serde::de::Error::custom("Invalid byte backup"));
                }
                (0..h.len())
                    .step_by(2)
                    .map(|n| u8::from_str_radix(&h[n..n + 2], 16).map_err(serde::de::Error::custom))
                    .collect()
            }
        }
    }
}
mod optional_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    #[derive(Serialize, Deserialize)]
    struct Bytes(#[serde(with = "super::bytes")] Vec<u8>);
    pub fn serialize<S: Serializer>(v: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        v.as_ref().map(|v| Bytes(v.clone())).serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        Ok(Option::<Bytes>::deserialize(d)?.map(|b| b.0))
    }
}
