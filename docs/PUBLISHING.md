# Publish a forkable GitHub project

Suggested repository name: **wow-steamos-installer**. Use a public repository so others can browse, fork, and propose changes. The MIT license is already present; do not generate a second license or README when creating the empty repository.

Before publishing, review the source and run the tests. The source archive contains no local Steam data or user credentials. The packaged release is deliberately labeled alpha because physical-device testing is outstanding.

## GitHub CLI route

From the extracted project root, with Git and GitHub CLI installed:

```bash
gh auth login
git init -b main
git add .
git commit -m "Initial WoW SteamOS Installer alpha"
gh repo create wow-steamos-installer --public --source=. --remote=origin --push
```

Before committing, configure a neutral project identity or your GitHub noreply email; inspect both author and committer metadata before pushing. GitHub provides a private noreply email option. If the directory already has a Git repository, skip `git init`. If the remote already exists, inspect it before adding or replacing anything.

Enable Issues and Discussions if useful. Enable private vulnerability reporting under repository security settings. The repository includes bug and feature issue templates, contributor instructions, and automated tests.

## Create the first prerelease

After the `main` branch passes CI:

```bash
git tag v0.1.0-alpha.2
git push origin v0.1.0-alpha.2
```

The tag workflow reruns tests, builds the ZIP/checksum file, and creates a GitHub prerelease. It uses GitHub's short-lived workflow token, not a committed personal access token.

Alternatively, after CI passes, open **Actions → Prerelease → Run workflow**, select `main`, and run it. The workflow reads the package version, reruns tests, creates that version's tag, and publishes the ZIP/checksum. It refuses to replace an existing tag. Update the package version before publishing another release.

If a tag already exists, do not move it. Fix the version and publish a new tag. Mark a release stable only after completing and documenting the device tests in `TESTING.md`.

## Browser route

Create a public repository with no generated files and upload the extracted source tree. Include `.github`, `.gitignore`, and `.gitattributes`, which file managers may hide. Uploading only the ZIP prevents normal code browsing and is not a substitute for committing the source. Git is preferred because it retains executable file modes.

If using the browser route, users can always start the program with `bash start.sh` even if executable flags are lost. The Prerelease workflow attaches the standalone `.desktop`, `.pyz`, source ZIP, and SHA256SUMS after tests pass.

## Suggested release text

Initial alpha of a WoW setup assistant for Steam Deck and native-Steam Linux desktops. Select editions, install via Battle.net, and register individual game executables in a shared Proton environment. Includes stable shortcut ownership, backup/recovery, and MIT-licensed source. Automated regression tests pass; physical Steam Deck/Bazzite validation is pending. Direct authentication and client availability vary by WoW build.
