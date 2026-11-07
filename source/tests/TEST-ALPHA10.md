# Historical alpha10 duplicate-shortcut test

The instructions below describe an older baseline, not Candidate37's current guided flow or Steam management. For current testing use the build-specific handoff and acceptance gates. Do not treat this historical document as new hardware or publication acceptance.

# Alpha10: duplicate-shortcut test

This build addresses the extra Classic / Forever Beta shortcut observed after
repairing an existing Forever setup. It does not fix automatic game launching.
Do not reinstall or remove your games for this test.

1. Extract this ZIP into a new folder. Keep alpha9 as your comparison build.
2. Open WoW-SteamOS-Rust-Linux-x86_64. Confirm the window says alpha.10.
3. Choose Check Steam shortcuts for the existing installation.
4. The review should name the extra shortcut it will remove. The original
   Forever shortcut should be retained. If the extra tile is not listed, stop
   and photograph the review with Show details enabled. Do not delete it by hand.
5. In Steam choose Steam → Exit. Return to the installer, confirm Steam is closed,
   and continue. This version does not shut Steam down automatically.
6. Reopen Steam. Confirm one Forever tile remains, its name and controller layout
   are retained, and the Retail, Era and unrelated tiles are still present.
7. Run Check Steam shortcuts again, exit Steam when prompted, and apply. Reopen
   Steam. The tile count should remain unchanged.
8. Launch the retained Forever tile. Confirm Battle.net selects the expected
   edition and the game works after Play. Record whether Play was necessary;
   needing it is the known unresolved issue, not proof this duplicate fix failed.
9. Run Test this build and photograph its results. Tell us separately whether
   steps 6–8 passed. Automatic checks do not certify gameplay or auto-launch.

Expected for the reference test setup: three WoW edition tiles (Retail, Era, Forever),
plus the Battle.net tile. This is not a universal count for every installation.

The repair is limited to recognized managed tiles in the selected installation.
It does not globally equate all beta installs with Forever. Multiple beta folders,
a distinct Forever client, or unrecognized tiles may require further review.

Local validation: 145 Rust tests passed (105 core/workflow + 40 integration),
including legacy missing associations, already-created duplicates, original tile
identity, a second repair, and negative cases for separate copies/unrelated tiles.
Rustfmt and clippy passed. See bundle validation output for native and packaged
self-check status. These are synthetic/runtime checks, not live-device passes.

Known remaining issue: Battle.net receives existing --exec launch requests but
Earlier alpha9 device tests required a manual Play click. The cause has not been
established. Alpha10 intentionally retains those commands, shared Proton data,
and Battle.net authentication. Do not claim this issue fixed based on unit tests.
