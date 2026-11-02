// Where each corner of a block's surface is drawn: waving plants and rolling
// water, from Bliss's waving code. Runs for the world and for the shadow map,
// so both move together. Reference: docs/api/shaders-reference.md.

fn vertex(input: VertexInput) -> vec3<f32> {
    var position = input.position;
    let world_pos = input.position + frame.camera_position;

    // Water rolls on its top surface; lava stays still (it glows instead).
    if input.fluid == 1u && input.face == 1u {
        position.y += water_rolling(world_pos);
    }

    // Plants and leaves wave; how much follows how lit the corner is, as
    // Bliss's use of the sky lightmap does. Material ids come from the manifest:
    // 10 leaves and azalea, 11-14 grass, crops and flowers, 21-22 vines and
    // glow lichen, 30 lily pads and dripleaves.
    let id = input.material;
    let sway = select(0.0, clamp(input.light.y * 0.9 + 0.1, 0.0, 1.0), id != 10u);
    if (id >= 10u && id <= 14u) || (id >= 21u && id <= 22u) || id == 30u || id == 31u {
        let w = select(leaves_wave(world_pos), plant_wave(world_pos), id != 10u && id != 31u);
        position += w * sway;
    }

    return position;
}
