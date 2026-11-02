// The colour of rain and snow streaks, blended over the world by their alpha.
// Horizon's light and fog don't apply to weather, so this picks its own from
// the light levels and the fog colour, the way Bliss tints its weather.
fn weather(input: WeatherInput) -> vec4<f32> {
    var color = input.color.rgb;

    // Lit by the sky (and a little by block light), darker in rain's own gloom.
    let sky = pow(input.light.y, 1.5);
    let block = pow(input.light.x, 2.0) * 0.6;
    let lit = 0.08 + sky * 0.85 + block;
    color *= lit * (1.0 - frame.rain * 0.25);

    if input.snow {
        color = mix(color, vec3<f32>(0.85, 0.9, 1.0), 0.4);
    }

    // Fade into the fog with distance, like every surface does.
    let distance = length(input.position);
    let fog = smoothstep(frame.fog_start, frame.fog_end, distance);
    color = mix(color, frame.fog_color, fog);

    return vec4<f32>(color, input.color.a);
}
