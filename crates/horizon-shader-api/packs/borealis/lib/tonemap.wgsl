// The film curves of the final grade. AgX is iolite's MIT minimal AgX
// (https://iolite-engine.com/blog_posts/minimal_agx_implementation, copyright
// Benjamin Wrensch, Missing Deadlines), the curve most packs default to; the
// others are the published ACES (Narkowicz), Lottes, Hable, Uchimura and
// Reinhard fits.

fn agx_contrast_approx(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2
        - 40.14 * x4 * x
        + 31.96 * x4
        - 6.868 * x2 * x
        + 0.4298 * x2
        + 0.1191 * x
        - 0.00232;
}

fn agx_look(val: vec3<f32>) -> vec3<f32> {
    let lw = vec3<f32>(0.2126, 0.7152, 0.0722);
    let luma = dot(val, lw);
    let sat = 1.25;
    return luma + sat * (val - luma);
}

// AgX, as Bliss's ToneMap_AgX_minimal (the "brights don't go to white" one).
fn tonemap_agx(color_in: vec3<f32>) -> vec3<f32> {
    var color = color_in;
    let inset = mat3x3<f32>(
        vec3<f32>(0.842479062253094, 0.0423282422610123, 0.0423756549057051),
        vec3<f32>(0.0784335999999992, 0.878468636469772, 0.0784336),
        vec3<f32>(0.0792237451477643, 0.0791661274605434, 0.879142973793104),
    );
    let outset = mat3x3<f32>(
        vec3<f32>(1.19687900512017, -0.0528968517574562, -0.0529716355144438),
        vec3<f32>(-0.0980208811401368, 1.15190312990417, -0.0980434501171241),
        vec3<f32>(-0.0990297440797205, -0.0989611768448433, 1.15107367264116),
    );
    let min_ev = -12.47393;
    let max_ev = 4.026069;

    color = inset * color;
    color = clamp(log2(max(color, vec3<f32>(1e-10))), vec3<f32>(min_ev), vec3<f32>(max_ev));
    color = (color - min_ev) / (max_ev - min_ev);
    color = agx_contrast_approx(color);
    color = agx_look(color);
    color = outset * color;
    color = pow(max(color, vec3<f32>(0.0)), vec3<f32>(2.2));
    return clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
}

// ACES film (Narkowicz's fit), as Bliss's ACESFilm.
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    let v = x * 0.9;
    return (v * (v * 2.51 + 0.03)) / (v * (v * 2.43 + 0.59) + 0.14);
}

// Lottes 2016, as Bliss's Tonemap_Lottes.
fn tonemap_lottes(x: vec3<f32>) -> vec3<f32> {
    let a = 1.6;
    let d = 0.977;
    let hdr_max = 8.0;
    let mid_in = 0.23;
    let mid_out = 0.267;
    let b = (-pow(mid_in, a) + pow(hdr_max, a) * mid_out) / ((pow(hdr_max, a * d) - pow(mid_in, a * d)) * mid_out);
    let c = (pow(hdr_max, a * d) * pow(mid_in, a) - pow(hdr_max, a) * pow(mid_in, a * d) * mid_out) / ((pow(hdr_max, a * d) - pow(mid_in, a * d)) * mid_out);
    return pow(x, vec3<f32>(a)) / (pow(x, vec3<f32>(a * d)) * b + c);
}

// Hable (Uncharted 2's fit), as Bliss's HableTonemap.
fn tonemap_hable(linear_color: vec3<f32>) -> vec3<f32> {
    let a = 0.6;
    let b = 0.5;
    let c = 0.1;
    let d = 0.5;
    let e = 0.01;
    let f = 0.3;
    let x = linear_color * 2.0;
    let color = ((x * (a * x + c * b) + d * e) / (x * (a * x + b) + d * f)) - e / f;
    let w = 11.0;
    let white = ((w * (a * w + c * b) + d * e) / (w * (a * w + b) + d * f)) - e / f;
    return color / white;
}

// Uchimura 2017, as Bliss's Tonemap_Uchimura.
fn tonemap_uchimura(x: vec3<f32>) -> vec3<f32> {
    let p = 1.0;
    let a = 1.0;
    let m = 0.12;
    let l = 0.22;
    let c = 1.0;
    let b = 0.0;
    let l0 = ((p - m) * l) / a;
    let lm0 = m - m / a;
    let lm1 = m + (1.0 - m) / a;
    let s0 = m + l0;
    let s1 = m + a * l0;
    let c2 = (a * p) / (p - s1);
    let cp = -c2 / p;

    let w0 = 1.0 - smoothstep(vec3<f32>(0.0), vec3<f32>(m), x);
    let w2 = select(vec3<f32>(0.0), vec3<f32>(1.0), x >= vec3<f32>(m + l0));
    let w1 = 1.0 - w0 - w2;

    let t = m * pow(max(x / m, vec3<f32>(0.0)), vec3<f32>(c)) + b;
    let s = p - (p - s1) * exp(cp * (x - s0));
    let lm = m + a * (x - m);

    return clamp(t * w0 + lm * w1 + s * w2, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Reinhard with Bliss's Full_Reinhard_Edit curve.
fn tonemap_reinhard(c: vec3<f32>) -> vec3<f32> {
    let v = pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.2));
    let whitepoint = 10.0;
    let lighten = 0.333;
    return (v * (1.0 + v / (whitepoint * whitepoint))) / (lighten + v);
}
