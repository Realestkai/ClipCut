// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Flash Zoom: the outgoing shot zooms in, the incoming one zooms out from
// close up, and a flash of white peaks on the cut between them.

struct Params { flash: f32 }

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let p = clamp(progress, 0.0, 1.0);
    let k = (p - 0.5) / 0.12;
    let f = clamp(params.flash, 0.0, 100.0) / 100.0 * exp(-k * k);
    var c: vec4<f32>;
    if (p < 0.5) {
        let zoom = 1.0 + 1.2 * p * p;
        c = from_at(clamp((uv - vec2<f32>(0.5)) / zoom + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0)));
    } else {
        let q = 1.0 - p;
        let zoom = 1.0 + 1.2 * q * q;
        c = to_at(clamp((uv - vec2<f32>(0.5)) / zoom + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0)));
    }
    return vec4<f32>(mix(c.rgb, vec3<f32>(1.0), f), c.a);
}
