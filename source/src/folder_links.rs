//! Local folder links. Never grants ownership of, moves, or deletes a link target.
use crate::{fsutil, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    ffi::{CString, OsString},
    fs,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::{OsStrExt, OsStringExt},
    },
    path::{Path, PathBuf},
};

const ROOT: &str = "World of Warcraft";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    uid: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Root {
    identity: Identity,
    staging: Option<String>,
    #[serde(default)]
    quarantine: Option<(String, Identity)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Link {
    name: String,
    label: String,
    target: PathBuf,
    identity: Identity,
    owners: Vec<String>,
    staging: Option<String>,
    removing: Option<String>,
}
#[derive(Default, Serialize, Deserialize)]
struct Ledger {
    schema: u32,
    root: Option<Root>,
    links: Vec<Link>,
}
#[derive(Clone)]
pub struct Target {
    pub label: String,
    pub path: PathBuf,
}
struct Directory {
    home: fs::File,
    dir: fs::File,
    identity: Identity,
}
fn c(s: &std::ffi::OsStr) -> Result<CString> {
    CString::new(s.as_bytes()).map_err(|_| "Invalid folder path".into())
}
fn name(s: &str) -> Result<CString> {
    if s.is_empty() || s == "." || s == ".." || s.contains('/') || s.as_bytes().len() > 240 {
        return Err("Invalid folder link name".into());
    }
    CString::new(s).map_err(|_| "Invalid folder link name".into())
}
fn error() -> String {
    std::io::Error::last_os_error().to_string()
}
fn open(path: &Path) -> Result<fs::File> {
    let path = c(path.as_os_str())?;
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(error());
    }
    Ok(unsafe { fs::File::from_raw_fd(fd) })
}
fn child(dir: &fs::File, n: &str) -> Result<fs::File> {
    let n = name(n)?;
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            n.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(error());
    }
    Ok(unsafe { fs::File::from_raw_fd(fd) })
}
fn stat(dir: &fs::File, n: &str) -> Result<Option<(Identity, u32)>> {
    let n = name(n)?;
    let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            dir.as_raw_fd(),
            n.as_ptr(),
            st.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        let e = std::io::Error::last_os_error();
        return if e.kind() == std::io::ErrorKind::NotFound {
            Ok(None)
        } else {
            Err(e.to_string())
        };
    }
    let st = unsafe { st.assume_init() };
    Ok(Some((
        Identity {
            dev: st.st_dev,
            ino: st.st_ino,
            uid: st.st_uid,
        },
        st.st_mode & libc::S_IFMT,
    )))
}
fn stat_fd(file: &fs::File) -> Result<(Identity, u32)> {
    let mut raw = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(file.as_raw_fd(), raw.as_mut_ptr()) } != 0 {
        return Err(error());
    }
    let st = unsafe { raw.assume_init() };
    Ok((
        Identity {
            dev: st.st_dev,
            ino: st.st_ino,
            uid: st.st_uid,
        },
        st.st_mode & libc::S_IFMT,
    ))
}
fn readlink(dir: &fs::File, n: &str) -> Result<PathBuf> {
    let n = name(n)?;
    let mut b = vec![0u8; 16384];
    let count =
        unsafe { libc::readlinkat(dir.as_raw_fd(), n.as_ptr(), b.as_mut_ptr().cast(), b.len()) };
    if count < 0 || count as usize == b.len() {
        return Err("Folder link could not be verified".into());
    }
    b.truncate(count as usize);
    Ok(PathBuf::from(OsString::from_vec(b)))
}
fn rename(dir: &fs::File, old: &str, new: &str) -> Result<()> {
    let a = name(old)?;
    let b = name(new)?;
    if unsafe {
        libc::renameat2(
            dir.as_raw_fd(),
            a.as_ptr(),
            dir.as_raw_fd(),
            b.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err(error());
    }
    Ok(())
}
fn rename_between(from: &fs::File, old: &str, to: &fs::File, new: &str) -> Result<()> {
    let a = name(old)?;
    let b = name(new)?;
    if unsafe {
        libc::renameat2(
            from.as_raw_fd(),
            a.as_ptr(),
            to.as_raw_fd(),
            b.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err(error());
    }
    Ok(())
}
fn link_matches(dir: &fs::File, n: &str, row: &Link) -> Result<bool> {
    Ok(
        stat(dir, n)?.as_ref() == Some(&(row.identity.clone(), libc::S_IFLNK))
            && readlink(dir, n)? == row.target,
    )
}
fn quarantine(data: &Path, d: &mut Ledger, dir: &Directory) -> Result<fs::File> {
    dir.attached()?;
    if d.root.as_ref().unwrap().quarantine.is_none() {
        let tmp = format!(".wow-link-private-{}", fsutil::unique());
        let n = name(&tmp)?;
        if unsafe { libc::mkdirat(dir.home.as_raw_fd(), n.as_ptr(), 0o700) } != 0 {
            return Err(error());
        }
        let file = child(&dir.home, &tmp)?;
        let (identity, kind) = stat_fd(&file)?;
        if kind != libc::S_IFDIR
            || stat(&dir.home, &tmp)?.as_ref() != Some(&(identity.clone(), libc::S_IFDIR))
        {
            return Err("Private link quarantine changed".into());
        }
        d.root.as_mut().unwrap().quarantine = Some((tmp, identity));
        save(data, d)?;
    }
    let (n, id) = d.root.as_ref().unwrap().quarantine.as_ref().unwrap();
    if !n.starts_with(".wow-link-private-")
        || stat(&dir.home, n)?.as_ref() != Some(&(id.clone(), libc::S_IFDIR))
    {
        return Err("Private link quarantine identity changed".into());
    }
    let file = child(&dir.home, n)?;
    if stat_fd(&file)? != (id.clone(), libc::S_IFDIR) || id.dev != dir.identity.dev {
        return Err("Private link quarantine is not on the verified filesystem".into());
    }
    use std::os::unix::fs::PermissionsExt;
    if file
        .metadata()
        .map_err(|e| e.to_string())?
        .permissions()
        .mode()
        & 0o077
        != 0
    {
        return Err("Private link quarantine permissions changed; links were preserved".into());
    }
    Ok(file)
}
fn verified_unlink(dir: &fs::File, n: &str, row: &Link) -> Result<()> {
    if !link_matches(dir, n, row)? {
        return Err("A replaced quarantine entry was preserved, not unlinked".into());
    }
    unlink(dir, n, 0)
}
fn unlink(dir: &fs::File, n: &str, flags: i32) -> Result<()> {
    let n = name(n)?;
    if unsafe { libc::unlinkat(dir.as_raw_fd(), n.as_ptr(), flags) } != 0 {
        return Err(error());
    }
    Ok(())
}
fn db(data: &Path) -> PathBuf {
    data.join("folder-links.json")
}
fn load(data: &Path) -> Result<Ledger> {
    let p = db(data);
    match fs::symlink_metadata(&p) {
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
            return Err("Folder-link ownership record is not a regular private file".into())
        }
        Ok(m) if m.len() > 4 * 1024 * 1024 => {
            return Err("Folder-link ownership record exceeds its limit".into())
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Ledger {
                schema: 1,
                ..Ledger::default()
            })
        }
        Err(e) => return Err(e.to_string()),
    }
    let d: Ledger = fsutil::load(&p)?;
    if d.schema != 1 || d.links.len() > 2048 {
        return Err("Invalid folder-link ownership record".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for l in &d.links {
        name(&l.name)?;
        if !seen.insert(l.name.clone()) || !l.target.is_absolute() {
            return Err("Invalid or duplicate folder-link record".into());
        }
        for n in [&l.staging, &l.removing].into_iter().flatten() {
            name(n)?;
            if !n.starts_with(".wow-") {
                return Err("Invalid pending folder-link name".into());
            }
        }
    }
    Ok(d)
}
fn save(data: &Path, d: &Ledger) -> Result<()> {
    fsutil::json(&db(data), d)
}
fn root(data: &Path, home: &Path, d: &mut Ledger, create: bool) -> Result<Directory> {
    let home = open(&fsutil::canonical(home)?)?;
    if d.root.is_none() {
        if !create {
            return Err("The access folder has not been created yet".into());
        }
        if stat(&home, ROOT)?.is_some() {
            return Err("The home World of Warcraft name is occupied by content this installer does not own; nothing was overwritten".into());
        }
        let tmp = format!(".wow-folders-{}", fsutil::unique());
        let n = name(&tmp)?;
        if unsafe { libc::mkdirat(home.as_raw_fd(), n.as_ptr(), 0o755) } != 0 {
            return Err(error());
        }
        let (identity, kind) = stat(&home, &tmp)?.ok_or("New access folder disappeared")?;
        if kind != libc::S_IFDIR {
            return Err("New access folder changed".into());
        }
        if stat_fd(&child(&home, &tmp)?)? != (identity.clone(), libc::S_IFDIR) {
            return Err("New access folder descriptor changed".into());
        }
        d.root = Some(Root {
            identity,
            staging: Some(tmp),
            quarantine: None,
        });
        save(data, d)?;
    }
    let saved = d.root.as_ref().unwrap().clone();
    if let Some(tmp) = &saved.staging {
        let final_id = stat(&home, ROOT)?;
        if final_id
            .as_ref()
            .is_some_and(|(id, k)| id == &saved.identity && *k == libc::S_IFDIR)
        {
            if create {
                d.root.as_mut().unwrap().staging = None;
                save(data, d)?;
            }
        } else {
            if !create {
                return Err("Access folder preparation is unfinished".into());
            }
            if stat(&home, tmp)?.as_ref() != Some(&(saved.identity.clone(), libc::S_IFDIR)) {
                return Err("Pending access folder changed; preserved without adoption".into());
            }
            rename(&home, tmp, ROOT).map_err(|_| {
                "Access folder name became occupied; existing content was preserved"
            })?;
            d.root.as_mut().unwrap().staging = None;
            save(data, d)?;
        }
    }
    if stat(&home, ROOT)?.as_ref() != Some(&(saved.identity.clone(), libc::S_IFDIR)) {
        return Err("The access folder was replaced or moved; no links changed".into());
    }
    let dir = child(&home, ROOT)?;
    if stat_fd(&dir)? != (saved.identity.clone(), libc::S_IFDIR) {
        return Err("Opened access-folder identity changed; no links changed".into());
    }
    Ok(Directory {
        home,
        dir,
        identity: saved.identity,
    })
}
impl Directory {
    fn attached(&self) -> Result<()> {
        if stat_fd(&self.dir)? != (self.identity.clone(), libc::S_IFDIR) {
            return Err("Access-folder descriptor identity differs; stopped".into());
        }
        if stat(&self.home, ROOT)?.as_ref() != Some(&(self.identity.clone(), libc::S_IFDIR)) {
            return Err("The access folder changed during the operation; stopped".into());
        }
        Ok(())
    }
    fn matches(&self, n: &str, l: &Link) -> Result<bool> {
        Ok(
            stat(&self.dir, n)?.as_ref() == Some(&(l.identity.clone(), libc::S_IFLNK))
                && readlink(&self.dir, n)? == l.target,
        )
    }
}
fn finish_create(data: &Path, d: &mut Ledger, dir: &Directory, n: usize) -> Result<()> {
    let row = d.links[n].clone();
    if let Some(tmp) = row.staging.as_deref() {
        dir.attached()?;
        if !dir.matches(&row.name, &row)? {
            if !dir.matches(tmp, &row)? {
                return Err("Pending folder link changed; no user content was replaced".into());
            }
            rename(&dir.dir, tmp, &row.name)
                .map_err(|_| "A folder-link name is occupied; content was not overwritten")?;
        }
        d.links[n].staging = None;
        save(data, d)?;
    }
    Ok(())
}
fn remove_entry(data: &Path, d: &mut Ledger, dir: &Directory, n: usize) -> Result<()> {
    let row = d.links[n].clone();
    let final_matches = dir.matches(&row.name, &row)?;
    let staged_matches = if let Some(tmp) = row.staging.as_deref() {
        dir.matches(tmp, &row)?
    } else {
        false
    };
    if final_matches && staged_matches {
        return Err("Two matching pending link names need review; neither was unlinked".into());
    }
    let live = if final_matches {
        row.name.as_str()
    } else if staged_matches {
        row.staging.as_deref().unwrap()
    } else {
        row.name.as_str()
    };
    let q = row
        .removing
        .clone()
        .unwrap_or_else(|| format!(".wow-remove-{}", fsutil::unique()));
    let private = quarantine(data, d, dir)?;
    dir.attached()?;
    if stat(&private, &q)?.is_none() {
        if stat(&dir.dir, live)?.is_none()
            && row
                .staging
                .as_deref()
                .map(|n| stat(&dir.dir, n))
                .transpose()?
                .flatten()
                .is_none()
        {
            d.links.remove(n);
            save(data, d)?;
            return Ok(());
        }
        if !dir.matches(live, &row)? {
            return Err("A replaced folder link or user file was preserved".into());
        }
        d.links[n].removing = Some(q.clone());
        save(data, d)?;
        rename_between(&dir.dir, live, &private, &q)?;
    }
    dir.attached()?;
    // Final no-follow verification occurs in a pinned private directory. This does not claim immunity to malicious same-UID edits.
    if let Err(e) = verified_unlink(&private, &q, &row) {
        let _ = rename_between(&private, &q, &dir.dir, live);
        return Err(e);
    }
    d.links.remove(n);
    save(data, d)?;
    Ok(())
}
pub fn sync(data: &Path, home: &Path, owner: &str, targets: &[Target]) -> Result<Value> {
    let mut d = load(data)?;
    let dir = root(data, home, &mut d, true)?;
    let mut messages = vec![];
    for n in (0..d.links.len()).rev() {
        let result = if d.links[n].removing.is_some() {
            remove_entry(data, &mut d, &dir, n)
        } else {
            finish_create(data, &mut d, &dir, n)
        };
        if let Err(e) = result {
            messages.push(e)
        }
    }
    let mut targets = targets.to_vec();
    targets.sort_by(|a, b| (&a.label, &a.path).cmp(&(&b.label, &b.path)));
    targets.dedup_by(|a, b| a.label == b.label && a.path == b.path);
    for target in targets {
        name(&target.label)?;
        let target_path = match target.path.canonicalize() {
            Ok(p) if p.is_dir() => p,
            _ => {
                messages.push(format!(
                    "{} is unavailable; its existing link is preserved",
                    target.label
                ));
                continue;
            }
        };
        if target_path.starts_with(home.join(ROOT)) {
            messages.push(format!(
                "{} cannot target the access folder itself",
                target.label
            ));
            continue;
        }
        if let Some(n) = d
            .links
            .iter()
            .position(|l| l.label == target.label && l.target == target_path)
        {
            if d.links[n].staging.is_none() && dir.matches(&d.links[n].name, &d.links[n])? {
                if !d.links[n].owners.iter().any(|x| x == owner) {
                    d.links[n].owners.push(owner.into());
                    save(data, &d)?
                }
            } else {
                messages.push(format!(
                    "{} has changed; it was not overwritten",
                    d.links[n].name
                ))
            }
            continue;
        }
        let base = target.label.clone();
        let chosen = if d.links.iter().any(|l| l.name == base) {
            format!(
                "{} — {}",
                base,
                &fsutil::hash(target_path.as_os_str().as_bytes())[..8]
            )
        } else {
            base
        };
        if stat(&dir.dir, &chosen)?.is_some() {
            messages.push(format!("{chosen} is occupied; content was not overwritten"));
            continue;
        }
        dir.attached()?;
        let tmp = format!(".wow-link-{}", fsutil::unique());
        let n = name(&tmp)?;
        let target_c = c(target_path.as_os_str())?;
        if unsafe { libc::symlinkat(target_c.as_ptr(), dir.dir.as_raw_fd(), n.as_ptr()) } != 0 {
            return Err(error());
        }
        let (identity, kind) = stat(&dir.dir, &tmp)?.ok_or("New link disappeared")?;
        if kind != libc::S_IFLNK {
            return Err("New link changed".into());
        }
        d.links.push(Link {
            name: chosen,
            label: target.label,
            target: target_path,
            identity,
            owners: vec![owner.into()],
            staging: Some(tmp),
            removing: None,
        });
        save(data, &d)?;
        let n = d.links.len() - 1;
        if let Err(e) = finish_create(data, &mut d, &dir, n) {
            messages.push(e)
        }
    }
    dir.attached()?;
    let mut report = status(data, home);
    report["messages"] = json!(messages);
    Ok(report)
}
pub fn status(data: &Path, home: &Path) -> Value {
    let path = home.join(ROOT);
    if !db(data).exists() {
        return json!({"path":path,"available":false,"state":"not_created","links":[],"message":"The access folder will be created without replacing existing user content."});
    }
    let read = || -> Result<Value> {
        let mut d = load(data)?;
        let dir = root(data, home, &mut d, false)?;
        let mut rows = vec![];
        for l in &d.links {
            let valid = l.staging.is_none() && l.removing.is_none() && dir.matches(&l.name, l)?;
            rows.push(json!({"name":l.name,"owned_unchanged":valid,"target_available":l.target.is_dir(),"pending":l.staging.is_some()||l.removing.is_some()}));
        }
        dir.attached()?;
        Ok(
            json!({"path":path,"available":true,"links":rows,"message":"Open this folder in Desktop Mode. AddOns links point to the detected real folders; extract each addon into the matching edition's AddOns link."}),
        )
    };
    read().unwrap_or_else(|e| json!({"path":path,"available":false,"links":[],"message":e}))
}
pub fn remove(data: &Path, home: &Path, owner: &str) -> Result<Value> {
    let mut d = load(data)?;
    if d.root.is_none() {
        return Ok(json!({"removed":0,"messages":[]}));
    }
    let dir = root(data, home, &mut d, false)?;
    let mut removed = 0;
    let mut messages = vec![];
    for n in (0..d.links.len()).rev() {
        if !d.links[n].owners.iter().any(|x| x == owner) {
            continue;
        }
        if d.links[n].owners.len() > 1 {
            d.links[n].owners.retain(|x| x != owner);
            save(data, &d)?;
            continue;
        }
        match remove_entry(data, &mut d, &dir, n) {
            Ok(()) => removed += 1,
            Err(e) => messages.push(e),
        }
    }
    // Never recursively delete the access folder or user files. Keep its ownership record for rerun.
    Ok(json!({"removed":removed,"messages":messages,"path":home.join(ROOT),"targets":"untouched"}))
}
pub fn child_directory(parent: &Path, wanted: &str) -> Result<Option<PathBuf>> {
    let mut matches = vec![];
    for entry in fs::read_dir(parent).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(wanted)
        {
            matches.push(entry.path())
        }
    }
    if matches.len() > 1 {
        return Err(format!(
            "Ambiguous {wanted} directory; no addon target was guessed"
        ));
    }
    match matches.pop() {
        Some(p) => {
            let p = p
                .canonicalize()
                .map_err(|_| format!("{wanted} is unavailable"))?;
            if p.is_dir() {
                Ok(Some(p))
            } else {
                Err(format!("{wanted} is not a directory"))
            }
        }
        None => Ok(None),
    }
}

pub fn missing_targets(data: &Path, home: &Path, targets: &[Target]) -> Vec<String> {
    let read = || -> Result<Vec<String>> {
        let mut d = load(data)?;
        let dir = root(data, home, &mut d, false)?;
        let mut missing = vec![];
        for target in targets {
            let Ok(path) = target.path.canonicalize() else {
                continue;
            };
            let linked = d.links.iter().any(|row| {
                row.label == target.label
                    && row.target == path
                    && row.staging.is_none()
                    && row.removing.is_none()
                    && dir.matches(&row.name, row).unwrap_or(false)
            });
            if !linked {
                missing.push(format!("{} access link is missing, changed or occupied; refresh never overwrites user content.",target.label))
            }
        }
        Ok(missing)
    };
    read().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        home: PathBuf,
        data: PathBuf,
        target: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let home = std::env::temp_dir().join(format!("folder-test-{}", fsutil::unique()));
            let data = home.join("private-data");
            let target = home.join("external-drive/installed/Interface/AddOns");
            fs::create_dir_all(&data).unwrap();
            fs::create_dir_all(&target).unwrap();
            Self { home, data, target }
        }
        fn targets(&self) -> Vec<Target> {
            vec![Target {
                label: "Retail AddOns".into(),
                path: self.target.clone(),
            }]
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.home);
        }
    }
    #[test]
    fn real_links_route_addons_rerun_reuses_and_removal_preserves_targets() {
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy-a", &f.targets()).unwrap();
        let link = f.home.join(ROOT).join("Retail AddOns");
        fs::write(link.join("ExampleAddon.lua"), b"addon sample").unwrap();
        assert_eq!(
            fs::read(f.target.join("ExampleAddon.lua")).unwrap(),
            b"addon sample"
        );
        let before = fs::read(db(&f.data)).unwrap();
        sync(&f.data, &f.home, "copy-a", &f.targets()).unwrap();
        assert_eq!(before, fs::read(db(&f.data)).unwrap());
        assert_eq!(fs::read_dir(f.home.join(ROOT)).unwrap().count(), 1);
        assert_eq!(remove(&f.data, &f.home, "copy-a").unwrap()["removed"], 1);
        assert!(fs::symlink_metadata(link).is_err());
        assert_eq!(
            fs::read(f.target.join("ExampleAddon.lua")).unwrap(),
            b"addon sample"
        );
    }
    #[test]
    fn occupied_root_file_directory_or_symlink_is_never_adopted() {
        for kind in 0..3 {
            let f = Fixture::new();
            let path = f.home.join(ROOT);
            match kind {
                0 => fs::write(&path, b"user").unwrap(),
                1 => {
                    fs::create_dir(&path).unwrap();
                    fs::write(path.join("user"), b"user").unwrap();
                }
                _ => std::os::unix::fs::symlink(&f.target, &path).unwrap(),
            }
            assert!(sync(&f.data, &f.home, "copy", &f.targets()).is_err());
            assert!(fs::symlink_metadata(&path).is_ok());
            assert!(!db(&f.data).exists());
        }
    }
    #[test]
    fn occupied_name_and_replaced_link_preserve_user_content() {
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy", &[]).unwrap();
        let link = f.home.join(ROOT).join("Retail AddOns");
        fs::write(&link, b"user occupied").unwrap();
        let report = sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        assert!(!report["messages"].as_array().unwrap().is_empty());
        assert_eq!(fs::read(&link).unwrap(), b"user occupied");
        fs::remove_file(&link).unwrap();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        fs::remove_file(&link).unwrap();
        fs::write(&link, b"replacement user file").unwrap();
        remove(&f.data, &f.home, "copy").unwrap();
        assert_eq!(fs::read(&link).unwrap(), b"replacement user file");
    }
    #[test]
    fn changed_private_quarantine_permissions_refuse_removal() {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        let mut ledger = load(&f.data).unwrap();
        let dir = root(&f.data, &f.home, &mut ledger, false).unwrap();
        let private = quarantine(&f.data, &mut ledger, &dir).unwrap();
        private
            .set_permissions(fs::Permissions::from_mode(0o777))
            .unwrap();
        let report = remove(&f.data, &f.home, "copy").unwrap();
        assert_eq!(report["removed"], 0);
        assert!(report["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("permissions changed")));
        assert!(f.home.join(ROOT).join("Retail AddOns").is_symlink());
        assert!(f.target.is_dir());
    }
    #[test]
    fn unavailable_drive_preserves_dangling_owned_link_until_reconnected() {
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        let original = fs::read(db(&f.data)).unwrap();
        let moved = f.home.join("drive-offline");
        fs::rename(f.home.join("external-drive"), &moved).unwrap();
        sync(&f.data, &f.home, "copy", &[]).unwrap();
        assert_eq!(original, fs::read(db(&f.data)).unwrap());
        assert_eq!(
            status(&f.data, &f.home)["links"][0]["target_available"],
            false
        );
        fs::rename(moved, f.home.join("external-drive")).unwrap();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        assert_eq!(original, fs::read(db(&f.data)).unwrap());
    }
    #[test]
    fn changed_parent_cannot_redirect_creation_or_unlink() {
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        let root = f.home.join(ROOT);
        let saved = f.home.join("preserved-access-folder");
        fs::rename(&root, &saved).unwrap();
        std::os::unix::fs::symlink(&f.target, &root).unwrap();
        assert!(sync(&f.data, &f.home, "copy", &f.targets()).is_err());
        assert!(remove(&f.data, &f.home, "copy").is_err());
        assert!(saved.join("Retail AddOns").is_symlink());
        assert_eq!(fs::read_dir(&f.target).unwrap().count(), 0);
    }
    #[test]
    fn pending_creation_and_removal_are_recoverable_without_target_writes() {
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        let mut d = load(&f.data).unwrap();
        let dir = root(&f.data, &f.home, &mut d, false).unwrap();
        rename(&dir.dir, "Retail AddOns", ".wow-test-staged").unwrap();
        d.links[0].staging = Some(".wow-test-staged".into());
        save(&f.data, &d).unwrap();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        assert!(f.home.join(ROOT).join("Retail AddOns").is_symlink());
        let mut d = load(&f.data).unwrap();
        let private = quarantine(&f.data, &mut d, &dir).unwrap();
        rename_between(&dir.dir, "Retail AddOns", &private, ".wow-test-removing").unwrap();
        d.links[0].removing = Some(".wow-test-removing".into());
        save(&f.data, &d).unwrap();
        assert_eq!(remove(&f.data, &f.home, "copy").unwrap()["removed"], 1);
        assert!(load(&f.data).unwrap().links.is_empty());
        assert!(f.target.is_dir());
    }
    #[test]
    fn multiple_copies_have_distinct_stable_names_and_shared_owners() {
        let f = Fixture::new();
        let other = f.home.join("second-drive/AddOns");
        fs::create_dir_all(&other).unwrap();
        let mut targets = f.targets();
        targets.push(Target {
            label: "Retail AddOns".into(),
            path: other,
        });
        sync(&f.data, &f.home, "first", &targets).unwrap();
        sync(&f.data, &f.home, "second", &targets).unwrap();
        assert_eq!(load(&f.data).unwrap().links.len(), 2);
        assert_eq!(remove(&f.data, &f.home, "first").unwrap()["removed"], 0);
        assert_eq!(remove(&f.data, &f.home, "second").unwrap()["removed"], 2);
    }
    #[test]
    fn actual_case_insensitive_addons_and_ambiguity_are_distinguished() {
        let f = Fixture::new();
        let client = f.home.join("client");
        fs::create_dir_all(client.join("interface/addons")).unwrap();
        let i = child_directory(&client, "Interface").unwrap().unwrap();
        assert_eq!(
            child_directory(&i, "AddOns").unwrap().unwrap(),
            client.join("interface/addons")
        );
        fs::create_dir(client.join("Interface")).unwrap();
        assert!(child_directory(&client, "Interface").is_err());
        assert_eq!(child_directory(&f.target, "Missing").unwrap(), None);
    }
    #[test]
    fn removing_owned_links_does_not_remove_untracked_files_or_correct_unowned_link() {
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        let dir = f.home.join(ROOT);
        fs::write(dir.join("My notes.txt"), b"notes").unwrap();
        std::os::unix::fs::symlink(&f.target, dir.join("User link")).unwrap();
        remove(&f.data, &f.home, "copy").unwrap();
        assert_eq!(fs::read(dir.join("My notes.txt")).unwrap(), b"notes");
        assert!(dir.join("User link").is_symlink());
        assert!(f.target.is_dir());
    }
    #[test]
    fn final_published_link_with_uncleared_staging_record_is_removed_safely() {
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        let mut d = load(&f.data).unwrap();
        d.links[0].staging = Some(".wow-no-longer-staged".into());
        save(&f.data, &d).unwrap();
        assert_eq!(remove(&f.data, &f.home, "copy").unwrap()["removed"], 1);
        assert!(load(&f.data).unwrap().links.is_empty());
        assert!(f.target.is_dir());
    }
    #[test]
    fn mismatched_open_directory_descriptor_is_refused() {
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        let d = load(&f.data).unwrap();
        let wrong = Directory {
            home: open(&f.home).unwrap(),
            dir: open(&f.target).unwrap(),
            identity: d.root.unwrap().identity,
        };
        assert!(wrong.attached().is_err());
        assert_eq!(fs::read_dir(&f.target).unwrap().count(), 0);
    }
    #[test]
    fn final_private_unlink_refuses_replaced_regular_file() {
        let f = Fixture::new();
        sync(&f.data, &f.home, "copy", &f.targets()).unwrap();
        let mut d = load(&f.data).unwrap();
        let dir = root(&f.data, &f.home, &mut d, false).unwrap();
        let private = quarantine(&f.data, &mut d, &dir).unwrap();
        let row = d.links[0].clone();
        rename_between(&dir.dir, &row.name, &private, ".wow-check").unwrap();
        unlink(&private, ".wow-check", 0).unwrap();
        let base = fsutil::canonical(&f.home).unwrap().join(
            d.root
                .as_ref()
                .unwrap()
                .quarantine
                .as_ref()
                .unwrap()
                .0
                .clone(),
        );
        fs::write(base.join(".wow-check"), b"replacement user data").unwrap();
        assert!(verified_unlink(&private, ".wow-check", &row).is_err());
        assert_eq!(
            fs::read(base.join(".wow-check")).unwrap(),
            b"replacement user data"
        );
    }
}
