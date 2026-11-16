// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Roll Up: the two shots stacked, the outgoing above, carried up the frame
// on an eased move, smeared along it by how fast they are going.

struct Params { blur: f32 }

fn stacked(x: f32, y: f32) -> vec4<f32> {
    if (y < 1.0) {
        return from_at(vec2<f32>(x, clamp(y, 0.0, 1.0)));
    }
    return to_at(vec2<f32>(x, clamp(y - 1.0, 0.0, 1.0)));
}

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let p = clamp(progress, 0.0, 1.0);
    let t = p * p * (3.0 - 2.0 * p);
    let speed = 6.0 * p * (1.0 - p);
    let smear = clamp(params.blur, 0.0, 100.0) / 100.0 * 0.05 * speed;
    var acc = vec4<f32>(0.0);
    for (var k = 0; k < 8; k++) {
        let s = (f32(k) / 7.0 - 0.5) * smear;
        acc += stacked(uv.x, uv.y + t + s);
    }
    return acc / 8.0;
}
