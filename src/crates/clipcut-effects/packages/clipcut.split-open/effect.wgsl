// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Split Open: the outgoing picture cut in two at the middle, each half
// carried off to its own side, the incoming one in the gap.

struct Params { direction: f32 }

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let p = clamp(progress, 0.0, 1.0);
    let gap = 0.5 * p * p * (3.0 - 2.0 * p);
    if (params.direction > 0.5) {
        if (uv.y < 0.5 - gap) {
            return from_at(vec2<f32>(uv.x, uv.y + gap));
        }
        if (uv.y > 0.5 + gap) {
            return from_at(vec2<f32>(uv.x, uv.y - gap));
        }
        return to_at(uv);
    }
    if (uv.x < 0.5 - gap) {
        return from_at(vec2<f32>(uv.x + gap, uv.y));
    }
    if (uv.x > 0.5 + gap) {
        return from_at(vec2<f32>(uv.x - gap, uv.y));
    }
    return to_at(uv);
}
