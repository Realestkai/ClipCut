# What's new, one file per version

`<version>.json` holds the highlights of that version, written for the
person using ClipCut:

```json
{
  "items": [
    { "title": "A short name for the change", "text": "One or two sentences on what it does for you." }
  ]
}
```

Two things read these files:

- **The app.** `src/crates/clipcut/build.rs` compiles every file here into
  the program. The first launch of a version that has a file shows its
  items in the What's new dialog, once, and the launch screen's What's new
  card opens it again (`src/crates/clipcut/src/whats_new.rs`).
- **The website.** `site/site.js` shows a version's items at the top of
  its entry on `whats-new.html`, above the list of commits the release
  workflow writes.

A release needs one new file, named for the `version` in `src/Cargo.toml`.
A version with no file still works: the app shows no dialog and the card
opens the website, as it did before.

The words are English only, like the names of effects: they are data, not
interface strings, and have no lines in `locales/`.
