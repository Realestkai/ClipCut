// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Shake Cut: a jolt and a punch-in that peak on the cut, which happens in
// the middle of them.

struct Params { amount: f32 }

fn wob(t: f32, seed: f32) -> f32 {
    return sin(t * 91.7 + seed) * 0.6 + sin(t * 37.3 + seed * 2.1) * 0.4;
}

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let p = clamp(progress, 0.0, 1.0);
    let k = (p - 0.5) / 0.18;
    let hit = exp(-k * k);
    let amount = clamp(params.amount, 0.0, 100.0) / 100.0;
    let shift = vec2<f32>(wob(p, 1.3), wob(p, 4.7)) * 0.04 * amount * hit;
    let zoom = 1.0 + 0.08 * amount * hit;
    let at = clamp((uv - vec2<f32>(0.5) + shift) / zoom + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    if (p < 0.5) {
        return from_at(at);
    }
    return to_at(at);
}
