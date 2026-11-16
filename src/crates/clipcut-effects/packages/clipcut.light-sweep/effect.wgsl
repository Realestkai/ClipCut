// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Light Sweep: a soft diagonal band added over the picture, crossing it
// once a loop. Each loop starts with the band off the picture.

struct Params { strength: f32, speed: f32, width: f32 }

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let loops = mix(0.15, 1.2, clamp(params.speed, 0.0, 100.0) / 100.0);
    let at = fract(frame.clip_time * loops) * 2.8 - 0.4;
    let w = mix(0.04, 0.2, clamp(params.width, 0.0, 100.0) / 100.0);
    let d = (uv.x + uv.y) - at;
    let band = exp(-(d * d) / (w * w)) * clamp(params.strength, 0.0, 100.0) / 100.0;
    return vec4<f32>(c.rgb + vec3<f32>(band), c.a);
}
