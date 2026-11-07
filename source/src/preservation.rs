// Strict settings preservation for removal. Never infer absence from game health.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PreservedSettings {
    source: PathBuf,
    relative: PathBuf,
    digest: String,
}
#[derive(Serialize, Deserialize)]
struct SettingsInventory {
    token: String,
    items: Vec<PreservedSettings>,
}
fn settings_roots(i: &Installation) -> Vec<PathBuf> {
    vec![
        i.root.join("games/World of Warcraft"),
        i.prefix.join("pfx/drive_c/Program Files/World of Warcraft"),
        i.prefix
            .join("pfx/drive_c/Program Files (x86)/World of Warcraft"),
    ]
}
fn settings_read_dir(path: &Path) -> Result<Vec<PathBuf>> {
    fs::read_dir(path)
        .map_err(|_| "Settings discovery is unreadable; removal paused.")?
        .map(|e| {
            e.map(|e| e.path())
                .map_err(|_| "Settings discovery is incomplete; removal paused.".into())
        })
        .collect()
}
fn settings_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Settings discovery is unreadable; removal paused.".into()),
    }
}
fn settings_actual(job: &Cleanup, original: &Path) -> Result<Option<PathBuf>> {
    for removal in &job.plan.remove {
        if let Ok(relative) = original.strip_prefix(&removal.path) {
            if removal.removed {
                return Ok(None);
            }
            if settings_metadata(&removal.quarantine)?.is_some() {
                return Ok(Some(removal.quarantine.join(relative)));
            }
            return Ok(Some(original.to_path_buf()));
        }
    }
    Ok(None) // Outside this removal: retained, never copied/deleted here.
}
fn settings_edition(i: &Installation, parent: &Path) -> Result<String> {
    let mut saved: BTreeSet<_> = i
        .bindings
        .iter()
        .filter(|b| {
            b.game
                .as_deref()
                .is_some_and(|g| Path::new(g).parent() == Some(parent))
        })
        .filter_map(|b| b.edition.as_ref())
        .collect();
    for game in &i.game_associations {
        if game.executable.parent()==Some(parent) {saved.insert(&game.id);}
    }
    if saved.len() > 1 {
        return Err("Conflicting settings edition associations; removal paused.".into());
    }
    if let Some(id) = saved.into_iter().next() {
        if catalog::by_id(id).is_some() {
            return Ok(id.clone());
        }
    }
    catalog::by_folder(parent.file_name().and_then(|n|n.to_str()).unwrap_or(""))
        .map(|e|e.id.to_string()).ok_or_else(||"Settings were found in an unrecognized client folder. Preserve them manually; removal paused.".into())
}
fn settings_inventory_with(
    ctx: &Context,
    job: &Cleanup,
    mut budget: usize,
    max_depth: usize,
    read_dir: &impl Fn(&Path) -> Result<Vec<PathBuf>>,
) -> Result<Vec<PreservedSettings>> {
    let _ = ctx; // Context kept explicit for destructive-operation helpers.
    let mut result = vec![];
    let mut relative_paths = BTreeSet::new();
    let mut editions = std::collections::BTreeMap::new();
    for original in settings_roots(&job.plan.installation) {
        let Some(actual) = settings_actual(job, &original)? else {
            continue;
        };
        let Some(meta) = settings_metadata(&actual)? else {
            continue;
        };
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("Linked or invalid game folders block settings preservation.".into());
        }
        let mut stack = vec![(original, actual, 0usize)];
        while let Some((original, actual, depth)) = stack.pop() {
            crate::cancelled()?;
            if budget == 0 {
                return Err(
                    "Settings discovery limit reached; completeness is uncertain. Removal paused."
                        .into(),
                );
            }
            budget -= 1;
            for path in read_dir(&actual)? {
                let name = path.file_name().ok_or("Invalid settings discovery path")?;
                let source = original.join(name);
                let meta = settings_metadata(&path)?
                    .ok_or("Settings changed during discovery; removal paused.")?;
                let settings = ["Interface", "WTF"]
                    .into_iter()
                    .find(|n| name.to_string_lossy().eq_ignore_ascii_case(n));
                if let Some(folder) = settings {
                    if !meta.is_dir() || meta.file_type().is_symlink() {
                        return Err(
                            "Linked or unreadable settings cannot be preserved; removal paused."
                                .into(),
                        );
                    }
                    let id = settings_edition(&job.plan.installation, &original)?;
                    if editions
                        .insert(id.clone(), original.clone())
                        .is_some_and(|old| old != original)
                    {
                        return Err("Multiple settings clients share an edition; removal paused to avoid merging copies.".into());
                    }
                    let relative = PathBuf::from(id).join(folder);
                    if !relative_paths.insert(relative.clone()) {
                        return Err(
                            "Duplicate settings folders would merge; removal paused.".into()
                        );
                    }
                    result.push(PreservedSettings {
                        source,
                        relative,
                        digest: tree_digest(&path)?,
                    });
                } else if meta.is_dir() && !meta.file_type().is_symlink() {
                    if depth >= max_depth {
                        return Err("Settings discovery depth reached; completeness is uncertain. Removal paused.".into());
                    }
                    stack.push((source, path, depth + 1));
                }
            }
        }
    }
    result.sort_by(|a, b| a.source.cmp(&b.source));
    Ok(result)
}
fn settings_inventory(ctx: &Context, job: &Cleanup) -> Result<Vec<PreservedSettings>> {
    settings_inventory_with(ctx, job, 50_000, 24, &settings_read_dir)
}
fn settings_subset(source: &Path, backup: &Path) -> Result<()> {
    fsutil::check_tree(source)?;
    for path in settings_read_dir(source)? {
        let target = backup.join(path.file_name().ok_or("Invalid settings path")?);
        let m = fs::symlink_metadata(&path).map_err(|_| "Settings changed; removal paused.")?;
        let other = fs::symlink_metadata(&target)
            .map_err(|_| "Settings changed after backup; removal paused.")?;
        if m.is_dir() && other.is_dir() && !other.file_type().is_symlink() {
            settings_subset(&path, &target)?;
        } else if m.is_file()
            && other.is_file()
            && !other.file_type().is_symlink()
            && fsutil::read(&path)? == fsutil::read(&target)?
        {
        } else {
            return Err("Settings changed after backup; removal paused.".into());
        }
    }
    Ok(())
}
fn verify_cleanup_backup(ctx: &Context, job: &Cleanup) -> Result<()> {
    let backup = ctx.data.join("settings-backups").join(&job.plan.token);
    let marker: String = fsutil::load(&backup.join(".complete.json"))?;
    let inventory:SettingsInventory=fsutil::load(&backup.join(".inventory.json"))
        .map_err(|_|"Settings backup lacks a verifiable inventory. Removal paused; existing backups are kept.".to_string())?;
    if marker != job.plan.token || inventory.token != job.plan.token {
        return Err("Settings backup identity mismatch; removal paused.".into());
    }
    let mut seen = BTreeSet::new();
    for item in &inventory.items {
        let parts: Vec<_> = item.relative.components().collect();
        if parts.len() != 2
            || !parts
                .iter()
                .all(|p| matches!(p, std::path::Component::Normal(_)))
            || catalog::by_id(parts[0].as_os_str().to_str().unwrap_or("")).is_none()
            || !["Interface", "WTF"].contains(&parts[1].as_os_str().to_str().unwrap_or(""))
            || !settings_roots(&job.plan.installation)
                .iter()
                .any(|p| item.source.starts_with(p))
            || !job
                .plan
                .remove
                .iter()
                .any(|r| item.source.starts_with(&r.path))
            || !seen.insert(item.source.clone())
            || tree_digest(&backup.join(&item.relative))? != item.digest
        {
            return Err("Settings backup contents or inventory changed; removal paused.".into());
        }
    }
    for current in settings_inventory(ctx, job)? {
        let saved = inventory
            .items
            .iter()
            .find(|old| old.source == current.source && old.relative == current.relative)
            .ok_or("New settings appeared after backup; removal paused.")?;
        let actual = settings_actual(job, &current.source)?.ok_or("Settings scope changed")?;
        let quarantined = job
            .plan
            .remove
            .iter()
            .any(|r| current.source.starts_with(&r.path) && r.quarantine.exists());
        if quarantined {
            settings_subset(&actual, &backup.join(&saved.relative))?;
        } else if current.digest != saved.digest {
            return Err("Settings changed after backup; removal paused.".into());
        }
    }
    Ok(())
}
fn preserve_cleanup_settings(ctx: &Context, job: &mut Cleanup) -> Result<()> {
    if !job.plan.keep_settings {
        return Ok(());
    }
    let backup = ctx.data.join("settings-backups").join(&job.plan.token);
    if settings_metadata(&backup)?.is_none() {
        if job.settings_saved {
            return Err("Saved settings backup is missing; removal paused.".into());
        }
        for removal in &job.plan.remove {
            if !removal.removed
                && (settings_metadata(&removal.quarantine)?.is_some()
                    || settings_metadata(&removal.path)?.is_none())
            {
                return Err("Cannot start settings preservation after a reviewed folder moved or disappeared. Removal paused.".into());
            }
        }
        let items = settings_inventory(ctx, job)?;
        let staging = ctx.data.join(format!(".settings-{}", fsutil::unique()));
        fsutil::private_dir(&staging)?;
        for item in &items {
            let actual = settings_actual(job, &item.source)?.ok_or("Settings scope changed")?;
            copy_tree(&actual, &staging.join(&item.relative))?;
            if tree_digest(&staging.join(&item.relative))? != item.digest {
                return Err("Settings copy did not match its source; removal paused.".into());
            }
        }
        if items != settings_inventory(ctx, job)? {
            return Err("Settings changed during preservation; removal paused.".into());
        }
        fsutil::json(
            &staging.join(".inventory.json"),
            &SettingsInventory {
                token: job.plan.token.clone(),
                items,
            },
        )?;
        fsutil::json(&staging.join(".complete.json"), &job.plan.token)?;
        fs::create_dir_all(backup.parent().unwrap()).map_err(|e| e.to_string())?;
        if settings_metadata(&backup)?.is_some() {
            return Err("Settings backup changed during copying; removal paused.".into());
        }
        fs::rename(staging, &backup).map_err(|e| e.to_string())?;
        fs::File::open(backup.parent().unwrap())
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
    }
    verify_cleanup_backup(ctx, job)?;
    job.settings_saved = true;
    fsutil::json(&ctx.data.join("cleanup.json"), job)
}
