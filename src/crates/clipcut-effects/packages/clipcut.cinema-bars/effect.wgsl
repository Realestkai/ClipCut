// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 ClipCut contributors

// Cinema Bars: the frame cut to a shape, top and bottom when the shape is
// wider than the frame, left and right when it is narrower.

struct Params { ratio: f32, opacity: f32 }

fn shape() -> f32 {
    let pick = i32(round(clamp(params.ratio, 0.0, 3.0)));
    if (pick == 1) {
        return 2.0;
    }
    if (pick == 2) {
        return 1.85;
    }
    if (pick == 3) {
        return 4.0 / 3.0;
    }
    return 2.39;
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let aspect = frame.size.x / frame.size.y;
    let want = shape();
    var covered = false;
    if (aspect < want) {
        let bar = 0.5 * (1.0 - aspect / want);
        covered = uv.y < bar || uv.y > 1.0 - bar;
    } else if (aspect > want) {
        let bar = 0.5 * (1.0 - want / aspect);
        covered = uv.x < bar || uv.x > 1.0 - bar;
    }
    if (!covered) {
        return c;
    }
    let k = clamp(params.opacity, 0.0, 100.0) / 100.0;
    return vec4<f32>(c.rgb * (1.0 - k), mix(c.a, 1.0, k));
}
