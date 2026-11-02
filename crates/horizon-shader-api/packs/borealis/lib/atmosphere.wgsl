// The physical sky: single-scattering along the view ray through a standard
// Earth atmosphere (Rayleigh + Mie + an ozone band), lit by a sun and moon
// whose colours come from the same atmosphere's transmittance. The algorithm
// is the standard one used across the field (Hillaire 2020 and friends); this
// WGSL was written for Horizon and runs per sky pixel, at a scale that lands
// in 0..8 for the final grade's tone curve.

// --- constants (Bliss lib/ROBOBO_sky.glsl and lib/settings.glsl defaults) ---
const SKY_PLANET_RADIUS: f32 = 6731000.0;
const SKY_ATMOSPHERE_HEIGHT: f32 = 110000.0;
const SKY_ATMOSPHERE_RADIUS: f32 = SKY_PLANET_RADIUS + SKY_ATMOSPHERE_HEIGHT;
const SKY_ATMOSPHERE_RADIUS_SQUARED: f32 = SKY_ATMOSPHERE_RADIUS * SKY_ATMOSPHERE_RADIUS;
const SKY_SCALE_HEIGHTS: vec2<f32> = vec2<f32>(8000.0, 1200.0);
const SKY_INVERSE_SCALE_HEIGHTS: vec2<f32> = 1.0 / SKY_SCALE_HEIGHTS;
const SKY_SCALED_PLANET_RADIUS: vec2<f32> = vec2<f32>(SKY_PLANET_RADIUS) * SKY_INVERSE_SCALE_HEIGHTS;
// sky_coefficientRayleigh(R,G,B) 5.8, 1.35, 3.31 and Mie 3.0, from Bliss settings,
// folded into one attenuation matrix with the ozone band.
const SKY_RAYLEIGH: vec3<f32> = vec3<f32>(5.8e-6, 1.35e-5, 3.31e-5);
const SKY_MIE: vec3<f32> = vec3<f32>(3.0e-6, 3.0e-6, 3.0e-6);
const SKY_OZONE: vec3<f32> = vec3<f32>(4.51103766177301e-21, 3.2854797958699e-21, 1.96774621921165e-22)
    * (2.5035422e25 * 8e-6 * 0.2e-6);
const SKY_ATTENUATION: mat3x3<f32> = mat3x3<f32>(SKY_RAYLEIGH, SKY_MIE, SKY_OZONE);
const R_LOG2: f32 = 1.4426950408889634; // 1 / ln(2)

// A Planck-locus colour for the sun and moon tints; the sun is 6300 K, the
// moon 15000 K.
fn sky_blackbody(temp: f32) -> vec3<f32> {
    let t = pow(temp, -1.5);
    let lt = log(temp);
    var col = vec3<f32>(
        220000.0 * t + 0.58039215686,
        select(0.39231372549 * lt - 2.44549019608, 138039.215686 * t + 0.72156862745, temp > 6500.0),
        0.76078431372 * lt - 5.68078431373,
    );
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    // sRGB to linear, as Bliss returns it.
    return col * (col * (col * 0.305306011 + 0.682171111) + 0.012522878);
}

const SUN_COLOR: vec3<f32> = vec3<f32>(1.0, 0.85, 0.7); // sky_blackbody(6300), roughly
const MOON_COLOR: vec3<f32> = vec3<f32>(0.55, 0.65, 1.0); // sky_blackbody(15000), roughly

fn sky_rayleigh_phase(cos_theta: f32) -> f32 {
    return cos_theta * 0.0318309886184 + 0.0891262812; // [Elek09], / 4 pi
}

fn sky_mie_phase(cos_theta: f32, g: f32) -> f32 {
    let gg = g * g;
    return (gg * -0.25 + 0.25) * 0.0795774715459 * pow(-(2.0 * g) * cos_theta + (gg + 1.0), -1.5);
}

fn sky_density(center_distance: f32) -> vec3<f32> {
    let rayleigh_mie = exp(center_distance * -SKY_INVERSE_SCALE_HEIGHTS + SKY_SCALED_PLANET_RADIUS);
    // Ozone distribution curve by Sergeant Sarcasm,
    // https://www.desmos.com/calculator/j0wozszdwa
    let ozone = exp(-max(0.0, (35000.0 - center_distance) - SKY_PLANET_RADIUS) / 5000.0)
        * exp(-max(0.0, (center_distance - 35000.0) - SKY_PLANET_RADIUS) / 15000.0);
    return vec3<f32>(rayleigh_mie, ozone);
}

// Ray-sphere, as Bliss's `rsi`: x the near hit, y the far hit, y < 0 a miss.
fn rsi(origin: vec3<f32>, direction: vec3<f32>, radius: f32) -> vec2<f32> {
    let a = dot(origin, direction);
    let b = dot(origin, origin) - radius * radius;
    let d = a * a - b;
    if d < 0.0 {
        return vec2<f32>(1e5, -1e5);
    }
    let s = sqrt(d);
    return vec2<f32>(-a - s, -a + s);
}

fn sky_airmass(origin: vec3<f32>, direction: vec3<f32>, ray_length: f32, steps: i32) -> vec3<f32> {
    let step_size = ray_length / f32(steps);
    var position = origin + direction * step_size * 0.5;
    var airmass = vec3<f32>(0.0);
    for (var i = 0; i < steps; i++) {
        airmass += sky_density(length(position));
        position += direction * step_size;
    }
    return airmass * step_size;
}

// The transmittance from `origin` toward `direction` through the atmosphere.
fn sky_transmittance(origin: vec3<f32>, direction: vec3<f32>, steps: i32) -> vec3<f32> {
    var ray_length = dot(origin, direction);
    ray_length = ray_length * ray_length + SKY_ATMOSPHERE_RADIUS_SQUARED - dot(origin, origin);
    if ray_length < 0.0 {
        return vec3<f32>(1.0);
    }
    ray_length = sqrt(ray_length) - dot(origin, direction);
    let od = SKY_ATTENUATION * sky_airmass(origin, direction, ray_length, steps);
    return exp(-od * R_LOG2);
}

// calculateAtmosphere: the in-scattered light along `view` toward the sun and
// moon, with a fake multiple-scattering ambient term. 8 view steps, 3 light
// steps (Bliss defaults to 10 and 4; 8x3 keeps the check's expression budget).
const VIEW_STEPS: i32 = 8;
const LIGHT_STEPS: i32 = 3;

fn sky_atmosphere(view: vec3<f32>, sun: vec3<f32>, moon: vec3<f32>) -> vec3<f32> {
    // Darken the sky toward and below the horizon, as Bliss's planetGround does.
    let planet_ground = pow(clamp(view.y + 1.0, 0.0, 1.0), 2.0);
    let ground_darkening = max(planet_ground * 0.7 + 0.3, clamp(sun.y * 2.0, 0.0, 1.0));

    let view_pos = vec3<f32>(0.0, SKY_PLANET_RADIUS + 300.0, 0.0);
    let aid = rsi(view_pos, view, SKY_ATMOSPHERE_RADIUS);
    if aid.y < 0.0 {
        return vec3<f32>(0.0);
    }
    let pid = rsi(view_pos, view, SKY_PLANET_RADIUS * 0.998);
    let planet_intersected = pid.y >= 0.0;
    let t_start = select(max(aid.x, 0.0), pid.y, planet_intersected && pid.x < 0.0);
    let t_end = select(aid.y, pid.x, planet_intersected && pid.x > 0.0);

    let step_size = (t_end - t_start) / f32(VIEW_STEPS);
    let increment = view * step_size;
    var position = view * t_start + view_pos;

    let phase_sun = vec2<f32>(sky_rayleigh_phase(dot(view, sun)), sky_mie_phase(dot(view, sun), 0.8));
    let phase_moon = vec2<f32>(sky_rayleigh_phase(dot(view, moon)), sky_mie_phase(dot(view, moon), 0.8));

    var scattering_sun = vec3<f32>(0.0);
    var scattering_moon = vec3<f32>(0.0);
    var scattering_ambient = vec3<f32>(0.0);
    var transmittance = vec3<f32>(1.0);

    // make sunrise less blue, and allow sunset to be bluer (Bliss).
    let high_sun = clamp(pow(sun.y + 0.6, 5.0), 0.0, 1.0) * 3.0;
    let low_sun = clamp(((1.0 - abs(sun.y)) * 3.0) - high_sun, 1.0, 1.8);

    for (var i = 0; i < VIEW_STEPS; i++) {
        let density = sky_density(length(position));
        let step_airmass = density * step_size;
        let step_od = SKY_ATTENUATION * step_airmass;
        let step_transmittance = exp2(-step_od * R_LOG2);
        let step_fraction = clamp((step_transmittance - 1.0) / -step_od, vec3<f32>(0.0), vec3<f32>(1.0));
        let visible = transmittance * step_fraction * ground_darkening;

        scattering_sun += (SKY_RAYLEIGH * step_airmass.x * phase_sun.x + SKY_MIE * step_airmass.y * phase_sun.y)
            * visible * sky_transmittance(position, sun, LIGHT_STEPS) * planet_ground;
        scattering_moon += (SKY_RAYLEIGH * step_airmass.x * phase_moon.x + SKY_MIE * step_airmass.y * phase_moon.y)
            * visible * sky_transmittance(position, moon, LIGHT_STEPS) * planet_ground;
        scattering_ambient += (SKY_RAYLEIGH * step_airmass.x + SKY_MIE * step_airmass.y) * visible * low_sun;

        transmittance *= step_transmittance;
        position += increment;
    }

    // The driving light: the sun and moon through the atmosphere at the horizon,
    // standing in for Bliss's sunColorBase/moonColorBase (128000/50 lux at
    // 6300 K / 15000 K), with a night floor so the sky never goes fully black.
    let sun_through = sky_transmittance(view_pos, sun, LIGHT_STEPS);
    // A low sun warms the whole scattering, the way Bliss's sunrises redden.
    let low = smoothstep(0.35, 0.0, sun.y);
    let sun_light = SUN_COLOR * max(vec3<f32>(0.04), sun_through) * 26.0
        * mix(vec3<f32>(1.0), vec3<f32>(1.3, 0.72, 0.42), low);
    let moon_light = MOON_COLOR * 0.05;

    return scattering_sun * sun_light + scattering_moon * moon_light
        + scattering_ambient * (vec3<f32>(0.32, 0.45, 0.72) * (0.35 + daylight() * 0.65));
}

// The sun's disc and halo drawn over the sky, from Bliss's drawSun
// (lib/sky_gradient.glsl): a small, hot core with a tight glow.
fn sky_sun_disc(view: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    let cos_y = dot(view, sun);
    let core = pow(smoothstep(cos(0.0298), cos(0.0168), cos_y), 3.0);
    let halo = pow(smoothstep(cos(0.24), cos(0.03), cos_y), 4.0) * 0.35;
    let through = sky_transmittance(vec3<f32>(0.0, SKY_PLANET_RADIUS + 300.0, 0.0), sun, LIGHT_STEPS);
    let color = SUN_COLOR * max(vec3<f32>(0.05), through);
    return color * (core * 8.0 + halo) * sun_glow;
}
