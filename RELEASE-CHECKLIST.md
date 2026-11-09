# Release checklist

- Resolve `[STATUS LINE]` and `[FRESH-INSTALL RESULT]` with reviewed results before publishing the documentation page.
- For every release, update the Download button and numbered download step in `docs/index.html` to the intended release asset. Open both links without signing in and verify that the downloaded installer's SHA-256 matches that release's published checksum.
- Keep the repository name, publishing account and installer filename unchanged. Use Deckhand in release titles starting with Candidate 50.
- Enable GitHub Pages from `main` and `/docs` only after the reviewed page is ready on `main`. Do not publish draft placeholders.
