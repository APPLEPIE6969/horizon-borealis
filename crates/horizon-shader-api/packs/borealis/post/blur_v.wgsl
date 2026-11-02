// The other half of the bloom's blur, along y, into the target the final
// grade reads.

fn post(input: PostInput) -> vec4<f32> {
    let step = 1.25 / frame.resolution.y;
    let offsets = array<f32, 4>(1.3846153846, 3.2307692308, 5.0769230769, 6.9230769231);
    let weights = array<f32, 5>(0.2270270270, 0.3162162162, 0.0702702703, 0.0123787222, 0.0010729596);
    var sum = textureSampleLevel(bloom2, linear_sampler, input.uv, 0.0).rgb * weights[0];
    for (var i = 0; i < 4; i++) {
        let off = vec2<f32>(0.0, offsets[i] * step);
        sum += textureSampleLevel(bloom2, linear_sampler, input.uv + off, 0.0).rgb * weights[i + 1];
        sum += textureSampleLevel(bloom2, linear_sampler, input.uv - off, 0.0).rgb * weights[i + 1];
    }
    return vec4<f32>(sum, 1.0);
}
