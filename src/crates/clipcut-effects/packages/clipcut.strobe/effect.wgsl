// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Strobe: a quarter of every beat is a flash. The beat starts half way, so
// a clip opens on its own picture rather than on a flash.

struct Params { rate: f32, strength: f32, tone: f32 }

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let on = fract(frame.clip_time * max(params.rate, 0.1) + 0.5) < 0.25;
    if (!on) {
        return c;
    }
    let flash_rgb = select(vec3<f32>(1.0), vec3<f32>(0.0), params.tone > 0.5);
    let s = clamp(params.strength, 0.0, 100.0) / 100.0;
    return vec4<f32>(mix(c.rgb, flash_rgb, s), c.a);
}
