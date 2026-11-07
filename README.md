# WoW SteamOS Installer

**Play World of Warcraft on your Steam Deck.**

This free tool installs Blizzard's Battle.net app on your Steam Deck and adds one **Battle.net** button to your Steam library. Open it, pick your game, and press Play.

> **This is an early test version.** It has been tested on Steam Deck. Bazzite PCs are still being tested. See [What has been tested](#what-has-been-tested) before you start.

You do not need to know anything about coding or GitHub to use it. Just follow the steps below.

---

## What you need

- A **Steam Deck**
- A **Blizzard account** with access to the World of Warcraft version you want to play
- An **internet connection**

---

## How to install

1. **Switch to Desktop Mode.** Press the Steam button, choose **Power**, then **Switch to Desktop**.
2. **Open Steam once** and make sure you are signed in.
3. **Download the installer.** Go to the [download page](https://github.com/Coaxske/wow-steamos-installer/releases/tag/v0.2.0-alpha.10-candidate.44). Under **Assets**, download only this file:

   **`WoW-SteamOS-Rust-Linux-x86_64`**

   That is the only file you need to run. The other files provide checksums, instructions, licenses and source code.
4. **Move the file somewhere permanent.** Move it out of your Downloads folder into a folder you will keep. You will open this same file again later to repair or remove the setup.
5. **Allow the file to run.** Right-click the file, choose **Properties**, open the **Permissions** tab, check **Is executable**, then click **OK**.
6. **Open it.** Double-click the file. The installer opens and walks you through each step on screen.
7. **Sign in to Battle.net when asked.** The installer opens Blizzard's Battle.net app. Sign in there and download the game version you want. **The installer never asks for your password or authenticator codes.** You only type those into Battle.net itself.
8. **Finish the remaining steps on screen.**
   - Steam may close and reopen during setup. This is normal.
   - The installer will ask to turn on Battle.net's **close after Play** setting. This makes Battle.net close itself once your game starts, so Steam can bring you back to your library when you quit. Your original Battle.net settings are backed up first. This setting applies to Battle.net in both Desktop Mode and Gaming Mode.

---

## How to play

1. **Return to Gaming Mode.** Double-click **Return to Gaming Mode** on the desktop.
2. **Find Battle.net in your Steam library.** It is listed under **Non-Steam**.
3. **Open it, choose your game, and press Play.** You can choose Retail, Classic Era or Forever inside Battle.net. Battle.net closes on its own and the game keeps running.
4. **Quit from inside the game.** Press Esc, then choose **Exit Game**. Steam's own "Exit game" button closes everything at once, so use the game's menu when you can.

### Using the trackpad as a mouse

Battle.net needs a mouse pointer. The installer shows a short guide for this. In short, open the controller settings for the **Battle.net** button in Steam and set:

- **Right Trackpad** to act as a **Mouse**
- **Right Trackpad click** to **Left Mouse Click**

If you already have controls that work, you do not need to change them.

---

## Installing addons

After setup, a new folder called **World of Warcraft** appears in your Home folder (in Desktop Mode). You can also click **Open folders** at the end of setup. Inside it you will find shortcuts such as **Retail AddOns**, **Classic Era AddOns** and **Forever AddOns**.

To add an addon:

1. Download an addon made for your game version.
2. Unzip it.
3. Drag the addon's folder into the matching AddOns shortcut.

The result should look like `Retail AddOns/ExampleAddon/ExampleAddon.toc`. Make sure you do not end up with a folder inside a folder.

The installer does not download or update addons for you.

---

## Repair or remove

Open the installer again (the same file you downloaded) and choose what you want to do:

- **Repair** fixes the Battle.net button and finds newly installed game versions. It does not make duplicate buttons.
- **Remove access links only** removes the installer's verified folder shortcuts. Your games and addons stay where they are. The empty World of Warcraft folder may remain.
- **Remove this installation** deletes the game installation that the installer set up. Choose **Keep addon and game settings** first to back up your addon settings and game settings (the Interface and WTF folders). Screenshots and other files are **not** backed up, so copy anything else you want to keep before removing.

The installer shows you exactly what it will remove and asks you to confirm before anything is deleted.

---

## What has been tested

- **Steam Deck, Gaming Mode:** Retail and Classic Era start from the Battle.net button, Battle.net closes itself while the game keeps running, and quitting the game returns you to the same Battle.net button. Testing reached the character screen. Full gameplay has not been formally tested.
- **Trackpad mouse:** Worked in Battle.net and game menus.
- **Forever:** Works on Steam Deck. Earlier recorded tests reached character selection. Build 1.60.1.70862 did not receive a separate recorded test.
- **Addon folders:** Checked with automated tests, not yet on a real Steam Deck.
- **Bazzite:** Testing in progress.

If you see **error 134** or **WOW51900319**, these appeared in earlier testing and the cause is not known yet. Please report them (see below).

**Not available yet:** separate buttons for each game version, automatic controller setup, and addon manager support.

### About the testing tools in this version

This test version also contains tools the developer uses to test the installer. Normal setup does not use any of them.

- A **Test this build** checklist under **Help & advanced options**. It runs read-only checks and saves your answers in the installer's own data folder. You can ignore it.
- Tools that can take screenshots, move the mouse, and read the list of open windows for automated testing. The source includes command-line options for these tools.
- Unused code for downloading addon managers (CurseForge or WowUp). Its buttons are hidden.

These tools will be removed from public downloads in the next version.

---

## Getting help

If something goes wrong, open an [Issue](https://github.com/Coaxske/wow-steamos-installer/issues) and describe what you were doing and what happened.

**For your privacy, do not attach** Steam configuration files, Battle.net settings files, or the installer's backup folders. They can contain account details.

---

## For advanced users

- [Detailed install, repair and removal guide](INSTALL-AND-REMOVE.md)
- [Full test results](ACCEPTANCE-SUMMARY.md)
- [Supported versions and limitations](SUPPORTED-EDITIONS-AND-LIMITATIONS.md)
- [Release notes](RELEASE-NOTES.md)
- [Source code and build instructions](source/README.md)

Each release includes a `SHA256SUMS` file so you can confirm the download was not damaged.

This project is free and open source under the [MIT License](LICENSE).

---

*This is a community project. It is not made by, endorsed by, or affiliated with Blizzard Entertainment or Valve. World of Warcraft and Battle.net are trademarks of Blizzard Entertainment. Steam and Steam Deck are trademarks of Valve Corporation.*
