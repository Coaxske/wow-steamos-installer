//! Associate bounded public installation metadata with a specific client folder.
use crate::{catalog, fsutil, Result};
use std::path::Path;
pub fn valid(p: &str) -> bool {
    p.len() <= 80
        && (p == "wow" || p == "wowt" || p == "wowxptr" || p.starts_with("wow_"))
        && p.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}
pub fn product_for(id: &str) -> Option<&'static str> {
    Some(match id {
        "retail" => "wow",
        "forever" => "wow_forever",
        "era" => "wow_classic_era",
        "progression" => "wow_classic",
        "anniversary" => "wow_classic_anniversary",
        "ptr" => "wowt",
        "ptr_next" => "wowxptr",
        "beta" => "wow_beta",
        "classic_ptr" => "wow_classic_ptr",
        "classic_beta" => "wow_classic_beta",
        "era_ptr" => "wow_classic_era_ptr",
        "alpha" => "wow_alpha",
        "titan" => "wow_classic_titan",
        _ => return None,
    })
}
pub fn parse(text: &str) -> Vec<String> {
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    let head: Vec<_> = lines
        .next()
        .unwrap_or("")
        .split('|')
        .map(|s| s.split('!').next().unwrap_or(""))
        .collect();
    let Some(product) = head.iter().position(|x| *x == "Product") else {
        return vec![];
    };
    let active = head.iter().position(|x| *x == "Active");
    let mut out = vec![];
    for line in lines.take(5000) {
        let fields: Vec<_> = line.split('|').collect();
        if active.is_some_and(|a| fields.get(a) != Some(&"1")) {
            continue;
        }
        if let Some(p) = fields.get(product) {
            if valid(p) && !out.iter().any(|x| x == p) {
                out.push((*p).to_string());
            }
        }
    }
    out
}
fn read(folder: &Path) -> Vec<String> {
    let path = folder.join(".build.info");
    let Ok(m) = std::fs::symlink_metadata(&path) else {
        return vec![];
    };
    if !m.is_file() || m.len() > 1024 * 1024 {
        return vec![];
    }
    fsutil::read(&path)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .map(|s| parse(&s))
        .unwrap_or_default()
}
pub fn installed(folder: &Path) -> Option<String> {
    let local = read(folder);
    let id = catalog::by_folder(folder.file_name()?.to_str()?).map(|e| e.id);
    let expected = id.and_then(product_for);
    let parent = folder.parent().map(read).unwrap_or_default();
    if let Some(p) = expected {
        if local.iter().chain(parent.iter()).any(|x| x == p) {
            return Some(p.into());
        }
    }
    // Unknown products require an unambiguous metadata file beside the executable.
    // Reused known beta folders never become a different stable edition.
    if id.is_none() && local.len() == 1 {
        return local.into_iter().next();
    }
    None
}
pub fn options(prefix: &Path, id: &str, product: Option<&str>) -> Result<String> {
    if let Some(code) = catalog::by_id(id).and_then(|e| e.command) {
        return crate::steam::launch_options(prefix, Some(code));
    }
    let mut base = crate::steam::launch_options(prefix, None)?;
    if let Some(p) = product.or_else(|| product_for(id)) {
        if !valid(p) {
            return Err("Invalid Battle.net product".into());
        }
        base.push_str(&format!(" --game={p}"));
    }
    Ok(base)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_metadata_rules() {
        assert_eq!(
            parse("Product!STRING:0|Active!DEC:1\nwow|1\nwow_beta|0\nevil|1"),
            vec!["wow"]
        );
        assert!(!valid("wow;rm"));
    }
    #[test]
    fn selection_is_not_launch_code() {
        let p = Path::new("/example/prefix");
        assert!(options(p, "progression", None)
            .unwrap()
            .ends_with("--game=wow_classic"));
        assert!(options(p, "forever", None).unwrap().contains("launch WoWF"));
    }
}
