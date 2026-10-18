# Security and privacy

This tool runs as the desktop user and refuses root. It never requests game credentials, uploads telemetry, schedules tasks, or deletes game data. Its only built-in network operation downloads the official Battle.net installer over HTTPS. Blizzard and Steam themselves perform their normal network operations separately.

A Windows PE header check rejects malformed downloads but is not Authenticode verification. The printed SHA-256 identifies the received file; it does not independently establish Blizzard authorship. Redirects outside Blizzard-controlled domain suffixes are refused. Use `--installer` with a manually downloaded official installer if Blizzard changes its delivery host.

Local backups can contain Steam account configuration and local filesystem paths. Keep them private. Do not attach `config.vdf`, `loginusers.vdf`, whole backups, Battle.net settings, or credentials to public issues.

For a vulnerability, use GitHub's private vulnerability reporting once the repository owner enables it. If unavailable, open a minimal issue requesting a private contact route, without exploit details or personal data. This project has no separate upload service.

Backups, a journal, file-hash conflict checks, process checks, and a setup lock reduce accidental damage. They are not an operating-system lock against Steam starting at exactly the same time. Keep Steam fully closed during each edit step.
