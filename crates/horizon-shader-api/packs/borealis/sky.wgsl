// The sky along each view ray: Bliss's physical atmosphere, a sun drawn by the
// pack, and stars that fade in over the night. The sun, moon and stars are the
// pack's (`sun = false`, `stars = false`); vanilla's clouds still draw over
// this. Reference: docs/api/shaders-reference.md.
override sun_glow: f32 = 1.0;
override stars_brightness: f32 = 1.0;

fn sky(input: SkyInput) -> vec4<f32> {
    var color = input.color.rgb;

    if frame.dimension == 0 {
        let view = input.direction;
        var atmosphere = sky_atmosphere(view, frame.sun_direction, frame.moon_direction);

        // Bliss mixes its sky toward the average sky colour in rain, and grays it.
        let rain = frame.rain;
        atmosphere = mix(atmosphere, frame.fog_color * 0.9, rain * 0.75);
        atmosphere *= 1.0 - rain * 0.35;

        // Keep a fingerprint of vanilla's biome sky so deserts and swamps stay
        // themselves, the way Bliss's LUT bakes the world's sky colour in.
        let biome_mix = 0.25 * smoothstep(-0.1, 0.3, view.y);
        color = mix(atmosphere, color * 1.2, biome_mix);

        // The pack's sun over the day sky (it fades as it sets; the moon stays
        // vanilla's, drawn over the sky by the engine).
        color += sky_sun_disc(view, frame.sun_direction) * smoothstep(-0.06, 0.02, frame.sun_direction.y) * (1.0 - rain);

        // Stars at night, hidden by rain, with Photon's colour and twinkle.
        let night = smoothstep(0.0, -0.18, frame.sun_direction.y);
        let star = stars(view, 0.0, frame.time) * stars_brightness * night * (1.0 - rain)
            * smoothstep(0.0, 0.15, view.y);
        color += star * 2.0;

        // Underwater, the sky goes out quickly.
        color = mix(color, vec3<f32>(0.05, 0.13, 0.18), frame.in_water * 0.85);
    }

    // Blindness and darkness pull the sky in; night vision lifts it a little.
    color *= 1.0 - frame.blindness * 0.9;
    color *= 1.0 - frame.darkness * 0.9;
    color += vec3<f32>(0.0, 0.2, 0.12) * frame.night_vision * 0.25;

    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}
