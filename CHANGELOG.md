# Changelog

## 0.1.0-alpha.2 — 2026-10-18

- Renamed the project to WoW SteamOS Installer, with Steam Deck and Linux support stated in the description.
- Added a self-contained `.desktop` launcher and an executable Python zipapp.
- Loaded the edition catalog from package resources for standalone execution.
- Removed personal references from distributed files and added guidance for private commit attribution. Historical metadata cleanup is a separate repository maintenance step.
- Retained the existing data root and shortcut ownership for earlier installs.
- Added packaged-launcher regression checks; physical-device testing is still outstanding.

## Initial alpha

- Added multi-edition setup, shared Proton settings, and direct game registration.
- Added stable shortcut ownership, backups, recovery, and shortcut-only removal.
- Added MIT licensing, tests, source packaging, and release documentation.
