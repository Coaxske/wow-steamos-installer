// Conservative metadata-backed shortcut cleanup; never deletes game data or Proton.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ClientRemoval {
    appid: u32,
    edition: String,
    game: PathBuf,
    metadata_hash: String,
}
fn uninstall_evidence(i: &Installation, edition: &str, game: &std::path::Path) -> Result<String> {
    let uncertain = || {
        "Client uninstall is uncertain. Keep its shortcut; finish installation/update or restore access, then review again.".to_string()
    };
    if !i.owned {
        return Err(uncertain());
    }
    let marker: Installation = fsutil::load(&i.root.join(".wow-rust-owner.json"))?;
    if marker.id != i.id
        || marker.root != i.root
        || marker.prefix != i.prefix
        || fsutil::canonical(&i.root)? != i.root
        || i.prefix != i.root.join("compatdata")
    {
        return Err(uncertain());
    }
    let folder = game.parent().ok_or_else(uncertain)?;
    let parent = folder.parent().ok_or_else(uncertain)?;
    let known = catalog::by_id(edition).ok_or_else(uncertain)?;
    if !known
        .folders
        .iter()
        .any(|n| folder.file_name().is_some_and(|f| f == *n))
        || !parent.starts_with(&i.root)
        || fsutil::canonical(parent)? != parent
    {
        return Err(uncertain());
    }
    // Read the accessible exact subtree independently; broad scans may skip unavailable roots.
    let mut scoped = Scan::default();
    let mut seen = std::collections::HashSet::new();
    let mut budget = 50_000;
    discovery::walk(&i.root, &mut scoped, &mut seen, &mut budget)?;
    if !scoped.warnings.is_empty() || scoped.games.iter().any(|g| g.executable == game) {
        return Err(uncertain());
    }
    let path = parent.join(".build.info");
    let m = fs::symlink_metadata(&path).map_err(|_| uncertain())?;
    if !m.is_file() || m.len() > 1024 * 1024 {
        return Err(uncertain());
    }
    let bytes = fsutil::read(&path)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| uncertain())?;
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    let head: Vec<_> = lines
        .next()
        .ok_or_else(uncertain)?
        .split('|')
        .map(|s| s.split('!').next().unwrap_or(""))
        .collect();
    let product = head
        .iter()
        .position(|s| *s == "Product")
        .ok_or_else(uncertain)?;
    let active = head
        .iter()
        .position(|s| *s == "Active")
        .ok_or_else(uncertain)?;
    let expected = products::product_for(edition).ok_or_else(uncertain)?;
    let mut sibling = false;
    let mut count = 0;
    for row in lines {
        count += 1;
        if count > 5000 {
            return Err(uncertain());
        }
        let cells: Vec<_> = row.split('|').collect();
        if cells.len() != head.len()
            || !products::valid(cells[product])
            || !["0", "1"].contains(&cells[active])
            || cells[product] == expected
        {
            return Err(uncertain());
        }
        if cells[active] == "1"
            && scoped.games.iter().any(|g| {
                g.path.parent() == Some(parent)
                    && products::product_for(&g.id) == Some(cells[product])
            })
        {
            sibling = true;
        }
    }
    if !sibling {
        return Err(uncertain());
    }
    // Installed data or an incomplete downloader directory is not a confirmed uninstall.
    match fs::symlink_metadata(folder) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {
            for e in fs::read_dir(folder).map_err(|_| uncertain())? {
                let e = e.map_err(|_| uncertain())?;
                let t = e.file_type().map_err(|_| uncertain())?;
                let n = e.file_name();
                if !((t.is_file()
                    && e.path()
                        .extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("dll")))
                    || (t.is_dir() && ["Interface", "WTF"].iter().any(|s| n == *s)))
                {
                    return Err(uncertain());
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(uncertain()),
    }
    Ok(fsutil::hash(&bytes))
}
fn obsolete_clients(
    i: &Installation,
    games: &[Game],
    root: &mut vdf::Object,
) -> Result<Vec<ClientRemoval>> {
    let mut removals = vec![];
    for b in &i.bindings {
        let (Some(edition), Some(game)) = (&b.edition, &b.game) else {
            continue;
        };
        if b.launcher || games.iter().any(|g| g.executable == PathBuf::from(game)) {
            continue;
        }
        let entries = vdf::object(root, "shortcuts").ok_or("Missing shortcuts")?;
        let candidates: Vec<_> = entries
            .iter()
            .filter_map(|(_, v)| match v {
                Value::Object(e) if vdf::number(e, "appid") == Some(b.appid) => Some(e),
                _ => None,
            })
            .collect();
        if candidates.len() != 1
            || !steam::owned(candidates[0], &i.id)
            || vdf::string(candidates[0], "wowRustGame") != Some(game)
            || vdf::string(candidates[0], "wowRustEdition") != Some(edition)
        {
            continue;
        }
        discovery::require_games_closed_for("confirming an uninstalled client's shortcut cleanup")?;
        let metadata_hash = uninstall_evidence(i, edition, std::path::Path::new(game))?;
        removals.push(ClientRemoval {
            appid: b.appid,
            edition: edition.clone(),
            game: game.into(),
            metadata_hash,
        });
    }
    vdf::ensure(root,"shortcuts")?.retain(|(_,v)|!matches!(v,Value::Object(e) if removals.iter().any(|r|vdf::number(e,"appid")==Some(r.appid))));
    Ok(removals)
}
