// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Pixel Dissolve: the frame goes blocky towards the middle of the cut and
// each block changes over at a moment of its own.

struct Params { size: f32 }

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let p = clamp(progress, 0.0, 1.0);
    let bell = sin(3.14159265 * p);
    let cells = mix(60.0, 10.0, bell * clamp(params.size, 0.0, 100.0) / 100.0);
    let aspect = frame.size.x / frame.size.y;
    let grid = vec2<f32>(cells * aspect, cells);
    let cell = floor(uv * grid);
    let centre = (cell + vec2<f32>(0.5)) / grid;
    let at = mix(uv, centre, bell);
    let turn = hash(cell, 11.0) * 0.98 + 0.01;
    let w = step(turn, p);
    return mix(from_at(at), to_at(at), w);
}
