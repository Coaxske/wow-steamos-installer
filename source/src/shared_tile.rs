// Shared Steam tile migration. No controller bindings, game data or vendor settings are edited.
fn shared_tile_candidates(root: &vdf::Object, i: &Installation, games: &[Game]) -> Result<Vec<vdf::Object>> {
    let mut out = vec![];
    for (_,value) in vdf::object(root,"shortcuts").ok_or("Missing shortcuts")? {
        let Value::Object(e)=value else {continue};
        let id=vdf::number(e,"appid").ok_or("Missing shortcut ID")?;
        let binding=i.bindings.iter().find(|b|b.appid==id);
        let is_game=vdf::string(e,"wowRustRole")==Some("launcher") || binding.is_some_and(|b|b.launcher) || vdf::string(e,"wowRustRole")==Some("shared_game") || vdf::string(e,"wowRustGame").is_some() || vdf::string(e,"wowRustEdition").is_some() || vdf::string(e,"LaunchOptions").and_then(steam::edition).is_some()
            || binding.is_some_and(|b|b.shared || b.game.is_some() || b.edition.is_some());
        if steam::native_addon(e) {continue}
        if !is_game { if steam::owned(e,&i.id) {return Err("An installer-owned shortcut has an unrecognized role. Preserve it and review this copy before consolidation.".into())} continue }
        if !steam::owned(e,&i.id) {
            if binding.is_some() {return Err("A saved game shortcut changed ownership or route. Preserve it and review this installation before consolidating.".into())}
            continue
        }
        let target=Path::new(vdf::string(e,"exe").unwrap_or("").trim_matches('"'));
        let launcher=i.launcher.as_ref().is_some_and(|p|steam::launcher_alias(target,p));
        let direct=games.iter().any(|g|g.executable==target);
        let bootstrap=id==i.bootstrap_id && target==i.root.join("Battle.net-Setup.exe");
        if (!launcher && !direct && !bootstrap) || (steam::compat_prefix(vdf::string(e,"LaunchOptions").unwrap_or("")).as_ref()!=Some(&i.prefix) && !(binding.is_some() && vdf::string(e,"LaunchOptions").unwrap_or("").trim().is_empty())) {
            return Err("An installer-owned shortcut has a changed target or prefix. No consolidation is safe; review it in Steam first.".into())
        }
        out.push(e.clone());
    }
    Ok(out)
}
fn shared_tile_keeper(candidates:&[vdf::Object], req:&Json)->Result<Option<u32>> {
    let chosen=req.get("shared_tile_appid").filter(|v|!v.is_null());
    if let Some(value)=chosen {
        let id=value.as_u64().and_then(|n|u32::try_from(n).ok()).ok_or("Choose a valid existing tile identity")?;
        if candidates.iter().any(|e|vdf::number(e,"appid")==Some(id)) {return Ok(Some(id))}
        return Err("The selected shared-tile identity changed. Review the current owned tiles again.".into())
    }
    if candidates.len()>1 {
        return Err("Choose which existing installer-owned tile supplies Battle.net's identity and controller association. The other layouts are preserved, not merged.".into())
    }
    Ok(candidates.first().and_then(|e|vdf::number(e,"appid")))
}
// Recover only the one explicitly saved shared identity. Never borrow an occupied ID.
fn missing_shared_tile_identity(root:&vdf::Object,i:&Installation)->Result<Option<u32>> {
    let saved:Vec<_>=i.bindings.iter().filter(|b|b.shared).map(|b|b.appid).collect();
    let id=match saved.as_slice() {
        []=>return Ok(None),
        [id]=>*id,
        _=>return Err("The saved Battle.net tile identity is ambiguous. Preserve this copy and review its setup records; no shortcut changed.".into()),
    };
    if id==0 || vdf::object(root,"shortcuts").ok_or("Missing shortcuts")?.iter().any(|(_,v)|matches!(v,Value::Object(e) if vdf::number(e,"appid")==Some(id))) {
        return Err("The saved Battle.net tile identity is already used or invalid. No unrelated shortcut or controller association was changed.".into());
    }
    Ok(Some(id))
}
fn shared_migration_needed(plan:&Plan)->Result<bool> {
    Ok(plan.kind=="repair" && plan.shared_migration)
}
fn backup_shared_migration(ctx:&Context,plan:&Plan)->Result<PathBuf> {
    let dest=ctx.data.join("shared-tile-backups").join(&plan.token);
    let journal=serde_json::to_vec_pretty(&plan.journal).map_err(|e|e.to_string())?;
    let i=&plan.installation;
    let mut files=std::collections::BTreeSet::new();
    let roots=[
        i.steam.join("steamapps/common/Steam Controller Configs"),
        i.steam.join("userdata").join(&i.user).join("config/controller_configs"),
        i.steam.join("config/controller_configs"),
        i.steam.join("userdata").join(&i.user).join("241100"),
        i.steam.join("controller_base"),
    ];
    fn collect(root:&Path,out:&mut std::collections::BTreeSet<PathBuf>)->Result<()> {
        let meta=fs::symlink_metadata(root).map_err(|e|e.to_string())?;
        if meta.file_type().is_symlink() {return Err("A linked controller record cannot be backed up safely; no shortcuts changed.".into())}
        if meta.is_file() {out.insert(root.to_path_buf());}
        else if meta.is_dir() {for entry in fs::read_dir(root).map_err(|e|e.to_string())? {collect(&entry.map_err(|e|e.to_string())?.path(),out)?;}}
        else {return Err("Unsupported controller backup entry".into())}
        if out.len()>10000 {return Err("Controller backup is too large; preserve the layouts and review manually.".into())}
        Ok(())
    }
    for root in roots {if root.exists(){collect(&root,&mut files)?;}}
    // Copy opaque local controller-selection metadata without parsing or exposing its contents.
    let local=i.steam.join("userdata").join(&i.user).join("config/localconfig.vdf");
    if local.exists(){collect(&local,&mut files)?;}
    if dest.exists() {
        let old=fsutil::read(&dest.join("journal.json"))?;
        if old!=journal {return Err("The shared-tile backup no longer matches this review. Preserve it and create a new review.".into())}
        let raw=fsutil::read(&dest.join("inventory.json"))?;
        let seal:Json=fsutil::load(&dest.join(".complete.json"))?;
        let records:Vec<Json>=serde_json::from_slice(&raw).map_err(|e|e.to_string())?;
        if seal["inventory_sha256"]!=fsutil::hash(&raw) || seal["journal_sha256"]!=fsutil::hash(&journal) || seal["records"]!=records.len() || records.is_empty() {
            return Err("Shared-tile backup is incomplete or changed; no shortcuts changed.".into())
        }
        let recorded_sources:std::collections::BTreeSet<PathBuf>=records.iter().filter_map(|v|v["source"].as_str().map(PathBuf::from)).collect();
        if recorded_sources!=files {return Err("Controller backup coverage changed; preserve the backup and create a new review.".into())}
        let mut seen=std::collections::BTreeSet::new();
        for v in records {
            let file=v["file"].as_str().ok_or("Invalid migration backup")?;
            if Path::new(file).file_name().and_then(|n|n.to_str())!=Some(file) || !seen.insert(file.to_string()) {
                return Err("Invalid or duplicate migration backup entry".into())
            }
            if fsutil::hash(&fsutil::read(&dest.join(file))?)!=v["sha256"].as_str().unwrap_or("") {return Err("Shared-tile backup verification failed; no shortcut changed.".into())}
            if let Some(source)=v["source"].as_str() {
                if fsutil::hash(&fsutil::read(Path::new(source))?)!=v["sha256"].as_str().unwrap_or("") {
                    return Err("A controller record changed after its backup; preserve it and create a new review.".into())
                }
            }
        }
        if !seen.contains("journal.json") {return Err("Migration journal backup is missing".into())}
        return Ok(dest)
    }
    let mut captured=vec![];let mut total=journal.len();
    for file in files {
        let size=fs::metadata(&file).map_err(|e|e.to_string())?.len();
        if size>64*1024*1024 || (total as u64).saturating_add(size)>64*1024*1024 {return Err("Controller backup exceeds64MiB; no shortcut changed.".into())}
        let bytes=fsutil::read(&file)?;total+=bytes.len();
        if total>64*1024*1024 {return Err("Controller backup exceeds64MiB; no shortcut changed.".into())}
        captured.push((file,bytes));
    }
    fsutil::private_dir(&dest)?;
    fsutil::atomic(&dest.join("journal.json"),&journal)?;
    let mut inventory=vec![json!({"file":"journal.json","sha256":fsutil::hash(&journal)})];
    for (n,(source,bytes)) in captured.iter().enumerate() {
        let name=format!("controller-{n}.bin");fsutil::atomic(&dest.join(&name),bytes)?;
        inventory.push(json!({"file":name,"source":source,"sha256":fsutil::hash(bytes)}));
    }
    fsutil::json(&dest.join("inventory.json"),&inventory)?;
    let count=inventory.len();
    for record in inventory {
        if fsutil::hash(&fsutil::read(&dest.join(record["file"].as_str().unwrap()))?)!=record["sha256"].as_str().unwrap() {return Err("Shared-tile backup verification failed; no shortcuts changed.".into())}
    }
    fsutil::json(&dest.join(".complete.json"),&json!({"inventory_sha256":fsutil::hash(&fsutil::read(&dest.join("inventory.json"))?),"journal_sha256":fsutil::hash(&journal),"records":count}))?;
    Ok(dest)
}

fn require_single_tile(_ctx:&Context,i:&Installation)->Result<()> {
    let mut root=steam::read_shortcuts(&steam::shortcuts_path(&i.steam,&i.user)?)?;
    steam::restore_identity(&mut root,i)?;
    let owned:Vec<_>=vdf::object(&root,"shortcuts").ok_or("Missing shortcuts")?.iter().filter_map(|(_,v)|match v {Value::Object(e) if steam::owned(e,&i.id)&&!steam::native_addon(e)=>Some(e),_=>None}).collect();
    if owned.len()!=1 {return Err("Review the single Battle.net Steam tile before completing setup.".into())}
    let e=owned[0];let options=vdf::string(e,"LaunchOptions").unwrap_or("");
    let target=Path::new(vdf::string(e,"exe").unwrap_or("").trim_matches('"'));
    if vdf::string(e,"appname")!=Some("Battle.net") || vdf::string(e,"wowRustRole")!=Some("shared_game")
        || !i.launcher.as_ref().is_some_and(|p|steam::launcher_alias(target,p))
        || steam::compat_prefix(options).as_ref()!=Some(&i.prefix) || !steam::ordinary_launcher(options) {
        return Err("The single Battle.net tile is not verified as the ordinary edition chooser. Review shortcut repair; no game was launched.".into())
    }
    Ok(())
}
