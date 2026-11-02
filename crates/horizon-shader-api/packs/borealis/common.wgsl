// Borealis — a shader pack written for Horizon's shader API. This file holds
// the few helpers every stage shares. Reference: docs/api/shaders-reference.md.

const PI: f32 = 3.14159265358979;
const TAU: f32 = 6.283185307179586;

fn tau() -> f32 {
    return TAU;
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// 1 while the sun is up in the Overworld, easing out around the horizon.
fn daylight() -> f32 {
    return select(0.0, smoothstep(-0.1, 0.1, frame.sun_direction.y), frame.dimension == 0);
}

// 1 when the sun sits on the horizon, 0 from a quarter of the way up.
fn sun_low() -> f32 {
    return 1.0 - smoothstep(0.0, 0.35, abs(frame.sun_direction.y));
}

// A small float hash of the common 0.1031 family (Dave Hoskins's
// "Hash without Sine"), used for dithering.
fn hash13(p3_in: vec3<f32>) -> f32 {
    var p3 = p3_in;
    p3 = fract(p3 * 0.1031);
    p3 += dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

// Interleaved gradient noise (Jimenez 2014), as Bliss's passes dither with.
fn ign(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(0.06711056 * pixel.x + 0.00583715 * pixel.y));
}
