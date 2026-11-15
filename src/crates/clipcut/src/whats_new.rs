// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and ClipCut contributors

//! What changed in each version, for the What's new dialog.
//!
//! The words are data: one `site/whats-new/<version>.json` per version,
//! which the website shows too and the build script compiles in. A release
//! adds a file and nothing here names it. A version with no file has no
//! dialog, and the launch screen's card opens the website instead.

use serde::Deserialize;

// `NOTES`: every file's version and its JSON, written by build.rs.
include!(concat!(env!("OUT_DIR"), "/whats_new.rs"));

/// The version this build is.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One change, as the dialog lists it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Item {
    /// A short name for the change.
    pub title: String,
    /// What it does for the person using it, in a sentence or two.
    #[serde(default)]
    pub text: String,
}

/// One version's file.
#[derive(Debug, Deserialize)]
struct Notes {
    items: Vec<Item>,
}

/// What `version` changed: empty when it has no file, or one that does
/// not read, which the log names.
pub fn items(version: &str) -> Vec<Item> {
    let Some((_, json)) = NOTES.iter().find(|(named, _)| *named == version) else {
        return Vec::new();
    };
    match serde_json::from_str::<Notes>(json) {
        Ok(notes) => notes.items,
        Err(error) => {
            log::warn!(
                "whats-new: the notes for {version} do not read: {error}; no dialog is shown for it"
            );
            Vec::new()
        }
    }
}

/// Whether a launch of `version` should put the dialog up by itself: it
/// has something to say, and `seen` - the last version whose notes were
/// shown, from the preferences - is not this one. A fresh install has
/// seen nothing, so it shows too.
pub fn unseen(seen: Option<&str>, version: &str) -> bool {
    seen != Some(version) && !items(version).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file compiled in reads, says something, and is named for a
    /// version: a typo in one would otherwise be a dialog that never shows.
    #[test]
    fn every_version_file_reads() {
        for (version, json) in NOTES {
            let numbers: Vec<&str> = version.split('.').collect();
            assert!(
                numbers.len() == 3 && numbers.iter().all(|part| part.parse::<u32>().is_ok()),
                "{version}.json is not named for a version"
            );
            let notes: Notes = serde_json::from_str(json)
                .unwrap_or_else(|error| panic!("{version}.json does not read: {error}"));
            assert!(!notes.items.is_empty(), "{version}.json lists nothing");
            for item in &notes.items {
                assert!(!item.title.trim().is_empty(), "{version}.json: empty title");
            }
        }
    }

    /// The dialog comes up once per version that has notes, and never for
    /// one that has none.
    #[test]
    fn a_version_is_shown_once() {
        let Some((version, _)) = NOTES.first() else {
            return;
        };
        assert!(unseen(None, version));
        assert!(unseen(Some("0.0.1"), version));
        assert!(!unseen(Some(version), version));
        assert!(!unseen(None, "0.0.0"));
        assert!(items("0.0.0").is_empty());
    }
}
