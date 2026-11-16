// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Ink Spread: the incoming shot shows where a ragged field - distance from
// the centre roughened by noise - falls below the cut's progress.

struct Params { scale: f32 }

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    let a = hash(i, 3.0);
    let b = hash(i + vec2<f32>(1.0, 0.0), 3.0);
    let c = hash(i + vec2<f32>(0.0, 1.0), 3.0);
    let d = hash(i + vec2<f32>(1.0, 1.0), 3.0);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let p = clamp(progress, 0.0, 1.0);
    let aspect = frame.size.x / frame.size.y;
    let q = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5);
    let detail = mix(2.0, 9.0, clamp(params.scale, 0.0, 100.0) / 100.0);
    let rough = 0.6 * vnoise(q * detail) + 0.4 * vnoise(q * detail * 2.3 + vec2<f32>(7.1, 2.3));
    let field = clamp(0.55 * length(q) / max(0.5 * aspect, 0.5) + 0.45 * rough, 0.0, 1.0);
    let soft = 0.06;
    let reveal = p * (1.0 + 2.0 * soft) - soft;
    let w = smoothstep(field - soft, field + soft, reveal);
    return mix(from_at(uv), to_at(uv), w);
}
