// Water waves and plant movement. The water is a port of Photon's Gerstner
// wave stack (shaders/include/surface/water_normal.glsl, Photon Shaders by
// SixthSurge; its license, which permits redistributing modified portions,
// ships with this pack as LICENSE-photon.txt). The plant movement is this
// pack's own sine sway.

override wavy_plants: bool = true;
override water_waves: bool = true;

const WATER_WAVE_ITERATIONS: i32 = 3;
const WATER_WAVE_FREQUENCY: f32 = 0.7;
const WATER_WAVE_PERSISTENCE: f32 = 0.5;
const WATER_WAVE_LACUNARITY: f32 = 1.7;
const WATER_WAVE_SPEED_STILL: f32 = 1.0;

// Gerstner wave function, from Photon (credited there to Belmu in #snippets,
// modified).
fn gerstner_wave(coord: vec2<f32>, wave_dir: vec2<f32>, t: f32, noise: f32, wavelength: f32) -> f32 {
    let g = 9.8;
    let k = tau() / wavelength;
    let w = sqrt(g * k);
    let x = w * t - k * (dot(wave_dir, coord) + noise);
    let s = sin(x) * 0.5 + 0.5;
    return s * s;
}

// Photon's water_waves_setup, for still water (this pack does not track flow
// directions): a fixed 30-degree direction and the golden-angle rotation
// between octaves.
fn wave_params() -> mat2x2<f32> {
    let angle = 30.0 * 0.017453292519943295;
    return mat2x2<f32>(
        vec2<f32>(cos(2.399963229728653), sin(2.399963229728653)),
        vec2<f32>(-sin(2.399963229728653), cos(2.399963229728653)),
    );
}

// The wave stack's height at a world xz, from Photon's get_water_height, with
// its noise texture samples read from the pack's `noise` texture.
fn water_height(coord: vec2<f32>) -> f32 {
    let t = 0.5 * WATER_WAVE_SPEED_STILL * frame.time;
    let wave_dir = vec2<f32>(cos(30.0 * 0.017453292519943295), sin(30.0 * 0.017453292519943295));
    let wave_rot = wave_params();

    let wave_frequency = 0.7 * WATER_WAVE_FREQUENCY;
    let persistence = 0.5 * WATER_WAVE_PERSISTENCE;
    let lacunarity = 1.7 * WATER_WAVE_LACUNARITY;

    let noise_frequency = 0.007;
    let noise_strength = 2.0;

    // The amplitude normalisation of the geometric series the octaves sum to.
    let amplitude_normalization_factor = (1.0 - persistence)
        / (1.0 - pow(persistence, f32(WATER_WAVE_ITERATIONS)));

    var height = 0.0;
    var amplitude = 1.0;
    var frequency = wave_frequency;
    var wave_length = 1.0;
    var dir = wave_dir;
    var noise_coord = (coord + vec2<f32>(0.0, 0.25 * t)) * noise_frequency;
    for (var i = 0; i < WATER_WAVE_ITERATIONS; i++) {
        // The pack's noise texture keeps its smooth value noise in blue.
        let noise = textureSampleLevel(noise, noise_sampler, noise_coord, 0.0).z;
        height += gerstner_wave(coord * frequency, dir, t, noise * noise_strength, wave_length) * amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
        wave_length *= 1.5;
        dir = wave_rot * dir;
        noise_coord *= 2.5;
    }
    return height * amplitude_normalization_factor;
}

// The water surface's normal at world position `pos`, from Photon's
// get_water_normal: height differences over a small step, with the
// influence rising in rain and falling with distance.
fn water_wave_normal(pos: vec3<f32>, flat_normal: vec3<f32>, skylight: f32) -> vec3<f32> {
    let h = 0.1;
    let wave0 = water_height(pos.xz);
    let wave1 = water_height(pos.xz + vec2<f32>(h, 0.0));
    let wave2 = water_height(pos.xz + vec2<f32>(0.0, h));

    var normal_influence = mix(0.01, 0.04 + 0.15 * frame.rain, clamp(skylight, 0.0, 1.0));
    // prevent noise when looking horizontally (Photon's dampen)
    normal_influence *= smoothstep(0.0, 0.15, abs(dot(flat_normal, normalize(pos))));

    var normal = vec3<f32>(wave1 - wave0, wave2 - wave0, h);
    normal.x *= normal_influence;
    normal.y *= normal_influence;
    return normalize(normal);
}

// The rolling of the water surface itself, for the vertex stage: how far up
// the top of a water block sits, in blocks (small, so it never leaves its
// block).
fn water_rolling(world_pos: vec3<f32>) -> f32 {
    if !water_waves {
        return 0.0;
    }
    return water_height(world_pos.xz) * 0.08;
}

// The sway of grass and crops, anchored where they meet the ground (the
// caller scales it by how far up the plant the corner sits).
fn plant_wave(world_pos: vec3<f32>) -> vec3<f32> {
    if !wavy_plants {
        return vec3<f32>(0.0);
    }
    let wt = frame.time * 2.0 * PI * 0.66;
    let magnitude = abs(sin(dot(vec4<f32>(frame.time, world_pos * 0.005), vec4<f32>(1.0, 1.0, 1.0, 1.0))) * 0.5 + 0.72) * 0.013;
    let ret = (sin(vec2<f32>(wt * 0.0252, wt * 0.006) - world_pos.xz + vec2<f32>(world_pos.y * 0.05)) + 0.1) * magnitude;
    return vec3<f32>(ret.x, -length(ret), ret.y) * 5.0 * 0.55;
}

// Leaves wave uniformly, off of the air.
fn leaves_wave(world_pos: vec3<f32>) -> vec3<f32> {
    if !wavy_plants {
        return vec3<f32>(0.0);
    }
    let wt = frame.time * 2.0 * PI * 0.66;
    let magnitude = abs(sin(dot(vec4<f32>(frame.time, world_pos * 0.005), vec4<f32>(1.0, 1.0, 1.0, 1.0))) * 0.5 + 0.72) * 0.013;
    return sin(vec3<f32>(wt * 0.00945, wt * 0.0336, wt * 0.00225) - world_pos) * magnitude * 5.0 * vec3<f32>(0.55, 0.11, 0.55);
}
