// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Zoom Pulse: on every beat the frame jumps in a little and decays back
// out before the next, timed from the clip's own start.

struct Params { amount: f32, bpm: f32 }

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let beat = fract(frame.clip_time * max(params.bpm, 1.0) / 60.0);
    let kick = exp(-beat * 6.0);
    let zoom = 1.0 + clamp(params.amount, 0.0, 100.0) / 100.0 * 0.12 * kick;
    let at = (uv - vec2<f32>(0.5)) / zoom + vec2<f32>(0.5);
    return sample(clamp(at, vec2<f32>(0.0), vec2<f32>(1.0)));
}
