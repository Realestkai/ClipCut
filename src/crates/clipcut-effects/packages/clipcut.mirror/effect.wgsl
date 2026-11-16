// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Mirror: the half chosen is shown as it is and reflected over the middle.

struct Params { mode: f32 }

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let pick = i32(round(clamp(params.mode, 0.0, 3.0)));
    var at = uv;
    if (pick == 0 || pick == 3) {
        at.x = select(1.0 - uv.x, uv.x, uv.x < 0.5);
    }
    if (pick == 1) {
        at.x = select(uv.x, 1.0 - uv.x, uv.x < 0.5);
    }
    if (pick == 2 || pick == 3) {
        at.y = select(1.0 - uv.y, uv.y, uv.y < 0.5);
    }
    return sample(at);
}
