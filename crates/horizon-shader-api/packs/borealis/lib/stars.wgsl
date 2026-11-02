// The night sky's stars, ported from Photon's shaders/include/sky/stars.glsl
// (Photon Shaders by SixthSurge; its license, which permits redistributing
// modified portions, ships with this pack as LICENSE-photon.txt). The hash and
// the star field's structure are Photon's; the WGSL is this pack's own.

const STARS_INTENSITY: f32 = 1.0;
const STARS_COVERAGE: f32 = 0.5;

// hash4, from Photon's include/utility/random.glsl.
fn hash4(p: vec2<f32>) -> vec4<f32> {
    var p4 = fract(vec4<f32>(p.xyxy) * vec4<f32>(0.1031, 0.1030, 0.0973, 0.1099));
    p4 += dot(p4, p4.wzxy + 33.33);
    return fract((p4.xxyz + p4.yzzw) * p4.zywx);
}

// blackbody, from Photon's include/utility/color.glsl: Planck's law sampled at
// three Rec.2020 primary wavelengths, normalised.
fn star_blackbody(temperature: f32) -> vec3<f32> {
    let lambda = vec3<f32>(660.0, 550.0, 440.0); // nm, Photon's Rec.2020 primaries
    let lambda2 = lambda * lambda;
    let lambda5 = lambda2 * lambda2 * lambda;

    let h = 6.63e-16; // Planck constant
    let k = 1.38e-5; // Boltzmann constant
    let c = 3.0e17; // Speed of light

    let a = lambda5 / (2.0 * h * c * c);
    let b = (h * c) / (k * lambda);
    let d = exp(b / temperature);

    let rgb = a * d - a;
    return min(min(rgb.x, rgb.y), rgb.z) / rgb;
}

fn linear_step(low: f32, high: f32, x: f32) -> f32 {
    return clamp((x - low) / (high - low), 0.0, 1.0);
}

fn cubic_smooth(x: f32) -> f32 {
    return x * x * (3.0 - 2.0 * x);
}

fn unstable_star_field(coord: vec2<f32>, star_threshold: f32, time: f32) -> vec3<f32> {
    let min_temp = 4500.0;
    let max_temp = 8500.0;

    let noise = hash4(coord);

    var star = linear_step(star_threshold, 1.0, noise.x);
    star = pow(star, 16.0) * STARS_INTENSITY;

    let temp = mix(min_temp, max_temp, noise.y);
    let color = star_blackbody(temp);

    let twinkle_speed = 2.0;
    let twinkle_amount = noise.z;
    let twinkle_offset = tau() * noise.w;
    star *= 1.0 - twinkle_amount * cos(time * twinkle_speed + twinkle_offset);

    return star * color;
}

// Stabilises the field by interpolating the four neighbouring lattice points.
fn stable_star_field(coord_in: vec2<f32>, star_threshold: f32, time: f32) -> vec3<f32> {
    let coord = abs(coord_in) + 33.3 * step(vec2<f32>(0.0), coord_in);
    let i = floor(coord);
    let f_raw = coord - i;
    let f = vec2<f32>(cubic_smooth(f_raw.x), cubic_smooth(f_raw.y));

    return unstable_star_field(i, star_threshold, time) * (1.0 - f.x) * (1.0 - f.y)
        + unstable_star_field(i + vec2<f32>(1.0, 0.0), star_threshold, time) * f.x * (1.0 - f.y)
        + unstable_star_field(i + vec2<f32>(0.0, 1.0), star_threshold, time) * f.y * (1.0 - f.x)
        + unstable_star_field(i + vec2<f32>(1.0, 1.0), star_threshold, time) * f.x * f.y;
}

// The stars' colour along the view ray, from Photon's draw_stars: the ray is
// projected onto a plane, the field sampled there. Time drives the twinkle.
fn stars(ray_dir: vec3<f32>, threshold_boost: f32, time: f32) -> vec3<f32> {
    // The threshold drops as the night deepens, brightest stars first.
    let star_threshold = 1.0 - 0.05 * STARS_COVERAGE * smoothstep(-0.2, 0.05, -frame.sun_direction.y)
        - threshold_boost;

    // Project the ray direction onto the plane (Photon's projection).
    var coord = ray_dir.xy * (1.0 / (abs(ray_dir.z) + length(ray_dir.xy)))
        + 41.21 * sign(ray_dir.z);
    coord *= 600.0;

    return stable_star_field(coord, star_threshold, time);
}
