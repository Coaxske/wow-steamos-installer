//! Native, FUSE-free launcher. Extracts only the compile-time embedded payload.
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    process::{Command, Stdio},
};
#[path = "update_handoff.rs"]
mod updater_wrapper;
static PAYLOAD: &[u8] = include_bytes!(env!("WOW_PAYLOAD"));
fn run() -> Result<i32, String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let portable_pin = if args.is_empty() {
        updater_wrapper::PortablePin::capture()
    } else {
        None
    };
    let mut random = [0; 16];
    fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut random))
        .map_err(|e| e.to_string())?;
    let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
    // Extract on disk rather than SteamOS's RAM-backed /tmp. Each process owns
    // a private directory; stale directories are removed only when both PIDs died.
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache")
        })
        .join("wow-steamos-runtime");
    if !base.is_absolute() {
        return Err("HOME must identify an absolute directory".into());
    }
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    if fs::symlink_metadata(&base)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Runtime cache must not be a symbolic link".into());
    }
    fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let startup = if args.is_empty() {
        updater_wrapper::Startup::read(&base)?
    } else {
        None
    };
    let uid = fs::metadata(&base).map_err(|e| e.to_string())?.uid();
    for entry in fs::read_dir(&base).map_err(|e| e.to_string())?.flatten() {
        let p = entry.path();
        let Ok(meta) = fs::symlink_metadata(&p) else {
            continue;
        };
        if !meta.is_dir()
            || meta.uid() != uid
            || !entry.file_name().to_string_lossy().starts_with("run-")
        {
            continue;
        }
        if updater_wrapper::retained_recovery(&p) {
            continue;
        }
        if let Ok(text) = fs::read_to_string(p.join("processes")) {
            let ids: Vec<u32> = text
                .split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect();
            if !ids.is_empty()
                && ids
                    .iter()
                    .all(|id| !PathBuf::from(format!("/proc/{id}")).exists())
            {
                let _ = fs::remove_dir_all(p);
            }
        }
    }
    let dir = base.join(format!("run-{suffix}"));
    fs::create_dir(&dir).map_err(|e| e.to_string())?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    fs::write(dir.join("processes"), std::process::id().to_string()).map_err(|e| e.to_string())?;
    let mut update_finished = false;
    let result = (|| {
        let mut tar = Command::new("tar")
            .args(["-xzf", "-", "--no-same-owner", "-C"])
            .arg(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot unpack runtime (tar is required): {e}"))?;
        tar.stdin
            .take()
            .ok_or("Cannot write runtime")?
            .write_all(PAYLOAD)
            .map_err(|e| e.to_string())?;
        let output = tar.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "Runtime extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if args
            .first()
            .map(|x| x == "--extract-runtime")
            .unwrap_or(false)
        {
            let destination = args
                .get(1)
                .map(PathBuf::from)
                .ok_or("Pass a new destination folder after --extract-runtime")?;
            if destination.exists() {
                return Err("The extraction destination already exists".into());
            }
            fs::rename(&dir, &destination)
                .map_err(|e| format!("Choose a destination on the cache filesystem: {e}"))?;
            println!("Runtime extracted to {}", destination.display());
            return Ok(0);
        }
        let logdir = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
            })
            .join("wow-steamos-rust");
        let preview = args.iter().any(|a| a == "--demo" || a == "--self-check");
        let logpath = logdir.join("startup.log");
        let logfile = if preview {
            None
        } else {
            if !logdir.is_absolute() {
                return Err("HOME or XDG_DATA_HOME must identify an absolute folder".into());
            }
            fs::create_dir_all(&logdir).map_err(|e| e.to_string())?;
            if fs::symlink_metadata(&logdir)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("Setup data folder must not be a symbolic link".into());
            }
            fs::set_permissions(&logdir, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            // O_NOFOLLOW on Linux: never truncate a file reached through a log symlink.
            let file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .custom_flags(0x20000)
                .open(&logpath)
                .map_err(|e| e.to_string())?;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
            Some(file)
        };
        if startup.as_ref().is_some_and(|s| s.aborted()) {
            return Err("Update startup was cancelled before the interface opened".into());
        }
        let updater = if args.is_empty() {
            portable_pin.and_then(|pin| updater_wrapper::Session::bind(pin, &dir, &suffix))
        } else {
            None
        };
        if startup.as_ref().is_some_and(|s| s.aborted()) {
            return Err("Update startup was cancelled before the interface opened".into());
        }
        let mut cmd = Command::new(dir.join("bin/wow-steamos"));
        cmd.args(&args).env("WOW_BUNDLED", "1");
        for key in [
            "LD_LIBRARY_PATH",
            "QT_PLUGIN_PATH",
            "QML2_IMPORT_PATH",
            "QML_IMPORT_PATH",
        ] {
            cmd.env(
                format!("WOW_ORIGINAL_{key}"),
                std::env::var_os(key).unwrap_or_default(),
            );
        }
        cmd.env("LD_LIBRARY_PATH", dir.join("lib"))
            .env("QT_PLUGIN_PATH", dir.join("plugins"))
            .env("QML2_IMPORT_PATH", dir.join("qml"))
            .env("QML_IMPORT_PATH", dir.join("qml"));
        updater_wrapper::clear_context(&mut cmd, true);
        if let Some(session) = &updater {
            session.configure(&mut cmd);
        }
        if let Some(file) = logfile {
            cmd.stderr(file);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Cannot open the application: {e}"))?;
        fs::write(
            dir.join("processes"),
            format!("{} {}", std::process::id(), child.id()),
        )
        .map_err(|e| e.to_string())?;
        let status = updater_wrapper::wait_native(&mut child, startup.as_ref())?;
        if updater_wrapper::is_update_exit(status.code()) {
            let session = updater
                .as_ref()
                .ok_or("Verified update restart had no bound portable session")?;
            if !session.has_request()? {
                return Err("Verified update restart had no owned request. The current portable is unchanged.".into());
            }
            session.finish()?;
            update_finished = true;
            return Ok(0);
        }
        // JSON callers consume the engine's structured refusal and exit code.
        // A modal startup dialog would hide ordinary safe refusals and block
        // unattended callers until someone dismisses it.
        if args.first().map(|a| a == "--engine-json").unwrap_or(false) {
            return Ok(status.code().unwrap_or(1));
        }
        if !status.success() {
            if preview {
                return Err("Preview/self-check failed. See terminal output.".into());
            }
            let details = fs::read_to_string(&logpath).unwrap_or_default();
            return Err(format!(
                "Setup could not start or stopped unexpectedly.\n\n{}\n\nStartup log: {}",
                details.chars().take(1500).collect::<String>(),
                logpath.display()
            ));
        }
        Ok(0)
    })();
    let retained = !update_finished && updater_wrapper::retained_recovery(&dir);
    if dir.exists() && !retained {
        let _ = fs::remove_dir_all(&dir);
    }
    if retained {
        result.map_err(|e| {
            format!(
                "{e}\n\nUpdate recovery files retained at: {}",
                dir.display()
            )
        })
    } else {
        result
    }
}
fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("{message}");
            if std::env::args_os()
                .nth(1)
                .map(|a| a == "--engine-json")
                .unwrap_or(false)
            {
                std::process::exit(1);
            }
            let first = std::env::args_os().nth(1);
            if updater_wrapper::suppress_startup_dialog()
                || first.as_deref().is_some_and(|a| {
                    a == "--engine-json" || a == "--build-info" || a == "--version" || a == "--help"
                })
            {
                std::process::exit(1);
            }
            let shown = Command::new("kdialog")
                .args(["--title", "WoW SteamOS Installer", "--error", &message])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !shown {
                let _ = Command::new("zenity")
                    .args([
                        "--error",
                        "--title=WoW SteamOS Installer",
                        "--text",
                        &message,
                    ])
                    .status();
            }
            std::process::exit(1);
        }
    }
}
