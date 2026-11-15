# Contributing to ClipCut

Thanks for being here. ClipCut is a video editor that runs entirely on the
user's machine, and it gets better mostly through people using it hard and
reporting what broke.

## The most valuable thing you can do

Grab a build from the [Releases](https://github.com/Realestkai/ClipCut/releases) page
and edit a real video with it. Real footage finds what no reading of the
source does. A good bug report (what you did, what happened, your
OS, the media you used) is worth more than most patches.

## Before you write code

Open an issue first for anything beyond a small fix. Large
areas are already in progress or intentionally deferred, and it is
no fun to review a big PR that has to be turned down for reasons that were
invisible from outside.

## Setting up

You will need Rust (see `rust-version` in `src/Cargo.toml`), the FFmpeg
7+ development libraries (`brew install ffmpeg`; see `src/README.md` for
Windows), cmake and a C++ compiler. Then:

```sh
cd src && cargo run -p clipcut
```

That is the editor window. Everything - the engine, the host layer and the
Slint UI - is one Cargo workspace under `src/`.

## Layout

| Path | What lives there |
|---|---|
| `src/crates/` | The engine (core, media, render, effects, export, project), the host layer (`clipcut-host`, `clipcut-speech`), the API (`clipcut-api`, `clipcut-server`), the CLI, and `clipcut`, the Slint editor window |

[`src/README.md`](src/README.md) explains how the crates fit together and
where the sharp edges are. Read it before touching the engine.

## Checks

Run these before opening a PR:

```sh
cd src && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

If you added or changed text the interface shows, also run
`python3 scripts/locales.py` so the string inventory follows; CI checks it.
Translations live in one JSON file per language; see
[`TRANSLATING.md`](TRANSLATING.md).

If you added or changed a downloadable model, put it in
[`models/manifest.toml`](models/manifest.toml) as well as the engine table it
belongs to, and run `python3 scripts/models.py --check`; CI checks that too.
The models themselves are downloaded from the mirror named in
`models/manifest.toml`, so a new model also has to exist there or at its
`upstream` address.

Write what happens through the `log` facade (`log::info!`, `log::warn!`,
`log::error!`) rather than to standard output or standard error. Every run
writes `<app data>/logs/clipcut-<when>.log`, and a packaged build has no
terminal for anything that goes anywhere else; `CLIPCUT_LOG=debug` turns the
level up. See `src/crates/clipcut-host/src/logs.rs`.

New source files need a licence header (see below). Match the style of the code
around you; the engine avoids cleverness on purpose.

## Licensing your contribution

ClipCut is published under **AGPL-3.0-or-later** ([`LICENSE`](LICENSE)); see
[`LICENSING.md`](LICENSING.md). Contributions are accepted under the same
licence.

### File headers

Every source file starts with:

```rust
// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah
```

Keep that header on the files that carry it: the licence requires
copyright notices to stay.

### Third-party code

If your patch brings in code you did not write, say so in the PR: what it is,
where it came from, and its licence. It has to be under a licence compatible with
AGPL-3.0-or-later. Licence problems are much cheaper to catch before a merge
than after a release.

## Conduct

[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) applies to the repo and its
Discussions.
