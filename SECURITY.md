# Security and privacy

## Your account

The installer **never asks for your Blizzard password or authenticator codes.** You sign in only inside Blizzard's own Battle.net app.

## What the installer connects to

- It downloads the official Battle.net installer directly from Blizzard over a secure (HTTPS) connection. It checks redirect addresses and checks that the file is a Windows program before using it. Candidate 44 has a known limitation in its redirect check. It does not check Blizzard's digital signature on that file.
- When you ask, it opens official download pages in your web browser.
- Proton and other Steam tools are downloaded by Steam itself, not by the installer.
- The installer has no tracking and does not send information about you anywhere.

## What the installer changes on your Steam Deck

- It adds one **Battle.net** button to your Steam library.
- With your approval, it turns on Battle.net's **close after Play** setting. Your original Battle.net settings are backed up first.
- It creates a **World of Warcraft** folder in your Home folder with shortcuts to your game and AddOns folders.
- It keeps its records and backups in its own data folder, normally `~/.local/share/wow-steamos-rust`.
- While it is open, it unpacks itself into `~/.cache/wow-steamos-runtime` and cleans that up when it closes.

Nothing is deleted without a review screen and your confirmation. Before deleting, the installer checks that each folder is still the one you reviewed, and it refuses to follow shortcuts that point somewhere else.

## Testing tools in this version

This test version contains tools the developer uses for automated testing, including tools that can take screenshots, move the mouse, and read the list of open windows. The Help & advanced options page also has a Test this build checklist. Some developer tools are available from command-line options. Normal setup does not need them. They will be removed from public downloads in the next version.

## Keep these files private

The installer's backups, Steam configuration files and Battle.net settings files can contain account details and folder names from your computer. **Do not post them** in an Issue or anywhere public. If you need help, describe what happened and copy only the error message.

## Reporting a security problem

Please report security problems privately. On this project's page, open the **Security** tab and choose **Report a vulnerability**. Do not post security details in a public Issue.
