# Engine interface 1.1.0 — Candidate 8 production engine

JSON schema version 1, with the same documented commands/exit codes as the separate Candidate 6 QA CLI. This adapter links the unmodified production engine used by the GUI. It does NOT request strict_review, and the production engine contains no QA strict-freshness guard. Native safe merges of unrelated changes remain supported; conflicting managed changes must still refuse.

Commands: interface, discover, status, plan (operation setup/repair/remove/reinstall/adopt/addon), repair/remove/reinstall (planning only), apply, recover, undo, cancel-recovery, settings-backups, restore-settings, adoption-candidates. One JSON request on stdin: {"schema_version":1,"request_id":"optional","params":{...}}. Response includes engine version and compiled source fingerprint; success data or structured error kind/message. Exit 0 success, 2 protocol, 3 engine refusal/operation error, 4 transport, 5 caught panic. Apply needs the reviewed token and explicit REMOVE THIS COPY for destructive operations. Engine locking, current-user Steam detection, game protection, verified backups, ownership/reference and conflict checks remain authoritative. Discovery may update installer discovery state.

Build offline from source root with CARGO_TARGET_DIR outside this source tree:
CARGO_TARGET_DIR=/output/private/interface-target cargo build --release --locked --offline --manifest-path scripts/engine-interface/Cargo.toml

Never run fixture cases on live data. Run the shipped QA scripts only in the documented isolated container, with no live Steam/DBus/device mounts. The legacy wow-steamos-cli also dispatches this same engine and remains available. Original Candidate 6 strict experiments remain in their separate candidate6-engine-qa directory and are not promoted.
