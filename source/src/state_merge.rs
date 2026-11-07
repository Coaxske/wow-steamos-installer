//! Undo only the state fields changed by a transaction, preserving later progress.
use crate::{model::State, workflow, Result};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

fn conflict() -> String {
    "Setup records changed in a way that conflicts with Undo. Nothing was overwritten. Use shortcut repair or keep the current setup.".into()
}
fn stage(v: &Value) -> Option<usize> {
    [
        "launcher_install",
        "games",
        "test_games",
        "addons",
        "complete",
    ]
    .iter()
    .position(|s| v.as_str() == Some(s))
}
fn keyed(v: &Value) -> Result<Map<String, Value>> {
    let mut out = Map::new();
    for i in v.as_array().ok_or_else(conflict)? {
        let id = i["id"].as_str().ok_or_else(conflict)?;
        if out.insert(id.into(), i.clone()).is_some() {
            return Err(conflict());
        }
    }
    Ok(out)
}
fn merge(b: &Value, a: &Value, c: &Value, path: &[String]) -> Result<Value> {
    if b == a || a == c {
        return Ok(c.clone());
    }
    if b == c {
        return Ok(a.clone());
    }
    if path.len() == 1 && path[0] == "installations" {
        let (b, a, c) = (keyed(b)?, keyed(a)?, keyed(c)?);
        let out = fields(&b, &a, &c, path)?;
        // Keep current order; append restored installations deterministically.
        let mut ids: Vec<_> = c.keys().cloned().collect();
        for id in out.keys() {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        return Ok(Value::Array(
            ids.iter().filter_map(|id| out.get(id).cloned()).collect(),
        ));
    }
    // Finishing the guided steps after repair is expected. Rewind that copy to
    // the stage belonging to the restored shortcuts; never rewind removed copies.
    if path.len() == 3 && path[0] == "installations" && path[2] == "stage" {
        if let (Some(base), Some(target), Some(now)) = (stage(b), stage(a), stage(c)) {
            if target < base && now >= base {
                return Ok(a.clone());
            }
        }
    }
    match (b, a, c) {
        (Value::Object(b), Value::Object(a), Value::Object(c)) => {
            Ok(Value::Object(fields(b, a, c, path)?))
        }
        _ => Err(conflict()),
    }
}
fn fields(
    b: &Map<String, Value>,
    a: &Map<String, Value>,
    c: &Map<String, Value>,
    path: &[String],
) -> Result<Map<String, Value>> {
    let mut out = c.clone();
    let keys: BTreeSet<_> = b.keys().chain(a.keys()).collect();
    for key in keys {
        let (bv, av, cv) = (b.get(key), a.get(key), c.get(key));
        if bv == av || av == cv {
            continue;
        }
        let value = if bv == cv {
            av.cloned()
        } else {
            let mut child = path.to_vec();
            child.push(key.clone());
            match (bv, av, cv) {
                (Some(b), Some(a), Some(c)) => Some(merge(b, a, c, &child)?),
                _ => return Err(conflict()),
            }
        };
        match value {
            Some(v) => {
                out.insert(key.clone(), v);
            }
            None => {
                out.remove(key);
            }
        }
    }
    Ok(out)
}
pub fn undo(base: &[u8], restored: &[u8], current: &[u8]) -> Result<Vec<u8>> {
    let parse = |bytes: &[u8]| -> Result<Value> {
        let state: State = serde_json::from_slice(bytes).map_err(|_| conflict())?;
        workflow::state(&state)?;
        serde_json::to_value(state).map_err(|_| conflict())
    };
    let out = merge(&parse(base)?, &parse(restored)?, &parse(current)?, &[])?;
    let state: State = serde_json::from_value(out).map_err(|_| conflict())?;
    workflow::state(&state)?;
    serde_json::to_vec_pretty(&state).map_err(|_| conflict())
}
