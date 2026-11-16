// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// A point folded back into the picture, as a mirror at each edge would.
fn fold(uv: vec2<f32>) -> vec2<f32> {
    let m = abs(uv - 2.0 * floor(uv * 0.5 + vec2<f32>(0.5)));
    return clamp(m, vec2<f32>(0.0), vec2<f32>(1.0));
}

// Kaleidoscope: every point is turned into one wedge around the centre and
// mirrored inside it, so the wedges repeat round the circle.

struct Params { segments: f32, spin: f32 }

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let aspect = frame.size.x / frame.size.y;
    var d = uv - vec2<f32>(0.5);
    d.x = d.x * aspect;
    let r = length(d);
    let n = max(round(params.segments), 2.0);
    let wedge = 6.28318530 / n;
    var a = atan2(d.y, d.x) + frame.clip_time * clamp(params.spin, 0.0, 100.0) / 100.0;
    a = a - wedge * floor(a / wedge);
    a = abs(a - wedge * 0.5);
    var p = vec2<f32>(cos(a), sin(a)) * r;
    p.x = p.x / aspect;
    return sample(fold(p + vec2<f32>(0.5)));
}
