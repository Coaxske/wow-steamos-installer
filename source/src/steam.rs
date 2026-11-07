use crate::{
    fsutil,
    vdf::{self, Object, Value},
    Result,
};
use std::path::{Path, PathBuf};
pub const OWNER: &str = "WoW SteamOS Rust";
pub fn canonical_title(name: &str) -> String {
    let name = name
        .strip_suffix(" [WoW SteamOS Installer]")
        .or_else(|| name.strip_suffix(" - WoW SteamOS Installer"))
        .unwrap_or(name);
    match name {
        "WoW Retail" | "World of Warcraft (Retail)" => "World of Warcraft",
        "WoW Forever" => "World of Warcraft Forever",
        "World of Warcraft Classic Era" => "WoW Classic Era",
        _ => name,
    }
    .to_string()
}
pub fn shortcuts_path(root: &Path, user: &str) -> Result<PathBuf> {
    if user.is_empty() || !user.chars().all(|c| c.is_ascii_digit()) || user == "0" {
        return Err("Select a valid Steam profile".into());
    }
    let path = root
        .join("userdata")
        .join(user)
        .join("config/shortcuts.vdf");
    if !path.parent().unwrap().is_dir() {
        return Err("Selected Steam profile has no config directory".into());
    }
    Ok(path)
}
pub fn read_shortcuts(path: &Path) -> Result<Object> {
    if !path.exists() {
        return Ok(vec![("shortcuts".into(), Value::Object(vec![]))]);
    }
    let o = vdf::parse_binary(&fsutil::read(path)?)?;
    if vdf::object(&o, "shortcuts").is_none() {
        return Err("Steam shortcuts root missing".into());
    }
    let mut ids = std::collections::HashSet::new();
    for (_, value) in vdf::object(&o, "shortcuts").unwrap() {
        let entry = match value {
            Value::Object(e) => e,
            _ => return Err("Malformed shortcut entry; file left unchanged".into()),
        };
        let id = vdf::number(entry, "appid")
            .ok_or("A shortcut is missing its numeric ID; manual review is needed")?;
        if !ids.insert(id) {
            return Err("Multiple shortcuts share an ID. Automatic edits are blocked to protect controller/artwork associations.".into());
        }
    }
    Ok(o)
}
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = !0u32;
    for b in bytes {
        c ^= *b as u32;
        for _ in 0..8 {
            c = (c >> 1) ^ if c & 1 != 0 { 0xedb88320 } else { 0 };
        }
    }
    !c
}
pub fn app_id(exe: &str, title: &str) -> u32 {
    crc32(format!("{exe}{title}").as_bytes()) | 0x80000000
}
pub fn game_id(app: u32) -> u64 {
    ((app as u64) << 32) | 0x02000000
}
/// New native tiles need a per-installation namespace: Steam merges remote
/// shortcuts, so the common /usr/bin/env + title ID can collide across hosts.
/// Existing stored identities still win in review/upsert; do not migrate them.
pub fn native_entry(title: &str, options: &str, owner_id: &str) -> Result<Object> {
    let seed = format!("wow-steamos-native\0{owner_id}\0{title}");
    let id = crc32(seed.as_bytes()) | 0x80000000;
    entry(
        title,
        Path::new("/usr/bin/env"),
        options,
        owner_id,
        Some(id),
    )
}
pub fn entry(
    title: &str,
    target: &Path,
    options: &str,
    owner_id: &str,
    id: Option<u32>,
) -> Result<Object> {
    let exe = fsutil::steam_quote(target)?;
    let mut out = vec![];
    for (k, v) in [
        ("appname", title.to_string()),
        ("exe", exe.clone()),
        (
            "StartDir",
            fsutil::steam_quote(target.parent().ok_or("No target parent")?)?,
        ),
        ("icon", String::new()),
        ("ShortcutPath", String::new()),
        ("LaunchOptions", options.to_string()),
        ("FlatpakAppID", String::new()),
    ] {
        out.push((k.into(), Value::Text(v)));
    }
    for (k, n) in [
        ("appid", id.unwrap_or_else(|| app_id(&exe, title))),
        ("IsHidden", 0),
        ("AllowDesktopConfig", 1),
        ("AllowOverlay", 1),
        ("OpenVR", 0),
        ("Devkit", 0),
        ("LastPlayTime", 0),
    ] {
        out.push((k.into(), Value::I32(n)));
    }
    if title == "Battle.net" || title == "Install Battle.net" {
        vdf::set(&mut out, "wowRustRole", Value::Text("launcher".into()));
    }
    out.push((
        "tags".into(),
        Value::Object(vec![
            ("0".into(), Value::Text(OWNER.into())),
            ("1".into(), Value::Text(format!("wow-rust:{owner_id}"))),
        ]),
    ));
    Ok(out)
}
pub fn owned(o: &Object, id: &str) -> bool {
    vdf::object(o, "tags")
        .map(|tags| {
            tags.iter()
                .any(|(_, v)| matches!(v,Value::Text(t)if t==&format!("wow-rust:{id}")))
        })
        .unwrap_or(false)
}
pub fn launch_options(prefix: &Path, product: Option<&str>) -> Result<String> {
    let mut s = format!(
        "STEAM_COMPAT_DATA_PATH={} %command%",
        fsutil::shell_quote(prefix)?
    );
    if let Some(p) = product {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err("Invalid product code".into());
        }
        s.push_str(&format!(" --exec=\"launch {p}\""));
    }
    Ok(s)
}
/// Preserve only a constrained product code, never execute imported shell options.
pub fn product(options: &str) -> Option<String> {
    let start = options.find("--exec=\"launch ")? + 15;
    let rest = &options[start..];
    let end = rest.find('"')?;
    let p = &rest[..end];
    if !p.is_empty() && p.len() < 40 && p.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(p.into())
    } else {
        None
    }
}
pub fn selected_product(options: &str) -> Option<String> {
    let words = shell_words(options)?;
    words.iter().find_map(|(_, _, w)| {
        w.strip_prefix("--game=")
            .filter(|p| crate::products::valid(p))
            .map(str::to_string)
    })
}
pub fn edition(options: &str) -> Option<&'static str> {
    if let Some(code) = product(options) {
        return crate::catalog::EDITIONS
            .iter()
            .find(|e| e.command == Some(code.as_str()))
            .map(|e| e.id);
    }
    let p = selected_product(options)?;
    crate::catalog::EDITIONS
        .iter()
        .find(|e| crate::products::product_for(e.id) == Some(p.as_str()))
        .map(|e| e.id)
}
pub fn launcher_alias(target: &Path, launcher: &Path) -> bool {
    target == launcher
        || (target.parent() == launcher.parent()
            && target.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .eq_ignore_ascii_case("Battle.net Launcher.exe")
            }))
}
// Token spans only: never evaluate imported options or invoke a shell.
fn shell_words(s: &str) -> Option<Vec<(usize, usize, String)>> {
    let mut out = vec![];
    let mut start = None;
    let mut quote = None;
    let mut word = String::new();
    let mut escape = false;
    for (n, c) in s.char_indices() {
        if escape {
            word.push(c);
            escape = false;
            continue;
        }
        if c == '\\' && quote != Some('\'') {
            start.get_or_insert(n);
            escape = true;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                word.push(c);
            }
            continue;
        }
        if c == '\'' || c == '"' {
            start.get_or_insert(n);
            quote = Some(c);
        } else if c.is_whitespace() {
            if let Some(begin) = start.take() {
                out.push((begin, n, std::mem::take(&mut word)));
            }
        } else {
            start.get_or_insert(n);
            word.push(c);
        }
    }
    if quote.is_some() || escape {
        return None;
    }
    if let Some(begin) = start {
        out.push((begin, s.len(), word));
    }
    Some(out)
}
pub fn ordinary_launcher(options: &str) -> bool {
    options.matches("%command%").count() == 1
        && shell_words(options).is_some_and(|words| {
            words.iter().filter(|(_, _, w)| w == "%command%").count() == 1
                && words.iter().all(|(_, _, w)| {
                    !["--exec", "--game", "--lang"]
                        .iter()
                        .any(|key| w == key || w.starts_with(&format!("{key}=")))
                })
        })
}
pub fn preserve_options(old: &str, desired: &str) -> Result<String> {
    if old.trim().is_empty() {
        return Ok(desired.into());
    }
    if old == desired {
        return Ok(old.into());
    }
    let words = shell_words(old)
        .ok_or("Launch options have unmatched quotes. Correct them in Steam before repair.")?;
    let replacements = shell_words(desired).ok_or("Invalid generated launch options")?;
    if ordinary_launcher(desired)
        && words
            .iter()
            .any(|(_, _, w)| ["--exec", "--game", "--lang"].contains(&w.as_str()))
    {
        return Err("Spaced edition-routing arguments need review in Steam before consolidation; no options were guessed.".into());
    }
    let mut out = old.to_string();
    for (start, end, word) in words.iter().rev() {
        if word.starts_with("STEAM_COMPAT_DATA_PATH=")
            || word.starts_with("--exec=")
            || word.starts_with("--game=")
            || word.starts_with("--lang=")
        {
            out.replace_range(*start..*end, "");
        }
    }
    if !out.contains("%command%") {
        // Steam treats options without a command placeholder as application arguments.
        out = format!("%command% {}", out.trim());
    }
    if out.matches("%command%").count() != 1 {
        return Err(
            "Custom launch options need exactly one %command%. Review them in Steam before repair."
                .into(),
        );
    }
    let prefix = replacements
        .iter()
        .find(|(_, _, w)| w.starts_with("STEAM_COMPAT_DATA_PATH="))
        .ok_or("Missing generated prefix")?;
    out = format!("{} {}", &desired[prefix.0..prefix.1], out.trim());
    for (start, end, word) in replacements {
        if word.starts_with("--exec=") || word.starts_with("--game=") || word.starts_with("--lang=")
        {
            out.push(' ');
            out.push_str(&desired[start..end]);
        }
    }
    Ok(out)
}
pub fn upsert(root: &mut Object, mut desired: Object, install: &str) -> Result<u32> {
    let entries = vdf::ensure(root, "shortcuts")?;
    let title = vdf::string(&desired, "appname").unwrap_or("").to_string();
    let mut matches = vec![];
    for (n, (_, v)) in entries.iter().enumerate() {
        if let Value::Object(e) = v {
            if owned(e, install)
                && if vdf::string(&desired, "wowRustRole") == Some("shared_game") {
                    vdf::number(e, "appid") == vdf::number(&desired, "appid") && !native_addon(e)
                } else if vdf::string(&desired, "wowRustRole") == Some("launcher") {
                    vdf::string(e, "wowRustRole") == Some("launcher")
                } else {
                    vdf::string(e, "appname") == Some(&title)
                        || (vdf::string(&desired, "wowRustGame").is_some()
                            && vdf::string(e, "wowRustGame")
                                == vdf::string(&desired, "wowRustGame")
                            && vdf::string(e, "wowRustEdition")
                                == vdf::string(&desired, "wowRustEdition"))
                        || (vdf::string(&desired, "wowRustRole") == Some("launcher")
                            && vdf::string(e, "wowRustRole") == Some("launcher"))
                }
            {
                matches.push(n);
            }
        }
    }
    matches.sort_by_key(|n| match &entries[*n].1 {
        Value::Object(e) => u8::from(vdf::string(e, "appname") != Some(&title)),
        _ => 1,
    });
    let id = if let Some(n) = matches.first() {
        let old = match &entries[*n].1 {
            Value::Object(e) => e,
            _ => unreachable!(),
        };
        let id = vdf::number(old, "appid").ok_or("Managed shortcut has no ID")?;
        vdf::set(&mut desired, "appid", Value::I32(id));
        let native = native_addon(&desired);
        let mut merged = old.clone();
        for (k, v) in desired {
            if [
                "icon",
                "LastPlayTime",
                "AllowDesktopConfig",
                "AllowOverlay",
                "OpenVR",
                "ShortcutPath",
                "IsHidden",
            ]
            .iter()
            .any(|x| k.eq_ignore_ascii_case(x))
            {
                continue;
            }
            if k.eq_ignore_ascii_case("tags") {
                if let Value::Object(tags) = v {
                    let current = vdf::ensure(&mut merged, "tags")?;
                    for (_, value) in tags {
                        if !current.iter().any(|(_, old)| old == &value) {
                            current.push((current.len().to_string(), value));
                        }
                    }
                }
            } else if k.eq_ignore_ascii_case("LaunchOptions") {
                if let Value::Text(desired) = v {
                    let old = vdf::string(&merged, "LaunchOptions").unwrap_or("");
                    let options = if native {
                        desired
                    } else {
                        preserve_options(old, &desired)?
                    };
                    vdf::set(&mut merged, &k, Value::Text(options));
                }
            } else {
                vdf::set(&mut merged, &k, v);
            }
        }
        if vdf::string(&merged, "wowRustRole") == Some("shared_game") {
            merged.retain(|(k, _)| {
                !["wowRustGame", "wowRustEdition"]
                    .iter()
                    .any(|x| k.eq_ignore_ascii_case(x))
            });
        }
        entries[*n].1 = Value::Object(merged);
        let mut duplicates = matches.iter().skip(1).copied().collect::<Vec<_>>();
        duplicates.sort_unstable();
        for n in duplicates.into_iter().rev() {
            entries.remove(n);
        }
        id
    } else {
        let id = vdf::number(&desired, "appid").unwrap();
        if entries
            .iter()
            .any(|(_, v)| matches!(v,Value::Object(e)if vdf::number(e,"appid")==Some(id)))
        {
            return Err(
                "A shortcut ID collides with another installation. No changes were made.".into(),
            );
        }
        entries.push((String::new(), Value::Object(desired)));
        id
    };
    for (n, (k, _)) in entries.iter_mut().enumerate() {
        *k = n.to_string();
    }
    Ok(id)
}
pub fn remove_proton(bytes: &[u8], ids: &[u32]) -> Result<Vec<u8>> {
    let mut root =
        vdf::parse_text(std::str::from_utf8(bytes).map_err(|_| "Steam config is not UTF-8")?)?;
    let original = root.clone();
    let mut map = &mut root;
    for key in [
        "InstallConfigStore",
        "Software",
        "Valve",
        "Steam",
        "CompatToolMapping",
    ] {
        map = vdf::ensure(map, key)?;
    }
    map.retain(|(key, _)| !key.parse::<u32>().ok().is_some_and(|id| ids.contains(&id)));
    if root == original {
        Ok(bytes.to_vec())
    } else {
        Ok(vdf::text(&root)?.into_bytes())
    }
}

pub fn set_proton(bytes: Option<&[u8]>, ids: &[u32], proton: &str) -> Result<Vec<u8>> {
    if proton.is_empty()
        || proton.len() > 100
        || !proton
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_- .".contains(c))
    {
        return Err("Invalid Proton tool name".into());
    }
    let mut root = match bytes {
        Some(b) => {
            vdf::parse_text(std::str::from_utf8(b).map_err(|_| "Steam config is not UTF-8")?)?
        }
        None => return Err("Steam config.vdf missing. Run Steam once first.".into()),
    };
    let original = root.clone();
    let mut map = &mut root;
    for key in [
        "InstallConfigStore",
        "Software",
        "Valve",
        "Steam",
        "CompatToolMapping",
    ] {
        map = vdf::ensure(map, key)?;
    }
    for id in ids {
        let entry = vdf::ensure(map, &id.to_string())?;
        vdf::set(entry, "name", Value::Text(proton.into()));
        vdf::set(entry, "config", Value::Text(String::new()));
        vdf::set(entry, "priority", Value::Text("250".into()));
    }
    if root == original {
        return Ok(bytes.unwrap().to_vec());
    }
    Ok(vdf::text(&root)?.into_bytes())
}
pub fn proton_for(config: &[u8], id: u32) -> Result<Option<String>> {
    let root =
        vdf::parse_text(std::str::from_utf8(config).map_err(|_| "Steam config is not UTF-8")?)?;
    let mut o = &root;
    for key in [
        "InstallConfigStore",
        "Software",
        "Valve",
        "Steam",
        "CompatToolMapping",
    ] {
        match vdf::object(o, key) {
            Some(v) => o = v,
            None => return Ok(None),
        }
    }
    Ok(vdf::object(o, &id.to_string())
        .and_then(|x| vdf::string(x, "name"))
        .map(str::to_string))
}
/// Read a single leading compatdata assignment without evaluating shell syntax.
pub fn compat_prefix(options: &str) -> Option<PathBuf> {
    let words = shell_words(options)?;
    if !words.iter().any(|(_, _, w)| w == "%command%") {
        return None;
    }
    let mut values = words
        .iter()
        .filter_map(|(_, _, w)| w.strip_prefix("STEAM_COMPAT_DATA_PATH="));
    let path = values.next()?;
    if values.next().is_some() || path.contains(['$', '`', '\\']) || !Path::new(path).is_absolute()
    {
        return None;
    }
    Some(PathBuf::from(path))
}

/// Installer-created Linux addon tiles are independent of a WoW prefix.
pub fn native_addon(entry: &Object) -> bool {
    vdf::string(entry, "exe") == Some("\"/usr/bin/env\"")
        && vdf::string(entry, "LaunchOptions").is_some_and(|s| {
            s.starts_with("APPIMAGE_EXTRACT_AND_RUN=1 ")
                // Steam may discard unknown VDF fields on save. The native
                // host route survives; retain it conservatively without relying
                // on wowRustNativeAddon. Creation still validates the Gear entry.
                || s.strip_prefix("gio launch ")
                    .is_some_and(|target| !target.trim().is_empty())
        })
}

pub fn bindings(root: &Object, install: &str) -> Vec<crate::model::ShortcutBinding> {
    vdf::object(root, "shortcuts")
        .into_iter()
        .flatten()
        .filter_map(|(_, v)| {
            let Value::Object(e) = v else { return None };
            if !owned(e, install) || native_addon(e) {
                return None;
            }
            Some(crate::model::ShortcutBinding {
                shared: vdf::string(e, "wowRustRole") == Some("shared_game"),
                appid: vdf::number(e, "appid")?,
                edition: vdf::string(e, "wowRustEdition").map(str::to_string),
                game: vdf::string(e, "wowRustGame").map(str::to_string),
                launcher: vdf::string(e, "wowRustRole") == Some("launcher"),
            })
        })
        .collect()
}
pub fn restore_identity(root: &mut Object, i: &crate::model::Installation) -> Result<()> {
    for (_, v) in vdf::ensure(root, "shortcuts")? {
        let Value::Object(e) = v else { continue };
        let Some(id) = vdf::number(e, "appid") else {
            continue;
        };
        if native_addon(e) {
            continue;
        }
        let binding = i.bindings.iter().find(|b| b.appid == id);
        if !owned(e, &i.id) && binding.is_none() {
            continue;
        }
        let target = Path::new(vdf::string(e, "exe").unwrap_or("").trim_matches('"'));
        let options = vdf::string(e, "LaunchOptions").unwrap_or("");
        let same_target = i
            .launcher
            .as_ref()
            .is_some_and(|p| launcher_alias(target, p))
            || target == i.root.join("Battle.net-Setup.exe");
        // An ID alone is not ownership. Reject collisions with changed targets/prefixes.
        let missing_prefix = binding.is_some()
            && shell_words(options).is_some_and(|words| {
                !words
                    .iter()
                    .any(|(_, _, word)| word.starts_with("STEAM_COMPAT_DATA_PATH="))
            });
        if !same_target || (compat_prefix(options).as_ref() != Some(&i.prefix) && !missing_prefix) {
            continue;
        }
        if let Some(b) = binding {
            if let Some(id) = &b.edition {
                vdf::set(e, "wowRustEdition", Value::Text(id.clone()));
            }
            if let Some(game) = &b.game {
                vdf::set(e, "wowRustGame", Value::Text(game.clone()));
            }
            if b.shared {
                vdf::set(e, "wowRustRole", Value::Text("shared_game".into()));
                e.retain(|(k, _)| {
                    !["wowRustGame", "wowRustEdition"]
                        .iter()
                        .any(|x| k.eq_ignore_ascii_case(x))
                });
            }
            if b.launcher {
                vdf::set(e, "wowRustRole", Value::Text("launcher".into()));
            }
            if !owned(e, &i.id) {
                let tags = vdf::ensure(e, "tags")?;
                tags.push((
                    tags.len().to_string(),
                    Value::Text(format!("wow-rust:{}", i.id)),
                ));
            }
        } else if vdf::string(e, "wowRustEdition").is_none()
            && vdf::string(e, "wowRustRole").is_none()
        {
            // Upgrade old state by the constrained launch route, not the display name.
            if let Some(id) = edition(options) {
                vdf::set(e, "wowRustEdition", Value::Text(id.into()));
            } else if !options.contains("--game") && !options.contains("--exec") {
                vdf::set(e, "wowRustRole", Value::Text("launcher".into()));
            }
        }
    }
    Ok(())
}
