# WoW SteamOS Installer — Candidate 44

A portable Rust/Qt installer for native Steam on Linux x86_64. Tested on Steam Deck in Game Mode through the unchanged C43 launch/controller behavior. C44 adds automated-verified filesystem access links. Bazzite testing in progress.

This release manages exactly one Steam tile for the selected copy: **Battle.net**. Installer choices select which WoW editions to set up. Open Battle.net from Steam, choose an installed edition/account inside the ordinary launcher, and press Play. With the guided Close after Play setting verified, the main launcher exits while WoW keeps running. Quit through WoW's own menu to return to the same Battle.net tile. Separate WoW/edition tiles and addon-manager integration are deferred.

Working launcher, prefix and Proton are reused. Fresh setup, rerun and repair converge on the same tile. Older edition tiles or C42's two-tile setup require an explicit identity choice when ambiguous. A verified private outside-installation metadata/controller backup precedes consolidation. Only validated installer-owned shortcuts are retired. Existing games, addons, preferences, custom layouts and artwork are preserved; layouts are not merged, reapplied or deleted.

## Games and AddOns folders

After setup, open **~/World of Warcraft** in Desktop Mode, or choose **Open folders** on the installer's completion screen. It contains filesystem links named **Battle.net**, **Retail installation**, **Classic Era installation**, **Forever installation**, and matching **Retail AddOns**, **Classic Era AddOns**, **Forever AddOns** for actual detected installations and existing Interface/AddOns directories. Only installed/detected clients receive links. Games on indexed Steam libraries or explicitly selected search locations on another drive are supported. Use the normal search-location control to include another location, reconnect an unavailable drive, then **Create / refresh links**.

To install an addon, download the edition-appropriate addon and extract its addon folder into that edition's AddOns link. For example, Retail AddOns/ExampleAddon/ExampleAddon.toc must resolve to the Retail client's real Interface/AddOns/ExampleAddon/ExampleAddon.toc. Avoid an extra nested archive folder. Follow the addon's own compatibility instructions. Files written through a link go directly to the existing installation; the installer does not download, update or validate third-party addons.

Paths are resolved locally. Games/addons are not moved or copied. Reruns reuse verified correct links. Multiple detected copies get distinct stable labels. An occupied user name or unowned existing World of Warcraft folder is preserved and reported; the installer does not overwrite or silently adopt it. Missing or ambiguous AddOns folders are reported without guessing or creating a substitute. Finish the official installation, ensure the actual Interface/AddOns directory exists, then refresh. Disconnected drives leave existing links intact; reconnect before use.

**Remove access links only — keep games and addons** removes only unchanged, verified installer-created links. Targets and sample/user addons remain in place. Changed user entries are preserved. The empty access folder and private ownership record may remain for safe reuse. This differs from full installation removal: review its separate scope and backup first. Its personal-data backup covers Interface and WTF (including SavedVariables), not Screenshots, game files, the prefix/sign-in or every other folder. Make a separate verified backup for other data.

## Launcher and controls

Automatic exit is a required normal guided setup step. Open installed Battle.net, sign in privately if needed, then close it and WoW normally before review. Explicitly approve the installation-wide change in BOTH Desktop and Game Mode, review, and Apply. The installer verifies an original-settings backup before changing only the required field and verifies the current typed setting afterward. Already-enabled settings can be confirmed without a vendor write. Reruns preserve the choice and never silently reapply changed preferences. A setting write alone is not runtime process-exit evidence.

Automatic pointer configuration is unavailable. The normal manual guide targets this same Battle.net tile. Preserve working layouts. If an edit is needed, save/export the original and use Steam's unique-configuration-for-this-device option before changing shared controls. Right-trackpad Mouse and Left Mouse Click are pointer/menu controls, not a complete gameplay layout. Brightness, display power, sleep and lock policy remain under the operating system.

## Evidence and limitations

C43 actual Steam Deck Game Mode acceptance passed Era and Retail automatic main-launcher exit while the same correct game survived, own-menu quit and return to the same Battle.net tile. Era's physical pointer/menu check passed using the existing generic Battle.net layout. C44 changes folder access only; accepted launch/controller behavior is carried forward by source review. C44 folder setup/rerun/repair/removal, routing sample addon bytes, external/unavailable drives, occupied names and file-manager dispatch are automated disposable checks, not a new device/game session.

Earlier Forever coverage is inherited and bounded to character screens. The changed Forever 1.60.1.70862 file was not tested; that limitation is accepted for this release. No full gameplay, performance or continuous process trace is claimed. Earlier Era Error134 and WOW51900319 remain separate unexplained historical records, not proven fixes. See accompanying acceptance/release notes for the original observation limits.

Build/package: scripts/build.sh and packaging/README.md. Matching Source.zip contains installer and vendored Rust/SDL inputs. Runtime corresponding sources, licenses and notices accompany release assets. Private QA tools, device records, captures, logs, settings, backups, credentials and transfer packets are excluded.
