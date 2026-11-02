// The bloom's source: the bright pixels of the picture, written to the pack's
// half-resolution `bloom` target. The sky is bright by day, so it glows only a
// fraction as much, as in Bliss's bloom (dimensions/composite10.fsh).

fn post(input: PostInput) -> vec4<f32> {
    let c = input.color.rgb;
    let sky = select(1.0, 0.15, input.depth == 0.0);
    let weight = smoothstep(0.72, 1.15, luminance(c));
    return vec4<f32>(c * weight * sky, 1.0);
}
