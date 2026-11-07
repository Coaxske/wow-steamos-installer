//! Publisher downloads. Each redirect is checked before the next HTTP request.
use crate::{fsutil, Result};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    thread,
    time::Duration,
};
pub fn is_appimage(b: &[u8]) -> bool {
    b.len() > 1024
        && b.starts_with(b"\x7fELF\x02\x01")
        && b.get(8..11) == Some(b"AI\x02")
        && b.get(18..20) == Some(&[62, 0])
}
pub fn allowed(url: &str, manager: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let Some((host, path)) = rest.split_once('/') else {
        return false;
    };
    if url.chars().any(|c| c.is_control() || c == '\\' || c == ' ')
        || host.contains(['@', ':', '%'])
    {
        return false;
    }
    match manager {
        "wowup_cf" | "wowup_wago" => {
            let repo = if manager == "wowup_cf" {
                "WowUp.CF"
            } else {
                "WowUp"
            };
            (host == "api.github.com" && path == format!("repos/WowUp/{repo}/releases/latest"))
                || (host == "github.com"
                    && path.starts_with(&format!("WowUp/{repo}/releases/download/")))
                || host == "release-assets.githubusercontent.com"
        }
        "curseforge" => ["download.overwolf.com", "curseforge.overwolf.com"].contains(&host),
        _ => false,
    }
}
pub fn asset(v: &Value, manager: &str) -> Result<(String, Option<String>)> {
    if v["prerelease"] == true || v["draft"] == true {
        return Err("Publisher returned a prerelease; choose a stable release.".into());
    }
    let a = v["assets"]
        .as_array()
        .ok_or("Publisher release has no assets")?;
    let candidates: Vec<_> = a
        .iter()
        .filter(|x| {
            let n = x["name"].as_str().unwrap_or("");
            n.ends_with(".AppImage")
                && !n.to_ascii_lowercase().contains("arm")
                && !n.contains("i386")
        })
        .collect();
    if candidates.len() != 1 {
        return Err("Could not identify one x86-64 AppImage. Download from the publisher page and select it here.".into());
    }
    let a = candidates[0];
    let url = a["browser_download_url"]
        .as_str()
        .ok_or("Missing download URL")?;
    if !allowed(url, manager) {
        return Err("Publisher asset URL is outside the allowed repository".into());
    }
    let hash = match a["digest"].as_str() {
        Some(s) => {
            let h = s
                .strip_prefix("sha256:")
                .ok_or("Unsupported publisher checksum")?;
            if h.len() != 64 || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("Invalid publisher checksum".into());
            }
            Some(h.to_lowercase())
        }
        None => None,
    };
    Ok((url.into(), hash))
}
fn fetch(url: &str, manager: &str, dest: &Path, limit: u64) -> Result<()> {
    let mut next = url.to_string();
    let header = dest.with_extension("headers");
    let result = (|| {
        for _ in 0..6 {
            if !allowed(&next, manager) {
                return Err("Download redirect left the publisher allowlist.".into());
            }
            let mut child = crate::engine::host_command("curl")
                .args([
                    "--silent",
                    "--show-error",
                    "--proto",
                    "=https",
                    "--connect-timeout",
                    "20",
                    "--max-time",
                    "600",
                    "--max-filesize",
                    &limit.to_string(),
                    "--user-agent",
                    "WoW-SteamOS-Rust",
                    "--dump-header",
                ])
                .arg(&header)
                .arg("--output")
                .arg(dest)
                .arg(&next)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| "curl is required for downloads")?;
            loop {
                if crate::cancelled().is_err() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Download cancelled; existing files are unchanged.".into());
                }
                if fs::metadata(dest).is_ok_and(|m| m.len() > limit) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Download exceeds its size limit".into());
                }
                match child.try_wait().map_err(|e| e.to_string())? {
                    Some(status) => {
                        if !status.success() {
                            return Err("Download failed. Check the connection and retry, or use the publisher page.".into());
                        }
                        break;
                    }
                    None => thread::sleep(Duration::from_millis(100)),
                }
            }
            let text = fs::read_to_string(&header).map_err(|e| e.to_string())?;
            let mut status = 0;
            let mut location = None;
            for line in text.lines() {
                if line.starts_with("HTTP/") {
                    status = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse::<u16>().ok())
                        .unwrap_or(0);
                    location = None;
                } else if let Some((k, v)) = line.split_once(':') {
                    if k.eq_ignore_ascii_case("location") {
                        location = Some(v.trim().to_string());
                    }
                }
            }
            if (200..300).contains(&status) {
                return Ok(());
            }
            if (300..400).contains(&status) {
                next = location.ok_or("Missing redirect destination")?;
                continue;
            }
            return Err(format!(
                "Publisher returned HTTP {status}; no installed file was replaced."
            ));
        }
        Err("Too many publisher redirects".into())
    })();
    let _ = fs::remove_file(header);
    if result.is_err() {
        let _ = fs::remove_file(dest);
    }
    result
}
/// Published Linux AppImage link on Overwolf's official CurseForge page.
/// Verified 2026-10-26; no guessed API, mirror, .deb or store package.
pub const CURSEFORGE_OFFICIAL: &str =
    "https://curseforge.overwolf.com/downloads/curseforge-latest-linux.AppImage";
fn cached_curseforge(dir: &Path) -> Option<PathBuf> {
    let record: Value =
        serde_json::from_slice(&fs::read(dir.join("curseforge-official.json")).ok()?).ok()?;
    if record["source"] != CURSEFORGE_OFFICIAL {
        return None;
    }
    let name = record["file"].as_str()?;
    if !name.starts_with("curseforge-")
        || !name.ends_with(".AppImage")
        || Path::new(name).components().count() != 1
    {
        return None;
    }
    let path = dir.join(name);
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 512 * 1024 * 1024
    {
        return None;
    }
    let bytes = fs::read(&path).ok()?;
    if !is_appimage(&bytes) || record["sha256"].as_str() != Some(fsutil::hash(&bytes).as_str()) {
        return None;
    }
    Some(path)
}
pub fn download(data: &Path, manager: &str, url: Option<&str>) -> Result<PathBuf> {
    if !["wowup_cf", "wowup_wago", "curseforge"].contains(&manager) {
        return Err("Select a supported addon manager".into());
    }
    let dir = data.join("addon-downloads");
    fsutil::private_dir(&dir)?;
    let automatic = manager == "curseforge" && url.map_or(true, |u| u.trim().is_empty());
    if automatic {
        if let Some(cached) = cached_curseforge(&dir) {
            return Ok(cached);
        }
    }
    let (url, checksum) = if manager == "curseforge" {
        let u = if automatic {
            CURSEFORGE_OFFICIAL
        } else {
            url.unwrap_or("")
        };
        if !allowed(u, manager) {
            return Err("Choose the official Linux AppImage; its link must remain on the publisher download hosts.".into());
        }
        (u.into(), None)
    } else {
        let repo = if manager == "wowup_cf" {
            "WowUp.CF"
        } else {
            "WowUp"
        };
        let metadata = dir.join(format!("{}.json", fsutil::unique()));
        fetch(
            &format!("https://api.github.com/repos/WowUp/{repo}/releases/latest"),
            manager,
            &metadata,
            4 * 1024 * 1024,
        )?;
        let result = fsutil::read(&metadata)
            .and_then(|b| {
                serde_json::from_slice(&b).map_err(|_| "Publisher metadata is invalid".into())
            })
            .and_then(|v| asset(&v, manager));
        let _ = fs::remove_file(metadata);
        result?
    };
    let stage = dir.join(format!("{}.part", fsutil::unique()));
    fetch(&url, manager, &stage, 512 * 1024 * 1024)?;
    let result = (|| {
        let bytes = fsutil::read(&stage)?;
        if !is_appimage(&bytes) {
            return Err(
                "The download is not an x86-64 Linux AppImage; it was not installed.".into(),
            );
        }
        if checksum.is_some_and(|hash| fsutil::hash(&bytes) != hash) {
            return Err("Publisher checksum did not match; it was not installed.".into());
        }
        // Version by content: updating a download never replaces an executable in use.
        let target = dir.join(format!(
            "{manager}-{}.AppImage",
            &fsutil::hash(&bytes)[..16]
        ));
        fsutil::atomic(&target, &bytes)?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        if automatic {
            fsutil::json(
                &dir.join("curseforge-official.json"),
                &serde_json::json!({"source":CURSEFORGE_OFFICIAL,"file":target.file_name().and_then(|n|n.to_str()).ok_or("Invalid downloaded filename")?,"sha256":fsutil::hash(&bytes)}),
            )?;
        }
        Ok(target)
    })();
    let _ = fs::remove_file(stage);
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_redirect_lookalikes() {
        for u in [
            "http://github.com/WowUp/WowUp/releases/download/v1/a",
            "https://github.com.evil/WowUp/WowUp/releases/download/v1/a",
            "https://github.com/other/repo/releases/download/v1/a",
            "https://user@github.com/WowUp/WowUp/releases/download/v1/a",
        ] {
            assert!(!allowed(u, "wowup_wago"));
        }
    }
    #[test]
    fn requires_actual_x64_appimage() {
        let mut b = vec![0; 2048];
        b[..6].copy_from_slice(b"\x7fELF\x02\x01");
        b[8..11].copy_from_slice(b"AI\x02");
        b[18] = 62;
        assert!(is_appimage(&b));
        b[18] = 183;
        assert!(!is_appimage(&b));
    }
    #[test]
    fn refuses_ambiguous_assets() {
        let a = serde_json::json!({"name":"a.AppImage","browser_download_url":"https://github.com/WowUp/WowUp/releases/download/v1/a.AppImage"});
        assert!(asset(&serde_json::json!({"assets":[a.clone(),a]}), "wowup_wago").is_err());
    }
    #[test]
    fn checks_publisher_digest_format() {
        assert!(asset(&serde_json::json!({"assets":[{"name":"a.AppImage","browser_download_url":"https://github.com/WowUp/WowUp/releases/download/v1/a.AppImage","digest":"sha256:bad"}]}),"wowup_wago").is_err());
    }
}

/// Read-only handoff from a fresh scan; never edits a manager's private database.
pub fn game_handoff(game: &crate::model::Game) -> Option<PathBuf> {
    let folder = game.path.canonicalize().ok()?;
    let executable = game.executable.canonicalize().ok()?;
    if !executable.is_file() || executable.parent()? != folder {
        return None;
    }
    let prefix = game.prefix.as_ref()?.canonicalize().ok()?;
    if !folder.starts_with(prefix) {
        return None;
    }
    // CurseForge's supported picker accepts the World of Warcraft parent folder.
    let parent = folder.parent()?;
    if parent.file_name()?.to_str()? == "World of Warcraft" {
        Some(parent.to_path_buf())
    } else {
        Some(folder)
    }
}
#[cfg(test)]
mod handoff_tests {
    use super::*;
    #[test]
    fn verified_folder_spaces_missing_and_escape() {
        let temp = std::env::temp_dir().join(format!("wow-handoff-{}", std::process::id()));
        let prefix = temp.join("prefix");
        let folder = prefix.join("Program Files (x86)/World of Warcraft/_classic_era_");
        fs::create_dir_all(&folder).unwrap();
        let exe = folder.join("WowClassic.exe");
        fs::write(&exe, b"fixture").unwrap();
        let mut g = crate::model::Game {
            id: "era".into(),
            name: "Classic Era".into(),
            path: folder.clone(),
            executable: exe.clone(),
            prefix: Some(prefix.clone()),
        };
        assert_eq!(
            game_handoff(&g),
            Some(folder.parent().unwrap().to_path_buf())
        );
        g.prefix = Some(temp.join("unavailable"));
        assert_eq!(game_handoff(&g), None);
        g.prefix = Some(prefix);
        fs::remove_file(&exe).unwrap();
        assert_eq!(game_handoff(&g), None);
        let outside = temp.join("outside.exe");
        fs::write(&outside, b"fixture").unwrap();
        std::os::unix::fs::symlink(&outside, &exe).unwrap();
        assert_eq!(game_handoff(&g), None);
        fs::remove_dir_all(temp).unwrap();
    }
}

/// Validate the required local input before installing any manager/runtime.
pub fn validate_appimage_input(raw: &str) -> Result<PathBuf> {
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    let hint="Choose the official Linux CurseForge AppImage with Browse, or download it from the publisher first. No software was installed.";
    if raw.trim().is_empty() {
        return Err(hint.into());
    }
    let path = Path::new(raw)
        .canonicalize()
        .map_err(|_| format!("The selected AppImage is unavailable. {hint}"))?;
    let meta =
        fs::metadata(&path).map_err(|_| format!("The selected AppImage cannot be read. {hint}"))?;
    if !meta.is_file() || meta.permissions().mode() & 0o444 == 0 {
        return Err(format!(
            "Select a readable AppImage file, not a folder. {hint}"
        ));
    }
    if meta.len() <= 1024 {
        return Err(format!("This is not a Linux x86_64 AppImage. {hint}"));
    }
    let mut header = vec![0; 1025];
    fs::File::open(&path)
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|_| format!("The selected AppImage cannot be read. {hint}"))?;
    if !is_appimage(&header) {
        return Err(format!("This is not a Linux x86_64 AppImage. {hint}"));
    }
    Ok(path)
}
#[cfg(test)]
mod input_validation_tests {
    use super::*;
    #[test]
    fn c12_actionable_input_validation() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("wow-c12-appimage-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(validate_appimage_input("").unwrap_err().contains("Browse"));
        assert!(
            validate_appimage_input(dir.join("missing").to_str().unwrap())
                .unwrap_err()
                .contains("unavailable")
        );
        assert!(validate_appimage_input(dir.to_str().unwrap()).is_err());
        let file = dir.join("official download.AppImage");
        fs::write(&file, b"<html>not a payload</html>").unwrap();
        assert!(validate_appimage_input(file.to_str().unwrap())
            .unwrap_err()
            .contains("x86_64"));
        let mut bytes = vec![0; 2048];
        bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
        bytes[8..11].copy_from_slice(b"AI\x02");
        bytes[18] = 62;
        fs::write(&file, bytes).unwrap();
        assert_eq!(
            validate_appimage_input(file.to_str().unwrap()).unwrap(),
            file
        );
        fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
        assert!(validate_appimage_input(file.to_str().unwrap()).is_err());
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod candidate24_cache_tests {
    use super::*;
    #[test]
    fn c24_official_cache_rejects_tampering_and_escapes() {
        let dir = std::env::temp_dir().join(format!("c24-cache-{}", fsutil::unique()));
        fs::create_dir_all(&dir).unwrap();
        let mut bytes = vec![0; 2048];
        bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
        bytes[8..11].copy_from_slice(b"AI\x02");
        bytes[18] = 62;
        let name = "curseforge-fixture.AppImage";
        let path = dir.join(name);
        fs::write(&path, &bytes).unwrap();
        let receipt = serde_json::json!({"source":CURSEFORGE_OFFICIAL,"file":name,"sha256":fsutil::hash(&bytes)});
        fsutil::json(&dir.join("curseforge-official.json"), &receipt).unwrap();
        assert_eq!(cached_curseforge(&dir), Some(path.clone()));
        fs::write(&path, b"tampered").unwrap();
        assert!(cached_curseforge(&dir).is_none());
        fs::write(&path, &bytes).unwrap();
        for name in [
            "../curseforge-fixture.AppImage",
            "/tmp/curseforge-fixture.AppImage",
            "other.AppImage",
        ] {
            let mut bad = receipt.clone();
            bad["file"] = name.into();
            fsutil::json(&dir.join("curseforge-official.json"), &bad).unwrap();
            assert!(cached_curseforge(&dir).is_none());
        }
        let mut bad = receipt;
        bad["source"] = "https://untrusted.invalid/file".into();
        fsutil::json(&dir.join("curseforge-official.json"), &bad).unwrap();
        assert!(cached_curseforge(&dir).is_none());
        assert!(allowed(CURSEFORGE_OFFICIAL, "curseforge"));
        assert!(!allowed(
            "https://curseforge.overwolf.com.evil/download.AppImage",
            "curseforge"
        ));
        fs::remove_dir_all(dir).unwrap();
    }
}
