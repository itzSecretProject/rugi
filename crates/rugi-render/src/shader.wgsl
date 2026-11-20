// Rugi UI uber-shader: one instanced quad per primitive, analytic SDF coverage.
// Kinds: 0 rect, 1 shadow, 2 glyph, 3 image, 4 line, 5 glass, 6 backdrop, 255 none.

struct Globals {
    screen: vec2<f32>,   // logical size
    scale: f32,          // physical px per logical px
    output_srgb: f32,    // 1.0 when the target applies sRGB encoding
    blur_ready: f32,     // 1.0 when blur_tex holds a snapshot of the frame
    blur_mips: f32,      // mip levels in blur_tex
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var blur_tex: texture_2d<f32>;
@group(0) @binding(4) var blur_samp: sampler;
@group(0) @binding(5) var noise_tex: texture_2d<f32>;
@group(0) @binding(6) var noise_samp: sampler;
@group(1) @binding(0) var image_tex: texture_2d<f32>;

struct VIn {
    @builtin(vertex_index) vi: u32,
    @location(0) rect: vec4<f32>,
    @location(1) radii: vec4<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) params: vec4<f32>,
    @location(4) c0: vec4<f32>,
    @location(5) c1: vec4<f32>,
    @location(6) cb: vec4<f32>,
    @location(7) kind: u32,
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) p: vec2<f32>,
    @location(1) tex_uv: vec2<f32>,
    @location(2) @interpolate(flat) rect: vec4<f32>,
    @location(3) @interpolate(flat) radii: vec4<f32>,
    @location(4) @interpolate(flat) uv: vec4<f32>,
    @location(5) @interpolate(flat) params: vec4<f32>,
    @location(6) @interpolate(flat) c0: vec4<f32>,
    @location(7) @interpolate(flat) c1: vec4<f32>,
    @location(8) @interpolate(flat) cb: vec4<f32>,
    @location(9) @interpolate(flat) kind: u32,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    return vertex(v, 1.0 / g.scale);
}

// Backdrop layer fill: quads grow by a few pixels so bilinear sampling of the
// half-resolution layer never reaches outside a backdrop's own pixels.
@vertex
fn vs_backdrop_fill(v: VIn) -> VOut {
    return vertex(v, 4.0);
}

fn vertex(v: VIn, aa: f32) -> VOut {
    var out: VOut;
    let corner = vec2<f32>(f32(v.vi & 1u), f32(v.vi >> 1u));
    var bmin = v.rect.xy;
    var bmax = v.rect.xy + v.rect.zw;
    switch v.kind {
        case 0u, 3u, 5u, 6u: { bmin -= vec2<f32>(aa); bmax += vec2<f32>(aa); }
        case 1u: { let e = v.params.y * 3.0; bmin -= vec2<f32>(e); bmax += vec2<f32>(e); }
        case 4u: {
            let e = v.params.x * 0.5 + aa;
            bmin = min(v.uv.xy, v.uv.zw) - vec2<f32>(e);
            bmax = max(v.uv.xy, v.uv.zw) + vec2<f32>(e);
        }
        case 2u: {}
        default: { bmin = vec2<f32>(0.0); bmax = vec2<f32>(0.0); }
    }
    let p = mix(bmin, bmax, corner);
    let ndc = p / g.screen * 2.0 - 1.0;
    out.pos = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.p = p;
    let rel = (p - v.rect.xy) / max(v.rect.zw, vec2<f32>(1e-6));
    out.tex_uv = mix(v.uv.xy, v.uv.zw, rel);
    out.rect = v.rect;
    out.radii = v.radii;
    out.uv = v.uv;
    out.params = v.params;
    out.c0 = v.c0;
    out.c1 = v.c1;
    out.cb = v.cb;
    out.kind = v.kind;
    return out;
}

// Signed distance to a rounded box; radii = (tl, tr, br, bl).
fn sd_round_rect(p: vec2<f32>, center: vec2<f32>, half: vec2<f32>, radii: vec4<f32>) -> f32 {
    let q = p - center;
    var r: f32;
    if (q.x < 0.0) {
        r = select(radii.w, radii.x, q.y < 0.0);
    } else {
        r = select(radii.z, radii.y, q.y < 0.0);
    }
    let d = abs(q) - half + vec2<f32>(r);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

// Rounded-rectangle shadow (after Evan Wallace, "Fast rounded rectangle shadows").
fn gaussian(x: f32, sigma: f32) -> f32 {
    let pi = 3.141592653589793;
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * pi) * sigma);
}

fn erf2(x: vec2<f32>) -> vec2<f32> {
    let s = sign(x);
    let a = abs(x);
    var r = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    r = r * r;
    return s - s / (r * r);
}

fn shadow_x(x: f32, y: f32, sigma: f32, corner: f32, half: vec2<f32>) -> f32 {
    let delta = min(half.y - corner - sqrt(max(0.0, corner * corner - y * y)), 0.0);
    let curved = half.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    let integral = 0.5 + 0.5 * erf2((vec2<f32>(x) + vec2<f32>(-curved, curved)) * (sqrt(0.5) / sigma));
    return integral.y - integral.x;
}

fn rounded_box_shadow(half: vec2<f32>, point: vec2<f32>, sigma: f32, corner: f32) -> f32 {
    let low = point.y - half.y;
    let high = point.y + half.y;
    let start = clamp(-3.0 * sigma, low, high);
    let end = clamp(3.0 * sigma, low, high);
    let step = (end - start) / 4.0;
    var y = start + step * 0.5;
    var value = 0.0;
    for (var i = 0; i < 4; i++) {
        value += shadow_x(point.x, point.y - y, sigma, corner, half) * gaussian(y, sigma) * step;
        y += step;
    }
    return value;
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// ---------------------------------------------------------------- noise ---

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// Value noise from a 256x256 random texture: one bilinear tap at a
// smoothstep-warped coordinate gives the same C1-smooth result as four
// hashed lattice values, at a fraction of the ALU cost.
fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return textureSampleLevel(noise_tex, noise_samp, (i + u + vec2<f32>(0.5)) / 256.0, 0.0).r;
}

fn fbm(p0: vec2<f32>) -> f32 {
    var p = p0;
    var v = 0.0;
    var a = 0.5;
    let rot = mat2x2<f32>(0.8, 0.6, -0.6, 0.8);
    for (var i = 0; i < 5; i++) {
        v += a * vnoise(p);
        p = rot * p * 2.03 + vec2<f32>(17.0, 9.2);
        a *= 0.5;
    }
    return v;
}

// --------------------------------------------------------------- glass ---

// Samples the blurred snapshot with a 4-tap tent filter at `lod` (smooths
// bilinear blockiness of the small mips).
fn blur_at(uv: vec2<f32>, lod: f32) -> vec3<f32> {
    let dims = vec2<f32>(textureDimensions(blur_tex, 0));
    let texel = exp2(lod) / dims;
    var c = textureSampleLevel(blur_tex, blur_samp, uv + texel * vec2<f32>(-0.5, -0.5), lod).rgb;
    c += textureSampleLevel(blur_tex, blur_samp, uv + texel * vec2<f32>(0.5, -0.5), lod).rgb;
    c += textureSampleLevel(blur_tex, blur_samp, uv + texel * vec2<f32>(-0.5, 0.5), lod).rgb;
    c += textureSampleLevel(blur_tex, blur_samp, uv + texel * vec2<f32>(0.5, 0.5), lod).rgb;
    var rgb = c * 0.25;
    if (g.output_srgb > 0.5) {
        rgb = to_srgb(rgb);
    }
    return rgb;
}

fn glass(in: VOut, center: vec2<f32>, half: vec2<f32>) -> vec4<f32> {
    let d = sd_round_rect(in.p, center, half, in.radii);
    let cov = clamp(0.5 - d * g.scale, 0.0, 1.0);
    if (cov <= 0.0) {
        return vec4<f32>(0.0);
    }
    let blur = in.uv.x;
    let refr = in.uv.y;
    let bevel = in.uv.z;
    let grain = in.uv.w;
    let bw = in.params.x;
    let sat = in.params.y;
    let highlight = in.params.z;
    let disp = in.params.w;
    let opacity = in.c1.a;

    // Outward normal from the SDF gradient.
    let e = 0.75;
    let nx = sd_round_rect(in.p + vec2<f32>(e, 0.0), center, half, in.radii)
        - sd_round_rect(in.p - vec2<f32>(e, 0.0), center, half, in.radii);
    let ny = sd_round_rect(in.p + vec2<f32>(0.0, e), center, half, in.radii)
        - sd_round_rect(in.p - vec2<f32>(0.0, e), center, half, in.radii);
    let n = normalize(vec2<f32>(nx, ny) + vec2<f32>(1e-5, 1e-5));
    let depth = -d;

    // Refraction: a convex rim bends the backdrop inward.
    var offset = vec2<f32>(0.0);
    if (bevel > 0.0 && refr != 0.0) {
        let k = 1.0 - clamp(depth / bevel, 0.0, 1.0);
        let curve = 1.0 - sqrt(max(1.0 - k * k, 0.0));
        offset = -n * refr * curve;
    }

    var bg: vec3<f32>;
    if (g.blur_ready > 0.5) {
        let lod = clamp(0.4 + blur * 4.6, 0.0, g.blur_mips - 1.0);
        let uv = (in.p + offset) / g.screen;
        if (disp > 0.0 && refr != 0.0) {
            let du = offset * disp * 0.45 / g.screen;
            bg = vec3<f32>(blur_at(uv + du, lod).r, blur_at(uv, lod).g, blur_at(uv - du, lod).b);
        } else {
            bg = blur_at(uv, lod);
        }
    } else {
        // No snapshot (host-owned render pass): fall back to the tint.
        bg = in.c0.rgb * 0.6;
    }

    let lum = dot(bg, vec3<f32>(0.2126, 0.7152, 0.0722));
    bg = mix(vec3<f32>(lum), bg, sat);
    bg = mix(bg, in.c0.rgb, in.c0.a);

    // Specular rim light, strongest on the upper-left edge, plus a soft sheen.
    let rim_w = max(bevel * 0.45, 5.0);
    let rim = pow(1.0 - clamp(depth / rim_w, 0.0, 1.0), 3.0);
    let dir = 0.45 + 0.55 * max(dot(n, normalize(vec2<f32>(-0.55, -0.85))), 0.0);
    let rel_y = clamp((in.p.y - in.rect.y) / max(in.rect.w, 1.0), 0.0, 1.0);
    bg += in.c1.rgb * highlight * (rim * dir * 0.55 + (1.0 - rel_y) * 0.045);

    if (grain > 0.0) {
        bg += (hash21(floor(in.p * g.scale)) - 0.5) * grain * 0.05;
    }
    if (bw > 0.0) {
        let inner = clamp(0.5 + (d + bw) * g.scale, 0.0, 1.0);
        bg = mix(bg, in.cb.rgb, in.cb.a * inner);
    }
    let a = cov * opacity;
    return vec4<f32>(clamp(bg, vec3<f32>(0.0), vec3<f32>(1.0)) * a, a);
}

// ------------------------------------------------------------ backdrops ---

fn backdrop(in: VOut, center: vec2<f32>, half: vec2<f32>) -> vec4<f32> {
    let d = sd_round_rect(in.p, center, half, in.radii);
    let cov = clamp(0.5 - d * g.scale, 0.0, 1.0);
    if (cov <= 0.0) {
        return vec4<f32>(0.0);
    }
    var col = backdrop_color(in);
    col += backdrop_grain(in);
    let a = cov * in.c0.a;
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)) * a, a);
}

// Per-pixel film grain (mesh style), added at full resolution.
fn backdrop_grain(in: VOut) -> vec3<f32> {
    if (u32(in.uv.w + 0.5) == 5u) {
        return vec3<f32>((hash21(floor(in.p * g.scale)) - 0.5) * 0.045);
    }
    return vec3<f32>(0.0);
}

// Unmasked backdrop color (sRGB space) at in.p.
fn backdrop_color(in: VOut) -> vec3<f32> {
    let t = in.uv.x;
    let sc = in.uv.y;
    let intensity = in.uv.z;
    let kind = u32(in.uv.w + 0.5);
    let seed = in.params.x;
    let unit = max(min(in.rect.z, in.rect.w), 1.0) / 1.6 * sc;
    let q = (in.p - in.rect.xy) / unit + vec2<f32>(seed * 7.31, seed * 3.17);
    let pq = (in.params.yz - in.rect.xy) / unit + vec2<f32>(seed * 7.31, seed * 3.17);
    let response = in.params.w;
    let rel = (in.p - in.rect.xy) / max(in.rect.zw, vec2<f32>(1.0));
    let base = in.c0.rgb;
    let ca = in.c1.rgb;
    let cbb = in.cb.rgb;
    let dp = q - pq;
    let near = exp(-dot(dp, dp) * 3.0) * response;
    var col = base;

    switch kind {
        case 0u: { // Aurora
            let w = fbm(vec2<f32>(q.x * 0.7 + t * 0.03, t * 0.04));
            for (var i = 0; i < 3; i++) {
                let fi = f32(i);
                let cy = 0.30 + 0.16 * fi + 0.10 * sin(q.x * 1.1 + t * 0.13 + fi * 2.1 + w * 2.5);
                let k = (rel.y - cy) * (7.0 - fi * 1.5);
                let band = exp(-k * k);
                let rays = 0.55 + 0.45 * fbm(vec2<f32>(q.x * 5.0 + fi * 3.0, t * 0.12 + fi));
                let c = mix(ca, cbb, clamp(fi * 0.5 + 0.25 * sin(t * 0.1 + q.x), 0.0, 1.0));
                col += c * band * rays * 0.42 * intensity;
            }
            col += mix(ca, cbb, 0.5) * near * 0.12;
        }
        case 1u: { // Smoke
            var p2 = q * 1.1;
            p2 += vec2<f32>(-dp.y, dp.x) * near * 0.6;
            let o1 = vec2<f32>(fbm(p2 + vec2<f32>(0.0, t * 0.035)), fbm(p2 + vec2<f32>(5.2, 1.3) - vec2<f32>(t * 0.03)));
            let o2 = vec2<f32>(
                fbm(p2 + 3.5 * o1 + vec2<f32>(1.7, 9.2) + vec2<f32>(t * 0.05, 0.0)),
                fbm(p2 + 3.5 * o1 + vec2<f32>(8.3, 2.8)),
            );
            let f = fbm(p2 + 3.5 * o2);
            col = mix(base, ca, clamp(f * f * 2.4, 0.0, 1.0) * intensity);
            col = mix(col, cbb, clamp(length(o2) * f * 0.55, 0.0, 1.0) * 0.75 * intensity);
        }
        case 2u: { // Plasma
            let c2 = vec2<f32>(sin(t * 0.21), cos(t * 0.17)) * 1.2;
            var v = sin(q.x * 2.2 + t * 0.6) + sin(q.y * 2.6 + t * 0.45);
            v += sin((q.x + q.y) * 1.7 + t * 0.35) + sin(length(q - c2) * 3.0 - t * 0.7);
            v += near * 2.0;
            let m = 0.5 + 0.5 * sin(v * 1.1);
            let glow = 0.5 + 0.5 * sin(v * 2.0 + 1.3);
            col = base + mix(ca, cbb, m) * (0.18 + 0.42 * glow * glow) * intensity;
        }
        case 3u: { // Starfield
            col = mix(base * 1.25, base * 0.8, rel.y);
            col += cbb * fbm(q * 1.3 + vec2<f32>(t * 0.01, 0.0)) * 0.22 * intensity;
            for (var l = 0; l < 3; l++) {
                let fl = f32(l);
                let cell = 46.0 + fl * 34.0;
                let drift = vec2<f32>(t * (2.0 + fl * 3.0), -t * (1.0 + fl * 1.5));
                let sp = (in.p - in.rect.xy + drift) / cell + vec2<f32>(seed * 13.0 + fl * 7.0);
                let id = floor(sp);
                let f = fract(sp);
                let rnd = hash22(id);
                let on = step(0.45, hash21(id + 3.7));
                let dist = length(f - (0.15 + rnd * 0.7)) * cell;
                let r = 0.55 + rnd.x * (0.7 + fl * 0.45);
                let tw = 0.55 + 0.45 * sin(t * (1.5 + rnd.y * 3.0) + rnd.x * 6.283);
                let star = smoothstep(r + 0.9, r - 0.4, dist) * tw * on;
                let halo = exp(-dist * 0.45) * 0.10 * on * tw;
                col += ca * (star + halo) * (0.5 + 0.25 * fl) * intensity * (1.0 + near * 2.0);
            }
        }
        case 4u: { // Filament
            let y = in.p.y - in.rect.y;
            let x = in.p.x - in.rect.x;
            let w = in.rect.z;
            let jitter = (fbm(vec2<f32>(y * 0.015, t * 1.1 + seed)) - 0.5) * w * 0.08;
            let cx = w * 0.5 + w * 0.17 * sin(y * 0.021 + t * 1.4 + seed) + jitter;
            let dd = abs(x - cx);
            let glow = 1.0 / (1.0 + dd * 0.09);
            let core = exp(-dd * dd * 0.08);
            let cx2 = w * 0.5 + w * 0.22 * sin(y * 0.013 - t * 0.9 + 2.0 + seed);
            let d2 = abs(x - cx2);
            let glow2 = 1.0 / (1.0 + d2 * 0.18);
            col = base + ca * (glow * 0.55 + glow2 * 0.18) * intensity + cbb * core * intensity;
        }
        default: { // Mesh gradient
            var acc = vec3<f32>(0.0);
            var wsum = 0.0;
            for (var i = 0; i < 4; i++) {
                let fi = f32(i);
                let ctr = vec2<f32>(
                    0.5 + 0.38 * sin(t * (0.11 + fi * 0.03) + fi * 1.7 + seed),
                    0.5 + 0.38 * cos(t * (0.09 + fi * 0.04) + fi * 2.3 + seed),
                );
                var c = ca;
                if (i == 1) { c = cbb; }
                if (i == 2) { c = mix(ca, cbb, 0.5) * 1.15; }
                if (i == 3) { c = base; }
                let dd = rel - ctr;
                let wgt = exp(-dot(dd, dd) * 5.0);
                acc += c * wgt;
                wsum += wgt;
            }
            col = mix(base, acc / max(wsum, 1e-4), clamp(intensity, 0.0, 1.0));
        }
    }
    // Soft vignette keeps foreground content readable.
    let v = length(rel - vec2<f32>(0.5)) * 1.45;
    col *= 1.0 - 0.38 * smoothstep(0.55, 1.25, v);
    return col;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let center = in.rect.xy + in.rect.zw * 0.5;
    let half = in.rect.zw * 0.5;
    // Premultiplied result in sRGB space.
    var col = vec4<f32>(0.0);
    switch in.kind {
        case 0u: {
            let d = sd_round_rect(in.p, center, half, in.radii);
            let cov = clamp(0.5 - d * g.scale, 0.0, 1.0);
            // Gradient: project onto the rect-relative vector uv.xy -> uv.zw.
            let dir = in.uv.zw - in.uv.xy;
            let rel = (in.p - in.rect.xy) / max(in.rect.zw, vec2<f32>(1e-6));
            let len2 = dot(dir, dir);
            var t = 0.0;
            if (len2 > 1e-6) { t = clamp(dot(rel - in.uv.xy, dir) / len2, 0.0, 1.0); }
            let fill = mix(in.c0, in.c1, t);
            var pm = vec4<f32>(fill.rgb * fill.a, fill.a);
            let bw = in.params.x;
            if (bw > 0.0) {
                let inner = clamp(0.5 + (d + bw) * g.scale, 0.0, 1.0);
                let bpm = vec4<f32>(in.cb.rgb * in.cb.a, in.cb.a);
                pm = mix(pm, bpm, inner);
            }
            col = pm * cov;
        }
        case 1u: {
            let a = rounded_box_shadow(half, in.p - center, in.params.y, in.radii.x) * in.c0.a;
            col = vec4<f32>(in.c0.rgb * a, a);
        }
        case 2u: {
            let a = textureSampleLevel(atlas_tex, samp, in.tex_uv, 0.0).r * in.c0.a;
            col = vec4<f32>(in.c0.rgb * a, a);
        }
        case 3u: {
            let d = sd_round_rect(in.p, center, half, in.radii);
            let cov = clamp(0.5 - d * g.scale, 0.0, 1.0);
            let tex = textureSampleLevel(image_tex, samp, clamp(in.tex_uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0) * in.c0;
            let a = tex.a * cov;
            col = vec4<f32>(tex.rgb * a, a);
        }
        case 4u: {
            let pa = in.p - in.uv.xy;
            let ba = in.uv.zw - in.uv.xy;
            let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
            let d = length(pa - ba * h) - in.params.x * 0.5;
            let a = clamp(0.5 - d * g.scale, 0.0, 1.0) * in.c0.a;
            col = vec4<f32>(in.c0.rgb * a, a);
        }
        default: {}
    }
    if (g.output_srgb > 0.5 && col.a > 0.0) {
        let straight = col.rgb / col.a;
        col = vec4<f32>(to_linear(straight) * col.a, col.a);
    }
    return col;
}

fn encode_output(c: vec4<f32>) -> vec4<f32> {
    if (g.output_srgb > 0.5 && c.a > 0.0) {
        let straight = c.rgb / c.a;
        return vec4<f32>(to_linear(straight) * c.a, c.a);
    }
    return c;
}

// Effects get their own pipelines so their register pressure does not lower
// the occupancy of the common primitives in fs_main.
@fragment
fn fs_glass(in: VOut) -> @location(0) vec4<f32> {
    return encode_output(glass(in, in.rect.xy + in.rect.zw * 0.5, in.rect.zw * 0.5));
}

@fragment
fn fs_backdrop(in: VOut) -> @location(0) vec4<f32> {
    return encode_output(backdrop(in, in.rect.xy + in.rect.zw * 0.5, in.rect.zw * 0.5));
}

// Writes the unmasked backdrop color into the half-resolution layer.
@fragment
fn fs_backdrop_fill(in: VOut) -> @location(0) vec4<f32> {
    return encode_output(vec4<f32>(clamp(backdrop_color(in), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}

// Composites a backdrop from the half-resolution layer (bound as image_tex)
// with a full-resolution rounded mask and grain.
@fragment
fn fs_backdrop_layer(in: VOut) -> @location(0) vec4<f32> {
    let center = in.rect.xy + in.rect.zw * 0.5;
    let d = sd_round_rect(in.p, center, in.rect.zw * 0.5, in.radii);
    let cov = clamp(0.5 - d * g.scale, 0.0, 1.0);
    if (cov <= 0.0) {
        return vec4<f32>(0.0);
    }
    var rgb = textureSampleLevel(image_tex, samp, in.p / g.screen, 0.0).rgb;
    if (g.output_srgb > 0.5) {
        rgb = to_srgb(rgb);
    }
    rgb += backdrop_grain(in);
    let a = cov * in.c0.a;
    return encode_output(vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * a, a));
}
