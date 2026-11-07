//! Three-way merge: apply only reviewed VDF fields, preserve unrelated changes.
use crate::{
    vdf::{self, Object, Value},
    Result,
};
use std::{collections::BTreeSet, path::Path};
fn unique(o: &Object) -> Result<()> {
    let mut keys = BTreeSet::new();
    for (k, v) in o {
        if !keys.insert(k.to_ascii_lowercase()) {
            return Err("Duplicate VDF keys require manual review.".into());
        }
        if let Value::Object(o) = v {
            unique(o)?;
        }
    }
    Ok(())
}
fn eq(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            a.len() == b.len() && a.iter().all(|(k, v)| eq(Some(v), vdf::get(b, k)))
        }
        _ => a == b,
    }
}
fn object(before: &Object, after: &Object, current: &Object) -> Result<Object> {
    let mut out = current.clone();
    let keys: BTreeSet<_> = before
        .iter()
        .chain(after)
        .map(|(k, _)| k.to_ascii_lowercase())
        .collect();
    for key in keys {
        let b = vdf::get(before, &key);
        let a = vdf::get(after, &key);
        let c = vdf::get(current, &key);
        if eq(b, a) || eq(a, c) {
            continue;
        }
        let value = if eq(b, c) {
            a.cloned()
        } else {
            match (b,a,c) {
                (Some(Value::Object(b)),Some(Value::Object(a)),Some(Value::Object(c))) => Some(Value::Object(object(b,a,c)?)),
                // A previously absent parent can gain unrelated children after review.
                (None,Some(Value::Object(a)),Some(Value::Object(c))) => Some(Value::Object(object(&vec![],a,c)?)),
                _ => return Err("A setting changed in Steam that this operation also changes. Nothing was overwritten. Review again or stop this operation.".into()),
            }
        };
        match value {
            Some(v) => vdf::set(&mut out, &key, v),
            None => out.retain(|(k, _)| !k.eq_ignore_ascii_case(&key)),
        }
    }
    Ok(out)
}
fn parse(path: &Path, bytes: &[u8]) -> Result<Object> {
    if path.file_name().is_some_and(|n| n == "shortcuts.vdf") {
        let mut root = vdf::parse_binary(bytes)?;
        for (k, v) in vdf::ensure(&mut root, "shortcuts")? {
            let Value::Object(e) = v else {
                return Err("Invalid shortcut".into());
            };
            *k = vdf::number(e, "appid")
                .ok_or("Missing shortcut ID")?
                .to_string();
        }
        unique(&root)?;
        Ok(root)
    } else {
        let root =
            vdf::parse_text(std::str::from_utf8(bytes).map_err(|_| "Invalid VDF encoding")?)?;
        unique(&root)?;
        Ok(root)
    }
}
pub fn apply(path: &Path, before: &[u8], after: &[u8], current: &[u8]) -> Result<Vec<u8>> {
    if before == after || current == after {
        return Ok(current.to_vec());
    }
    if current == before {
        return Ok(after.to_vec());
    }
    if !["config.vdf", "shortcuts.vdf"]
        .iter()
        .any(|n| path.file_name().is_some_and(|p| p == *n))
    {
        return Err(
            "Local setup data changed. Stop this operation and inspect the saved progress.".into(),
        );
    }
    let b = parse(path, before)?;
    let a = parse(path, after)?;
    let c = parse(path, current)?;
    let mut result = object(&b, &a, &c)?;
    if eq(
        Some(&Value::Object(result.clone())),
        Some(&Value::Object(c)),
    ) {
        return Ok(current.to_vec());
    }
    if path.file_name().is_some_and(|n| n == "shortcuts.vdf") {
        for (n, (k, _)) in vdf::ensure(&mut result, "shortcuts")?
            .iter_mut()
            .enumerate()
        {
            *k = n.to_string();
        }
        vdf::binary(&result)
    } else {
        Ok(vdf::text(&result)?.into_bytes())
    }
}
