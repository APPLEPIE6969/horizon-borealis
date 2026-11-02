//! The crafted worlds the gallery and the golden images render: a small
//! procedural block atlas and a handful of scenes (a plains at noon, a sunset
//! over water, a starry night, an underwater view, a shadow study), built from
//! the same vertex layout the renderer feeds Horizon's `[vertex]` and
//! `[material]` stages.

pub const ATLAS_COLS: u32 = 8;
pub const ATLAS_ROWS: u32 = 2;
pub const SPRITE: u32 = 16;

pub const GRASS_TOP: u32 = 0;
pub const GRASS_SIDE: u32 = 1;
pub const DIRT: u32 = 2;
pub const PLANKS: u32 = 3;
pub const GLASS: u32 = 4;
pub const LEAVES: u32 = 5;
pub const WATER: u32 = 6;
pub const STONE: u32 = 7;
pub const COBBLE: u32 = 8;
pub const LOG: u32 = 9;
pub const GLOWSTONE: u32 = 10;
pub const SAND: u32 = 11;
pub const SNOW: u32 = 12;

pub const LEAVES_MATERIAL: u32 = 10; // the manifest's id for *_leaves
pub const GLASS_TAG: u32 = 1 << 29;
pub const TRANSLUCENT_BIT: u32 = 1 << 30;
pub const FLUID_BIT: u32 = 1 << 31;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    /// Camera-relative position, in blocks.
    pub position: [f32; 3],
    /// Inside the sprite, 0..1.
    pub uv: [f32; 2],
    /// Block and sky light, 0..1.
    pub light: [f32; 2],
    /// Biome tint, face shade, ambient occlusion.
    pub tint: [f32; 4],
    /// Vanilla's light colour for this light this frame.
    pub lightmap: [f32; 3],
    /// The `[[materials]]` id (0 none).
    pub material: u32,
    /// face (0..2 bits) | glass tag 29 | translucent 30 | fluid 31 | emission 20..24 (0..15).
    pub flags: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SkyParams {
    /// The time of day, 0..1: 0 sunrise, 0.25 noon, 0.5 sunset, 0.75 midnight.
    pub day_time: f32,
    pub rain: f32,
    pub in_water: f32,
    pub water_plane: f32,
}

pub struct Scene {
    pub name: &'static str,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub atlas: Vec<u8>,
    pub sky: SkyParams,
    /// The camera: eye and target, world blocks.
    pub eye: [f32; 3],
    pub target: [f32; 3],
    /// Reflection experiment mode: 0 none (the shipped default), 1 the game
    /// reports a mirror whose buffer holds garbage where its pass did not
    /// draw, 2 a fully drawn mirror.
    pub reflection_mode: u8,
}

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    // Thomas Wang style mix, deterministic across runs.
    let mut h = x
        .wrapping_mul(374761393)
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(2147483647));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

fn sprite_pixel(sprite: u32, x: u32, y: u32) -> [f32; 4] {
    let n = hash(x, y, sprite + 7);
    let n2 = hash(x.wrapping_mul(3), y.wrapping_mul(5), sprite + 91);
    match sprite {
        GRASS_TOP => {
            let v = 0.86 + n * 0.24;
            [0.34 * v, 0.52 * v, 0.22 * v, 1.0]
        }
        GRASS_SIDE => {
            if y < 4 {
                let v = 0.85 + n * 0.25;
                [0.34 * v, 0.52 * v, 0.22 * v, 1.0]
            } else {
                let v = 0.85 + n2 * 0.25;
                [0.44 * v, 0.31 * v, 0.20 * v, 1.0]
            }
        }
        DIRT => {
            let v = 0.85 + n * 0.25;
            [0.45 * v, 0.32 * v, 0.21 * v, 1.0]
        }
        PLANKS => {
            let line = (y % 5 == 0) as u32;
            let v = (0.9 + n * 0.15) * (1.0 - line as f32 * 0.22);
            [0.62 * v, 0.47 * v, 0.28 * v, 1.0]
        }
        GLASS => {
            let edge = x == 0 || y == 0 || x == 15 || y == 15;
            let inner = x == 1 || y == 1 || x == 14 || y == 14;
            if edge {
                [0.85, 0.92, 0.95, 1.0]
            } else if inner {
                [0.88, 0.95, 0.98, 0.35]
            } else {
                [0.7, 0.85, 0.9, 0.08]
            }
        }
        LEAVES => {
            if n < 0.22 {
                [0.0, 0.0, 0.0, 0.0] // cutout holes
            } else {
                let v = 0.8 + n2 * 0.3;
                [0.25 * v, 0.42 * v, 0.15 * v, 1.0]
            }
        }
        WATER => {
            let v = 0.92 + n * 0.12;
            [0.22 * v, 0.40 * v, 0.72 * v, 0.75]
        }
        STONE => {
            let v = 0.9 + n * 0.18;
            [0.44 * v, 0.44 * v, 0.46 * v, 1.0]
        }
        COBBLE => {
            let blob = hash(x / 4, y / 4, sprite + 3);
            let v = 0.8 + blob * 0.3 + n * 0.1;
            [0.45 * v, 0.45 * v, 0.46 * v, 1.0]
        }
        LOG => {
            let stripe = hash(x, 0, sprite + 5);
            let v = 0.85 + stripe * 0.2 + n * 0.06;
            [0.42 * v, 0.32 * v, 0.19 * v, 1.0]
        }
        GLOWSTONE => {
            let blotch = hash(x / 3, y / 3, sprite + 13);
            let v = 0.75 + blotch * 0.35;
            [1.0 * v, 0.88 * v, 0.55 * v, 1.0]
        }
        SAND => {
            let v = 0.94 + n * 0.1;
            [0.83 * v, 0.77 * v, 0.56 * v, 1.0]
        }
        SNOW => {
            let v = 0.96 + n * 0.04;
            [v, v, v * 1.02, 1.0]
        }
        _ => [1.0, 0.0, 1.0, 1.0],
    }
}

pub fn build_atlas() -> Vec<u8> {
    let mut atlas = vec![0u8; (ATLAS_COLS * ATLAS_ROWS * SPRITE * SPRITE * 4) as usize];
    for sprite in 0..ATLAS_COLS * ATLAS_ROWS {
        let sx = (sprite % ATLAS_COLS) * SPRITE;
        let sy = (sprite / ATLAS_COLS) * SPRITE;
        for y in 0..SPRITE {
            for x in 0..SPRITE {
                let px = sprite_pixel(sprite, x, y);
                let i = (((sy + y) * ATLAS_COLS * SPRITE + sx + x) * 4) as usize;
                atlas[i] = (px[0].clamp(0.0, 1.0) * 255.0) as u8;
                atlas[i + 1] = (px[1].clamp(0.0, 1.0) * 255.0) as u8;
                atlas[i + 2] = (px[2].clamp(0.0, 1.0) * 255.0) as u8;
                atlas[i + 3] = (px[3].clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    }
    atlas
}

/// Sprite uv for a corner: the atlas is one strip of sprites.
fn sprite_uv(sprite: u32, u: f32, v: f32) -> [f32; 2] {
    let sx = (sprite % ATLAS_COLS) as f32;
    let sy = (sprite / ATLAS_COLS) as f32;
    // Half a texel in from each edge: quad corners land on texel centres, so
    // nothing bleeds across into the sprite beside it.
    let w = ATLAS_COLS * SPRITE;
    let h = ATLAS_ROWS * SPRITE;
    [
        (sx * SPRITE as f32 + 0.5 + u * (SPRITE - 1) as f32) / w as f32,
        (sy * SPRITE as f32 + 0.5 + v * (SPRITE - 1) as f32) / h as f32,
    ]
}

/// Vanilla's face shades: down, up, north, south, west, east.
fn face_shade(face: u32) -> f32 {
    match face {
        0 => 0.5,
        1 => 1.0,
        2 | 3 => 0.8,
        _ => 0.6,
    }
}

fn day_factor(day_time: f32) -> f32 {
    // The sun's up-ness, 0..1, with smooth dusk and dawn.
    ((day_time * std::f32::consts::TAU).sin() * 2.0).clamp(0.0, 1.0)
}

fn vanilla_lightmap(block: f32, sky: f32, day: f32) -> [f32; 3] {
    let sky_l = 0.04 + 0.96 * sky * (0.25 + 0.75 * day);
    let block_l = 0.05 + 0.95 * block.powf(1.15);
    let sky_col = [sky_l; 3];
    let block_col = [block_l, block_l * 0.8, block_l * 0.6];
    let mut out = [0.0f32; 3];
    for i in 0..3 {
        out[i] = sky_col[i].max(block_col[i]);
    }
    out
}

#[allow(clippy::too_many_arguments)]
struct Builder {
    name: &'static str,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    sky: SkyParams,
    eye: [f32; 3],
    target: [f32; 3],
}

impl Builder {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            vertices: Vec::new(),
            indices: Vec::new(),
            sky: SkyParams::default(),
            eye: [0.0; 3],
            target: [0.0; 3],
        }
    }

    fn day(&self) -> f32 {
        day_factor(self.sky.day_time)
    }

    /// One quad: corners in world blocks, sprite, face (0 down, 1 up, 2 north,
    /// 3 south, 4 west, 5 east), lights per corner, material id, flags.
    #[allow(clippy::too_many_arguments)]
    fn quad(
        &mut self,
        corners: [[f32; 3]; 4],
        sprite: u32,
        face: u32,
        lights: [[f32; 2]; 4],
        ao: [f32; 4],
        material: u32,
        mut flags: u32,
        emission: f32,
    ) {
        let shade = face_shade(face);
        let day = self.day();
        for i in 0..4 {
            let (u, v) = match i {
                0 => (0.0, 0.0),
                1 => (1.0, 0.0),
                2 => (1.0, 1.0),
                _ => (0.0, 1.0),
            };
            let l = lights[i];
            self.vertices.push(Vertex {
                position: corners[i],
                uv: sprite_uv(sprite, u, v),
                light: l,
                tint: [1.0, 1.0, 1.0, 1.0],
                lightmap: vanilla_lightmap(l[0], l[1], day),
                material,
                flags: flags | (((emission * 15.0).round() as u32) << 20),
            });
            let _ = shade;
            let _ = &mut flags;
        }
        let b = self.vertices.len() as u32 - 4;
        self.indices.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
        // Face shade and AO fold into the tint, as vanilla's vertex colour does.
        let start = self.vertices.len() - 4;
        for i in 0..4 {
            self.vertices[start + i].tint = [1.0, 1.0, 1.0, 1.0].map(|c| c * shade * ao[i]);
        }
    }

    /// A full block's visible faces (only the given faces are emitted).
    #[allow(clippy::too_many_arguments)]
    fn block(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        sprite: u32,
        faces: &[u32],
        light: [f32; 2],
        ao: f32,
        material: u32,
        emission: f32,
        flags: u32,
    ) {
        let (x, y, z) = (x as f32, y as f32, z as f32);
        let a = [ao; 4];
        for &face in faces {
            let c = match face {
                0 => [
                    [x, y, z],
                    [x + 1.0, y, z],
                    [x + 1.0, y, z + 1.0],
                    [x, y, z + 1.0],
                ],
                1 => [
                    [x, y + 1.0, z],
                    [x + 1.0, y + 1.0, z],
                    [x + 1.0, y + 1.0, z + 1.0],
                    [x, y + 1.0, z + 1.0],
                ],
                // North (-z) and south (+z) walls: vertical in y.
                2 => [
                    [x, y, z],
                    [x + 1.0, y, z],
                    [x + 1.0, y + 1.0, z],
                    [x, y + 1.0, z],
                ],
                3 => [
                    [x, y, z + 1.0],
                    [x + 1.0, y, z + 1.0],
                    [x + 1.0, y + 1.0, z + 1.0],
                    [x, y + 1.0, z + 1.0],
                ],
                4 => [
                    [x, y, z],
                    [x, y + 1.0, z],
                    [x, y + 1.0, z + 1.0],
                    [x, y, z + 1.0],
                ],
                _ => [
                    [x + 1.0, y, z + 1.0],
                    [x + 1.0, y + 1.0, z + 1.0],
                    [x + 1.0, y + 1.0, z],
                    [x + 1.0, y, z],
                ],
            };
            self.quad(c, sprite, face, [light; 4], a, material, flags, emission);
        }
    }

    fn finish(self) -> Scene {
        Scene {
            name: self.name,
            vertices: self.vertices,
            indices: self.indices,
            atlas: build_atlas(),
            sky: self.sky,
            eye: self.eye,
            target: self.target,
            reflection_mode: 0,
        }
    }
}

/// The shared world: ground, a hill, a small house with glass windows and
/// glowstone lamps, a pond, two trees, a stone pillar. Returns it with the
/// camera and sky the scene asks for.
pub fn world(name: &'static str, sky: SkyParams, eye: [f32; 3], target: [f32; 3]) -> Scene {
    let mut b = Builder::new(name);
    b.sky = sky;
    b.eye = eye;
    b.target = target;
    let bright = [0.0, 1.0];
    let indoor = [1.0, 0.25];
    let dim = [0.0, 0.55];

    // The ground, 24 by 24, grass on dirt; a sand corner and a snowy one.
    for x in -12..12 {
        for z in -12..12 {
            if (4..9).contains(&x) && (-9..-4).contains(&z) {
                continue; // the pond's hole
            }
            let (sprite, side) = if x >= 6 && z >= 6 {
                (SAND, SAND)
            } else if x <= -7 && z <= -7 {
                (SNOW, SNOW)
            } else {
                (GRASS_TOP, GRASS_SIDE)
            };
            let raised = ((-3..=1).contains(&x) && (-3..=1).contains(&z)) as i32; // the hill the house sits on
            let top = raised;
            b.block(x, top - 1, z, sprite, &[1], bright, 1.0, 0, 0.0, 0);
            if top == 0 {
                // Sides at the world's rim only, so the ground reads as a slab.
                if x == -12 {
                    b.block(x, -1, z, side, &[5], dim, 1.0, 0, 0.0, 0);
                }
                if x == 11 {
                    b.block(x, -1, z, side, &[4], dim, 1.0, 0, 0.0, 0);
                }
                if z == -12 {
                    b.block(x, -1, z, side, &[3], dim, 1.0, 0, 0.0, 0);
                }
                if z == 11 {
                    b.block(x, -1, z, side, &[2], dim, 1.0, 0, 0.0, 0);
                }
            } else {
                // The hill's sides.
                b.block(x, top - 1, z, side, &[0, 2, 3, 4, 5], dim, 0.9, 0, 0.0, 0);
            }
        }
    }

    // The pond, dug into the ground's south-east: water flush with the
    // ground, over a dirt bottom. The ground loop skips these columns.
    for x in 4..9 {
        for z in -9..-4 {
            b.block(x, -2, z, DIRT, &[1], bright, 1.0, 0, 0.0, 0);
            b.block(
                x,
                -1,
                z,
                WATER,
                &[1],
                bright,
                1.0,
                0,
                0.0,
                TRANSLUCENT_BIT | FLUID_BIT,
            );
        }
    }
    for x in 4..9 {
        b.block(x, 1, -10, COBBLE, &[0, 1, 2, 3], bright, 1.0, 0, 0.0, 0);
    }

    // The house: planks on the hill, glass windows, a glowstone lamp inside.
    for x in -2..2 {
        for z in -2..2 {
            b.block(x, 0, z, PLANKS, &[1], indoor, 0.95, 0, 0.0, 0);
        }
    }
    // Walls with windows (glass on the second row).
    for y in 1..4 {
        for x in -2..2 {
            for &z in &[-3i32, 2] {
                let glass = y == 2 && (x == -1 || x == 0);
                let sprite = if glass { GLASS } else { PLANKS };
                let flags = if glass { GLASS_TAG } else { 0 };
                b.block(
                    x,
                    y,
                    z,
                    sprite,
                    &[0, 1, 2, 3, 4, 5],
                    indoor,
                    0.95,
                    0,
                    0.0,
                    flags,
                );
            }
            for &x2 in &[-3i32, 2] {
                for z2 in -2..2 {
                    let glass = y == 2 && (z2 == -1 || z2 == 0);
                    let sprite = if glass { GLASS } else { PLANKS };
                    let flags = if glass { GLASS_TAG } else { 0 };
                    b.block(
                        x2,
                        y,
                        z2,
                        sprite,
                        &[0, 1, 2, 3, 4, 5],
                        indoor,
                        0.95,
                        0,
                        0.0,
                        flags,
                    );
                }
            }
        }
    }
    // The roof.
    for x in -3..3 {
        for z in -3..3 {
            b.block(x, 4, z, LOG, &[0, 1], indoor, 0.95, 0, 0.0, 0);
        }
    }
    // The lamp inside, and one on the roof.
    b.block(
        -1,
        1,
        -1,
        GLOWSTONE,
        &[1, 0, 2, 3, 4, 5],
        indoor,
        1.0,
        0,
        1.0,
        0,
    );
    b.block(
        3,
        5,
        3,
        GLOWSTONE,
        &[0, 1, 2, 3, 4, 5],
        bright,
        1.0,
        0,
        1.0,
        0,
    );

    // The stone pillar, for the shadow map.
    for y in 1..6 {
        b.block(-8, y, 4, STONE, &[0, 1, 2, 3, 4, 5], bright, 1.0, 0, 0.0, 0);
    }

    // Two oak trees.
    for (tx, tz) in [(-9i32, -6i32), (7i32, 2i32)] {
        for y in 0..4 {
            b.block(tx, y, tz, LOG, &[0, 1, 2, 3, 4, 5], bright, 1.0, 0, 0.0, 0);
        }
        for dx in -2i32..3 {
            for dy in 3..6 {
                for dz in -2i32..3 {
                    let edge = dx.abs() == 2 || dz.abs() == 2;
                    let top = dy == 5;
                    if edge && (top || dx.abs() + dz.abs() == 4) {
                        continue;
                    }
                    b.block(
                        tx + dx,
                        dy,
                        tz + dz,
                        LEAVES,
                        &[0, 1, 2, 3, 4, 5],
                        bright,
                        0.9,
                        LEAVES_MATERIAL,
                        0.0,
                        0,
                    );
                }
            }
        }
    }

    b.finish()
}

pub fn noon_plains() -> Scene {
    world(
        "noon",
        SkyParams {
            day_time: 0.25,
            ..Default::default()
        },
        [10.0, 7.5, 13.0],
        [-1.0, 2.0, -1.0],
    )
}

/// A new world starts at sunrise, so the first minutes of play run the sun at
/// a quarter of the way up the sky — the regime the first shipped screenshots
/// were taken in. The sky's low-sun behaviour and the grade's exposure are
/// watched here.
pub fn dawn() -> Scene {
    world(
        "dawn",
        SkyParams {
            day_time: 0.045,
            ..Default::default()
        },
        [10.0, 7.5, 13.0],
        [-1.0, 2.0, -1.0],
    )
}

/// Open water to the horizon at dawn — the far-field regime the shared world
/// is too small to reach, and where a game mirror pass that drew only the
/// near field would show. Used by the reflection probes and the
/// garbage-mirror regression test; not a golden.
pub fn dawn_ocean() -> Scene {
    let mut b = Builder::new("dawn-ocean");
    b.sky = SkyParams {
        day_time: 0.045,
        water_plane: 0.0,
        ..Default::default()
    };
    b.eye = [0.0, 3.0, 30.0];
    b.target = [0.0, 0.5, -30.0];
    // The ocean in 16-block tiles: a single quad would stretch one sprite
    // across the whole plane. Face 1 (up) only, the same fluid flags the
    // pond's water uses.
    let light = [0.0, 1.0];
    for x in -10..10 {
        for z in -10..10 {
            let (x0, z0) = ((x * 16) as f32, (z * 16) as f32);
            b.quad(
                [
                    [x0, 0.0, z0],
                    [x0 + 16.0, 0.0, z0],
                    [x0 + 16.0, 0.0, z0 + 16.0],
                    [x0, 0.0, z0 + 16.0],
                ],
                WATER,
                1,
                [light; 4],
                [1.0; 4],
                0,
                TRANSLUCENT_BIT | FLUID_BIT,
                0.0,
            );
        }
    }
    // A sand spit for orientation, so the near field is not empty.
    for x in -2..3 {
        for z in 18..23 {
            b.block(x, -1, z, SAND, &[1], light, 1.0, 0, 0.0, 0);
        }
    }
    b.finish()
}

pub fn sunset_water() -> Scene {
    world(
        "sunset",
        SkyParams {
            day_time: 0.475,
            water_plane: 0.0,
            ..Default::default()
        },
        [16.0, 3.0, -7.0],
        [-8.0, 1.2, -6.0],
    )
}

pub fn night() -> Scene {
    world(
        "night",
        SkyParams {
            day_time: 0.75,
            ..Default::default()
        },
        [12.0, 6.0, 10.0],
        [-1.0, 3.0, -1.0],
    )
}

pub fn underwater() -> Scene {
    let mut s = world(
        "underwater",
        SkyParams {
            day_time: 0.3,
            water_plane: 0.0,
            ..Default::default()
        },
        [6.5, -0.45, -6.5],
        [1.0, 0.9, -2.5],
    );
    s.sky.in_water = 1.0;
    s
}

pub fn shadows() -> Scene {
    world(
        "shadows",
        SkyParams {
            day_time: 0.42,
            ..Default::default()
        },
        [-2.0, 4.5, 11.0],
        [-8.0, 1.0, 4.0],
    )
}

pub fn rain() -> Scene {
    let mut s = world(
        "rain",
        SkyParams {
            day_time: 0.32,
            water_plane: 0.0,
            ..Default::default()
        },
        [10.0, 7.5, 13.0],
        [-1.0, 2.0, -1.0],
    );
    s.sky.rain = 0.85;
    s
}
