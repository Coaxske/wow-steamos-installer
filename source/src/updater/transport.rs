//! Fixed public GitHub transport. No configurable production feed or trust roots.
use super::{cancelled, fsutil, read_regular, Result, PORTABLE, REPOSITORY};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};
const UA: &str = "WoW-SteamOS-Updater/1";
const API: &str = "https://api.github.com/repos/Coaxske/wow-steamos-installer/releases";
struct Scratch(PathBuf, super::Artifact);
impl Scratch {
    fn new(data: &Path, parent: &Path) -> Result<Self> {
        let nonce = fsutil::unique();
        let p = parent.join(format!(".wow-update-tmp-{nonce}"));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&p)
            .map_err(|e| e.to_string())?;
        let tracked = super::Artifact::new(data, &p, "scratch", &nonce, None)?;
        Ok(Self(p, tracked))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        self.1.finish();
    }
}
fn new_file(p: &Path, artifact: &mut super::Artifact) -> Result<File> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(p)
        .map_err(|e| e.to_string())?;
    artifact.add(p)?;
    Ok(file)
}
fn clean_command(program: &Path) -> Command {
    let mut c = Command::new(program);
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C");
    c
}
fn terminate(child: &mut Child) {
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.wait();
}
fn run(
    mut command: Command,
    dir: &Path,
    files: &[(&Path, u64)],
    stdout_limit: u64,
    deadline: Instant,
    artifact: &mut super::Artifact,
) -> Result<ExitStatus> {
    let stdout = dir.join("stdout");
    let stderr = dir.join("stderr");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(new_file(&stdout, artifact)?))
        .stderr(Stdio::from(new_file(&stderr, artifact)?));
    let lease_fd = artifact.lease_fd();
    unsafe {
        command.pre_exec(move || {
            if libc::setpgid(0, 0) != 0 || libc::fcntl(lease_fd, libc::F_SETFD, 0) == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    cancelled()?;
    if Instant::now() >= deadline {
        return Err("Update request timed out".into());
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Cannot start the bounded update helper")?;
    loop {
        let oversized = files
            .iter()
            .any(|(p, n)| fs::metadata(p).map(|m| m.len() > *n).unwrap_or(false))
            || fs::metadata(&stdout)
                .map(|m| m.len() > stdout_limit)
                .unwrap_or(false)
            || fs::metadata(&stderr)
                .map(|m| m.len() > 8192)
                .unwrap_or(false);
        if oversized || Instant::now() >= deadline || cancelled().is_err() {
            terminate(&mut child);
            return Err(if oversized {
                "Update response exceeded its size limit"
            } else {
                "Update request cancelled or timed out"
            }
            .into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if Instant::now() >= deadline || cancelled().is_err() {
                    return Err("Update request cancelled or timed out".into());
                }
                if files
                    .iter()
                    .any(|(p, n)| fs::metadata(p).map(|m| m.len() > *n).unwrap_or(false))
                    || fs::metadata(&stdout)
                        .map(|m| m.len() > stdout_limit)
                        .unwrap_or(false)
                    || fs::metadata(&stderr)
                        .map(|m| m.len() > 8192)
                        .unwrap_or(false)
                {
                    return Err("Update response exceeded its size limit".into());
                }
                return Ok(status);
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => {
                terminate(&mut child);
                return Err("Cannot observe update helper completion".into());
            }
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Api,
    Asset,
}
fn split(url: &str) -> Result<(&str, &str, &str)> {
    if url.len() > 16384
        || !url.is_ascii()
        || url.bytes().any(|b| b <= 32 || b == 127)
        || url.contains('\\')
        || url.contains('#')
    {
        return Err("Update URL is invalid".into());
    }
    let rest = url
        .strip_prefix("https://")
        .ok_or("Updates require HTTPS")?;
    let (host, tail) = rest.split_once('/').ok_or("Update URL is missing a path")?;
    if host.is_empty()
        || host.contains(':')
        || host.contains('@')
        || !host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
    {
        return Err("Update URL authority is invalid".into());
    }
    let (path, query) = tail.split_once('?').unwrap_or((tail, ""));
    if path.contains('%')
        || path
            .split('/')
            .any(|x| x == "." || x == ".." || x.is_empty())
    {
        return Err("Update URL path is invalid".into());
    }
    Ok((host, path, query))
}
fn initial(url: &str) -> Result<Kind> {
    let (host, path, query) = split(url)?;
    if host == "api.github.com"
        && path == format!("repos/{REPOSITORY}/releases")
        && (query == "per_page=100" || (1..=10).any(|n| query == format!("per_page=100&page={n}")))
    {
        return Ok(Kind::Api);
    }
    if host == "github.com" && query.is_empty() {
        let prefix = format!("{REPOSITORY}/releases/download/");
        if let Some(rest) = path.strip_prefix(&prefix) {
            if let Some((tag, name)) = rest.split_once('/') {
                if tag.starts_with('v')
                    && super::Version::parse(tag).is_ok()
                    && [PORTABLE, "SHA256SUMS", &format!("{PORTABLE}.sig")].contains(&name)
                {
                    return Ok(Kind::Asset);
                }
            }
        }
    }
    Err("Update URL is outside the approved repository".into())
}
fn allowed_redirect(initial_url: &str, url: &str, kind: Kind) -> Result<()> {
    let (host, path, query) = split(url)?;
    if url == initial_url {
        return Ok(());
    }
    if kind == Kind::Api {
        return Err("Release-list redirect left its exact repository endpoint".into());
    }
    if host == "github.com" {
        return Err("Asset redirect changed the selected repository, tag or filename".into());
    }
    let prefix = match host {
        "release-assets.githubusercontent.com" => "github-production-release-asset/1424725268/",
        "objects.githubusercontent.com" => "github-production-release-asset-2e65be/1424725268/",
        _ => return Err("Asset redirect host is not approved".into()),
    };
    let id = path
        .strip_prefix(prefix)
        .ok_or("Asset redirect repository ID does not match")?;
    if id.is_empty()
        || id.contains('/')
        || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        || query.is_empty()
    {
        return Err("Asset redirect path is invalid".into());
    }
    Ok(())
}
fn location(headers: &[u8], expected: u16) -> Result<Option<String>> {
    let text = std::str::from_utf8(headers).map_err(|_| "Invalid update response headers")?;
    let mut status = None;
    let mut value = None;
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if line.starts_with("HTTP/") {
            let n = line
                .split_whitespace()
                .nth(1)
                .ok_or("Missing HTTP status")?
                .parse::<u16>()
                .map_err(|_| "Invalid HTTP status")?;
            status = Some(n);
            value = None;
        } else if let Some((name, content)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("location") {
                if value.is_some() {
                    return Err("Ambiguous update redirect".into());
                }
                value = Some(content.trim().to_owned());
            }
        }
    }
    if status != Some(expected) {
        return Err("HTTP status and response headers disagree".into());
    }
    Ok(value)
}
fn get_until(data: &Path, url: &str, max: u64, deadline: Instant) -> Result<Vec<u8>> {
    let kind = initial(url)?;
    let mut scratch = Scratch::new(data, data)?;
    let mut current = url.to_owned();
    for hop in 0..=5 {
        cancelled()?;
        allowed_redirect(url, &current, kind)?;
        let dir = scratch.0.join(hop.to_string());
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|e| e.to_string())?;
        scratch.1.add(&dir)?;
        let body = dir.join("body");
        let headers = dir.join("headers");
        drop(new_file(&body, &mut scratch.1)?);
        drop(new_file(&headers, &mut scratch.1)?);
        let mut c = clean_command(Path::new("/usr/bin/curl"));
        c.arg("--disable")
            .arg("--globoff")
            .arg("--silent")
            .arg("--show-error")
            .args([
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--connect-timeout",
                "8",
                "--max-time",
                "180",
                "--max-redirs",
                "0",
                "--retry",
                "0",
                "--user-agent",
                UA,
                "--header",
                if kind == Kind::Api {
                    "Accept: application/vnd.github+json"
                } else {
                    "Accept: application/octet-stream"
                },
                "--header",
                "X-GitHub-Api-Version: 2022-11-28",
                "--write-out",
                "%{http_code}",
                "--max-filesize",
            ])
            .arg(max.to_string())
            .arg("--dump-header")
            .arg(&headers)
            .arg("--output")
            .arg(&body)
            .arg("--url")
            .arg(&current);
        let status = run(
            c,
            &dir,
            &[(&body, max), (&headers, 64 * 1024)],
            8,
            deadline,
            &mut scratch.1,
        )?;
        if !status.success() {
            return Err("The update download did not complete".into());
        }
        let code = String::from_utf8(read_regular(&dir.join("stdout"), 8)?)
            .map_err(|_| "Invalid download status")?
            .parse::<u16>()
            .map_err(|_| "Invalid download status")?;
        let next = location(&read_regular(&headers, 64 * 1024)?, code)?;
        if code == 200 {
            cancelled()?;
            return read_regular(&body, max);
        }
        if ![301, 302, 303, 307, 308].contains(&code) || hop == 5 {
            return Err("Update server did not return an approved download".into());
        }
        let target = next.ok_or("Update redirect has no destination")?;
        current = if target.starts_with('/') && !target.starts_with("//") {
            let (host, _, _) = split(&current)?;
            format!("https://{host}{target}")
        } else {
            target
        };
        allowed_redirect(url, &current, kind)?;
    }
    Err("Too many update redirects".into())
}
pub(super) fn get(data: &Path, url: &str, max: u64) -> Result<Vec<u8>> {
    let seconds = if max > 2 * 1024 * 1024 { 180 } else { 20 };
    get_until(
        data,
        url,
        max,
        Instant::now() + Duration::from_secs(seconds),
    )
}
pub(super) fn releases(data: &Path) -> Result<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut all = Vec::<Value>::new();
    for page in 1..=10 {
        cancelled()?;
        let bytes = get_until(
            data,
            &format!("{API}?per_page=100&page={page}"),
            2 * 1024 * 1024,
            deadline,
        )?;
        let entries: Vec<Value> =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid releases response")?;
        if entries.len() > 100 {
            return Err("Too many release entries".into());
        }
        let complete = entries.len() < 100;
        all.extend(entries);
        if complete {
            return serde_json::to_vec(&all).map_err(|e| e.to_string());
        }
    }
    Err("Release history exceeds the bounded update check".into())
}
pub(super) fn build_info(data: &Path, portable: &Path) -> Result<Value> {
    let parent = portable.parent().ok_or("Missing staged directory")?;
    let mut scratch = Scratch::new(data, parent)?;
    let mut command = clean_command(portable);
    command
        .arg("--build-info")
        .env("XDG_CACHE_HOME", scratch.0.join("cache"))
        .env("XDG_DATA_HOME", scratch.0.join("data"))
        .env("XDG_CONFIG_HOME", scratch.0.join("config"));
    let home = std::env::var_os("HOME").ok_or("Home directory unavailable")?;
    command.env("HOME", home);
    let status = run(
        command,
        &scratch.0,
        &[],
        16384,
        Instant::now() + Duration::from_secs(120),
        &mut scratch.1,
    )?;
    if !status.success() {
        return Err("Signed portable build identity could not be read".into());
    }
    let value: Value = serde_json::from_slice(&read_regular(&scratch.0.join("stdout"), 16384)?)
        .map_err(|_| "Signed portable build identity is not valid JSON")?;
    cancelled()?;
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_exact_repository_initial_urls() {
        assert!(initial(&format!("{API}?per_page=100&page=1")).is_ok());
        assert!(initial(&super::super::asset(
            "v0.2.0-alpha.10-candidate.50",
            PORTABLE
        ))
        .is_ok());
        for u in ["http://github.com/Coaxske/wow-steamos-installer/releases/download/v1.0.0/x","https://github.com@evil.test/Coaxske/wow-steamos-installer/releases/download/v1.0.0/x","https://github.com:443/Coaxske/wow-steamos-installer/releases/download/v1.0.0/x","https://github.com/Coaxske/wow-steamos-installer/../evil","https://github.com/Coaxske%2fwow-steamos-installer/releases/download/v1.0.0/x","https://release-assets.githubusercontent.com/github-production-release-asset/1424725268/abcd?x=y"]{assert!(initial(u).is_err(),"{u}");}
    }
    #[test]
    fn redirects_preserve_repo_binding() {
        let start = super::super::asset("v1.0.0", PORTABLE);
        assert!(allowed_redirect(&start,"https://release-assets.githubusercontent.com/github-production-release-asset/1424725268/0123-abcd?sp=r",Kind::Asset).is_ok());
        for u in ["https://release-assets.githubusercontent.com/github-production-release-asset/999/0123-abcd?sp=r","https://release-assets.githubusercontent.com.evil.test/github-production-release-asset/1424725268/0123-abcd?sp=r","https://release-assets.githubusercontent.com/github-production-release-asset/1424725268/../abcd?sp=r","https://release-assets.githubusercontent.com/github-production-release-asset/1424725268/%61bcd?sp=r","https://github.com/other/repo/releases/download/v1.0.0/file"]{assert!(allowed_redirect(&start,u,Kind::Asset).is_err(),"{u}");}
    }
    #[test]
    fn response_header_status_and_location_are_unambiguous() {
        assert_eq!(
            location(
                b"HTTP/2 302\r\nlocation: https://example.test/a\r\n\r\n",
                302
            )
            .unwrap(),
            Some("https://example.test/a".into())
        );
        assert!(location(b"HTTP/2 302\nlocation: a\nLocation: b\n", 302).is_err());
        assert!(location(b"HTTP/2 200\n", 302).is_err());
    }
}
