// SPDX-License-Identifier: AGPL-3.0-or-later

//! A CapCut template link made into a ClipCut project.
//!
//! A template's share link (`https://www.capcut.com/tv2/...`, or the
//! `template-detail` page it opens) is a public page carrying the
//! template's name, creator, length, frame size, how many clips it is cut
//! from, and its finished preview video. The template's own project - the
//! effects and text inside it - is not on the page; CapCut keeps that to
//! its app. So the import rebuilds what the page shows:
//!
//! - the frame size and length of the template;
//! - its sound, on a track of its own, so the edit keeps its beat;
//! - its cuts, found in the preview where the picture changes outright,
//!   as many as the template has clips: each one a slot on the main track,
//!   holding the template's own footage for that moment, so it is plain
//!   what goes where.
//!
//! Put a clip of your own over a slot (or delete the slot and drop yours
//! in its place) and it lands on the template's beat. Everything is an
//! ordinary clip, to move, trim, restyle or delete. The project is written
//! through the CapCut draft import (`clipcut_project::capcut`): the page
//! is made into a draft, and the draft into a project.

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use clipcut_project::capcut::{Draft, DraftTimeline};
use serde_json::{Value, json};

use crate::capcut::Import;
use crate::projects;

/// The most a template's preview may weigh; they are a few megabytes.
const MOST_BYTES: u64 = 400 * 1024 * 1024;

/// What a template's page says about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    /// CapCut's number for it.
    pub id: String,
    /// Its title, as the creator wrote it.
    pub title: String,
    /// Who made it.
    pub author: String,
    /// Its length, in seconds.
    pub seconds: f64,
    /// How many clips it is cut from.
    pub slots: u32,
    /// Its frame, in pixels.
    pub width: u32,
    /// Its frame, in pixels.
    pub height: u32,
    /// The finished preview video.
    pub video: String,
}

/// The first CapCut link in `text`, which may be a whole shared message
/// ("Use this template on CapCut: https://www.capcut.com/tv2/..."). `None`
/// when there is none.
pub fn find_link(text: &str) -> Option<String> {
    text.split_whitespace()
        .filter_map(|word| word.find("http").map(|at| &word[at..]))
        .map(|word| {
            word.trim_end_matches(|c: char| matches!(c, ')' | ']' | '"' | '\'' | ',' | '>'))
        })
        .find(|word| is_capcut(word))
        .map(str::to_owned)
}

/// Whether `url` is on capcut.com or capcut.net.
fn is_capcut(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["capcut.com", "capcut.net"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}

/// Reads a template page's HTML: the data the page is drawn from, which it
/// carries in a `__MODERN_ROUTER_DATA__` script.
pub fn read_page(html: &str) -> Result<Template, String> {
    let marker = "__MODERN_ROUTER_DATA__";
    let at = html
        .find(marker)
        .ok_or("this page is not a CapCut template (no template data on it)")?;
    let open = html[at..]
        .find('>')
        .map(|i| at + i + 1)
        .ok_or("the page is cut short")?;
    let close = html[open..]
        .find("</script>")
        .map(|i| open + i)
        .ok_or("the page is cut short")?;
    let data: Value = serde_json::from_str(&html[open..close])
        .map_err(|error| format!("the template data does not read: {error}"))?;
    let detail = data
        .pointer("/loaderData/$/templateDetail")
        .filter(|detail| detail.is_object())
        .ok_or("the link is not a template, or CapCut does not show it in this region")?;
    let text = |key: &str| match detail.get(key) {
        Some(Value::String(text)) => text.trim().to_owned(),
        Some(Value::Number(number)) => number.to_string(),
        _ => String::new(),
    };
    let number = |key: &str| {
        text(key)
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n > 0.0)
    };
    let video = text("videoUrl");
    if !video.starts_with("https://") {
        return Err("the template's page has no preview video".to_owned());
    }
    let even = |side: f64| ((side as u32).clamp(2, 8192)) & !1;
    Ok(Template {
        id: text("templateId"),
        title: text("title"),
        author: detail
            .pointer("/author/name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        seconds: number("templateDuration").map_or(0.0, |ms| ms / 1000.0),
        slots: number("segmentAmount").map_or(1, |n| (n as u32).clamp(1, 500)),
        width: even(number("videoWidth").unwrap_or(1080.0)),
        height: even(number("videoHeight").unwrap_or(1920.0)),
        video,
    })
}

/// Where the template's clips change: up to `slots - 1` instants, in
/// order, picked from `changes` (see [`crate::media::picture_changes`]) as
/// the strongest changes that stand out from the footage around them, no
/// two closer than a fifth of a second, so a flash between two shots is
/// one cut, not two. A template whose preview shows fewer
/// clean cuts gets fewer slots, not invented ones.
pub fn slot_cuts(changes: &[(f64, f64)], slots: u32, seconds: f64) -> Vec<f64> {
    const GAP: f64 = 0.2;
    if slots < 2 || changes.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<f64> = changes.iter().map(|(_, score)| *score).collect();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    let floor = (median * 2.0).max(8.0);
    // A peak: higher than the frames either side of it.
    let mut peaks: Vec<(f64, f64)> = changes
        .iter()
        .enumerate()
        .filter(|(i, (at, score))| {
            *score >= floor
                && *at > GAP
                && (seconds <= 0.0 || *at < seconds - GAP)
                && changes
                    .get(i.wrapping_sub(1))
                    .is_none_or(|(_, s)| s <= score)
                && changes.get(i + 1).is_none_or(|(_, s)| s < score)
        })
        .map(|(_, change)| *change)
        .collect();
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut cuts: Vec<f64> = Vec::new();
    for (at, _) in peaks {
        if cuts.len() + 1 >= slots as usize {
            break;
        }
        if cuts.iter().all(|cut| (cut - at).abs() >= GAP) {
            cuts.push(at);
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts
}

/// The draft the template makes: its frame, its sound on a track of its
/// own, and its preview cut at `cuts` into one muted slot a clip.
pub fn draft_for(
    template: &Template,
    video: &Path,
    sound: &Path,
    cuts: &[f64],
    folder: &Path,
) -> Draft {
    let micros = |seconds: f64| (seconds * 1_000_000.0).round() as i64;
    let total = template.seconds.max(0.1);
    let mut bounds = vec![0.0];
    bounds.extend(
        cuts.iter()
            .copied()
            .filter(|cut| *cut > 0.0 && *cut < total),
    );
    bounds.push(total);
    let segments: Vec<Value> = bounds
        .windows(2)
        .map(|pair| {
            let (start, length) = (micros(pair[0]), micros(pair[1] - pair[0]));
            json!({
                "material_id": "slots",
                "target_timerange": { "start": start, "duration": length },
                "source_timerange": { "start": start, "duration": length },
                "volume": 0.0,
            })
        })
        .collect();
    let name = if template.title.is_empty() {
        "CapCut template".to_owned()
    } else {
        template.title.clone()
    };
    let edit = json!({
        "canvas_config": { "width": template.width, "height": template.height },
        "fps": 30.0,
        "duration": micros(total),
        "materials": {
            "videos": [{
                "id": "slots",
                "type": "video",
                "path": video.to_string_lossy(),
                "material_name": format!("{name} (template)"),
                "duration": micros(total),
                "width": template.width,
                "height": template.height,
                "has_audio": true,
            }],
            "audios": [{
                "id": "sound",
                "type": "music",
                "path": sound.to_string_lossy(),
                "name": format!("{name} (sound)"),
                "duration": micros(total),
            }],
        },
        "tracks": [
            { "type": "video", "segments": segments },
            { "type": "audio", "segments": [{
                "material_id": "sound",
                "target_timerange": { "start": 0, "duration": micros(total) },
                "source_timerange": { "start": 0, "duration": micros(total) },
            }]},
        ],
    });
    Draft {
        name,
        folder: folder.to_string_lossy().into_owned(),
        timelines: vec![DraftTimeline {
            name: String::new(),
            edit,
        }],
    }
}

fn agent() -> String {
    format!("ClipCut/{}", crate::updates::Version::current())
}

/// Fetches the template page `url` points at.
pub fn fetch(url: &str) -> Result<Template, String> {
    let response = crate::net::get(
        url,
        Duration::from_secs(20),
        Duration::from_secs(30),
        crate::net::Trust::System,
        &|request| request.set("User-Agent", &agent()),
    )
    .map_err(|failure| format!("CapCut did not answer: {failure}"))?;
    let html = response
        .into_string()
        .map_err(|error| format!("CapCut's page was cut short: {error}"))?;
    read_page(&html)
}

/// Downloads `url` to `to`, reporting each chunk's running total.
fn download(url: &str, to: &Path, report: &mut dyn FnMut(u64, u64)) -> Result<(), String> {
    let response = crate::net::get(
        url,
        Duration::from_secs(20),
        Duration::from_secs(60),
        crate::net::Trust::System,
        &|request| request.set("User-Agent", &agent()),
    )
    .map_err(|failure| format!("the template's video did not download: {failure}"))?;
    let total = response
        .header("Content-Length")
        .and_then(|length| length.parse().ok())
        .unwrap_or(0);
    let mut reader = response.into_reader().take(MOST_BYTES);
    let mut file = std::fs::File::create(to)
        .map_err(|error| format!("could not write {}: {error}", to.display()))?;
    let mut buffer = vec![0u8; 256 * 1024];
    let mut received = 0u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("the template's video was cut short: {error}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|error| format!("could not write {}: {error}", to.display()))?;
        received += read as u64;
        report(received, total);
    }
    if received == 0 {
        return Err("the template's video came back empty".to_owned());
    }
    Ok(())
}

/// Imports the template the CapCut link in `text` points at as a new
/// project under `location`, named after the template. `report` hears the
/// preview's download as it goes.
pub fn import(
    text: &str,
    location: &str,
    report: &mut dyn FnMut(u64, u64),
) -> Result<Import, String> {
    let url = find_link(text).ok_or("that is not a CapCut link")?;
    let template = fetch(&url)?;
    let base = if template.title.is_empty() {
        "CapCut template".to_owned()
    } else {
        template.title.clone()
    };
    let name = crate::capcut::free_name(location, &base);
    let project = projects::create(
        location,
        &name,
        template.width,
        template.height,
        30,
        1,
        clipcut_project::model::VideoSettings::default().canvas,
    )?;
    let folder = Path::new(&project.path).join("Template");
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("could not make {}: {error}", folder.display()))?;
    let video = folder.join("template.mp4");
    download(&template.video, &video, report)?;
    // The sound as a file of its own, so the sound track and the slots are
    // separate media: replacing or deleting a slot leaves the music alone.
    let sound = folder.join("template sound.mp4");
    std::fs::copy(&video, &sound)
        .map_err(|error| format!("could not write {}: {error}", sound.display()))?;

    let changes = crate::media::picture_changes(&video.to_string_lossy()).unwrap_or_default();
    let mut template = template;
    if template.seconds <= 0.0 {
        template.seconds = changes.last().map_or(0.0, |(at, _)| *at);
    }
    let cuts = slot_cuts(&changes, template.slots, template.seconds);
    let draft = draft_for(&template, &video, &sound, &cuts, Path::new(&project.path));
    let imported = clipcut_project::capcut::import(&draft).map_err(|error| error.to_string())?;
    let mut settings = imported.settings.clone();
    settings.name = name;
    let document = clipcut_project::to_document(&settings, &imported.project);
    projects::save(&project.path, &document)?;

    let mut notes = Vec::new();
    let found = cuts.len() + 1;
    if found < template.slots as usize {
        notes.push(format!(
            "The template has {} clips; {found} of them could be told apart in its video.",
            template.slots
        ));
    }
    notes.push(
        "CapCut keeps a template's effects and text in its own app, so they are in the template's video, not separate."
            .to_owned(),
    );
    log::info!(
        "capcut link: {url} imported as {} · {} by {:?}, {:.2} s, {found} slot(s) of {}",
        project.path,
        template.id,
        template.author,
        template.seconds,
        template.slots
    );
    Ok(Import { project, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_is_found_in_a_shared_message() {
        assert_eq!(
            find_link("Use this template on CapCut! https://www.capcut.com/tv2/ZSpgP5tXF/ #trend"),
            Some("https://www.capcut.com/tv2/ZSpgP5tXF/".to_owned())
        );
        assert_eq!(
            find_link("https://capcut.net/abc"),
            Some("https://capcut.net/abc".to_owned())
        );
        assert_eq!(find_link("https://evilcapcut.com/x"), None);
        assert_eq!(find_link("https://www.capcut.com.evil.io/x"), None);
        assert_eq!(find_link("no link here"), None);
    }

    #[test]
    fn the_page_data_reads_into_a_template() {
        let html = r#"<html><script id="__MODERN_ROUTER_DATA__" type="application/json">{"loaderData":{"$":{"templateDetail":{"templateId":"765","title":"Trending NEW EDIT ","author":{"name":"ryduzz"},"templateDuration":17655,"segmentAmount":38,"videoWidth":"720","videoHeight":961,"videoUrl":"https://v16.example/v.mp4"}}}}</script></html>"#;
        let template = read_page(html).expect("reads");
        assert_eq!(template.title, "Trending NEW EDIT");
        assert_eq!(template.author, "ryduzz");
        assert_eq!(template.slots, 38);
        assert!((template.seconds - 17.655).abs() < 1e-9);
        assert_eq!((template.width, template.height), (720, 960));
        assert_eq!(template.video, "https://v16.example/v.mp4");
        assert!(read_page("<html>nothing</html>").is_err());
    }

    #[test]
    fn cuts_are_the_strongest_changes_and_no_more_than_the_slots() {
        // 30 fps, a quiet hum with spikes at 1.0, 2.0, 2.04 (the same cut
        // spread over two frames) and 3.0 s.
        let mut changes: Vec<(f64, f64)> = (1..150).map(|i| (i as f64 / 30.0, 2.0)).collect();
        for (at, score) in [(30, 80.0), (60, 60.0), (61, 50.0), (90, 70.0)] {
            changes[at - 1].1 = score;
        }
        assert_eq!(slot_cuts(&changes, 10, 5.0), vec![1.0, 2.0, 3.0]);
        // Two slots: the one strongest cut.
        assert_eq!(slot_cuts(&changes, 2, 5.0), vec![1.0]);
        assert!(slot_cuts(&changes, 1, 5.0).is_empty());
    }

    #[test]
    fn the_draft_has_a_slot_a_cut_and_the_sound_whole() {
        let template = Template {
            id: "1".into(),
            title: "T".into(),
            author: String::new(),
            seconds: 3.0,
            slots: 3,
            width: 720,
            height: 960,
            video: "https://x/v.mp4".into(),
        };
        let draft = draft_for(
            &template,
            Path::new("/p/v.mp4"),
            Path::new("/p/s.mp4"),
            &[1.0, 2.0],
            Path::new("/p"),
        );
        let imported = clipcut_project::capcut::import(&draft).expect("imports");
        assert_eq!(imported.settings.width, 720);
        assert_eq!(imported.settings.height, 960);
        let clips = &imported.project.timelines[0].clips;
        assert_eq!(clips.len(), 4, "three slots and the sound");
    }
}
