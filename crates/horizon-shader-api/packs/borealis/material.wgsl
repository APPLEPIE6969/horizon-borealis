// The lit colour of each point of a block's surface, rewritten from Bliss's
// terrain and water shading (dimensions/all_solid.vsh, all_translucent.fsh,
// composite1.fsh) to Horizon's material stage: light from the lightmap curves,
// soft sun shadows with the colour of what they pass through, waving-water
// shading with planar reflections and refraction, emissive blocks, and the
// pack's own fog. The alpha test and blending stay Horizon's.
// Reference: docs/api/shaders-reference.md.

override reflections: bool = true;
override water_clarity: f32 = 0.5;

fn material(input: MaterialInput) -> Surface {
    var color = input.albedo.rgb * input.tint.rgb;
    var alpha = input.albedo.a;
    let view = normalize(input.position);
    let to_eye = -view;
    let light = direct_sun_color();
    let is_water = input.fluid == 1u && input.translucent;
    let is_lava = input.fluid == 2u;
    let is_glass = (input.tags & 1u) != 0u;

    // Non-overworld dimensions keep vanilla's light: the atmosphere does not
    // reach the Nether or the End.
    if frame.dimension != 0 {
        var lit = input.albedo.rgb * input.tint.rgb * input.lightmap;
        if is_lava {
            lit = input.albedo.rgb * input.tint.rgb * 2.4;
        }
        lit += input.albedo.rgb * input.emission * 2.5;
        lit = apply_fog(lit, input.position);
        return Surface(vec4<f32>(lit, alpha));
    }

    var smoothness = 0.0;
    var surface_normal = input.normal;

    if is_water {
        // The wave normal (or the flat one), the mirror and the sun's glint.
        if water_waves {
            surface_normal = normalize(mix(input.normal, water_wave_normal(input.position, input.normal, input.light.y), 0.85));
        }
        let ndv = max(dot(surface_normal, to_eye), 0.0);
        let fresnel = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);

        // The world mirrored about the water plane, sampled where the point
        // sits on the screen; the sky's colour stands in when it is off.
        var reflected = vec3<f32>(0.0);
        if reflections && frame.has_reflection == 1u {
            reflected = textureSampleLevel(reflection, reflection_sampler, input.screen, 0.0).rgb;
            reflected *= 0.35 + daylight() * 0.65;
        } else {
            reflected = fog_color_toward(view) * 1.2;
        }

        // Refraction: the world behind the surface, pulled by the wave normal,
        // with the depth guard keeping the sample behind the surface.
        var behind = vec3<f32>(0.0);
        let texel = vec2<i32>(input.screen * frame.resolution);
        if frame.in_water < 0.5 {
            let bend = select(vec2<f32>(0.0), surface_normal.xz * 0.5 / max(length(input.position), 1.0), water_waves);
            let probe = texel + vec2<i32>(bend * frame.resolution);
            let probe_depth = textureLoad(opaque_depth, clamp(probe, vec2<i32>(0), vec2<i32>(frame.resolution) - vec2<i32>(1)), 0);
            let own_depth = textureLoad(opaque_depth, texel, 0);
            if probe_depth <= own_depth {
                behind = textureLoad(opaque_color, probe, 0).rgb;
            } else {
                behind = textureLoad(opaque_color, texel, 0).rgb;
            }
        }

        // Water body: the vanilla tint over what lies behind, absorbed with
        // distance and the clarity option (Bliss's water colour, kept simple).
        let murk = 1.0 - water_clarity * 0.75;
        var body = mix(behind, behind * vec3<f32>(0.35, 0.66, 0.85), murk);
        let distance_absorb = 1.0 - exp(-length(input.position) * 0.015 * murk);
        body = mix(body, body * vec3<f32>(0.25, 0.5, 0.75), distance_absorb);
        body *= 0.35 + 0.65 * (input.light.y * 0.9 + input.light.x * 0.1);
        body *= input.tint.rgb * 1.1;

        var water_light = body * (ambient_light(input.light.y) * 1.4 + block_light(input.light.x, input.position));

        // The reflected sky and world, the sun's glint, and shadow on the body.
        let reach = shadow_reach(input.position, input.normal, input.tint.r, input.light.y, input.screen * frame.resolution);
        // A wider, brighter glint than terrain gets: water is a mirror.
        let glint = sun_glint(surface_normal, to_eye, 0.8, light * reach.rgb) * 2.2;
        water_light += reflected * fresnel + glint;
        water_light *= mix(1.0, reach.r, 0.55 * (1.0 - fresnel));
        water_light *= ambient_amount * 0.0 + 1.0; // ambient already folded in above

        // Seen from underwater, the surface is a dim ceiling.
        let from_below = select(1.0, 0.25, frame.in_water > 0.5);
        water_light *= from_below;

        // Bliss corrects alpha with fresnel: a= a + fresnel*(1-a).
        alpha = clamp(alpha + fresnel * (1.0 - alpha), 0.0, 1.0);
        color = water_light * (1.0 - fresnel * 0.35) + reflected * fresnel * 0.35;

        let fogged = apply_fog(color, input.position);
        return Surface(vec4<f32>(fogged, alpha));
    }

    // --- everything solid, cutout or translucent-but-not-water ---

    // The light this surface receives.
    var direct = light;
    var ambient = ambient_light(input.light.y);
    var block = block_light(input.light.x, input.position);

    // Sun shadows, with the tint of water or glass between the point and the
    // light. Lava needs none; it shines on its own.
    if !is_lava {
        let reach = shadow_reach(input.position, input.normal, input.tint.r, input.light.y, input.screen * frame.resolution);
        direct *= reach.r * reach.rgb;
    }

    // Leaves and plants let some light through and glow when backlit (Bliss's
    // subsurface scattering, folded into one term).
    let foliage = (input.material >= 10u && input.material <= 14u) || input.material == 31u;
    if foliage {
        let back = pow(max(dot(view, frame.light_direction), 0.0), 3.0);
        direct += vec3<f32>(0.55, 0.42, 0.2) * back * 0.55 * daylight() * (1.0 - frame.rain);
        ambient *= 1.08;
    }

    // Emissive blocks shine on their own.
    block += input.albedo.rgb * input.emission * 2.5;
    if is_lava {
        direct = vec3<f32>(0.0);
        ambient *= 0.3;
        block += input.albedo.rgb * 2.4;
    }

    // LabPBR smoothness, for the sun's glint and the post stage's record.
    if block_pbr_maps() {
        let spec = block_specular_map(input.uv);
        smoothness = spec.r;
        if spec.a > 0.0 && spec.a < 1.0 {
            block += input.albedo.rgb * (spec.a * 3.0);
        }
    } else {
        smoothness = select(0.0, 0.35, is_glass);
    }

    var lit = color * (ambient + block) + color * direct * max(dot(input.normal, frame.light_direction), 0.0);

    // The sun's reflection on smooth faces; glass gets a touch of sky sheen.
    lit += sun_glint(input.normal, to_eye, smoothness, direct);
    if is_glass {
        let ndv = max(dot(input.normal, to_eye), 0.0);
        lit += fog_color_toward(view) * pow(1.0 - ndv, 3.0) * 0.25;
    }

    // The record the final pass glints from, for solid terrain only.
    if !input.translucent {
        set_surface(input.normal, smoothness, 0.0);
    }

    lit = apply_fog(lit, input.position);
    return Surface(vec4<f32>(lit, alpha));
}
