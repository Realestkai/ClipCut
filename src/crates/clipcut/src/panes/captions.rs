// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and ClipCut contributors

//! The captions sheet: the tray's Captions tool, as a form and then as a
//! progress report.
//!
//! One switch says what it captions: "Use Sample Voice", on, takes
//! the words from a clip with sound, chosen in the sheet from the clips on
//! the timeline that have any; off, from a script typed in it. It opens on
//! when a clip with sound is selected, or when a clip's menu opened it on
//! that clip, and off otherwise. Either lands as one batch of title clips,
//! one undo step.
//! The transcriber runs on a worker and reports as
//! [`CaptionsMsg::Progress`], ending as [`CaptionsMsg::Finished`].
//!
//! The sheet also reads a SubRip file in as captions, with the same look,
//! and writes the timeline's captions out as one.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clipcut_project::Command;
use clipcut_project::commands::ClipPatch;
use clipcut_project::model::{ClipKind, ClipMotion, ClipMove, RunStyle, TextRun, TextStyle};
use clipcut_project::srt;
use clipcut_speech::JobStage;
use clipcut_speech::transcribe::Segment;

use crate::host::{on_ui_in_project, spawn_in_project};
use crate::i18n::{t, tf};
use crate::panes::Msg;
use crate::panes::pace::{Pace, Stage, Timings};
use crate::panes::settings::installed;
use crate::studio::Studio;
use crate::ui::CaptionsSheetData;

/// Where a caption sits, by the sheet's row: a frame-height fraction from
/// the centre, positive down. Bottom, centre, top.
const CAPTION_OFFSETS: [f64; 3] = [0.35, 0.0, -0.35];
/// A caption's cap height by the sheet's row, as a fraction of the frame.
const CAPTION_SIZES: [f64; 3] = [0.04, 0.05, 0.065];
/// A rough speaking rate, for a script's timing and the speech sheet's
/// estimate.
pub const CHARS_PER_SECOND: f32 = 14.0;

/// How a line's words are shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Wording {
    /// The whole line, for as long as it is said.
    #[default]
    Lines,
    /// A few words at a time, each group for as long as it is said.
    Words,
    /// The whole line, with the word being said in a look of its own.
    Highlight,
}

/// What a run is asked for beyond the look: how the words are shown, how
/// many at a time, and whether they spring on.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct Showing {
    wording: Wording,
    /// Words to a caption under [`Wording::Words`], one to three.
    group: usize,
    /// Each caption pops on; under [`Wording::Highlight`], each line.
    pop: bool,
}

/// The shortest a caption cut from a line is left: under this a word is a
/// flicker, and it is shown with its neighbour instead.
const SHORTEST_PIECE: f64 = 0.12;
/// How long a caption takes to pop on, at most.
const POP_SECONDS: f64 = 0.18;

/// Everything that can happen to the captions sheet.
#[derive(Clone, Debug)]
pub enum CaptionsMsg {
    /// The tray's Captions tool: the selected clip's voice when it has
    /// sound, the script otherwise.
    Open,
    /// A clip's menu: the sheet on that clip's voice.
    OpenOnClip(String),
    Close,
    /// The library's Captions page came into view, or left it: a desk's
    /// home for what the phone's sheet shows.
    Shown(bool),
    /// The "Use Sample Voice" switch.
    VoiceToggled(bool),
    /// Row in the list of clips with sound.
    SourceChanged(i32),
    TextEdited(String),
    ModelChanged(i32),
    /// Row in the language list: 0 is "Auto-detect", then
    /// [`clipcut_speech::transcribe::LANGUAGES`] in its order.
    LanguageChanged(i32),
    PlacementChanged(i32),
    SizeChanged(i32),
    /// Row in the "Show" control: lines, a few words, the spoken word.
    WordingChanged(i32),
    /// Row in "Words at a time": one, two, three.
    GroupChanged(i32),
    /// The "Pop in" switch.
    PopToggled(bool),
    /// Every caption on the captions' lane in the picked style.
    RestyleAll,
    /// Run the pass the sheet describes.
    Begin,
    Cancel,
    /// The transcriber's worker reporting the stage it is in.
    Progress(JobStage),
    /// The transcriber's worker is done: what was said, or why not.
    Finished(Result<Vec<Segment>, String>),
    /// Asks for a SubRip file to read in.
    ImportSrt,
    /// The SubRip file chosen: its cues as captions.
    ImportSrtFrom(PathBuf),
    /// Asks where to write the captions as SubRip.
    ExportSrt,
    /// Where they go.
    ExportSrtTo(PathBuf),
}

/// The clip a transcription is running over, as the finished segments
/// need it: where it starts on the timeline and how fast it plays.
#[derive(Clone, Debug)]
struct Subject {
    start: f64,
    speed: f64,
    /// The look chosen when the run began.
    look: (f64, f64),
    /// And how the words are shown.
    showing: Showing,
    /// The transcriber model, and the thirty-second windows whisper reads
    /// the sound in: what the run's timing is kept by.
    model: String,
    windows: f32,
}

/// Seconds whisper takes over one thirty-second window, per megabyte of
/// model, before a run on the model has been timed: about a second for
/// base and ten for medium on eight cores of a recent CPU.
const WINDOW_SECONDS_PER_MB: f32 = 1.0 / 160.0;

/// The captions sheet's state.
#[derive(Default)]
pub struct CaptionsPane {
    /// The sheet is up: a phone's, or a window too narrow for the library.
    pub open: bool,
    /// The library's Captions page is up, on a desk.
    pub shown: bool,
    /// "Use Sample Voice": the words come from [`Self::clip`]'s sound
    /// when on, from [`Self::text`] when off.
    pub from_voice: bool,
    /// The clip whose voice is transcribed: one with sound, chosen from the
    /// timeline's. Kept while the switch is off, so turning it back on
    /// returns to the same clip. The panel floats over a live editor, so
    /// a clip with sound selected since is the one shown and run instead;
    /// see [`Self::subject_clip`].
    pub clip: Option<String>,
    /// The sole selection when [`Self::clip`] was chosen - opened on, or
    /// picked from the list - so a later selection can be told from it.
    chosen_with: Option<String>,
    /// The script's words.
    pub text: String,
    /// Row in the installed transcriber list.
    pub model: usize,
    /// The language the voice speaks, by whisper's code; `None` is
    /// whisper's to hear. Remembered across runs as
    /// `Preferences::caption_language`, so a person who captions in
    /// Indonesian says so once. An English-only model ignores it.
    pub language: Option<String>,
    /// 0 bottom, 1 centre, 2 top.
    pub placement: usize,
    /// 0 small, 1 medium, 2 large.
    pub size: usize,
    /// How a line's words are shown.
    pub wording: Wording,
    /// Row in "Words at a time": 0 is one word, 2 is three.
    pub group: usize,
    /// Captions pop on.
    pub pop: bool,
    pub running: bool,
    /// How far the running transcription looks.
    pub pace: Pace,
    /// Why the last run failed, when it did.
    pub message: String,
    /// The clip under the running transcription.
    subject: Option<Subject>,
    /// How long each model took this session, for the next run's bar.
    timings: Timings,
    /// What the style picker's search holds.
    pub style_query: String,
}

impl CaptionsPane {
    /// Applies one message. The studio is the rest of the window; while
    /// this runs the studio's copy of the pane is a blank it must not read.
    pub fn update(&mut self, msg: CaptionsMsg, studio: &mut Studio) {
        match msg {
            // A run in progress is shown as it is, however the panel is
            // asked for again: closing it never stopped the run.
            CaptionsMsg::Open | CaptionsMsg::OpenOnClip(_) if self.running => {
                self.open = true;
                self.show_in_library(studio);
            }
            CaptionsMsg::Open => {
                let selected = studio.sole_selection();
                self.open_on(studio, selected, false);
                self.show_in_library(studio);
            }
            CaptionsMsg::OpenOnClip(id) => {
                self.open_on(studio, Some(id), true);
                self.show_in_library(studio);
            }
            CaptionsMsg::Close => self.open = false,
            CaptionsMsg::Shown(shown) => {
                // First shown with nothing asked for yet: the panel as the
                // tray's button would set it up, without the sheet.
                if shown
                    && !self.shown
                    && !self.open
                    && !self.running
                    && self.clip.is_none()
                    && self.text.is_empty()
                {
                    let selected = studio.sole_selection();
                    self.open_on(studio, selected, false);
                    self.open = false;
                }
                self.shown = shown;
            }
            CaptionsMsg::VoiceToggled(on) => {
                self.from_voice = on;
                self.message.clear();
                let sources = sources(studio);
                if on
                    && !self
                        .clip
                        .as_ref()
                        .is_some_and(|id| sources.iter().any(|s| &s.id == id))
                {
                    self.clip = sources.first().map(|source| source.id.clone());
                    self.chosen_with = studio.sole_selection();
                }
            }
            CaptionsMsg::SourceChanged(index) => {
                if let Some(source) = usize::try_from(index)
                    .ok()
                    .and_then(|index| sources(studio).into_iter().nth(index))
                {
                    self.clip = Some(source.id);
                    self.chosen_with = studio.sole_selection();
                    self.message.clear();
                }
            }
            CaptionsMsg::TextEdited(text) => self.text = text,
            CaptionsMsg::ModelChanged(index) => self.model = index.max(0) as usize,
            CaptionsMsg::LanguageChanged(index) => {
                self.language = language_at(index);
                studio.prefs.caption_language = self.language.clone();
                studio.prefs.save(&studio.host.dirs);
            }
            CaptionsMsg::PlacementChanged(index) => {
                self.placement = (index.max(0) as usize).min(2);
            }
            CaptionsMsg::SizeChanged(index) => self.size = (index.max(0) as usize).min(2),
            CaptionsMsg::WordingChanged(index) => {
                self.wording = match index {
                    1 => Wording::Words,
                    2 => Wording::Highlight,
                    _ => Wording::Lines,
                };
            }
            CaptionsMsg::GroupChanged(index) => self.group = (index.max(0) as usize).min(2),
            CaptionsMsg::PopToggled(on) => self.pop = on,
            CaptionsMsg::RestyleAll => restyle_all(studio),
            CaptionsMsg::Begin => {
                if self.from_voice {
                    self.run_sound(studio);
                } else {
                    self.run_script(studio);
                }
            }
            CaptionsMsg::Cancel => {
                studio.host.transcriber.cancel();
                self.running = false;
                self.open = false;
            }
            CaptionsMsg::Progress(stage) => self.pace.report(stage),
            CaptionsMsg::Finished(result) => {
                self.running = false;
                let subject = self.subject.take();
                if let (Ok(_), Some(subject)) = (&result, &subject) {
                    self.timings
                        .keep(&subject.model, self.pace.learned(subject.windows));
                }
                match result {
                    Ok(segments) => {
                        let Some(subject) = subject else {
                            return;
                        };
                        let look = subject.look;
                        let (base, font, motion) = caption_base(studio);
                        let lines = segments.into_iter().filter_map(|segment| {
                            let text = segment.text.trim().to_owned();
                            (!text.is_empty()).then(|| {
                                (
                                    text,
                                    subject.start + segment.start / subject.speed,
                                    ((segment.end - segment.start) / subject.speed).max(0.2),
                                )
                            })
                        });
                        let titles = caption_titles(
                            lines,
                            look,
                            base.as_ref(),
                            motion.as_ref(),
                            subject.showing,
                        );
                        let count = titles.len();
                        self.open = false;
                        if count == 0 {
                            log::info!("captions: placed none · nothing was said in the clip");
                            studio.notify(&t("captions.nothingSaidClip"), true);
                        } else {
                            log::info!(
                                "captions: placed {count} captions from {:.2} s",
                                subject.start
                            );
                            let _told = crate::studio::Asked::told();
                            studio.add_titles(font, titles);
                            studio.notify(&tf("captions.addedCaptions", &[&count]), false);
                        }
                    }
                    // Asked for: the sheet is already on its way down.
                    Err(error) if error.contains("cancel") => self.open = false,
                    // Closed while it ran: the panel is not there to say
                    // it, so the notice does, and the panel too next time.
                    Err(error) => {
                        if !self.open {
                            studio.notify(&error, true);
                        }
                        self.message = error;
                    }
                }
            }
            // Asked with the studio unborrowed, on a turn of its own; see
            // `host::after_dialog`.
            CaptionsMsg::ImportSrt => crate::host::pick_files_then(
                t("captions.importSrt"),
                Some((t("captions.subRipSubtitles"), &["srt"])),
                |studio, _, _, paths| {
                    if let Some(path) = paths.into_iter().next() {
                        studio.handle(Msg::Captions(CaptionsMsg::ImportSrtFrom(path)));
                    }
                },
            ),
            CaptionsMsg::ImportSrtFrom(path) => self.import_srt(studio, &path),
            CaptionsMsg::ExportSrt => self.ask_export_srt(studio),
            CaptionsMsg::ExportSrtTo(path) => self.export_srt(studio, &path),
        }
    }

    /// A SubRip file's cues as captions where the file times them, with
    /// the sheet's look: one batch, one undo step. A file that is not
    /// UTF-8 is read as best it can be.
    fn import_srt(&mut self, studio: &mut Studio, path: &Path) {
        let cues = std::fs::read(path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                srt::parse(&String::from_utf8_lossy(&bytes)).map_err(|error| error.to_string())
            });
        let cues = match cues {
            Ok(cues) => cues,
            Err(error) => {
                log::warn!(
                    "captions: reading the subtitles in {} failed: {error}; no captions were placed",
                    crate::studio::file_name_of(&path.to_string_lossy())
                );
                self.message = tf("captions.couldNotReadSubtitles", &[&error]);
                return;
            }
        };
        let look = self.look();
        let (base, font, motion) = caption_base(studio);
        let titles = caption_titles(
            srt_lines(cues),
            look,
            base.as_ref(),
            motion.as_ref(),
            self.showing(),
        );
        if titles.is_empty() {
            self.message = t("captions.noSubtitlesInFile");
            return;
        }
        let count = titles.len();
        self.open = false;
        log::info!(
            "captions: placed {count} captions from the subtitles in {}",
            crate::studio::file_name_of(&path.to_string_lossy())
        );
        let _told = crate::studio::Asked::told();
        studio.add_titles(font, titles);
        studio.notify(&tf("captions.addedCaptions", &[&count]), false);
    }

    /// Asks where to write the captions, offering the project's folder
    /// and its name. A phone has no such dialog: the file is written
    /// beside the project under a name nothing there has, and handed to
    /// the phone to put where its file manager shows it.
    fn ask_export_srt(&mut self, studio: &mut Studio) {
        if export_lane(studio).is_none() {
            self.message = t("captions.noCaptionsToExport");
            return;
        }
        let name = format!("{}.srt", file_stem(studio));
        // No project, no project folder: Documents, as anywhere else a
        // file is written with nothing better to offer.
        let folder = studio
            .session
            .as_ref()
            .map(|session| PathBuf::from(session.path()))
            .unwrap_or_else(|| PathBuf::from(crate::studio::home_folder("Documents")));
        if cfg!(any(target_os = "android", target_os = "ios")) {
            let path = unique_path(&folder, &name);
            self.export_srt(studio, &path);
            return;
        }
        let title = t("captions.exportSrt");
        let family = t("captions.subRipSubtitles");
        crate::host::after_dialog(
            move || {
                let chosen = crate::platform::save_file(
                    &title,
                    &folder,
                    &name,
                    (family.as_str(), &["srt"]),
                )?;
                Some(
                    if chosen
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("srt"))
                    {
                        chosen
                    } else {
                        chosen.with_extension("srt")
                    },
                )
            },
            |studio, _, _, path| studio.handle(Msg::Captions(CaptionsMsg::ExportSrtTo(path))),
        );
    }

    /// Writes the captions to `path` as SubRip, and says how many and
    /// where - on a phone, once the phone has put the file where it shows
    /// it.
    fn export_srt(&mut self, studio: &mut Studio, path: &Path) {
        let Some(lane) = export_lane(studio) else {
            self.message = t("captions.noCaptionsToExport");
            return;
        };
        let cues = srt::title_cues(studio.timeline(), Some(&lane.track_id));
        if let Err(error) = std::fs::write(path, srt::write(&cues)) {
            log::warn!(
                "captions: writing the subtitles to {} failed: {error}; nothing was exported",
                crate::studio::file_name_of(&path.to_string_lossy())
            );
            self.message = tf("captions.couldNotWriteSubtitles", &[&error]);
            return;
        }
        log::info!(
            "captions: exported {} captions to {}",
            cues.len(),
            crate::studio::file_name_of(&path.to_string_lossy())
        );
        self.open = false;
        let count = cues.len();
        // Which lane was written goes without saying when it was the
        // selected title's, or the only lane with titles; otherwise the
        // notice says it was the busiest, and how to pick another.
        let busiest = lane.busiest && srt::title_lanes(studio.timeline()) > 1;
        let say = move |studio: &mut Studio, place: &str| {
            // Each key spelled out where it is used: scripts/locales.py
            // finds the strings the source asks for by reading it.
            let said = if busiest {
                tf("captions.exportedBusiestLane", &[&count, &place])
            } else {
                tf("captions.exportedCaptions", &[&count, &place])
            };
            studio.notify(&said, false);
        };
        let written = path.to_path_buf();
        let handed = crate::platform::publish_document(
            path.to_path_buf(),
            "application/x-subrip",
            move |result| {
                crate::host::on_ui(move |studio, _, _| match result {
                    Ok(place) => say(studio, &place),
                    Err(error) => {
                        log::warn!(
                            "captions: wanted {} handed to the phone's files and it was not: {error}; it stays in the project's folder",
                            crate::studio::file_name_of(&written.to_string_lossy())
                        );
                        studio.notify(
                            &tf(
                                "captions.couldNotPublishSubtitles",
                                &[&written.display(), &error],
                            ),
                            true,
                        );
                    }
                });
            },
        );
        if !handed {
            say(studio, &path.display().to_string());
        }
    }

    /// The sheet opened afresh. `clip` is the clip it was opened on, if
    /// any: the switch starts on when that clip has sound, or when a clip's
    /// menu asked for its voice (`voice`), in which case a clip without
    /// sound falls to the first clip that has some.
    /// On a desk the panel is the library's Captions page, not a sheet:
    /// the library is put in the dock if it was taken out, and turned to
    /// that page. A phone, or a window too narrow for the library beside
    /// the work, keeps the sheet.
    fn show_in_library(&mut self, studio: &mut Studio) {
        if studio.meters.phone || studio.compact {
            return;
        }
        self.open = false;
        if !studio.dock.contains(crate::ui::PaneKind::Media) {
            crate::add_pane(studio, crate::ui::PaneKind::Media);
            studio.remember_layout_soon();
        }
        studio.captions_jump += 1;
    }

    fn open_on(&mut self, studio: &Studio, clip: Option<String>, voice: bool) {
        let installed = installed(&studio.settings.transcribers);
        let model = installed.iter().position(|model| model.active).unwrap_or(0);
        let sources = sources(studio);
        let sounding = clip.filter(|id| sources.iter().any(|source| &source.id == id));
        let from_voice = voice || sounding.is_some();
        let clip = sounding.or_else(|| {
            from_voice
                .then(|| sources.first().map(|source| source.id.clone()))
                .flatten()
        });
        *self = CaptionsPane {
            open: true,
            from_voice,
            clip,
            chosen_with: studio.sole_selection(),
            model,
            // A code the list no longer has is heard instead.
            language: studio
                .prefs
                .caption_language
                .clone()
                .filter(|code| language_row(Some(code)) > 0),
            placement: 0,
            size: 1,
            wording: self.wording,
            group: self.group,
            pop: self.pop,
            timings: std::mem::take(&mut self.timings),
            shown: self.shown,
            ..CaptionsPane::default()
        };
    }

    /// The clip the panel transcribes, and the one its list shows - the
    /// same answer for both, so it never runs a clip it is not showing.
    /// The editor stays live under the panel: a clip with sound selected
    /// since the panel's clip was chosen is the one meant now, and is
    /// taken. Otherwise the chosen clip while it is still on the timeline
    /// with sound, and none - nothing to run - when it is not.
    fn subject_clip(&self, studio: &Studio, sources: &[Source]) -> Option<String> {
        follow_selection(
            studio.sole_selection(),
            self.chosen_with.as_deref(),
            self.clip.as_deref(),
            |id| sources.iter().any(|source| source.id == id),
        )
    }

    /// A caption's look, by the sheet's rows: where it sits and its size.
    fn look(&self) -> (f64, f64) {
        (
            CAPTION_OFFSETS[self.placement.min(2)],
            CAPTION_SIZES[self.size.min(2)],
        )
    }

    /// How the words are shown, by the sheet's rows.
    fn showing(&self) -> Showing {
        Showing {
            wording: self.wording,
            group: self.group.min(2) + 1,
            pop: self.pop,
        }
    }

    /// The script as titles, one after another from the playhead.
    fn run_script(&mut self, studio: &mut Studio) {
        let lines = script_captions(&self.text);
        if lines.is_empty() {
            self.message = t("captions.nothingToCaptionYet");
            return;
        }
        let look = self.look();
        let mut at = f64::from(studio.playhead);
        let (base, font, motion) = caption_base(studio);
        let lines = lines.into_iter().map(|(text, seconds)| {
            let line = (text, at, seconds);
            at += seconds;
            line
        });
        let titles = caption_titles(lines, look, base.as_ref(), motion.as_ref(), self.showing());
        let count = titles.len();
        self.open = false;
        log::info!(
            "captions: placed {count} captions from a script, from {:.2} s",
            studio.playhead
        );
        let _told = crate::studio::Asked::told();
        studio.add_titles(font, titles);
        studio.notify(&tf("captions.addedCaptions", &[&count]), false);
    }

    /// The sheet's clip through the transcriber on a worker, reporting into
    /// the sheet as it goes.
    fn run_sound(&mut self, studio: &mut Studio) {
        // The clip the panel shows, and no other: see `subject_clip`.
        let Some(clip) = self
            .subject_clip(studio, &sources(studio))
            .and_then(|id| studio.clip(&id))
            .cloned()
        else {
            self.message = t("captions.clipNoLongerTimeline");
            return;
        };
        let Some(media) = studio.project().media_by_id(&clip.media_id).cloned() else {
            self.message = t("captions.clipHasNoFile");
            return;
        };
        let Some((model, megabytes)) = installed(&studio.settings.transcribers)
            .get(self.model)
            .map(|model| (model.id.clone(), model.megabytes))
        else {
            self.message = t("captions.downloadTranscriberModelFirst");
            return;
        };
        let request = clipcut_speech::transcribe::TranscribeRequest {
            path: media.path.clone(),
            audio_stream: clip.audio_stream,
            source_start: clip.source_start,
            window: clip.duration * clip.speed,
            model_id: model.clone(),
            language: (!clipcut_speech::transcribe::english_only(&model))
                .then(|| self.language.clone())
                .flatten(),
        };
        let dirs = studio.host.dirs.clone();
        let transcriber = Arc::clone(&studio.host.transcriber);
        // The run as the bar lays it out: the sound read in at a few
        // hundred times its length, the model loaded unless it still is,
        // and whisper a window at a time - at the pace the last run on
        // this model measured, or the estimate by its size.
        let windows = (request.window / 30.0).ceil().max(1.0) as f32;
        let learned = self.timings.get(&model);
        let loaded = studio.host.transcriber.loaded_model().as_deref() == Some(model.as_str());
        let load = if loaded {
            0.0
        } else {
            learned.load.unwrap_or(0.5 + megabytes / 400.0)
        };
        let per_window = learned
            .per_unit
            .unwrap_or(0.3 + megabytes * WINDOW_SECONDS_PER_MB);
        self.pace = Pace::new(vec![
            (Stage::Reading, 0.2 + request.window as f32 / 400.0),
            (Stage::Loading, load),
            (Stage::Working, windows * per_window),
        ]);
        self.subject = Some(Subject {
            start: clip.start,
            speed: clip.speed,
            look: self.look(),
            showing: self.showing(),
            model,
            windows,
        });
        self.running = true;
        self.message.clear();
        let epoch = crate::host::project_epoch();
        spawn_in_project(
            move || {
                transcriber.transcribe(&dirs, &request, move |stage| {
                    on_ui_in_project(epoch, move |studio, _, _| {
                        studio.handle(Msg::Captions(CaptionsMsg::Progress(stage)));
                    });
                })
            },
            |studio, _, _, result| studio.handle(Msg::Captions(CaptionsMsg::Finished(result))),
        );
    }

    /// Whether the model the sheet has chosen hears only English.
    fn english_only(&self, studio: &Studio) -> bool {
        installed(&studio.settings.transcribers)
            .get(self.model)
            .is_some_and(|model| clipcut_speech::transcribe::english_only(&model.id))
    }

    /// The sheet's language list: "Auto-detect", then every language
    /// whisper knows by its own name. An English-only model hears English
    /// and nothing else, so its list is English alone, and is not a choice.
    /// Empty while the sheet is closed, which is most of the time.
    pub fn language_rows(&self, studio: &Studio) -> Vec<slint::SharedString> {
        if !self.open && !self.shown {
            return Vec::new();
        }
        if self.english_only(studio) {
            return vec![ENGLISH.into()];
        }
        std::iter::once(t("captions.autoDetect").into())
            .chain(
                clipcut_speech::transcribe::LANGUAGES
                    .iter()
                    .map(|(_, name)| (*name).into()),
            )
            .collect()
    }

    /// The sheet as Slint shows it.
    pub fn data(&self, studio: &Studio) -> CaptionsSheetData {
        let sources = if self.open || self.shown {
            sources(studio)
        } else {
            Vec::new()
        };
        let source = self
            .subject_clip(studio, &sources)
            .and_then(|id| sources.iter().position(|source| source.id == id))
            .map_or(-1, |index| index as i32);
        let labels: Vec<slint::SharedString> = sources
            .into_iter()
            .map(|source| source.label.into())
            .collect();
        let english_only = self.english_only(studio);
        CaptionsSheetData {
            open: self.open,
            from_voice: self.from_voice,
            sources: std::rc::Rc::new(slint::VecModel::from(labels)).into(),
            source,
            text: self.text.as_str().into(),
            model: self.model as i32,
            language: if english_only {
                0
            } else {
                language_row(self.language.as_deref()) as i32
            },
            language_fixed: english_only,
            placement: self.placement as i32,
            size: self.size as i32,
            wording: match self.wording {
                Wording::Lines => 0,
                Wording::Words => 1,
                Wording::Highlight => 2,
            },
            group: self.group as i32,
            pop: self.pop,
            running: self.running,
            progress: self.pace.floor,
            ceiling: self.pace.ceiling,
            step: self.pace.step,
            left: self.pace.left,
            stage: match self.pace.stage() {
                Some(Stage::Loading) => t("captions.loadingTheModel"),
                Some(Stage::Working) => t("captions.transcribing"),
                _ => t("captions.readingTheSound"),
            }
            .into(),
            ready: !installed(&studio.settings.transcribers).is_empty(),
            message: self.message.as_str().into(),
        }
    }
}

/// English, as the language list names it.
const ENGLISH: &str = "English";

/// The language list's row for `code`: 0, "Auto-detect", for none and
/// for a code the list does not have.
fn language_row(code: Option<&str>) -> usize {
    code.and_then(|code| {
        clipcut_speech::transcribe::LANGUAGES
            .iter()
            .position(|(known, _)| *known == code)
    })
    .map_or(0, |index| index + 1)
}

/// The language at row `index` of the list; `None` for "Auto-detect" and
/// for a row past its end.
fn language_at(index: i32) -> Option<String> {
    usize::try_from(index)
        .ok()
        .and_then(|row| row.checked_sub(1))
        .and_then(|row| clipcut_speech::transcribe::LANGUAGES.get(row))
        .map(|(code, _)| (*code).to_owned())
}

/// Imported cues keep the file's timing, including captions shorter than
/// generated speech segments. The title command applies the engine's minimum
/// clip duration where needed.
#[cfg(test)]
fn srt_commands(cues: Vec<srt::Cue>, look: (f64, f64), base: Option<&TextStyle>) -> Vec<Command> {
    caption_titles(srt_lines(cues), look, base, None, Showing::default())
        .into_iter()
        .map(|(command, _)| command)
        .collect()
}

/// A SubRip file's cues as caption lines: the words, when, and for how
/// long. A cue with no words is not a caption.
fn srt_lines(cues: Vec<srt::Cue>) -> impl Iterator<Item = (String, f64, f64)> {
    cues.into_iter()
        .filter(|cue| !cue.text.trim().is_empty())
        .map(|cue| (cue.text.trim().to_owned(), cue.start, cue.end - cue.start))
}

/// One caption cut from a line: its words, when and for how long, the
/// characters of them being said - under [`Wording::Highlight`] - and
/// whether it opens or closes its line.
#[derive(Clone, PartialEq, Debug)]
struct Piece {
    text: String,
    start: f64,
    duration: f64,
    spoken: Option<(usize, usize)>,
    first: bool,
    last: bool,
}

/// A line as the captions it is shown as. Whole, it is one. Otherwise it
/// is cut at its words: a few at a time, or the whole line once per word
/// with that word marked. When each word is said is not known - the
/// transcriber times lines, a script and a subtitle file likewise - so a
/// word is given its share of the line's time by its length, which is
/// how long it takes to say near enough. The captions run edge to edge:
/// each starts where the one before ends, so on screen they are one
/// caption changing. A word too short to see is shown with its neighbour.
fn pieces(text: &str, start: f64, duration: f64, showing: Showing) -> Vec<Piece> {
    let whole = || {
        vec![Piece {
            text: text.to_owned(),
            start,
            duration,
            spoken: None,
            first: true,
            last: true,
        }]
    };
    if showing.wording == Wording::Lines {
        return whole();
    }
    // The words, as character spans: what a run counts in.
    let mut words: Vec<(usize, usize)> = Vec::new();
    let mut open = None;
    let mut length = 0;
    for (index, c) in text.chars().enumerate() {
        length = index + 1;
        match (c.is_whitespace(), open) {
            (false, None) => open = Some(index),
            (true, Some(from)) => {
                words.push((from, index));
                open = None;
            }
            _ => {}
        }
    }
    if let Some(from) = open {
        words.push((from, length));
    }
    if words.is_empty() || duration.is_nan() || duration <= 0.0 {
        return whole();
    }
    // A word's share of the time: its letters, and a beat for the gap.
    let weight = |(from, to): (usize, usize)| (to - from + 1) as f64;
    let total: f64 = words.iter().copied().map(weight).sum();
    let step = match showing.wording {
        Wording::Words => showing.group.clamp(1, 3),
        _ => 1,
    };
    // Each group: its characters and its seconds.
    let mut groups: Vec<((usize, usize), f64)> = words
        .chunks(step)
        .map(|chunk| {
            let share: f64 = chunk.iter().copied().map(weight).sum();
            (
                (chunk[0].0, chunk[chunk.len() - 1].1),
                duration * share / total,
            )
        })
        .collect();
    while groups.len() > 1 {
        let Some((short, _)) = groups
            .iter()
            .enumerate()
            .filter(|(_, (_, seconds))| *seconds < SHORTEST_PIECE)
            .min_by(|a, b| a.1.1.total_cmp(&b.1.1))
        else {
            break;
        };
        // Joined to the shorter of its neighbours.
        let with = if short == 0 {
            1
        } else if short + 1 == groups.len() || groups[short - 1].1 <= groups[short + 1].1 {
            short - 1
        } else {
            short + 1
        };
        let (low, high) = (short.min(with), short.max(with));
        groups[low] = (
            (groups[low].0.0, groups[high].0.1),
            groups[low].1 + groups[high].1,
        );
        groups.remove(high);
    }
    let count = groups.len();
    let letters: Vec<char> = text.chars().collect();
    // Where each caption starts, and the line's end after the last. On the
    // grid, so a caption's start plus its length is exactly where the next
    // begins - the next line's first caption too: a hair of overlap would
    // be read as two titles in the same place and push one up a lane.
    let mut edges = vec![on_grid(start)];
    let mut said = 0.0;
    for (_, seconds) in &groups[..count - 1] {
        said += seconds;
        edges.push(on_grid(start + said));
    }
    edges.push(on_grid(start + duration));
    groups
        .into_iter()
        .enumerate()
        .map(|(index, ((from, to), _))| Piece {
            text: match showing.wording {
                Wording::Words => letters[from..to].iter().collect(),
                _ => text.to_owned(),
            },
            start: edges[index],
            duration: edges[index + 1] - edges[index],
            spoken: (showing.wording == Wording::Highlight).then_some((from, to)),
            first: index == 0,
            last: index + 1 == count,
        })
        .collect()
}

/// A time on a grid of 1/1024 s, under a millisecond: fine enough that no
/// one sees the move, coarse enough that adding and subtracting times on
/// it is exact in floating point.
fn on_grid(seconds: f64) -> f64 {
    (seconds * 1024.0).round() / 1024.0
}

/// The look of the word being said, over the caption's own. A style that
/// marks some of its own words - a keyword preset - marks the spoken word
/// the same way; any other gets a colour that stands off both the words
/// and what is behind them.
fn spoken_look(base: Option<&TextStyle>) -> RunStyle {
    if let Some(run) = base.and_then(|base| base.runs.first()) {
        return run.style.clone();
    }
    fn rgb(colour: &str) -> Option<[i32; 3]> {
        let hex = colour.trim().strip_prefix('#')?;
        let part = |at: usize| i32::from_str_radix(hex.get(at..at + 2)?, 16).ok();
        Some([part(0)?, part(2)?, part(4)?])
    }
    let apart = |a: [i32; 3], b: Option<[i32; 3]>| {
        b.is_none_or(|b| (0..3).map(|i| (a[i] - b[i]).abs()).sum::<i32>() > 180)
    };
    let fill = rgb(base.map_or("#ffffff", |base| base.color.as_str()));
    let plate = base.and_then(|base| rgb(&base.background));
    let colour = ["#ffe14a", "#ff3d6e", "#39ff88", "#ffffff", "#111111"]
        .into_iter()
        .find(|colour| {
            rgb(colour).is_some_and(|candidate| apart(candidate, fill) && apart(candidate, plate))
        })
        .unwrap_or("#ffe14a");
    RunStyle {
        color: Some(colour.to_owned()),
        ..RunStyle::default()
    }
}

/// Lines - words, start, seconds - as the titles to place, in order, each
/// with how it moves: see [`pieces`] for how a line is cut. `motion` is
/// the picked style's own. A caption that pops does that and nothing
/// else; under [`Wording::Highlight`] a line's captions are one caption
/// to the eye, so only its first comes on and only its last goes off.
fn caption_titles(
    lines: impl IntoIterator<Item = (String, f64, f64)>,
    look: (f64, f64),
    base: Option<&TextStyle>,
    motion: Option<&ClipMotion>,
    showing: Showing,
) -> Vec<(Command, Option<ClipMotion>)> {
    let spoken = spoken_look(base);
    let one_caption = showing.wording == Wording::Highlight;
    let mut titles = Vec::new();
    for (text, start, duration) in lines {
        for piece in pieces(&text, start, duration, showing) {
            let comes_on = !one_caption || piece.first;
            let goes_off = !one_caption || piece.last;
            let moves = if showing.pop {
                comes_on.then(|| ClipMotion {
                    enter: Some(ClipMove {
                        kind: "pop".to_owned(),
                        seconds: POP_SECONDS.min(piece.duration / 2.0),
                    }),
                    ..ClipMotion::default()
                })
            } else {
                motion.map(|motion| ClipMotion {
                    enter: motion.enter.clone().filter(|_| comes_on),
                    exit: motion.exit.clone().filter(|_| goes_off),
                    cycle: motion.cycle.clone(),
                })
            }
            .and_then(ClipMotion::tidy);
            let runs = piece
                .spoken
                .map(|(start, end)| {
                    vec![TextRun {
                        start,
                        end,
                        style: spoken.clone(),
                    }]
                })
                .unwrap_or_default();
            titles.push((
                caption_clip(piece.text, piece.start, piece.duration, look, base, runs),
                moves,
            ));
        }
    }
    titles
}

/// Every caption on the captions' lane - the selected title's, else the
/// busiest; see [`export_lane`] - set in the picked style, as one undo
/// step. A caption keeps its words, its size, its place and the words it
/// marks; the family, weight, colours, outline, shadow and background
/// become the style's.
fn restyle_all(studio: &mut Studio) {
    let Some(lane) = export_lane(studio) else {
        studio.notify(&t("captions.noCaptionsToRestyle"), true);
        return;
    };
    let (base, font, _) = caption_base(studio);
    let base = base.unwrap_or_else(|| TextStyle {
        font_family: "Hanken Grotesk".to_owned(),
        font_weight: 600.0,
        ..TextStyle::default()
    });
    let commands: Vec<Command> = studio
        .timeline()
        .clips
        .iter()
        .filter(|clip| clip.kind == ClipKind::Text && clip.track_id == lane.track_id)
        .filter_map(|clip| {
            let held = clip.text.as_ref()?;
            let style = restyled(held, &base);
            (style != *held).then(|| Command::UpdateClip {
                clip_id: clip.id.clone(),
                patch: ClipPatch {
                    text: Some(Some(style)),
                    ..ClipPatch::default()
                },
            })
        })
        .collect();
    let count = commands.len();
    if count == 0 {
        studio.notify(&t("captions.noCaptionsToRestyle"), true);
        return;
    }
    log::info!("captions: restyled {count} captions on one lane");
    let _told = crate::studio::Asked::told();
    studio.apply(Command::Batch {
        commands: font.into_iter().chain(commands).collect(),
    });
    studio.notify(&tf("captions.restyledCaptions", &[&count]), false);
}

/// A caption in another style: `look`, around what is the caption's own -
/// its words, its size, its box and which words it marks. A mark is worn
/// the new style's way (see [`spoken_look`]): the old one's colour can
/// vanish into the new one's plate - green on a yellow highlighter.
fn restyled(held: &TextStyle, look: &TextStyle) -> TextStyle {
    let mark = spoken_look(Some(look));
    TextStyle {
        content: held.content.clone(),
        font_size: held.font_size,
        max_width: held.max_width,
        max_height: held.max_height,
        runs: held
            .runs
            .iter()
            .map(|run| TextRun {
                style: mark.clone(),
                ..run.clone()
            })
            .collect(),
        ..look.clone()
    }
}

/// The lane an SRT export writes out.
struct ExportLane {
    track_id: String,
    /// Chosen as the lane with the most titles, not by a selected title.
    busiest: bool,
}

/// The lane whose titles are written out as captions: the selected
/// title's; or with no title selected, the lane with the most titles -
/// captions come many to a lane, a title card one or two, and nothing
/// else in the document tells them apart (see `srt::busiest_title_lane`).
/// None when the timeline has no title.
fn export_lane(studio: &Studio) -> Option<ExportLane> {
    let selected = studio
        .sole_selection()
        .and_then(|id| studio.clip(&id))
        .filter(|clip| clip.kind == ClipKind::Text)
        .map(|clip| clip.track_id.clone());
    match selected {
        Some(track_id) => Some(ExportLane {
            track_id,
            busiest: false,
        }),
        None => srt::busiest_title_lane(studio.timeline()).map(|track_id| ExportLane {
            track_id,
            busiest: true,
        }),
    }
}

/// The name an export is offered under: the project's, as one file name,
/// or "captions" with no project or no usable name.
fn file_stem(studio: &Studio) -> String {
    let name = studio
        .session
        .as_ref()
        .map(|session| session.settings().name)
        .unwrap_or_default();
    let stem: String = name
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let stem = stem.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if stem.is_empty() {
        "captions".to_owned()
    } else {
        stem.to_owned()
    }
}

/// `name` in `dir`, or `name (2)`, `name (3)`... until nothing there is
/// written over.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|path| !path.exists())
        .unwrap_or(first)
}

/// [`CaptionsPane::subject_clip`] with the studio taken out: `selected`
/// the sole selection, `chosen_with` the selection when `clip` was chosen,
/// and `sounds` whether a clip is on the timeline with sound.
fn follow_selection(
    selected: Option<String>,
    chosen_with: Option<&str>,
    clip: Option<&str>,
    sounds: impl Fn(&str) -> bool,
) -> Option<String> {
    if let Some(selected) = selected.filter(|id| sounds(id))
        && Some(selected.as_str()) != chosen_with
    {
        return Some(selected);
    }
    clip.filter(|id| sounds(id)).map(str::to_owned)
}

/// A clip on the timeline whose voice can be captioned, as the sheet's
/// list shows it.
struct Source {
    id: String,
    label: String,
}

/// The clips on the current timeline that have sound, in the order they
/// play: an audio clip, or a video whose file has an audio stream.
fn sources(studio: &Studio) -> Vec<Source> {
    let project = studio.project();
    let mut clips: Vec<_> = studio
        .timeline()
        .clips
        .iter()
        .filter(|clip| studio.clip_has_sound(clip))
        .map(|clip| {
            let name = project
                .media_by_id(&clip.media_id)
                .map_or(clip.name.as_str(), |media| media.name.as_str());
            (clip.start, clip.id.clone(), name.to_owned())
        })
        .collect();
    clips.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    clips
        .into_iter()
        .map(|(start, id, name)| Source {
            id,
            label: source_label(&name, start),
        })
        .collect()
}

/// A clip as the list names it: its file and where it starts on the
/// timeline, so two clips of one file are told apart.
fn source_label(name: &str, start: f64) -> String {
    let seconds = start.max(0.0).floor() as u64;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{name} · {hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{name} · {minutes}:{seconds:02}")
    }
}

/// One caption as a title clip: its words, when and for how long, and
/// its look.
fn caption_clip(
    text: String,
    start: f64,
    duration: f64,
    look: (f64, f64),
    base: Option<&TextStyle>,
    runs: Vec<TextRun>,
) -> Command {
    let (offset_y, font_size) = look;
    // The kept look where there is one - family, weight, colours, stroke,
    // shadow, background - at the caption's own size and place; the
    // bundled face otherwise.
    let style = match base {
        Some(base) => TextStyle {
            content: text,
            font_size,
            max_width: 0.0,
            max_height: 0.0,
            // The style's runs are some words' looks, and these are
            // other words: only the word being said is marked.
            runs,
            ..base.clone()
        },
        None => TextStyle {
            content: text,
            font_family: "Hanken Grotesk".to_owned(),
            font_size,
            font_weight: 600.0,
            runs,
            ..TextStyle::default()
        },
    };
    Command::AddTextClip {
        track_id: None,
        above: true,
        start,
        style: Some(style),
        duration: Some(duration),
        offset_y: Some(offset_y),
    }
}

/// The look captions are set in, before their own size and place: the
/// text preset picked on the sheet (`Preferences::caption_style`), with
/// the font it brings registered on the project in the same undo step;
/// or, for the plain preset or none, the look a title was kept as. Last,
/// how the preset's titles move, when it says.
fn caption_base(studio: &Studio) -> (Option<TextStyle>, Option<Command>, Option<ClipMotion>) {
    let picked = studio
        .prefs
        .caption_style
        .as_deref()
        .filter(|id| *id != "default")
        .and_then(|id| studio.text_presets.iter().find(|preset| preset.id == id));
    match picked {
        Some(preset) => (
            Some(preset.style.clone()),
            crate::presets::install_font(&studio.host.dirs, preset)
                .map(|(family, path)| Command::AddFont { family, path }),
            preset.motion.clone(),
        ),
        None => (studio.prefs.title_style.clone(), None, None),
    }
}

/// The preset the sheet shows as picked: the one remembered, while it is
/// still on the list, else the plain one.
pub fn caption_style_id(studio: &Studio) -> &str {
    studio
        .prefs
        .caption_style
        .as_deref()
        .filter(|id| studio.text_presets.iter().any(|preset| preset.id == *id))
        .unwrap_or("default")
}

/// Longest a caption line gets before it is wrapped: about what two lines
/// of broadcast subtitle hold, and what a reader takes in at a glance.
const CAPTION_CHARS: usize = 42;

/// A script as caption lines, each with how long it stays up: a line's
/// reading time at [`CHARS_PER_SECOND`], held to one second at least so
/// a short word is not a flicker, and seven at most so a long line does
/// not hang. A line break in the script is a break the author asked for;
/// within a paragraph a sentence is a caption, and a long sentence wraps
/// at its words.
fn script_captions(text: &str) -> Vec<(String, f64)> {
    text.lines()
        .flat_map(sentences)
        .flat_map(|sentence| wrap_caption(&sentence))
        .map(|line| {
            let seconds =
                (line.chars().count() as f64 / f64::from(CHARS_PER_SECOND)).clamp(1.0, 7.0);
            (line, seconds)
        })
        .collect()
}

/// A paragraph's sentences. A full stop, question or exclamation mark ends
/// one when it is followed by space or by the end - so "3.5" and "e.g." hold
/// together - and the CJK marks end one on their own.
fn sentences(paragraph: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = paragraph.chars().peekable();
    while let Some(ch) = chars.next() {
        current.push(ch);
        let ends = match ch {
            '。' | '！' | '？' => true,
            '.' | '!' | '?' => chars.peek().is_none_or(|next| next.is_whitespace()),
            _ => false,
        };
        if ends {
            let sentence = current.trim();
            if !sentence.is_empty() {
                out.push(sentence.to_owned());
            }
            current.clear();
        }
    }
    let rest = current.trim();
    if !rest.is_empty() {
        out.push(rest.to_owned());
    }
    out
}

/// A sentence in lines of at most [`CAPTION_CHARS`], broken between words;
/// a word longer than a line, or a run of CJK with no spaces, is broken
/// where it must be.
fn wrap_caption(sentence: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_chars = 0;
    for word in sentence.split_whitespace() {
        let word_chars = word.chars().count();
        if line_chars > 0 && line_chars + 1 + word_chars > CAPTION_CHARS {
            lines.push(std::mem::take(&mut line));
            line_chars = 0;
        }
        if word_chars > CAPTION_CHARS {
            let mut piece = String::new();
            for ch in word.chars() {
                piece.push(ch);
                if piece.chars().count() == CAPTION_CHARS {
                    lines.push(std::mem::take(&mut piece));
                }
            }
            line = piece;
            line_chars = line.chars().count();
            continue;
        }
        if line_chars > 0 {
            line.push(' ');
            line_chars += 1;
        }
        line.push_str(word);
        line_chars += word_chars;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{script_captions, source_label};

    #[test]
    fn imported_srt_preserves_short_cues_and_multiline_text() {
        let cues = clipcut_project::srt::parse(
            "1\n00:00:01,000 --> 00:00:01,100\nOlá\nAção\n\n2\n00:00:02,000 --> 00:00:03,000\nFim\n",
        ).unwrap();
        let commands = super::srt_commands(cues.clone(), (0.35, 0.04), None);
        let mut editor = clipcut_project::Editor::new();
        editor
            .apply(clipcut_project::Command::Batch { commands })
            .unwrap();
        let exported = clipcut_project::srt::write(&clipcut_project::srt::title_cues(
            editor.project().active(),
            None,
        ));
        assert_eq!(clipcut_project::srt::parse(&exported).unwrap(), cues);
        assert!(editor.undo());
        assert!(editor.project().active().clips.is_empty());
        assert!(editor.redo());
        assert_eq!(
            clipcut_project::srt::title_cues(editor.project().active(), None).len(),
            2
        );
    }

    fn showing(wording: super::Wording, group: usize, pop: bool) -> super::Showing {
        super::Showing {
            wording,
            group,
            pop,
        }
    }

    /// A line cut two words at a time: each caption its own words, edge
    /// to edge across the line's time, longer words held longer.
    #[test]
    fn a_line_is_cut_a_few_words_at_a_time() {
        let cut = super::pieces(
            "the quick brown fox jumps",
            10.0,
            3.0,
            showing(super::Wording::Words, 2, false),
        );
        let words: Vec<&str> = cut.iter().map(|piece| piece.text.as_str()).collect();
        assert_eq!(words, ["the quick", "brown fox", "jumps"]);
        assert_eq!(cut[0].start, 10.0);
        for pair in cut.windows(2) {
            assert!((pair[0].start + pair[0].duration - pair[1].start).abs() < 1e-9);
        }
        let end = cut[2].start + cut[2].duration;
        assert!((end - 13.0).abs() < 1e-9, "{end}");
        assert!(cut[0].duration > cut[2].duration);
        assert!(cut.iter().all(|piece| piece.spoken.is_none()));
        assert!(cut[0].first && !cut[0].last && cut[2].last);
    }

    /// The spoken word marked: the whole line once per word, the mark
    /// counted in characters, accents and line breaks included.
    #[test]
    fn a_line_marks_the_word_being_said() {
        let line = "Olá Ação\nfim";
        let cut = super::pieces(line, 0.0, 3.0, showing(super::Wording::Highlight, 1, false));
        assert_eq!(cut.len(), 3);
        assert!(cut.iter().all(|piece| piece.text == line));
        let marks: Vec<(usize, usize)> = cut.iter().filter_map(|piece| piece.spoken).collect();
        assert_eq!(marks, [(0, 3), (4, 8), (9, 12)]);
    }

    /// A word too short to see is shown with its neighbour, and a line
    /// said in no time at all stays one caption.
    #[test]
    fn a_flicker_of_a_word_joins_its_neighbour() {
        let cut = super::pieces(
            "a b c d",
            0.0,
            0.3,
            showing(super::Wording::Words, 1, false),
        );
        assert!(cut.len() < 4, "{cut:?}");
        assert!(
            cut.iter()
                .all(|piece| piece.duration >= super::SHORTEST_PIECE)
        );
        let said: Vec<&str> = cut.iter().flat_map(|piece| piece.text.split(' ')).collect();
        assert_eq!(said, ["a", "b", "c", "d"]);
        let none = super::pieces("a b", 0.0, 0.0, showing(super::Wording::Words, 1, false));
        assert_eq!(none.len(), 1);
    }

    /// Captions cut from lines land on one lane, in order, where they
    /// were asked for: neighbours that share an edge do not push each
    /// other along. And a caption that pops says so, a line's first
    /// alone when the line is one caption to the eye.
    #[test]
    fn cut_captions_land_edge_to_edge_on_one_lane() {
        let lines = vec![
            ("one two three four five".to_owned(), 1.0, 2.5),
            ("six seven".to_owned(), 3.5, 1.1),
        ];
        let titles = super::caption_titles(
            lines.clone(),
            (0.35, 0.05),
            None,
            None,
            showing(super::Wording::Highlight, 1, true),
        );
        assert_eq!(titles.len(), 7);
        let popping: Vec<bool> = titles.iter().map(|(_, motion)| motion.is_some()).collect();
        assert_eq!(popping, [true, false, false, false, false, true, false]);
        let asked: Vec<f64> = titles
            .iter()
            .map(|(command, _)| match command {
                clipcut_project::Command::AddTextClip { start, .. } => *start,
                _ => unreachable!(),
            })
            .collect();
        let mut editor = clipcut_project::Editor::new();
        editor
            .apply(clipcut_project::Command::Batch {
                commands: titles.into_iter().map(|(command, _)| command).collect(),
            })
            .unwrap();
        let clips = &editor.project().active().clips;
        assert_eq!(clips.len(), 7);
        assert!(clips.iter().all(|clip| clip.track_id == clips[0].track_id));
        let mut placed: Vec<f64> = clips.iter().map(|clip| clip.start).collect();
        placed.sort_by(f64::total_cmp);
        for (placed, asked) in placed.iter().zip(&asked) {
            assert!((placed - asked).abs() < 1e-6, "{placed} for {asked}");
        }
        let marked = clips[1].text.as_ref().unwrap();
        assert_eq!(marked.runs.len(), 1);
        assert_eq!(marked.runs[0].style.color.as_deref(), Some("#ffe14a"));
    }

    /// The spoken word's colour stands off the words and the plate: a
    /// yellow highlighter's is not yellow, and a keyword style's own
    /// marked word is the mark.
    #[test]
    fn the_spoken_word_stands_off_its_caption() {
        use clipcut_project::model::{RunStyle, TextRun, TextStyle};
        let plain = super::spoken_look(None);
        assert_eq!(plain.color.as_deref(), Some("#ffe14a"));
        let plated = TextStyle {
            color: "#111111".to_owned(),
            background: "#fff04a".to_owned(),
            ..TextStyle::default()
        };
        assert_eq!(
            super::spoken_look(Some(&plated)).color.as_deref(),
            Some("#ff3d6e")
        );
        let keyword = TextStyle {
            runs: vec![TextRun {
                start: 0,
                end: 4,
                style: RunStyle {
                    color: Some("#39ff88".to_owned()),
                    font_weight: Some(900.0),
                    ..RunStyle::default()
                },
            }],
            ..TextStyle::default()
        };
        let look = super::spoken_look(Some(&keyword));
        assert_eq!(look.color.as_deref(), Some("#39ff88"));
        assert_eq!(look.font_weight, Some(900.0));
    }

    /// Restyling keeps what is a caption's own and takes the rest.
    #[test]
    fn a_restyled_caption_keeps_its_words_size_and_marks() {
        use clipcut_project::model::{RunStyle, TextRun, TextStyle};
        let held = TextStyle {
            content: "said".to_owned(),
            font_size: 0.04,
            runs: vec![TextRun {
                start: 0,
                end: 4,
                style: RunStyle {
                    color: Some("#ffe14a".to_owned()),
                    ..RunStyle::default()
                },
            }],
            ..TextStyle::default()
        };
        let look = TextStyle {
            content: "Style".to_owned(),
            font_family: "Anton".to_owned(),
            font_size: 0.2,
            stroke_width: 0.01,
            ..TextStyle::default()
        };
        let after = super::restyled(&held, &look);
        assert_eq!(after.content, "said");
        assert_eq!(after.font_size, 0.04);
        assert_eq!(after.runs.len(), 1);
        assert_eq!((after.runs[0].start, after.runs[0].end), (0, 4));
        assert_eq!(after.runs[0].style, super::spoken_look(Some(&look)));
        assert_eq!(after.font_family, "Anton");
        assert_eq!(after.stroke_width, 0.01);
        // Onto a yellow plate the yellow mark would vanish: it is worn the
        // new style's way instead.
        let plated = TextStyle {
            color: "#111111".to_owned(),
            background: "#fff04a".to_owned(),
            ..TextStyle::default()
        };
        let after = super::restyled(&held, &plated);
        assert_eq!(after.runs[0].style.color.as_deref(), Some("#ff3d6e"));
    }

    /// The panel floats over a live editor: a clip with sound selected
    /// since the panel's clip was chosen is the one it shows and runs; a
    /// pick from its list holds until the selection moves again; and a
    /// chosen clip gone from the timeline is not run unseen.
    #[test]
    fn the_captioned_clip_follows_a_new_selection_of_a_clip_with_sound() {
        let sounds = |id: &str| ["a", "b", "c"].contains(&id);
        let follow = |selected: Option<&str>, chosen_with: Option<&str>, clip: Option<&str>| {
            super::follow_selection(selected.map(str::to_owned), chosen_with, clip, sounds)
        };
        // Opened on A, A still selected: A.
        assert_eq!(
            follow(Some("a"), Some("a"), Some("a")).as_deref(),
            Some("a")
        );
        // B selected on the timeline since: B.
        assert_eq!(
            follow(Some("b"), Some("a"), Some("a")).as_deref(),
            Some("b")
        );
        // C picked from the list with B selected: C, until the selection moves.
        assert_eq!(
            follow(Some("b"), Some("b"), Some("c")).as_deref(),
            Some("c")
        );
        // A title selected - no sound - leaves the choice alone.
        assert_eq!(
            follow(Some("t"), Some("b"), Some("c")).as_deref(),
            Some("c")
        );
        // The chosen clip deleted: nothing to run, rather than a clip unseen.
        assert_eq!(follow(None, None, Some("gone")), None);
    }

    /// Row 0 is "Auto-detect", and a language's row reads back as the
    /// same language; a code the list lacks, or a row past it, is heard.
    #[test]
    fn a_language_row_reads_back_as_its_code() {
        assert_eq!(super::language_row(None), 0);
        assert_eq!(super::language_row(Some("klingon")), 0);
        assert_eq!(super::language_at(0), None);
        assert_eq!(super::language_at(-1), None);
        assert_eq!(super::language_at(100_000), None);
        let row = super::language_row(Some("id"));
        assert!(row > 0);
        assert_eq!(super::language_at(row as i32).as_deref(), Some("id"));
        assert_eq!(
            clipcut_speech::transcribe::LANGUAGES[row - 1].1,
            "Bahasa Indonesia"
        );
    }

    #[test]
    fn imported_srt_ignores_empty_captions() {
        let cues = vec![clipcut_project::srt::Cue {
            start: 0.0,
            end: 1.0,
            text: "  ".into(),
        }];
        assert!(super::srt_commands(cues, (0.35, 0.04), None).is_empty());
    }

    /// A clip in the voice list is its file and where it starts, in
    /// minutes and seconds, with hours once there are any.
    #[test]
    fn a_voice_is_named_by_its_file_and_where_it_starts() {
        assert_eq!(source_label("Interview.mp4", 0.0), "Interview.mp4 · 0:00");
        assert_eq!(source_label("Interview.mp4", 72.9), "Interview.mp4 · 1:12");
        assert_eq!(source_label("Talk.wav", 3_725.0), "Talk.wav · 1:02:05");
        assert_eq!(source_label("Talk.wav", -1.0), "Talk.wav · 0:00");
    }

    /// A script becomes one caption per sentence, a hand line break is
    /// kept, a long sentence wraps at its words, and each line is held for
    /// its reading time within one to seven seconds.
    #[test]
    fn a_script_is_cut_into_readable_lines() {
        let lines = script_captions(
            "Hello there. This is version 3.5, mind!\n\nA sentence that runs on for far \
             longer than a caption line has any business running on for. Ok?",
        );
        let text: Vec<&str> = lines.iter().map(|(line, _)| line.as_str()).collect();
        assert_eq!(
            text,
            [
                "Hello there.",
                "This is version 3.5, mind!",
                "A sentence that runs on for far longer",
                "than a caption line has any business",
                "running on for.",
                "Ok?",
            ]
        );
        assert!(
            lines
                .iter()
                .all(|(_, seconds)| (1.0..=7.0).contains(seconds))
        );
        assert_eq!(lines[0].1, 1.0);
        assert!(lines[2].1 > lines[0].1);
        assert!(script_captions("  \n ").is_empty());
        assert_eq!(script_captions("你好。再见！").len(), 2);
    }
}
