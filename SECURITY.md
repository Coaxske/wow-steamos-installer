# Security and privacy

## Your account

The installer never asks for your Blizzard password or authenticator codes. Sign in only inside Blizzard's Battle.net app.

## Connections and download checks

- The official Battle.net installer is downloaded over HTTPS. Download addresses and redirect destinations must match the permitted Blizzard host rules, including their hostname boundary. The file must look like a Windows program.
- This is not verification of Blizzard's digital signature. Keep the usual care when running downloaded software.
- Steam downloads Proton and required Steam runtimes. The installer may open official help or download pages when requested.
- There is no tracking or automatic upload of your records.

## Installer update checks

Candidate 49 can request this repository's public GitHub release list at most once every 24 hours, including failed attempts. Test releases are included and drafts are ignored. GitHub receives ordinary connection information such as the IP address and request headers. The fixed program label is WoW-SteamOS-Updater/1; no installer version or device identifiers are sent. No Steam or Battle.net configuration, game settings, account names or local records are sent.

Turn off **Check for installer updates once a day** on Home to stop automatic checks. Offline failures are silent. **Not now** keeps the current installer. Only **Update now** authorizes downloading and replacement. Actively running setup, repair or removal blocks checks and replacement. Idle unfinished saved setups still permit daily checks and the release-page link, but block automatic installation. Readiness is checked again before replacement.

Downloads must begin at this repository's HTTPS release addresses. Every redirect is checked before following it. The exact executable must match its SHA-256 entry in SHA256SUMS and its Ed25519 digital signature using the public key built into the installer. The authenticated file's own version must match the selected release, preventing a downgrade. An unknown key, missing signature or failed check discards the download and offers the release page.

The download is staged beside the portable, not in the extracted application cache. The newest verified previous executable is kept as a backup. Older completed backups and their recovery records are removed only when the updater's private ownership records and both files still match. Unrelated files, modified copies and unresolved recovery remain. If replacement or restart fails, the prior executable is retained or restored. If the new process's exit cannot be confirmed, the updater keeps recovery files and refuses an unsafe automatic rollback. Its error names the actual verified backup location when available, explains how to open a safe copy, and links to the fixed releases page. A failed update-check preference write restores the displayed switch from a fresh readback and shows an error. A folder that cannot be written requires downloading the next installer by hand.

The installer contains the production public key for verifying signed updates. The private signing key is stored outside the project and is not included in any build or release file. Private automated tests use separate test keys. Live signed-update testing and Candidate 49 Steam Deck acceptance remain pending. If the production key is lost or compromised, clients that cannot verify a new signature must use the project's fixed release page and manually install the reviewed replacement.

Candidate 48 users must download Candidate 49 once by hand to get the updater.

## Local changes

With your review and approval, setup adds one Battle.net button, configures Close after Play for the selected installation, and creates Home-folder links to detected game and AddOns folders.

Records and backups normally stay in the installer's own local data folder. They can contain account details and local folder names. Keep them private.

Removal has a separate review and confirmation. The installer checks ownership and the folders reviewed before making changes. Link-only removal preserves the linked games and addons.

## Public build

The public executable excludes the developer checklist, screen-capture, external mouse-control, window-reading and addon-manager download tools. Their source is available behind an explicit build switch for private developer builds. Basic installer preview and self-check options remain; they do not control other applications.

Normal controller navigation and manual pointer instructions remain available. Setup does not approve OS permissions or change display and sleep policy.

## Keep private material private

Do not upload Steam or Battle.net settings, tokens, passwords, logs, captures, device records or backup folders. Describe the problem and share only a nonsecret error message.

## Report a security problem

On the project's GitHub page, open Security and choose Report a vulnerability. Please report security issues privately rather than in a public Issue.
