// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Squeeze: the outgoing picture narrows to the middle, the incoming one
// lies under it, darkened a little where it is still covered.

struct Params { shade: f32 }

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let p = clamp(progress, 0.0, 1.0);
    let t = p * p * (3.0 - 2.0 * p);
    let wide = 1.0 - t;
    if (wide > 0.0001) {
        let dx = (uv.x - 0.5) / wide;
        if (abs(dx) <= 0.5) {
            return from_at(vec2<f32>(dx + 0.5, uv.y));
        }
    }
    let under = to_at(uv);
    let dark = clamp(params.shade, 0.0, 100.0) / 100.0 * 0.5 * (1.0 - t);
    return vec4<f32>(under.rgb * (1.0 - dark), under.a);
}
