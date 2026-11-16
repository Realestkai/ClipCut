// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and ClipCut contributors

//! Rendering the session to a file.
//!
//! The rendering is `clipcut-export`'s; what is here is the seam the window
//! uses: the session flattens itself, the window adds the destination and
//! the quality, and the one export that can run at a time reports through a
//! callback and stops through a flag.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use clipcut_export::{ExportClip, ExportRequest, Reporter, render_on};
pub use clipcut_media::{ColorRange, RateMode, VideoCodec};
pub use clipcut_render::WgpuCompositor;

use crate::jobs::{Job, SingleFlight};
use crate::session::Session;

/// What the window decides about an export: where it goes, how good it is,
/// and any rasterised titles rejoining as image clips.
#[derive(Clone, Debug, Default)]
pub struct ExportSpec {
    /// The file to write.
    pub output: String,
    /// Constant rate factor; lower is better quality and a bigger file.
    pub crf: u8,
    /// The x264 speed/size preset name, e.g. "medium".
    pub preset: String,
    /// What to encode to.
    pub codec: VideoCodec,
    /// Ten bits a channel rather than eight.
    pub ten_bit: bool,
    /// VBR (the CRF carries the quality) or CBR (the bitrate is the
    /// target). VBR by default, so the sheet's Advanced section being off
    /// means the file is what it always was.
    pub rate_mode: clipcut_media::RateMode,
    /// Target bitrate in kbps, used when `rate_mode` is CBR.
    pub bitrate_kbps: u32,
    /// The levels the file is written in and tagged with. Video range,
    /// 16-235, unless the sheet's Advanced section says full.
    /// #103
    pub color_range: ColorRange,
    /// An HDR timeline written as HDR, its own HLG or PQ, rather than
    /// tone-mapped to SDR. Nothing for an SDR timeline.
    pub hdr: bool,
}

/// One progress report: which frame of how many, in which stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Frames done.
    pub frame: i64,
    /// Frames in total.
    pub total: i64,
    /// What the exporter is doing: "rendering", "mixing audio", "muxing";
    /// "rendering in software" ([`clipcut_export::RENDERING_IN_SOFTWARE`])
    /// when the hardware encoder stopped part way and the video is being
    /// rendered again from its first frame on the CPU, the frame count
    /// back at nothing.
    pub stage: &'static str,
}

/// The session flattened into a request the engine can render.
pub fn request(session: &Session, spec: &ExportSpec, titles: Vec<ExportClip>) -> ExportRequest {
    let settings = session.settings();
    let mut clips = session.flattened_clips();
    clips.extend(titles);
    ExportRequest {
        output: spec.output.clone(),
        width: settings.width,
        height: settings.height,
        rate_num: settings.rate_num,
        rate_den: settings.rate_den,
        crf: spec.crf,
        preset: spec.preset.clone(),
        codec: spec.codec,
        ten_bit: spec.ten_bit,
        rate_mode: spec.rate_mode,
        bitrate_kbps: spec.bitrate_kbps,
        color_range: spec.color_range,
        color_space: session.video().color_space,
        hdr: spec.hdr,
        effect_time_offset: 0.0,
        canvas: session.video().canvas,
        clips,
    }
}

/// ClipCut's mark, the logo and the name, white with a soft shadow.
const WATERMARK: &[u8] = include_bytes!("../../../../assets/brand/clipcut-watermark.png");
/// Its size in pixels, for placing it.
const WATERMARK_SIZE: (f64, f64) = (620.0, 320.0);

/// Puts ClipCut's mark on `request`: every video exported from the window
/// or through the API carries it (see [`watermark`]).
pub fn add_watermark(request: &mut ExportRequest) {
    if let Some(mark) = watermark(&request.clips, request.width, request.height) {
        request.clips.push(mark);
    }
}

/// Every export carries ClipCut's mark in its top right corner, small and
/// a little see-through, the whole way along: a still on a lane of its own
/// over everything, so it goes through the same compositor as the edit
/// does on every path. `None` for an edit with no picture, and when the
/// mark could not be put where the renderer reads it from.
pub fn watermark(clips: &[ExportClip], width: u32, height: u32) -> Option<ExportClip> {
    let end = clips
        .iter()
        .map(|clip| clip.start + clip.duration)
        .fold(0.0_f64, f64::max);
    if end <= 0.0 || width == 0 || height == 0 {
        return None;
    }
    let path = std::env::temp_dir().join("clipcut-watermark-1.png");
    let current = std::fs::metadata(&path).is_ok_and(|meta| meta.len() == WATERMARK.len() as u64);
    if !current {
        std::fs::write(&path, WATERMARK).ok()?;
    }
    let (frame_w, frame_h) = (f64::from(width), f64::from(height));
    let (mark_w, mark_h) = WATERMARK_SIZE;
    // The same size on a tall frame as on a wide one: a share of the
    // shorter side, and a margin from the corner in proportion.
    let short = frame_w.min(frame_h);
    let wanted_w = 0.2 * short;
    let wanted_h = wanted_w * mark_h / mark_w;
    let margin = 0.025 * short;
    // Scale 1 fits the picture inside the frame.
    let fitted_w = mark_w * (frame_w / mark_w).min(frame_h / mark_h);
    let centre_x = frame_w - margin - wanted_w / 2.0;
    let centre_y = margin + wanted_h / 2.0;
    let track = clips
        .iter()
        .map(|clip| clip.track)
        .max()
        .map_or(0, |top| top + 1);
    Some(ExportClip {
        path: path.to_string_lossy().into_owned(),
        muted: true,
        volume: 0.0,
        has_audio: Some(false),
        media_width: Some(mark_w as u32),
        media_height: Some(mark_h as u32),
        scale: wanted_w / fitted_w,
        offset_x: (centre_x - frame_w / 2.0) / frame_w,
        offset_y: (centre_y - frame_h / 2.0) / frame_h,
        opacity: 0.8,
        ..ExportClip::blank(clipcut_export::ClipKind::Image, 0.0, end, track)
    })
}

/// Renders `request` and returns the path written, reporting through
/// `progress` and stopping at the next frame once `cancel` is set. Blocks for
/// the whole render: run it on its own thread. Draws on a device of its
/// own; [`run_on`] draws on one the caller lends.
pub fn run(
    request: &ExportRequest,
    cancel: &AtomicBool,
    progress: impl FnMut(Progress),
) -> Result<String, String> {
    run_on(request, None, cancel, progress)
}

/// [`run`] drawing on `compositor` where the caller has one to lend: the
/// window's, a sibling of its monitor's (see `Monitor::sibling`), so the
/// export draws on the device that has drawn the monitor all along rather
/// than opening a second one through the drivers, which on Windows laptops
/// with NVIDIA chips took the app down at the first frame (issue #202).
/// `None` draws as [`run`] does.
pub fn run_on(
    request: &ExportRequest,
    compositor: Option<WgpuCompositor>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> Result<String, String> {
    // What a death part way through needs to be read: what was asked for,
    // which stage it was in, and how far that stage had got. The stages
    // and every quarter of one are a handful of lines an export.
    let fps = request.rate_num as f64 / request.rate_den.max(1) as f64;
    log::info!(
        "export: {}x{} {fps:.3} fps · {}{} · {} kbps · {} clips",
        request.width,
        request.height,
        request.codec.name(),
        if request.ten_bit { " 10-bit" } else { "" },
        request.bitrate_kbps,
        request.clips.len(),
    );
    let mut stage_now = "";
    let mut quarter = 0;
    let mut reached = (0, 0);
    let mut report = |frame: i64, total: i64, stage: &'static str| {
        if stage != stage_now {
            log::info!("export: {stage} · {total} frames");
            stage_now = stage;
            quarter = 0;
        }
        let now = if total > 0 { frame * 4 / total } else { 0 };
        if (1..4).contains(&now) && now > quarter {
            log::info!("export: {stage} · frame {frame} of {total}");
            quarter = now;
        }
        reached = (frame, total);
        progress(Progress {
            frame,
            total,
            stage,
        });
    };
    let result = render_on(
        request,
        compositor,
        Reporter {
            progress: &mut report,
            cancel,
        },
    );
    // How it ended, and where its time went, is the engine's own line
    // (`render_on`); this adds only how far a stopped one had got.
    if result.is_err() {
        let (frame, total) = reached;
        log::info!("export: stopped in {stage_now} at frame {frame} of {total}");
    }
    result
}

/// The slot for the one export that can run at a time - enforced, not
/// documented: a second export while one runs is refused instead of racing
/// the first for its temp files and cancel flag.
#[derive(Clone, Default)]
pub struct Exporter {
    slot: Arc<SingleFlight>,
}

impl Exporter {
    /// An idle exporter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Claims the slot for a new export, whose cancel flag the render polls.
    pub fn begin(&self) -> Result<Job, String> {
        self.slot.begin("export")
    }

    /// Asks the running export to stop at the next frame. Idle is a no-op.
    pub fn cancel(&self) {
        self.slot.cancel();
    }

    /// Whether an export is running.
    pub fn is_busy(&self) -> bool {
        self.slot.is_busy()
    }
}
