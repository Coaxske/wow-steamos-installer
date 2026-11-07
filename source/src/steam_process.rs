//! Current-user Steam detection shared by the engine and Qt input adapter.
//! A pinned proc directory prevents a recycled PID from mixing two processes.
use crate::Result;
use std::{
    fs::{self, File},
    io::{self, Read},
    os::fd::{AsRawFd, FromRawFd},
};
const UNKNOWN: &str = "Steam state for this user could not be verified. No Steam files will be changed. Check process access, close Steam normally, then retry.";
fn gone(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(libc::ENOENT | libc::ESRCH))
}
fn ids(status: &[u8]) -> io::Result<[u32; 4]> {
    let line = status
        .split(|b| *b == b'\n')
        .find(|line| line.starts_with(b"Uid:"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing process UIDs"))?;
    let line = std::str::from_utf8(&line[4..])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid process UIDs"))?;
    let values: Vec<u32> = line
        .split_whitespace()
        .map(|v| v.parse::<u32>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid process UIDs"))?;
    values
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid process UIDs"))
}
fn dead(status: &[u8]) -> io::Result<bool> {
    let state = status
        .split(|b| *b == b'\n')
        .find(|line| line.starts_with(b"State:"))
        .and_then(|line| line[6..].iter().copied().find(|b| !b.is_ascii_whitespace()))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing process state"))?;
    if !b"RSDTtZXxIKWP".contains(&state) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid process state",
        ));
    }
    Ok(matches!(state, b'Z' | b'X' | b'x'))
}
fn inspect(mut read: impl FnMut(&str) -> io::Result<Vec<u8>>, uid: u32) -> io::Result<bool> {
    let first_status = read("status")?;
    let before = ids(&first_status)?;
    // Include real or effective ownership; never trust HOME or a caller-supplied UID.
    if before[0] != uid && before[1] != uid {
        return Ok(false);
    }
    let comm = read("comm")?;
    let name = comm.strip_suffix(b"\n").unwrap_or(&comm);
    if ![b"steam".as_slice(), b"steamwebhelper", b"steam-runtime-l"].contains(&name) {
        return Ok(false);
    }
    if dead(&first_status)? {
        return Ok(false);
    }
    let last_status = read("status")?;
    if ids(&last_status)? != before {
        return Err(io::Error::other(
            "process ownership changed during inspection",
        ));
    }
    Ok(!dead(&last_status)?)
}
fn read_relative(dir: &File, name: &str) -> io::Result<Vec<u8>> {
    let name = std::ffi::CString::new(name).unwrap();
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversize process metadata",
        ));
    }
    Ok(bytes)
}
fn combine(results: impl IntoIterator<Item = io::Result<bool>>) -> Result<bool> {
    let mut uncertain = false;
    for result in results {
        match result {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(e) if gone(&e) => {} // Confirmed exit: not permission/malformed-data errors.
            Err(_) => uncertain = true,
        }
    }
    if uncertain {
        Err(UNKNOWN.into())
    } else {
        Ok(false)
    }
}
pub fn current_user_running() -> Result<bool> {
    let uid = unsafe { libc::geteuid() };
    let entries = fs::read_dir("/proc").map_err(|_| UNKNOWN.to_string())?;
    combine(entries.map(|entry| {
        let entry = entry?;
        if !entry
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            return Ok(false);
        }
        // Opening a directory first pins its process identity across subsequent reads.
        let dir = File::open(entry.path())?;
        inspect(|name| read_relative(&dir, name), uid)
    }))
}
pub(crate) fn diagnostic_snapshot() -> serde_json::Value {
    let uid = unsafe { libc::geteuid() };
    let mut processes = vec![];
    let mut uncertain = 0;
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(pid) = name.to_str().and_then(|n| n.parse::<u32>().ok()) else {
                continue;
            };
            let Ok(dir) = File::open(entry.path()) else {
                continue;
            };
            let Ok(status) = read_relative(&dir, "status") else {
                uncertain += 1;
                continue;
            };
            let Ok(owners) = ids(&status) else {
                uncertain += 1;
                continue;
            };
            if owners[0] != uid && owners[1] != uid {
                continue;
            }
            let Ok(comm) = read_relative(&dir, "comm") else {
                continue;
            };
            let comm = comm.strip_suffix(b"\n").unwrap_or(&comm);
            if [b"steam".as_slice(), b"steamwebhelper", b"steam-runtime-l"].contains(&comm) {
                let state = status
                    .split(|b| *b == b'\n')
                    .find(|x| x.starts_with(b"State:"))
                    .and_then(|x| std::str::from_utf8(&x[6..]).ok())
                    .and_then(|x| x.split_whitespace().next())
                    .unwrap_or("?");
                processes.push(serde_json::json!({"pid":pid,"comm":String::from_utf8_lossy(comm),"state":state}));
            }
        }
    }
    serde_json::json!({"recognized_same_user_processes":processes,"unreadable_status_count":uncertain})
}
#[cfg(test)]
mod tests {
    use super::*;
    fn status(uid: u32) -> Vec<u8> {
        format!("Name:\ttest\nState:\tS (sleeping)\nUid:\t{uid}\t{uid}\t{uid}\t{uid}\n")
            .into_bytes()
    }
    #[test]
    fn qt_ffi_state_code_preserves_unknown() {
        assert_eq!(crate::steam_state_code(Ok(false)), 0);
        assert_eq!(crate::steam_state_code(Ok(true)), 1);
        assert_eq!(
            crate::steam_state_code(Err("process inspection denied".into())),
            -1
        );
    }
    #[test]
    fn same_user_and_all_recognized_names() {
        for name in ["steam", "steamwebhelper", "steam-runtime-l"] {
            assert!(inspect(
                |file| Ok(if file == "status" {
                    status(1000)
                } else {
                    format!("{name}\n").into_bytes()
                }),
                1000
            )
            .unwrap());
        }
    }
    #[test]
    fn other_user_is_ignored_without_reading_comm() {
        assert!(!inspect(
            |file| {
                assert_eq!(file, "status");
                Ok(status(1001))
            },
            1000
        )
        .unwrap());
    }
    #[test]
    fn effective_or_real_ownership_is_relevant() {
        for line in [
            b"Uid: 1001 1000 1000 1000".as_slice(),
            b"Uid: 1000 1001 1001 1001",
        ] {
            assert!(inspect(
                |f| Ok(if f == "status" {
                    {
                        let mut bytes = line.to_vec();
                        bytes.extend_from_slice(b"\nState: S (sleeping)\n");
                        bytes
                    }
                } else {
                    b"steam\n".to_vec()
                }),
                1000
            )
            .unwrap());
        }
    }
    #[test]
    fn exit_at_status_or_comm_read_is_benign() {
        for exit_stage in [0, 1, 2] {
            let mut n = 0;
            let result = inspect(
                |f| {
                    let stage = n;
                    n += 1;
                    if stage == exit_stage {
                        Err(io::Error::from_raw_os_error(libc::ESRCH))
                    } else {
                        Ok(if f == "status" {
                            status(1000)
                        } else {
                            b"steam\n".to_vec()
                        })
                    }
                },
                1000,
            );
            assert!(!combine([result]).unwrap());
        }
        assert!(!combine([Err(io::Error::from_raw_os_error(libc::ENOENT))]).unwrap());
    }
    #[test]
    fn inaccessible_or_malformed_metadata_is_unknown() {
        for code in [libc::EACCES, libc::EPERM, libc::EIO] {
            assert!(combine([Err(io::Error::from_raw_os_error(code))]).is_err());
        }
        assert!(combine([inspect(|_| Ok(b"bad metadata".to_vec()), 1000)]).is_err());
        assert!(combine([inspect(
            |f| if f == "status" {
                Ok(status(1000))
            } else {
                Err(io::Error::from_raw_os_error(libc::EACCES))
            },
            1000
        )])
        .is_err());
    }
    #[test]
    fn credential_transition_is_unknown() {
        let mut n = 0;
        assert!(combine([inspect(
            |f| {
                n += 1;
                Ok(if f == "comm" {
                    b"steam\n".to_vec()
                } else {
                    status(if n == 1 { 1000 } else { 1001 })
                })
            },
            1000
        )])
        .is_err());
    }
    #[test]
    fn known_running_overrides_other_uncertainty() {
        assert!(combine([Err(io::Error::from_raw_os_error(libc::EACCES)), Ok(true)]).unwrap());
    }
    #[test]
    fn pinned_directory_after_process_exit_is_benign() {
        let mut child = std::process::Command::new("sleep")
            .arg("10")
            .spawn()
            .unwrap();
        let dir = File::open(format!("/proc/{}", child.id())).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!combine([inspect(|f| read_relative(&dir, f), unsafe {
            libc::geteuid()
        })])
        .unwrap());
    }
    #[test]
    fn malformed_uid_field_is_not_accepted() {
        assert!(ids(b"Uid: 1000 broken 1000 1000").is_err());
        assert!(ids(b"Uid: 1000 1000 1000 1000 1000").is_err());
        assert!(ids(b"Uid: 1000 bad 1000 1000 1000").is_err());
    }
}

#[cfg(test)]
mod handoff_tests {
    use super::*;
    #[test]
    fn dead_matches_do_not_own_input() {
        for name in ["steam", "steamwebhelper", "steam-runtime-l"] {
            for state in ["Z", "X", "x"] {
                let status = format!("Uid: 1000 1000 1000 1000\nState: {state}\n");
                assert!(!inspect(
                    |f| Ok(if f == "status" {
                        status.as_bytes().to_vec()
                    } else {
                        format!("{name}\n").into_bytes()
                    }),
                    1000
                )
                .unwrap());
            }
        }
    }
    #[test]
    fn stopped_live_and_unknown_stay_protected() {
        for state in ["R", "S", "D", "T", "t"] {
            let status = format!("Uid: 1000 1000 1000 1000\nState: {state}\n");
            assert!(inspect(
                |f| Ok(if f == "status" {
                    status.as_bytes().to_vec()
                } else {
                    b"steam\n".to_vec()
                }),
                1000
            )
            .unwrap());
        }
        assert!(inspect(
            |f| Ok(if f == "status" {
                b"Uid: 1000 1000 1000 1000\n".to_vec()
            } else {
                b"steam\n".to_vec()
            }),
            1000
        )
        .is_err());
    }
    #[test]
    fn dies_between_snapshots_is_not_running() {
        let mut n = 0;
        assert!(!inspect(
            |f| {
                if f == "comm" {
                    return Ok(b"steam\n".to_vec());
                }
                n += 1;
                Ok(format!(
                    "Uid: 1000 1000 1000 1000\nState: {}\n",
                    if n == 1 { "S" } else { "Z" }
                )
                .into_bytes())
            },
            1000
        )
        .unwrap());
    }
}
