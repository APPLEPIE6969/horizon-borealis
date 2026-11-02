//! Renders a Horizon shader pack headless (wgpu) the way the game does: the
//! pack's stages are composed with the same `compose` the game and the check
//! use, driven through crafted scenes, and read back as PNGs. The goldens and
//! the gallery of `crates/horizon-shader-api/packs` come from here.

use crate::mat4::{
    invert, mul, mul_vec, orthographic_shadow, perspective_reversed, shadow_view, view,
};
use crate::scene::{Scene, Vertex};
use horizon_shader_api::{Pack, StageKind, compose};
use std::path::Path;
use wgpu::util::DeviceExt;

pub const WORLD_W: u32 = 480;
pub const WORLD_H: u32 = 270;
pub const SHADOW_RES: u32 = 1024;
pub const SHADOW_DISTANCE: f32 = 64.0;
pub const FAR: f32 = 160.0;

// The harness's own entries, appended after the pack's composed stage. They
// mirror the renderer's native_material: the atlas sampled in the fragment,
// the pack's `material` called with the same inputs, a second output carrying
// `set_surface`'s record, and vertex entries that call the pack's `vertex`.
const HARNESS_TERRAIN: &str = r#"
struct HarnessTerrainIn {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) light: vec2<f32>,
    @location(2) tint: vec4<f32>,
    @location(3) lightmap: vec3<f32>,
    @location(4) world: vec3<f32>,
    @location(5) @interpolate(flat) flags: u32,
    @location(6) @interpolate(flat) material: u32,
}
struct HarnessTerrainVertOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) light: vec2<f32>,
    @location(2) tint: vec4<f32>,
    @location(3) lightmap: vec3<f32>,
    @location(4) world: vec3<f32>,
    @location(5) @interpolate(flat) flags: u32,
    @location(6) @interpolate(flat) material: u32,
}
@group(0) @binding(0) var harness_atlas: texture_2d<f32>;
@group(0) @binding(1) var harness_atlas_sampler: sampler;

@vertex
fn harness_vs(
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) light: vec2<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) lightmap: vec3<f32>,
    @location(5) material: u32,
    @location(6) flags: u32,
) -> HarnessTerrainVertOut {
    let rel = position - frame.camera_position;
    let input = VertexInput(rel, uv, material, light, tint, flags & 7u, flags >> 31u);
    let moved = vertex(input);
    var out: HarnessTerrainVertOut;
    out.clip = frame.projection * frame.view * vec4<f32>(moved, 1.0);
    out.uv = uv;
    out.light = light;
    out.tint = tint;
    out.lightmap = lightmap;
    out.world = moved;
    out.flags = flags;
    out.material = material;
    return out;
}

@vertex
fn harness_vs_shadow(
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) light: vec2<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) lightmap: vec3<f32>,
    @location(5) material: u32,
    @location(6) flags: u32,
) -> @builtin(position) vec4<f32> {
    let rel = position - frame.camera_position;
    let input = VertexInput(rel, uv, material, light, tint, flags & 7u, flags >> 31u);
    let moved = vertex(input);
    return frame.shadow_view_proj * vec4<f32>(moved, 1.0);
}

@vertex
fn harness_vs_shadow_full(
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) light: vec2<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) lightmap: vec3<f32>,
    @location(5) material: u32,
    @location(6) flags: u32,
) -> HarnessTerrainVertOut {
    let rel = position - frame.camera_position;
    let input = VertexInput(rel, uv, material, light, tint, flags & 7u, flags >> 31u);
    let moved = vertex(input);
    var out: HarnessTerrainVertOut;
    out.clip = frame.shadow_view_proj * vec4<f32>(moved, 1.0);
    out.uv = uv;
    out.light = light;
    out.tint = tint;
    out.lightmap = lightmap;
    out.world = moved;
    out.flags = flags;
    out.material = material;
    return out;
}

// The translucent shadow's colour: the texture times the vertex colour, alpha
// 1 on a fluid and the texture's alpha (at most 0.99) elsewhere, as the
// reference's `shadow_color` says.
@fragment
fn harness_fs_shadow_color(in: HarnessTerrainIn) -> @location(0) vec4<f32> {
    let albedo = textureSampleLevel(harness_atlas, harness_atlas_sampler, in.uv, 0.0);
    if albedo.a < 0.5 {
        discard;
    }
    let fluid = (in.flags >> 31u) != 0u;
    let a = select(min(albedo.a, 0.99), 1.0, fluid);
    return vec4<f32>(albedo.rgb * in.tint.rgb, a);
}

fn harness_material(in: HarnessTerrainIn, translucent: bool) -> vec4<f32> {
    let albedo = textureSampleLevel(harness_atlas, harness_atlas_sampler, in.uv, 0.0);
    horizon_block_layer = 0u;
    horizon_block_lod = 0.0;
    let fluid = select(0u, select(2u, 1u, (in.flags >> 30u) != 0u), (in.flags >> 31u) != 0u);
    let normal = horizon_material_normal(in.world);
    let tangent = horizon_material_tangent(in.world, in.uv, normal);
    return material(MaterialInput(
        albedo, in.tint, in.light, in.lightmap, in.world, in.uv, translucent,
        (in.flags >> 29u) & 1u, in.material, in.clip.xy / frame.resolution,
        in.flags & 7u, fluid, f32((in.flags >> 20u) & 15u) / 15.0, normal, tangent,
    )).color;
}

struct HarnessSurfaceOut {
    @location(0) color: vec4<f32>,
    @location(1) surface: vec4<f32>,
}

@fragment
fn harness_fs_solid(in: HarnessTerrainIn) -> HarnessSurfaceOut {
    var color = harness_material(in, false);
    if color.a < 0.5 {
        discard;
    }
    if horizon_surface.w == 0.0 {
        set_surface(horizon_material_normal(in.world), 0.0, 0.0);
    }
    var out: HarnessSurfaceOut;
    out.color = vec4<f32>(color.rgb, 1.0);
    out.surface = vec4<f32>(horizon_surface.xyz, in.clip.z);
    return out;
}

@fragment
fn harness_fs_translucent(in: HarnessTerrainIn) -> @location(0) vec4<f32> {
    let color = harness_material(in, true);
    if color.a < 0.01 {
        discard;
    }
    return vec4<f32>(color.rgb, color.a);
}
"#;

// The harness's sky: a full-screen ray, vanilla's own sky colour along it, and
// the pack's `sky` called on the result.
const HARNESS_SKY: &str = r#"
fn harness_vanilla_sky(d: vec3<f32>) -> vec4<f32> {
    let up = clamp(d.y, 0.0, 1.0);
    let horizon = pow(1.0 - up, 3.0);
    var c = mix(frame.sky_color, frame.fog_color, horizon);
    let low = 1.0 - smoothstep(0.0, 0.45, abs(frame.sun_direction.y));
    let flat_d = normalize(vec3<f32>(d.x, 0.0, d.z) + vec3<f32>(1e-5));
    let flat_s = normalize(vec3<f32>(frame.sun_direction.x, 0.0, frame.sun_direction.z) + vec3<f32>(1e-5));
    let toward = max(dot(flat_d, flat_s), 0.0);
    c = mix(c, vec3<f32>(0.98, 0.5, 0.2), low * horizon * pow(toward, 2.0) * 0.55);
    return vec4<f32>(c, 1.0);
}

struct HarnessSkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ray_uv: vec2<f32>,
}
@vertex
fn harness_vs_sky(@builtin(vertex_index) i: u32) -> HarnessSkyOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: HarnessSkyOut;
    out.clip = vec4<f32>(p * 2.0 - 1.0, 0.9999, 1.0);
    out.ray_uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}
@fragment
fn harness_fs_sky(in: HarnessSkyOut) -> @location(0) vec4<f32> {
    let ndc = vec4<f32>(in.ray_uv.x * 2.0 - 1.0, 1.0 - in.ray_uv.y * 2.0, 1.0, 1.0);
    let view = frame.projection_inverse * ndc;
    let ray = normalize((frame.view_inverse * vec4<f32>(view.xyz, 0.0)).xyz);
    return sky(SkyInput(ray, harness_vanilla_sky(ray)));
}
"#;

const TERRAIN_ATTRIBUTES: [wgpu::VertexAttribute; 7] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 12,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 20,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 28,
        shader_location: 3,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 44,
        shader_location: 4,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Uint32,
        offset: 56,
        shader_location: 5,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Uint32,
        offset: 60,
        shader_location: 6,
    },
];

const ATLAS_LAYOUT: wgpu::BindGroupLayoutDescriptor<'static> = wgpu::BindGroupLayoutDescriptor {
    label: Some("atlas-layout"),
    entries: &[
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ],
};

fn texture_entry(
    binding: u32,
    sample_type: wgpu::TextureSampleType,
    dimension: wgpu::TextureViewDimension,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: dimension,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32, filtering: bool, comparison: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(if comparison {
            wgpu::SamplerBindingType::Comparison
        } else if filtering {
            wgpu::SamplerBindingType::Filtering
        } else {
            wgpu::SamplerBindingType::NonFiltering
        }),
        count: None,
    }
}

fn frame_entry(visibility: wgpu::ShaderStages, size: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size),
        },
        count: None,
    }
}

/// A pack loaded onto the GPU: its composed stage modules and the pipelines
/// the harness drives them with.
pub struct PackGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub pack: Pack,
    terrain_solid: wgpu::RenderPipeline,
    terrain_translucent: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    shadow_color: wgpu::RenderPipeline,
    atlas_group_layout: wgpu::BindGroupLayout,
    terrain_group_layout: wgpu::BindGroupLayout,
    shadow_group_layout: wgpu::BindGroupLayout,
    sky_group_layout: Option<wgpu::BindGroupLayout>,
    post_group_layouts: Vec<wgpu::BindGroupLayout>,
    sky_pipeline: Option<(wgpu::RenderPipeline, wgpu::BindGroupLayout)>,
    post_pipelines: Vec<(wgpu::RenderPipeline, String, Option<(String, f32)>)>,
    has_reflection_read: bool,
    pub shadow_res: u32,
    pub shadow_distance: f32,
}

struct Plane {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

impl Plane {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        label: &str,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            size,
        }
    }
}

pub fn frame_from_scene(scene: &Scene, w: f32, h: f32) -> horizon_shader_api::Frame {
    use horizon_shader_api::Frame;
    let mut f = Frame::default();
    let day_time = scene.sky.day_time;
    let theta = day_time * std::f32::consts::TAU;
    let sun = [theta.cos(), theta.sin(), 0.0];
    let moon = [-sun[0], -sun[1], 0.0];
    let elevation = sun[1];

    // Vanilla-ish sky and fog colours over the day, grayer in rain.
    let day = elevation.clamp(0.0, 1.0);
    let low = (1.0 - (elevation.abs() * 2.5).min(1.0)) * if elevation > -0.2 { 1.0 } else { 0.0 };
    let mut sky_c = mix3([0.011, 0.016, 0.035], [0.42, 0.62, 0.95], day);
    let mut fog_c = mix3([0.013, 0.016, 0.035], [0.70, 0.79, 0.95], day);
    let sunset = [0.95, 0.52, 0.28];
    sky_c = mix3(sky_c, sunset, low * 0.55);
    fog_c = mix3(fog_c, sunset, low * 0.65);
    let gray = mix3([0.42, 0.44, 0.47], [0.5, 0.52, 0.55], day);
    sky_c = mix3(sky_c, gray, scene.sky.rain * 0.75);
    fog_c = mix3(fog_c, gray, scene.sky.rain * 0.75);
    if scene.sky.in_water > 0.5 {
        sky_c = [0.05, 0.13, 0.19];
        fog_c = [0.04, 0.11, 0.16];
    }

    f.view = view(scene.eye, scene.target);
    f.projection = perspective_reversed(70f32.to_radians(), w / h, 0.05, FAR);
    f.view_inverse = invert(&f.view);
    f.projection_inverse = invert(&f.projection);
    f.camera_position = scene.eye;
    f.time = 12.0; // a fixed time so waves are deterministic
    f.sun_direction = sun;
    f.day_time = day_time;
    f.moon_direction = moon;
    f.game_time = day_time;
    f.sky_color = sky_c;
    f.rain = scene.sky.rain;
    f.fog_color = fog_c;
    f.thunder = 0.0;
    f.resolution = [w, h];
    f.eye_light = [
        if scene.sky.in_water > 0.5 { 0.0 } else { 0.15 },
        if scene.sky.in_water > 0.5 { 0.6 } else { 1.0 },
    ];
    f.fog_start = 72.0;
    f.fog_end = 96.0;
    f.render_distance = 96.0;
    f.far = FAR;
    f.night_vision = 0.0;
    f.blindness = 0.0;
    f.darkness = 0.0;
    f.brightness = 0.5;
    f.eye_in = if scene.sky.in_water > 0.5 { 1 } else { 0 };
    f.dimension = 0;
    f.frame_index = 0;
    f.hide_gui = 0;
    f.precipitation = if scene.sky.rain > 0.5 { 1 } else { 0 };
    f.temperature = 0.8;
    f.downfall = 0.5;
    f.wetness = scene.sky.rain;
    f.previous_view_projection = mul(&f.projection, &f.view);
    f.camera_motion = [0.0; 3];
    f.delta_time = 0.016;
    f.eye_light_smooth = f.eye_light;
    f.in_water = scene.sky.in_water;
    f.in_lava = 0.0;
    f.water_plane = scene.sky.water_plane;
    f.has_reflection = 0;
    f.reflection_resolution = [w * 0.5, h * 0.5];
    f.mods = Default::default();
    let light = if elevation > 0.0 { sun } else { moon };
    f.light_direction = light;
    f.has_shadow = 1;
    let lift = SHADOW_DISTANCE;
    let shadow_view_m = shadow_view(light, lift);
    let ortho = orthographic_shadow(SHADOW_DISTANCE, SHADOW_DISTANCE * 2.0);
    f.shadow_view_proj = mul(&ortho, &shadow_view_m);
    f.shadow_distance = SHADOW_DISTANCE;
    f.shadow_distortion = 0.0;
    f.shadow_resolution = SHADOW_RES as f32;
    f.shadow_center_texel = (SHADOW_DISTANCE * 2.0) / SHADOW_RES as f32;
    f
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn frame_size() -> u64 {
    std::mem::size_of::<horizon_shader_api::Frame>() as u64
}

impl PackGpu {
    /// Loads the pack at `path` onto a (possibly software) GPU.
    pub fn load(path: &Path) -> Self {
        let pack = Pack::read(path)
            .unwrap_or_else(|e| panic!("pack at {} unreadable: {e}", path.display()));
        Self::from_pack(pack)
    }

    pub fn from_pack(pack: Pack) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("no GPU adapter (set VK_ICD_FILENAMES to a software one for CI)");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).expect("no GPU device");

        let module = |kind: StageKind, file: &str, extra: &str| -> wgpu::ShaderModule {
            let composed = compose(&pack, kind, file);
            let mut text = composed.text;
            text.push_str(extra);
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&format!("{kind:?}:{file}")),
                source: wgpu::ShaderSource::Wgsl(text.into()),
            })
        };

        // A pack without a `[vertex]` stage — the shape a released game takes,
        // since that stage ships in no release yet — still has to give the
        // harness's terrain entry points their `fn vertex`: give them the
        // identity, which is what the game's own undisplaced path draws.
        let terrain_extra: std::borrow::Cow<'static, str> = if pack.manifest.vertex.is_some() {
            std::borrow::Cow::Borrowed(HARNESS_TERRAIN)
        } else {
            std::borrow::Cow::Owned(format!(
                "fn vertex(input: VertexInput) -> vec3<f32> {{\n    return input.position;\n}}\n\n{HARNESS_TERRAIN}"
            ))
        };
        let terrain_module = module(StageKind::Material, "material.wgsl", &terrain_extra);
        let sky_module = pack
            .manifest
            .sky
            .as_ref()
            .map(|s| module(StageKind::Sky, &s.file, HARNESS_SKY));
        let post_modules: Vec<(String, wgpu::ShaderModule)> = pack
            .manifest
            .post
            .iter()
            .map(|p| (p.file.clone(), module(StageKind::Post, &p.file, "")))
            .collect();

        let vertex_buffers = || -> [Option<wgpu::VertexBufferLayout<'static>>; 1] {
            [Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &TERRAIN_ATTRIBUTES,
            })]
        };

        let atlas_group_layout = device.create_bind_group_layout(&ATLAS_LAYOUT);
        let terrain_group_layout = terrain_group_layout_of(&device);
        let shadow_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shadow-group-layout"),
                entries: &[
                    frame_entry(wgpu::ShaderStages::VERTEX, frame_size()),
                    wgpu::BindGroupLayoutEntry {
                        // The waving vertex stage samples the noise.
                        binding: 16,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 24,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let atlas_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("atlas-pipeline-layout"),
                bind_group_layouts: &[
                    Some(&atlas_group_layout),
                    None,
                    None,
                    Some(&terrain_group_layout),
                ],
                immediate_size: 0,
            });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow-pipeline-layout"),
                bind_group_layouts: &[
                    Some(&atlas_group_layout),
                    None,
                    None,
                    Some(&shadow_group_layout),
                ],
                immediate_size: 0,
            });

        let depth = || wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::GreaterEqual), // reversed depth: nearer wins
            stencil: Default::default(),
            bias: Default::default(),
        };

        let solid_targets = [
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ];
        let translucent_targets = [Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba16Float,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let terrain_solid = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain-solid"),
            layout: Some(&atlas_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &terrain_module,
                entry_point: Some("harness_vs"),
                compilation_options: Default::default(),
                buffers: &vertex_buffers(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &terrain_module,
                entry_point: Some("harness_fs_solid"),
                compilation_options: Default::default(),
                targets: &solid_targets,
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth()),
            multisample: Default::default(),
            cache: None,
            multiview_mask: None,
        });

        let terrain_translucent = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain-translucent"),
            layout: Some(&atlas_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &terrain_module,
                entry_point: Some("harness_vs"),
                compilation_options: Default::default(),
                buffers: &vertex_buffers(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &terrain_module,
                entry_point: Some("harness_fs_translucent"),
                compilation_options: Default::default(),
                targets: &translucent_targets,
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                depth_write_enabled: Some(false),
                ..depth()
            }),
            multisample: Default::default(),
            cache: None,
            multiview_mask: None,
        });

        let shadow = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &terrain_module,
                entry_point: Some("harness_vs_shadow"),
                compilation_options: Default::default(),
                buffers: &vertex_buffers(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            cache: None,
            multiview_mask: None,
        });

        let shadow_color = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow-color"),
            layout: Some(&atlas_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &terrain_module,
                entry_point: Some("harness_vs_shadow_full"),
                compilation_options: Default::default(),
                buffers: &vertex_buffers(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &terrain_module,
                entry_point: Some("harness_fs_shadow_color"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            cache: None,
            multiview_mask: None,
        });

        // The sky's group 3: the frame and the pack's textures.
        let sky_group_layout = sky_module.as_ref().map(|_| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sky-group-layout"),
                entries: &[
                    frame_entry(
                        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        frame_size(),
                    ),
                    texture_entry(
                        16,
                        wgpu::TextureSampleType::Float { filterable: true },
                        wgpu::TextureViewDimension::D2,
                    ),
                    sampler_entry(24, true, false),
                ],
            })
        });
        let sky_pipeline = sky_module
            .as_ref()
            .zip(sky_group_layout.as_ref())
            .map(|(m, layout)| {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("sky-pipeline-layout"),
                        bind_group_layouts: &[None, None, None, Some(layout)],
                        immediate_size: 0,
                    });
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("sky"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: m,
                        entry_point: Some("harness_vs_sky"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: m,
                        entry_point: Some("harness_fs_sky"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    cache: None,
                    multiview_mask: None,
                });
                (pipeline, layout.clone())
            });
        let sky_group_layout = sky_group_layout;

        // The post passes' group 3: the scene, the shadow, the samplers, the
        // pack's textures and the pack's own targets.
        let n_targets = pack.manifest.post_targets().len();
        let post_group_layouts: Vec<wgpu::BindGroupLayout> = (0..pack.manifest.post.len())
            .map(|i| {
                let mut entries = vec![
                    frame_entry(wgpu::ShaderStages::FRAGMENT, frame_size()),
                    texture_entry(
                        1,
                        wgpu::TextureSampleType::Float { filterable: true },
                        wgpu::TextureViewDimension::D2,
                    ),
                    texture_entry(
                        2,
                        wgpu::TextureSampleType::Depth,
                        wgpu::TextureViewDimension::D2,
                    ),
                    texture_entry(
                        3,
                        wgpu::TextureSampleType::Float { filterable: true },
                        wgpu::TextureViewDimension::D2,
                    ),
                    sampler_entry(4, true, false),
                    sampler_entry(5, false, false),
                    texture_entry(
                        6,
                        wgpu::TextureSampleType::Depth,
                        wgpu::TextureViewDimension::D2,
                    ),
                    sampler_entry(7, true, true),
                    texture_entry(
                        8,
                        wgpu::TextureSampleType::Float { filterable: true },
                        wgpu::TextureViewDimension::D2,
                    ),
                    texture_entry(
                        9,
                        wgpu::TextureSampleType::Depth,
                        wgpu::TextureViewDimension::D2,
                    ),
                    texture_entry(
                        10,
                        wgpu::TextureSampleType::Float { filterable: true },
                        wgpu::TextureViewDimension::D2,
                    ),
                    texture_entry(
                        11,
                        wgpu::TextureSampleType::Float { filterable: false },
                        wgpu::TextureViewDimension::D2,
                    ),
                    texture_entry(
                        16,
                        wgpu::TextureSampleType::Float { filterable: true },
                        wgpu::TextureViewDimension::D2,
                    ),
                    sampler_entry(24, true, false),
                ];
                for t in 0..n_targets {
                    entries.push(texture_entry(
                        32 + t as u32,
                        wgpu::TextureSampleType::Float { filterable: true },
                        wgpu::TextureViewDimension::D2,
                    ));
                }
                let _ = i;
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("post-group-layout"),
                    entries: &entries,
                })
            })
            .collect();

        let post_pipelines: Vec<(wgpu::RenderPipeline, String, Option<(String, f32)>)> = pack
            .manifest
            .post
            .iter()
            .zip(post_modules.iter())
            .zip(post_group_layouts.iter())
            .map(|((entry, (_, m)), layout)| {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(&format!("post-layout:{}", entry.file)),
                        bind_group_layouts: &[None, None, None, Some(layout)],
                        immediate_size: 0,
                    });
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(&format!("post:{}", entry.file)),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: m,
                        entry_point: Some("horizon_vs_post"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: m,
                        entry_point: Some("horizon_fs_post"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    cache: None,
                    multiview_mask: None,
                });
                let output = entry
                    .output
                    .as_ref()
                    .map(|o| (o.clone(), entry.scale.unwrap_or(1.0)));
                (pipeline, entry.file.clone(), output)
            })
            .collect();

        let has_reflection_read = {
            let composed = compose(&pack, StageKind::Material, "material.wgsl");
            composed.text.contains("reflection")
        };

        let shadow_res = pack
            .manifest
            .shadow
            .as_ref()
            .map(|s| s.resolution)
            .unwrap_or(SHADOW_RES);
        let shadow_distance = pack
            .manifest
            .shadow
            .as_ref()
            .map(|s| s.distance)
            .unwrap_or(SHADOW_DISTANCE);

        Self {
            device,
            queue,
            pack,
            terrain_solid,
            terrain_translucent,
            shadow,
            shadow_color,
            atlas_group_layout,
            terrain_group_layout,
            shadow_group_layout,
            sky_group_layout,
            post_group_layouts,
            sky_pipeline,
            post_pipelines,
            has_reflection_read,
            shadow_res,
            shadow_distance,
        }
    }

    /// Renders `scene` through the pack and reads the final picture back.
    pub fn render(&self, scene: &Scene, w: u32, h: u32) -> Vec<u8> {
        self.render_with(scene, w, h, true)
    }

    /// `shadows = false` clears the shadow map (everything lit), standing in
    /// for the game's 1x1 blank, for the tests that compare the two.
    pub fn render_with(&self, scene: &Scene, w: u32, h: u32, shadows: bool) -> Vec<u8> {
        let device = &self.device;
        let mut frame = frame_from_scene(scene, w as f32, h as f32);
        frame.has_shadow = if shadows { 1 } else { 0 };

        // --- resources -----------------------------------------------------
        let frame_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frame"),
            contents: bytemuck::bytes_of(&frame),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        // The mirror pass's own frame: the mirrored camera, has_reflection 0.
        let mirror_frame = {
            let mut m = frame;
            m.has_reflection = 0;
            m.view = view(
                [
                    scene.eye[0],
                    2.0 * scene.sky.water_plane - scene.eye[1],
                    scene.eye[2],
                ],
                [
                    scene.target[0],
                    2.0 * scene.sky.water_plane - scene.target[1],
                    scene.target[2],
                ],
            );
            m.projection = perspective_reversed(70f32.to_radians(), w as f32 / h as f32, 0.05, FAR);
            m.view_inverse = invert(&m.view);
            m.projection_inverse = invert(&m.projection);
            m
        };
        let mirror_frame_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mirror-frame"),
            contents: bytemuck::bytes_of(&mirror_frame),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let light_grid_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("block-light-grid"),
            contents: bytemuck::bytes_of(&[0.0f32; 4]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let atlas = create_texture_init_rgba8(
            device,
            &self.queue,
            &scene.atlas,
            (
                crate::scene::ATLAS_COLS * crate::scene::SPRITE,
                crate::scene::ATLAS_ROWS * crate::scene::SPRITE,
            ),
        );
        let noise_data = self
            .pack
            .source_bytes("noise.png")
            .expect("the pack's noise texture");
        let noise = decode_png(noise_data).expect("noise.png decodes");
        let noise_tex = create_texture_init_rgba8(
            device,
            &self.queue,
            &noise.pixels,
            (noise.width, noise.height),
        );

        let world_color = Plane::new(
            device,
            wgpu::TextureFormat::Rgba16Float,
            (w, h),
            "world-color",
        );
        let out_color = Plane::new(
            device,
            wgpu::TextureFormat::Rgba16Float,
            (w, h),
            "out-color",
        );
        let world_depth = Plane::new(
            device,
            wgpu::TextureFormat::Depth32Float,
            (w, h),
            "world-depth",
        );
        let surface = Plane::new(device, wgpu::TextureFormat::Rgba16Float, (w, h), "surface");
        let opaque_color = Plane::new(
            device,
            wgpu::TextureFormat::Rgba16Float,
            (w, h),
            "opaque-color",
        );
        let opaque_depth = Plane::new(
            device,
            wgpu::TextureFormat::Depth32Float,
            (w, h),
            "opaque-depth",
        );
        let reflection = Plane::new(
            device,
            wgpu::TextureFormat::Rgba16Float,
            (w / 2, h / 2),
            "reflection",
        );
        let reflection_surface = Plane::new(
            device,
            wgpu::TextureFormat::Rgba16Float,
            (w / 2, h / 2),
            "reflection-surface",
        );
        let reflection_depth = Plane::new(
            device,
            wgpu::TextureFormat::Depth32Float,
            (w / 2, h / 2),
            "reflection-depth",
        );
        let shadow_map = Plane::new(
            device,
            wgpu::TextureFormat::Depth32Float,
            (self.shadow_res, self.shadow_res),
            "shadow-map",
        );
        let shadow_translucent = Plane::new(
            device,
            wgpu::TextureFormat::Depth32Float,
            (self.shadow_res, self.shadow_res),
            "shadow-translucent",
        );
        let shadow_color = Plane::new(
            device,
            wgpu::TextureFormat::Rgba8Unorm,
            (self.shadow_res, self.shadow_res),
            "shadow-color",
        );

        // Post targets, at the manifest's scales, in declaration order.
        let targets = self.pack.manifest.post_targets();
        let target_planes: Vec<Plane> = targets
            .iter()
            .map(|(name, scale)| {
                Plane::new(
                    device,
                    wgpu::TextureFormat::Rgba16Float,
                    ((w as f32 * scale) as u32, (h as f32 * scale) as u32),
                    &format!("target-{name}"),
                )
            })
            .collect();

        let block_light_3d = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("block-light"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 4,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_cmp = device.create_sampler(&wgpu::SamplerDescriptor {
            compare: Some(wgpu::CompareFunction::Less),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let atlas_view = atlas.create_view(&Default::default());
        let atlas_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas-group"),
            layout: &self.atlas_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&linear),
                },
            ],
        });
        let noise_view = noise_tex.create_view(&Default::default());
        let bl3_view = block_light_3d.create_view(&Default::default());
        // A 1x1 blank stands in for `reflection` while the mirror itself draws.
        let blank_1x1 = create_texture_init_rgba8(device, &self.queue, &[0, 0, 0, 255], (1, 1));
        let blank_view = blank_1x1.create_view(&Default::default());
        let blank_depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("blank-depth"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let blank_depth_view = blank_depth.create_view(&Default::default());
        let blank_color16 = Plane::new(
            device,
            wgpu::TextureFormat::Rgba16Float,
            (1, 1),
            "blank-color16",
        );

        let terrain_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain-group"),
            layout: &self.terrain_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&reflection.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&linear),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&shadow_map.view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&shadow_cmp),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&opaque_depth.view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&shadow_translucent.view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&shadow_color.view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&bl3_view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: light_grid_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&opaque_color.view),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: wgpu::BindingResource::TextureView(&noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 24,
                    resource: wgpu::BindingResource::Sampler(&repeat),
                },
            ],
        });
        let mirror_terrain_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mirror-terrain-group"),
            layout: &self.terrain_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: mirror_frame_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&blank_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&linear),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&shadow_map.view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&shadow_cmp),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&opaque_depth.view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&shadow_translucent.view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&shadow_color.view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&bl3_view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: light_grid_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&opaque_color.view),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: wgpu::BindingResource::TextureView(&noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 24,
                    resource: wgpu::BindingResource::Sampler(&repeat),
                },
            ],
        });
        // The translucent shadow's colour pass writes two of the textures the
        // terrain group holds, so it binds placeholders where the shader reads
        // none of them.
        let shadow_color_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow-color-group"),
            layout: &self.terrain_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&blank_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&linear),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&blank_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&shadow_cmp),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&blank_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&blank_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&blank_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&bl3_view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: light_grid_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&blank_color16.view),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: wgpu::BindingResource::TextureView(&noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 24,
                    resource: wgpu::BindingResource::Sampler(&repeat),
                },
            ],
        });
        let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow-group"),
            layout: &self.shadow_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: wgpu::BindingResource::TextureView(&noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 24,
                    resource: wgpu::BindingResource::Sampler(&repeat),
                },
            ],
        });

        let sky_group = self.sky_group_layout.as_ref().map(|layout| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sky-group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: frame_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 16,
                        resource: wgpu::BindingResource::TextureView(&noise_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 24,
                        resource: wgpu::BindingResource::Sampler(&repeat),
                    },
                ],
            })
        });

        let mut target_for: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for (i, (name, _)) in targets.iter().enumerate() {
            target_for.insert(name, i);
        }
        // Each pass binds the targets its own file names (plus the one it
        // writes); the rest get a placeholder, so a pass never holds a
        // resource view of the texture it renders into.
        let post_groups: Vec<wgpu::BindGroup> = self
            .post_pipelines
            .iter()
            .zip(self.post_group_layouts.iter())
            .map(|((_, file, _output), layout)| {
                // The pass's code with its comments stripped: a target is
                // really read only where its name appears as a word in code.
                let source: String = self
                    .pack
                    .source(file)
                    .unwrap_or_default()
                    .lines()
                    .map(|l| l.split("//").next().unwrap_or(l))
                    .collect::<Vec<_>>()
                    .join("\n");
                let uses = |name: &str| -> bool {
                    let mut from = 0;
                    while let Some(at) = source[from..].find(name) {
                        let a = from + at;
                        let before = source[..a].chars().next_back();
                        let after = source[a + name.len()..].chars().next();
                        let word = |c: Option<char>| {
                            c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                        };
                        if !word(before) && !word(after) {
                            return true;
                        }
                        from = a + name.len();
                    }
                    false
                };
                let mut real = std::collections::HashSet::new();
                for (i, (name, _)) in targets.iter().enumerate() {
                    if uses(name) {
                        real.insert(i);
                    }
                }

                let mut entries = vec![
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: frame_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&world_color.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&world_depth.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&world_color.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&linear),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&nearest),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(&shadow_map.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::Sampler(&shadow_cmp),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::TextureView(&world_color.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: wgpu::BindingResource::TextureView(&shadow_translucent.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: wgpu::BindingResource::TextureView(&shadow_color.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 11,
                        resource: wgpu::BindingResource::TextureView(&surface.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 16,
                        resource: wgpu::BindingResource::TextureView(&noise_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 24,
                        resource: wgpu::BindingResource::Sampler(&repeat),
                    },
                ];
                for (i, plane) in target_planes.iter().enumerate() {
                    let view = if real.contains(&i) {
                        &plane.view
                    } else {
                        &blank_color16.view
                    };
                    entries.push(wgpu::BindGroupEntry {
                        binding: 32 + i as u32,
                        resource: wgpu::BindingResource::TextureView(view),
                    });
                }
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("post-group"),
                    layout,
                    entries: &entries,
                })
            })
            .collect();

        // --- geometry --------------------------------------------------------
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            contents: bytemuck::cast_slice(&scene.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indices"),
            contents: bytemuck::cast_slice(&scene.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let index_count = scene.indices.len() as u32;

        // --- the frame -------------------------------------------------------
        let mut encoder = device.create_command_encoder(&Default::default());

        // 1. The shadow map: solid terrain, then the translucent roof (its
        //    colour into `shadow_color`). With `shadows` off the map clears to
        //    all-lit, standing in for the game's 1x1 blank.
        // 1.0 = nothing between the point and the light, per the frame's
        // depth convention; the `shadows` flag decides whether anything draws.
        let shadow_clear = 1.0;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &shadow_map.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(shadow_clear),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow);
            pass.set_bind_group(0, &atlas_group, &[]);
            pass.set_bind_group(3, &shadow_group, &[]);
            pass.set_vertex_buffer(0, vertex_buf.slice(..));
            pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
            if shadows {
                pass.draw_indexed(0..index_count, 0, 0..1);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow-translucent"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &shadow_color.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &shadow_translucent.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow_color);
            pass.set_bind_group(0, &atlas_group, &[]);
            pass.set_bind_group(3, &shadow_color_group, &[]);
            pass.set_vertex_buffer(0, vertex_buf.slice(..));
            pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
            if shadows {
                pass.draw_indexed(0..index_count, 0, 0..1);
            }
        }

        // 2. The water mirror: the world from a camera mirrored about the
        //    plane, with the pack's material but no reflection of its own.
        if self.has_reflection_read {
            let clear = {
                let c = frame.sky_color;
                wgpu::Color {
                    r: c[0] as f64,
                    g: c[1] as f64,
                    b: c[2] as f64,
                    a: 1.0,
                }
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("reflection"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &reflection.view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &reflection_surface.view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &reflection_depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.terrain_solid);
            pass.set_bind_group(0, &atlas_group, &[]);
            pass.set_bind_group(3, &mirror_terrain_group, &[]);
            pass.set_vertex_buffer(0, vertex_buf.slice(..));
            pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..index_count, 0, 0..1);
        }

        // 3. The world: the sky, then solid terrain, then a copy for
        //    `opaque_color`/`opaque_depth`, then translucent terrain.
        if let (Some((pipeline, _)), Some(group)) = (self.sky_pipeline.as_ref(), sky_group.as_ref())
        {
            let clear = {
                let c = frame.fog_color;
                wgpu::Color {
                    r: c[0] as f64,
                    g: c[1] as f64,
                    b: c[2] as f64,
                    a: 1.0,
                }
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sky"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &world_color.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(3, group, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let clear = {
                let c = frame.fog_color;
                wgpu::Color {
                    r: c[0] as f64,
                    g: c[1] as f64,
                    b: c[2] as f64,
                    a: 1.0,
                }
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain-solid"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &world_color.view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: if self.sky_pipeline.is_some() {
                                wgpu::LoadOp::Load
                            } else {
                                wgpu::LoadOp::Clear(clear)
                            },
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &surface.view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &world_depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.terrain_solid);
            pass.set_bind_group(0, &atlas_group, &[]);
            pass.set_bind_group(3, &terrain_group, &[]);
            pass.set_vertex_buffer(0, vertex_buf.slice(..));
            pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..index_count, 0, 0..1);
        }

        encoder.copy_texture_to_texture(
            world_color.texture.as_image_copy(),
            opaque_color.texture.as_image_copy(),
            world_color.texture.size(),
        );
        encoder.copy_texture_to_texture(
            world_depth.texture.as_image_copy(),
            opaque_depth.texture.as_image_copy(),
            world_depth.texture.size(),
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain-translucent"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &world_color.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &world_depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.terrain_translucent);
            pass.set_bind_group(0, &atlas_group, &[]);
            pass.set_bind_group(3, &terrain_group, &[]);
            pass.set_vertex_buffer(0, vertex_buf.slice(..));
            pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..index_count, 0, 0..1);
        }

        // 4. The post passes, in the manifest's order.
        for (i, (_, file, output)) in self.post_pipelines.iter().enumerate() {
            let view_ref: &wgpu::TextureView = match output {
                Some((name, _)) => &target_planes[target_for[name.as_str()]].view,
                None => &out_color.view,
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("post:{file}")),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view_ref,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.post_pipelines[i].0);
            pass.set_bind_group(3, &post_groups[i], &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit([encoder.finish()]);

        // --- readback: the last pass's output (a target, or the world's) ----
        let last_target = self
            .post_pipelines
            .last()
            .and_then(|(_, _, output)| output.as_ref().map(|(name, _)| target_for[name.as_str()]));
        let read_plane = match last_target {
            Some(i) => &target_planes[i],
            None => &out_color,
        };
        let (tw, th) = read_plane.size;
        read_rgba16_to_rgba8(device, &self.queue, &read_plane.texture, tw, th)
    }
}

fn terrain_group_layout_of(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("terrain-group-layout"),
        entries: &[
            frame_entry(
                wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                frame_size(),
            ),
            texture_entry(
                1,
                wgpu::TextureSampleType::Float { filterable: true },
                wgpu::TextureViewDimension::D2,
            ),
            sampler_entry(2, true, false),
            texture_entry(
                3,
                wgpu::TextureSampleType::Depth,
                wgpu::TextureViewDimension::D2,
            ),
            sampler_entry(4, true, true),
            texture_entry(
                6,
                wgpu::TextureSampleType::Depth,
                wgpu::TextureViewDimension::D2,
            ),
            texture_entry(
                7,
                wgpu::TextureSampleType::Depth,
                wgpu::TextureViewDimension::D2,
            ),
            texture_entry(
                8,
                wgpu::TextureSampleType::Float { filterable: true },
                wgpu::TextureViewDimension::D2,
            ),
            texture_entry(
                9,
                wgpu::TextureSampleType::Float { filterable: true },
                wgpu::TextureViewDimension::D3,
            ),
            wgpu::BindGroupLayoutEntry {
                binding: 10,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            },
            texture_entry(
                11,
                wgpu::TextureSampleType::Float { filterable: false },
                wgpu::TextureViewDimension::D2,
            ),
            wgpu::BindGroupLayoutEntry {
                binding: 16,
                // The waving vertex stage samples the noise too.
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 24,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_texture_init_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[u8],
    size: (u32, u32),
) -> wgpu::Texture {
    {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgba8"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.0 * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        texture
    }
}

/// Copies an RGBA16Float texture to a buffer and converts it to RGBA8 pixels
/// for the PNG. The pack's grade outputs display-ready colour, so the
/// conversion is the plain 0..1 clamp of each channel.
fn read_rgba16_to_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    w: u32,
    h: u32,
) -> Vec<u8> {
    let bytes_per_row = ((w * 8).div_ceil(256)) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (bytes_per_row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = buffer.slice(..).get_mapped_range().unwrap();

    let w = w as usize;
    let mut out = vec![0u8; w * h as usize * 4];
    for y in 0..h as usize {
        let row = &mapped[(y * bytes_per_row as usize)..][..w * 8];
        let halves: &[u16] = bytemuck::cast_slice(row);
        for x in 0..w {
            for c in 0..4 {
                let half = halves[x * 4 + c];
                let v = f16_to_f32(half).clamp(0.0, 1.0);
                out[(y * w + x) * 4 + c] = (v * 255.0 + 0.5) as u8;
            }
        }
    }
    out
}

/// f16 to f32, without half's dependency.
fn f16_to_f32(half: u16) -> f32 {
    let sign = ((half >> 15) as u32) << 31;
    let exp = ((half >> 10) & 0x1f) as u32;
    let frac = (half & 0x3ff) as u32;
    let bits = if exp == 0 {
        if frac == 0 {
            sign
        } else {
            // Subnormal: normalize.
            let mut e: i32 = -1;
            let mut f = frac;
            while f & 0x400 == 0 {
                f <<= 1;
                e -= 1;
            }
            f &= 0x3ff;
            sign | (((114 + e) as u32) << 23) | (f << 13)
        }
    } else if exp == 0x1f {
        sign | 0x7f80_0000u32 | (frac << 13)
    } else {
        sign | ((exp + 112) << 23) | (frac << 13)
    };
    f32::from_bits(bits)
}

// --- PNG ---------------------------------------------------------------------

pub struct DecodedPng {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub fn decode_png(data: &[u8]) -> Option<DecodedPng> {
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![
        0u8;
        reader
            .output_buffer_size()
            .unwrap_or(0)
            .max(reader.info().raw_bytes().next_power_of_two())
    ];
    let info = reader.next_frame(&mut buf).ok()?;
    Some(DecodedPng {
        width: info.width,
        height: info.height,
        pixels: buf[..info.buffer_size()].to_vec(),
    })
}

pub fn encode_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut writer = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    writer.set_color(png::ColorType::Rgba);
    writer.set_depth(png::BitDepth::Eight);
    let mut writer = writer.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}

/// The camera-relative point a post pass's `view_position` recovers, for the
/// tests that check depth-driven decisions.
pub fn view_position(frame: &horizon_shader_api::Frame, uv: [f32; 2], depth: f32) -> [f32; 3] {
    let clip = [uv[0] * 2.0 - 1.0, 1.0 - uv[1] * 2.0, depth, 1.0];
    let view = mul_vec(&frame.projection_inverse, clip);
    let world = mul_vec(
        &frame.view_inverse,
        [view[0] / view[3], view[1] / view[3], view[2] / view[3], 0.0],
    );
    [world[0], world[1], world[2]]
}

/// `Frame` for the tests that need the cameras without a scene.
pub fn test_frame(w: f32, h: f32) -> horizon_shader_api::Frame {
    let scene = crate::scene::noon_plains();
    frame_from_scene(&scene, w, h)
}

/// Where the sun is for `day_time`, matching the harness's frames (and, up to
/// the engine's own maths, the game's).
pub fn sun_direction(day_time: f32) -> [f32; 3] {
    let theta = day_time * std::f32::consts::TAU;
    [theta.cos(), theta.sin(), 0.0]
}
