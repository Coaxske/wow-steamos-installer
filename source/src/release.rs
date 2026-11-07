//! Exact-build identity and manually observed device evidence validation.
use crate::{fsutil, Result};
use serde_json::Value;
use std::path::Path;
#[path = "../build_support.rs"]
pub mod inputs;
pub const SOURCE: &str = env!("WOW_SOURCE_SHA256");
pub const AUTOMATED: &[&str] = &[
    "fresh_setup",
    "rerun",
    "repair",
    "folder_links",
    "addon_targets",
    "link_removal",
    "open_folders",
    "preservation",
    "package_identity",
    "privacy_licenses",
];
pub fn evidence(hash: &str, v: &Value) -> Result<()> {
    if v["schema"] != 3 || v["version"] != env!("CARGO_PKG_VERSION") || v["archive_sha256"] != hash
    {
        return Err("Evidence must identify this exact release executable and version.".into());
    }
    if AUTOMATED.iter().any(|k| v["automated"][k] != "passed") {
        return Err("Required automated folder/package checks are pending or failed.".into());
    }
    let base = &v["inherited_C43"];
    if base["portable_sha256"] != "36b8541321e80ec6bd681d83e419a6b19eb5f7b2f954c4a33ebbd5ef3f41e41b"
        || base["manual_archive_sha256"]
            != "63de7b41a06986b5b2e337b4e0fc6357c84a4036c9f002234d9bbe8e5e1854f8"
        || base["Era_Retail_Game_Mode"] != "passed_with_recorded_limits"
        || base["Era_pointer"] != "passed_existing_generic_layout"
        || v["launch_controller_diff"] != "unchanged_except_folder_hooks"
    {
        return Err("Identify the accepted C43 Game Mode evidence and reviewed unaffected launch/controller behavior.".into());
    }
    if v["Forever"]["current_version"] != "1.60.1.70862"
        || v["Forever"]["current_tested"] != false
        || v["Forever"]["accepted_historical_limit"] != true
    {
        return Err("Retain the accepted untested current Forever limitation.".into());
    }
    if v["platforms"]["Steam_Deck"] != "C43_Game_Mode_with_C44_automated_folder_checks"
        || v["platforms"]["Bazzite"] != "testing_in_progress"
    {
        return Err("Platform coverage must distinguish inherited device acceptance from automated successor checks.".into());
    }
    Ok(())
}
pub fn source(root: &Path) -> Result<()> {
    if inputs::fingerprint(root)? != SOURCE {
        return Err("Source changed since compilation. Rebuild before packaging.".into());
    }
    Ok(())
}
pub fn hash_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
pub fn manifest(binary: &Path) -> Result<Value> {
    Ok(
        serde_json::json!({"schema":1,"version":env!("CARGO_PKG_VERSION"),"source_sha256":SOURCE,"binary_sha256":hash_file(binary)?,"architecture":std::env::consts::ARCH,"device_testing":"pending"}),
    )
}
pub fn check_manifest(binary: &Path, v: &Value) -> Result<()> {
    if &manifest(binary)? != v {
        return Err("Executable or build identity does not match.".into());
    }
    Ok(())
}
pub fn read(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fsutil::read(path)?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record() -> Value {
        serde_json::json!({"schema":3,"version":env!("CARGO_PKG_VERSION"),"archive_sha256":"fixture",
 "automated":AUTOMATED.iter().map(|k|((*k).into(),serde_json::json!("passed"))).collect::<serde_json::Map<String,Value>>(),
 "inherited_C43":{"portable_sha256":"36b8541321e80ec6bd681d83e419a6b19eb5f7b2f954c4a33ebbd5ef3f41e41b","manual_archive_sha256":"63de7b41a06986b5b2e337b4e0fc6357c84a4036c9f002234d9bbe8e5e1854f8","Era_Retail_Game_Mode":"passed_with_recorded_limits","Era_pointer":"passed_existing_generic_layout"},
 "launch_controller_diff":"unchanged_except_folder_hooks",
 "Forever":{"current_version":"1.60.1.70862","current_tested":false,"accepted_historical_limit":true},
 "platforms":{"Steam_Deck":"C43_Game_Mode_with_C44_automated_folder_checks","Bazzite":"testing_in_progress"}})
    }
    #[test]
    fn exact_executable_and_inherited_device_evidence() {
        assert!(evidence("fixture", &record()).is_ok());
        assert!(evidence("changed", &record()).is_err());
    }
    #[test]
    fn missing_folder_gate_refuses() {
        let mut v = record();
        v["automated"]["folder_links"] = serde_json::json!("pending");
        assert!(evidence("fixture", &v).is_err());
    }
    #[test]
    fn untested_forever_cannot_be_promoted() {
        let mut v = record();
        v["Forever"]["current_tested"] = serde_json::json!(true);
        assert!(evidence("fixture", &v).is_err());
    }
}
