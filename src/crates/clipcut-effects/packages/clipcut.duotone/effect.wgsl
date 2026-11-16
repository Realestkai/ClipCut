// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Duotone: the picture's brightness mapped from a shadow colour to a
// highlight colour, mixed over the picture by Amount.

struct Params { preset: f32, amount: f32 }

struct Pair { low: vec3<f32>, high: vec3<f32> }

fn pair() -> Pair {
    let pick = i32(round(clamp(params.preset, 0.0, 3.0)));
    if (pick == 1) {
        return Pair(vec3<f32>(0.0, 0.04, 0.12), vec3<f32>(0.4, 0.9, 1.0));
    }
    if (pick == 2) {
        return Pair(vec3<f32>(0.1, 0.0, 0.06), vec3<f32>(1.0, 0.55, 0.7));
    }
    if (pick == 3) {
        return Pair(vec3<f32>(0.0, 0.05, 0.12), vec3<f32>(0.55, 1.0, 0.75));
    }
    return Pair(vec3<f32>(0.08, 0.02, 0.25), vec3<f32>(1.0, 0.6, 0.2));
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let p = pair();
    let tone = mix(p.low, p.high, clamp(luma(c.rgb), 0.0, 1.0));
    return vec4<f32>(mix(c.rgb, tone, clamp(params.amount, 0.0, 100.0) / 100.0), c.a);
}
