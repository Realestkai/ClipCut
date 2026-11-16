// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Diamond Wipe: the incoming shot inside a diamond that grows from the
// centre until it covers the corners.

struct Params { softness: f32 }

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let p = clamp(progress, 0.0, 1.0);
    let aspect = frame.size.x / frame.size.y;
    let d = abs(uv.x - 0.5) * aspect + abs(uv.y - 0.5);
    let soft = 0.005 + clamp(params.softness, 0.0, 100.0) / 100.0 * 0.06;
    let far = 0.5 * aspect + 0.5 + soft;
    let r = p * far;
    let w = 1.0 - smoothstep(r - soft, r, d);
    return mix(from_at(uv), to_at(uv), w);
}
