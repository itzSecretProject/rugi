// Post passes for glass: frame downsampling (blur mip chain) and the final
// copy of the offscreen scene to the target.

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// One triangle covering the viewport.
@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> VOut {
    var out: VOut;
    let xy = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    out.pos = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(xy.x, 1.0 - xy.y);
    return out;
}

// 13-tap downsample (Jimenez, "Next Generation Post Processing in Call of
// Duty"): a wide, alias-free 2x reduction. Chained over the mip levels it
// yields progressively stronger Gaussian-like blurs.
@fragment
fn fs_down(in: VOut) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(src, 0));
    let uv = in.uv;
    let a = textureSampleLevel(src, samp, uv + t * vec2<f32>(-2.0, -2.0), 0.0);
    let b = textureSampleLevel(src, samp, uv + t * vec2<f32>(0.0, -2.0), 0.0);
    let c = textureSampleLevel(src, samp, uv + t * vec2<f32>(2.0, -2.0), 0.0);
    let d = textureSampleLevel(src, samp, uv + t * vec2<f32>(-2.0, 0.0), 0.0);
    let e = textureSampleLevel(src, samp, uv, 0.0);
    let f = textureSampleLevel(src, samp, uv + t * vec2<f32>(2.0, 0.0), 0.0);
    let g = textureSampleLevel(src, samp, uv + t * vec2<f32>(-2.0, 2.0), 0.0);
    let h = textureSampleLevel(src, samp, uv + t * vec2<f32>(0.0, 2.0), 0.0);
    let i = textureSampleLevel(src, samp, uv + t * vec2<f32>(2.0, 2.0), 0.0);
    let j = textureSampleLevel(src, samp, uv + t * vec2<f32>(-1.0, -1.0), 0.0);
    let k = textureSampleLevel(src, samp, uv + t * vec2<f32>(1.0, -1.0), 0.0);
    let l = textureSampleLevel(src, samp, uv + t * vec2<f32>(-1.0, 1.0), 0.0);
    let m = textureSampleLevel(src, samp, uv + t * vec2<f32>(1.0, 1.0), 0.0);
    var o = e * 0.125;
    o += (a + c + g + i) * 0.03125;
    o += (b + d + f + h) * 0.0625;
    o += (j + k + l + m) * 0.125;
    return o;
}

// Exact copy (same size) of the scene to the presentation target.
@fragment
fn fs_copy(in: VOut) -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(in.pos.xy), 0);
}

// Copy for compositors that expect straight (non-premultiplied) alpha.
@fragment
fn fs_copy_straight(in: VOut) -> @location(0) vec4<f32> {
    let c = textureLoad(src, vec2<i32>(in.pos.xy), 0);
    if (c.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(c.rgb / c.a, c.a);
}
