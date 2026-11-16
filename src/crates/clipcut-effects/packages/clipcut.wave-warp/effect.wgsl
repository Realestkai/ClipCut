// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Wave Warp: each row is pushed sideways by a travelling sine.

struct Params { amount: f32, waves: f32, speed: f32 }

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let phase = uv.y * max(params.waves, 1.0) * 6.28318530
        + frame.clip_time * clamp(params.speed, 0.0, 100.0) / 100.0 * 8.0;
    let push = sin(phase) * clamp(params.amount, 0.0, 100.0) / 100.0 * 0.02;
    return sample(clamp(uv + vec2<f32>(push, 0.0), vec2<f32>(0.0), vec2<f32>(1.0)));
}
