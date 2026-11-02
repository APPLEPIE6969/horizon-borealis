// Lighting, shadow and fog for terrain, written for Horizon's material stage:
// light from the lightmap curves, soft sun shadows with the colour of what
// they pass through, and the pack's own fog. The shadow bias follows the
// technique Emin and Gri573 described for the Complementary lineage (credited
// in Bliss's lib/Shadows.glsl, and public knowledge since); the code here is
// this pack's own.
//
// Every colour here is linear and mostly HDR: the final grade's tone curve
// brings it back to the screen.

override soft_shadows: bool = true;
override shadow_strength: f32 = 0.85;
override shadow_distance: u32 = 2u; // choice index: 32 / 64 / 128 / 256 blocks
override sunlight_amount: f32 = 1.0;
override ambient_amount: f32 = 1.0;
override blocklight_amount: f32 = 1.0;
override blocklight_color: bool = true;
override specular: bool = true;
override fog_density: f32 = 1.0;
override sun_glow: f32 = 1.0;

const SHADOW_FADES: array<f32, 4> = array<f32, 4>(32.0, 64.0, 128.0, 256.0);

// --- light colours -----------------------------------------------------------

// The sun's light on terrain: warm white at noon, orange as it sets, gone at
// night (Bliss drives this through the atmosphere; here a compact stand-in
// with the same palette). HDR.
fn direct_sun_color() -> vec3<f32> {
    let low = sun_low();
    let day = daylight();
    let noon = vec3<f32>(1.0, 0.975, 0.93) * 3.0;
    let sunset = vec3<f32>(1.0, 0.56, 0.26) * 1.3;
    var c = mix(noon, sunset, low * low);
    // The moon takes over below the horizon.
    let moon = vec3<f32>(0.30, 0.40, 0.62) * 0.34;
    let night = smoothstep(0.05, -0.12, frame.sun_direction.y);
    c = mix(c, moon, night);
    return c * day * (1.0 - frame.rain * 0.85) * sunlight_amount;
}

// The sky's ambient on terrain, from the sky colour itself and the curves of
// Bliss's doIndirectLighting. HDR-ish, kept well under the direct sun so the
// shaded sides read as sky-lit, not as flat.
fn ambient_light(sky_light: f32) -> vec3<f32> {
    let curve = (pow(sky_light, 15.0) * 2.0 + pow(sky_light, 2.5)) * 0.5;
    var sky_tint = select(vec3<f32>(0.32, 0.45, 0.72), frame.sky_color * 1.1, frame.dimension == 0);
    // Some of the sky's own saturation stays, some becomes white light.
    sky_tint = mix(vec3<f32>(luminance(sky_tint)), sky_tint, 0.5);
    sky_tint *= 0.42 + daylight() * 0.63;
    sky_tint *= 1.0 - frame.rain * 0.5;
    // A warm bounce while the sun is low.
    sky_tint += vec3<f32>(0.35, 0.16, 0.05) * sun_low() * daylight() * 0.5;
    return sky_tint * curve * ambient_amount * 0.85;
}

// Block light, from Bliss's doBlockLightLighting: its curve on the light
// level, tinted by the pack's `[[lights]]` colours.
fn block_light(block_light_level: f32, position: vec3<f32>) -> vec3<f32> {
    let curve = pow(1.0 - sqrt(1.0 - clamp(block_light_level, 0.0, 1.0)), 2.0) * 2.0;
    if blocklight_color {
        let colored = block_light_color(position);
        let tint = mix(vec3<f32>(1.0), clamp(colored.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), min(colored.a, 1.0));
        return vec3<f32>(1.0, 0.75, 0.55) * tint * curve * blocklight_amount;
    }
    return vec3<f32>(1.0, 0.75, 0.55) * curve * blocklight_amount;
}

// --- shadows -----------------------------------------------------------------

// Gri's and Emin's shadow fix (Bliss lib/Shadows.glsl, credited there to
// https://www.complementary.dev/reimagined and gri573): bias along the normal
// so peter panning and acne balance, and zoom up to block centres where the
// sky light is low so light does not leak through walls.
fn shadow_fixed_position(world_pos: vec3<f32>, flat_normal: vec3<f32>, sky_light: f32) -> vec3<f32> {
    let minimum = 0.05;
    let res_multiplier = 1.0 + (frame.shadow_distance / 8.0) * (1.0 - min(frame.shadow_resolution, 2048.0) / 2048.0) * 0.3;
    let distance_multiplier = max(1.0 - max(1.0 - length(world_pos) / frame.shadow_distance, 0.0), minimum) * res_multiplier;
    var bias = flat_normal * distance_multiplier;
    // stop lightleaking by zooming up, centered on blocks
    let zoom = 0.25 - 0.5 * fract(world_pos + frame.camera_position + bias * 0.25);
    if sky_light < 0.1 {
        bias = zoom;
    }
    return world_pos + bias;
}

// How much of the sun reaches the point: 0 in shade, 1 lit, with the colour
// of whatever translucent roof it passes through. The samplers filter 2x2
// percentage-closer; soft_shadows adds four rotated taps (Bliss's
// Variable_Penumbra_Shadows, simplified to a fixed radius).
fn shadow_reach(world_pos: vec3<f32>, flat_normal: vec3<f32>, vanilla_ao: f32, sky_light: f32, pixel: vec2<f32>) -> vec4<f32> {
    let fade_blocks = min(SHADOW_FADES[shadow_distance], frame.shadow_distance);
    var reach = smoothstep(fade_blocks, fade_blocks * 0.72, length(world_pos));

    let p = shadow_position(shadow_fixed_position(world_pos, flat_normal, sky_light));
    var lit = textureSampleCompareLevel(shadow_map, shadow_sampler, p.xy, p.z);
    var lit_full = textureSampleCompareLevel(shadow_map_translucent, shadow_sampler, p.xy, p.z);
    if soft_shadows && frame.has_shadow == 1u {
        let texel = 1.5 * frame.shadow_center_texel;
        let angle = ign(pixel) * 2.0 * PI;
        let rot = vec2<f32>(cos(angle), sin(angle)) * texel;
        let taps = array<vec2<f32>, 4>(vec2<f32>(0.0, 0.0), vec2<f32>(rot.x, rot.y), vec2<f32>(-rot.y, rot.x), vec2<f32>(rot.y, -rot.x));
        for (var i = 0; i < 4; i++) {
            let q = p.xy + taps[i] / max(frame.shadow_resolution, 1.0) * 0.5;
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, q, p.z);
            lit_full += textureSampleCompareLevel(shadow_map_translucent, shadow_sampler, q, p.z);
        }
        lit = lit / 5.0;
        lit_full = lit_full / 5.0;
    }

    // What colour the light that passes through water or glass keeps: read the
    // translucent shadow colour where only the translucent map lit.
    let size = vec2<f32>(textureDimensions(shadow_color));
    let tint = textureLoad(shadow_color, vec2<i32>(p.xy * size), 0);
    let through = clamp(lit_full - lit, 0.0, 1.0);
    let shadow_alpha = pow(1.0 - pow(tint.a, 5.0), 0.2);
    let scalar = max(lit, through * (1.0 - shadow_alpha));
    let color = mix(vec3<f32>(1.0), tint.rgb, through);

    return vec4<f32>(mix(1.0, scalar, reach * shadow_strength), color);
}

// --- sun glint ---------------------------------------------------------------

// A GGX-ish sun glint on smooth surfaces (Bliss's specular lighting,
// simplified to the sun term only).
fn sun_glint(normal: vec3<f32>, view: vec3<f32>, smoothness: f32, light_color: vec3<f32>) -> vec3<f32> {
    if !specular || smoothness <= 0.0 {
        return vec3<f32>(0.0);
    }
    let h = normalize(view + frame.light_direction);
    let ndh = max(dot(normal, h), 0.0);
    let roughness = max(1.0 - smoothness, 0.04);
    let d = (ndh * ndh * (roughness * roughness - 1.0) + 1.0);
    let ndv = max(dot(normal, view), 0.0);
    let fresnel = 0.04 + 0.96 * pow(1.0 - ndv, 5.0);
    return light_color * fresnel * (1.0 / (d * d)) * 0.025 * smoothness;
}

// --- fog ---------------------------------------------------------------------

// Bliss's fogPhase (lib/overworld_fog.glsl): how much the fog glows toward
// the sun.
fn fog_phase(light_point: f32) -> f32 {
    let linear = 1.0 - clamp(light_point * 0.5 + 0.5, 0.0, 1.0);
    let linear2 = 1.0 - clamp(light_point, 0.0, 1.0);
    var exponential = exp2(pow(linear, 0.3) * -15.0) * 1.5;
    exponential += sqrt(exp2(sqrt(linear) * -12.5));
    return exponential * 0.5 + linear2 * 0.05;
}

// The fog colour at a view direction: vanilla's fog colour, warmed toward the
// sun the way Bliss's volumetric fog scatters.
fn fog_color_toward(view: vec3<f32>) -> vec3<f32> {
    let scatter = fog_phase(dot(normalize(view), frame.light_direction)) * daylight() * (1.0 - frame.rain * 0.6);
    return frame.fog_color + vec3<f32>(0.9, 0.5, 0.22) * scatter * 0.45 * sun_glow * 0.5;
}

// The pack's fog: vanilla's distance fog to its colour, a thicker band at the
// render distance border, underwater and lava fog, and the blindness and
// darkness effects (the pack fogs its own surfaces; see [material] fog).
fn apply_fog(color: vec3<f32>, view: vec3<f32>) -> vec3<f32> {
    let distance = length(view);
    var fog = smoothstep(frame.fog_start, frame.fog_end, distance) * fog_density;
    // The border to the sky: past the render distance everything meets the sky.
    let border = smoothstep(frame.render_distance * 0.85, frame.far * 0.98, distance);
    fog = max(fog, border);
    var fog_target = fog_color_toward(view);
    var alpha = 1.0;

    if frame.in_water > 0.0 {
        let deep = 1.0 - exp(-distance * 0.055);
        fog = max(fog, deep * frame.in_water);
        fog_target = mix(fog_target, vec3<f32>(0.04, 0.14, 0.18) * (0.15 + frame.eye_light.y * 0.85), frame.in_water);
    }
    if frame.in_lava > 0.0 {
        fog = max(fog, frame.in_lava);
        fog_target = mix(fog_target, vec3<f32>(0.6, 0.25, 0.05), frame.in_lava);
    }

    // Effects pull toward black (blindness) or gray (darkness).
    alpha = 1.0 - frame.blindness * smoothstep(2.0, 12.0, distance);
    alpha *= 1.0 - frame.darkness * 0.8;

    var out = mix(color, fog_target, clamp(fog, 0.0, 1.0));
    out *= alpha;
    // Night vision lifts the murk.
    out += vec3<f32>(0.0, 0.18, 0.1) * frame.night_vision * fog;
    return out;
}
