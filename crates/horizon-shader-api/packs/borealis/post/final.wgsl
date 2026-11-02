// The final grade: bloom composited over the picture, an exposure that
// follows the light at the camera, Bliss's tone curves, the night's blue
// shift, saturation, contrast, a vignette and a dither. Bliss does this in its
// composite and final passes (dimensions/composite11.fsh, final.fsh).
override bloom_strength: f32 = 1.0;
override exposure: f32 = 1.0;
override tonemap: u32 = 0u; // choice: AgX, ACES, Lottes, Hable, Uchimura, Reinhard
override saturation: f32 = 1.15;
override contrast: f32 = 1.0;
override vignette: bool = true;
override purkinje: f32 = 0.5;

// The exposure the frame gets, in place of Bliss's auto exposure: darker
// scenes lift, bright scenes settle, underwater and night vision nudge it.
fn frame_exposure() -> f32 {
    let eye = frame.eye_light_smooth.y * frame.eye_light_smooth.y;
    var e = mix(1.35, 1.0, smoothstep(0.0, 0.5, eye));
    e *= mix(1.0, 1.6, frame.in_water * 0.5);
    e *= 1.0 + frame.night_vision * 0.7;
    return e * exposure;
}

fn apply_tonemap(c: vec3<f32>) -> vec3<f32> {
    if tonemap == 1u {
        return tonemap_aces(c);
    }
    if tonemap == 2u {
        return tonemap_lottes(c);
    }
    if tonemap == 3u {
        return tonemap_hable(c);
    }
    if tonemap == 4u {
        return tonemap_uchimura(c);
    }
    if tonemap == 5u {
        return tonemap_reinhard(c);
    }
    return tonemap_agx(c);
}

fn post(input: PostInput) -> vec4<f32> {
    // Underwater, the picture wobbles gently with the water.
    var uv = input.uv;
    if frame.in_water > 0.01 {
        let t = frame.time * 1.4;
        let wobble = vec2<f32>(sin(uv.y * 28.0 + t), cos(uv.x * 24.0 - t * 0.8)) * 0.0022 * frame.in_water;
        uv = clamp(uv + wobble, vec2<f32>(0.001), vec2<f32>(0.999));
    }

    // The picture this pack grades: the world before any pass (the bloom
    // passes between wrote only their own targets).
    var c = textureSampleLevel(scene_color, linear_sampler, uv, 0.0).rgb;

    // The bloom: the blurred bright pass, over the frame.
    let bloom = textureSampleLevel(bloom3, linear_sampler, input.uv, 0.0).rgb;
    c += bloom * bloom_strength * 0.55;

    c *= frame_exposure();

    // Purkinje: in the dark, colour drains toward blue-grey (Bliss's
    // gameplay-friendly take on the eye's scotopic shift).
    let dark = 1.0 - smoothstep(0.02, 0.45, frame.eye_light_smooth.y);
    if purkinje > 0.0 && dark > 0.0 {
        let luma = luminance(c);
        let shifted = luma * vec3<f32>(0.75, 0.9, 1.25);
        c = mix(c, shifted, dark * purkinje * 0.7);
    }

    c = apply_tonemap(c);

    // Saturation and contrast, as golden_hour grades, after the curve.
    c = mix(vec3<f32>(luminance(c)), c, saturation);
    c = clamp((c - 0.5) * contrast + 0.5, vec3<f32>(0.0), vec3<f32>(1.0));

    if vignette {
        let aspect = frame.resolution.x / frame.resolution.y;
        let d = length((input.uv - 0.5) * vec2<f32>(aspect, 1.0)) / length(vec2<f32>(aspect, 1.0) * 0.5);
        c *= 1.0 - smoothstep(0.55, 1.25, d) * 0.32;
    }

    // A dither against banding, on the way out.
    c += (ign(input.pixel) - 0.5) / 255.0;

    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
