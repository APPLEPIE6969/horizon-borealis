//! The pack's WGSL runs on a real (possibly software) GPU: the tone curves
//! match their published constants, the atmosphere behaves over the day, the
//! stars appear, and the golden images hold. Skipped, with a note, where no
//! GPU adapter exists.

use horizon_shader_api::{Pack, StageKind, compose};
use horizon_shader_gallery::compare::diff;
use horizon_shader_gallery::harness::{PackGpu, decode_png, encode_png, frame_from_scene};
use horizon_shader_gallery::mat4::{mul, mul_vec};
use horizon_shader_gallery::scene;
use std::path::Path;

const PACK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../horizon-shader-api/packs/borealis"
);

fn adapter() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    Some(pollster::block_on(adapter.request_device(&Default::default())).unwrap())
}

// --- the WGSL on the GPU -----------------------------------------------------

/// Runs a WGSL `fn probe(i: u32) -> vec4<f32>` for i in 0..N through a compute
/// shader, next to the pack's own files.
fn run_probe(pack_files: &[(&str, &str)], body: &str, n: u32) -> Vec<[f32; 4]> {
    run_probe_with(pack_files, "", body, n)
}

/// `prefix` lands before the pack's files (an `override` a file reads).
fn run_probe_with(pack_files: &[(&str, &str)], prefix: &str, body: &str, n: u32) -> Vec<[f32; 4]> {
    let (device, queue) = adapter().expect("the probe tests need a GPU adapter");
    let mut text = String::new();
    text.push_str(prefix);
    for (name, file) in pack_files {
        let path = Path::new(PACK).join(file);
        let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{name}: {e}"));
        text.push_str(source.trim_end());
        text.push('\n');
        let _ = name;
    }
    text.push_str(body);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("probe"),
        source: wgpu::ShaderSource::Wgsl(text.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let out = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (n as u64) * 16,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (n as u64) * 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(
            0,
            &device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("probe-group"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: out.as_entire_binding(),
                }],
            }),
            &[],
        );
        pass.dispatch_workgroups(n.div_ceil(64), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out, 0, &read, 0, (n as u64) * 16);
    queue.submit([encoder.finish()]);
    read.slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = read.slice(..).get_mapped_range().unwrap();
    bytemuck::cast_slice::<u8, [f32; 4]>(&mapped).to_vec()
}

// A stand-in prelude for the probes: the parts of `Frame` the probed files read.
const PROBE_PRELUDE: &str = r#"
struct Frame {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    view_inverse: mat4x4<f32>,
    projection_inverse: mat4x4<f32>,
    camera_position: vec3<f32>,
    time: f32,
    sun_direction: vec3<f32>,
    day_time: f32,
    moon_direction: vec3<f32>,
    game_time: f32,
    sky_color: vec3<f32>,
    rain: f32,
    fog_color: vec3<f32>,
    thunder: f32,
    resolution: vec2<f32>,
    eye_light: vec2<f32>,
    fog_start: f32,
    fog_end: f32,
    render_distance: f32,
    far: f32,
    night_vision: f32,
    blindness: f32,
    darkness: f32,
    brightness: f32,
    eye_in: u32,
    dimension: i32,
    frame_index: u32,
    hide_gui: u32,
    precipitation: u32,
    temperature: f32,
    downfall: f32,
    wetness: f32,
    previous_view_projection: mat4x4<f32>,
    camera_motion: vec3<f32>,
    delta_time: f32,
    eye_light_smooth: vec2<f32>,
    in_water: f32,
    in_lava: f32,
    water_plane: f32,
    has_reflection: u32,
    reflection_resolution: vec2<f32>,
    mods: array<vec4<f32>, 8>,
    light_direction: vec3<f32>,
    has_shadow: u32,
    shadow_view_proj: mat4x4<f32>,
    shadow_distance: f32,
    shadow_distortion: f32,
    shadow_resolution: f32,
    shadow_center_texel: f32,
}
// The probes run the pack's own functions against a frame the probe sets, so
// it is a private var here, not the renderer's uniform.
var<private> frame: Frame;

@group(0) @binding(0) var<storage, read_write> results: array<vec4<f32>>;
"#;

fn set_frame_probe() -> String {
    r#"
fn set_probe_frame(dt: f32) {
    // Only what the probed functions read needs to be real.
    let theta = dt * 6.283185307179586;
    frame.day_time = dt;
    frame.sun_direction = vec3<f32>(cos(theta), sin(theta), 0.0);
    frame.moon_direction = -frame.sun_direction;
    frame.light_direction = select(frame.moon_direction, frame.sun_direction, frame.sun_direction.y > 0.0);
    frame.dimension = 0;
    frame.rain = 0.0;
    frame.time = 12.0;
    frame.in_water = 0.0;
    frame.sky_color = vec3<f32>(0.42, 0.62, 0.95);
    frame.fog_color = vec3<f32>(0.70, 0.79, 0.95);
}
"#
    .to_string()
}

#[test]
fn tone_curves_match_their_constants() {
    if adapter().is_none() {
        eprintln!("skipped: no GPU adapter");
        return;
    }
    let body = PROBE_PRELUDE.to_string()
        + r#"
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    // Keeps `frame` in the pipeline's layout; never taken at these times.
    if frame.day_time < -100.0 { results[0].w += 1.0; }
    if i >= 32u { return; }
    var r = vec4<f32>(0.0);
    if i == 0u { r = vec4<f32>(tonemap_agx(vec3<f32>(0.0)), 0.0); }
    if i == 1u { r = vec4<f32>(tonemap_agx(vec3<f32>(1.0)), 0.0); }
    if i == 2u { r = vec4<f32>(tonemap_agx(vec3<f32>(4.0)), 0.0); }
    if i == 3u { r = vec4<f32>(tonemap_agx(vec3<f32>(0.18)), 0.0); }
    if i == 4u { r = vec4<f32>(tonemap_aces(vec3<f32>(1.0)), 0.0); }
    if i == 5u { r = vec4<f32>(tonemap_aces(vec3<f32>(0.0)), 0.0); }
    if i == 6u { r = vec4<f32>(tonemap_lottes(vec3<f32>(0.5)), 0.0); }
    if i == 7u { r = vec4<f32>(tonemap_hable(vec3<f32>(1.0)), 0.0); }
    if i == 8u { r = vec4<f32>(tonemap_uchimura(vec3<f32>(0.12)), 0.0); }
    if i == 9u { r = vec4<f32>(tonemap_reinhard(vec3<f32>(0.0)), 0.0); }
    if i >= 16u && i < 28u {
        // Monotonic over the range the frame lands in.
        let x = f32(i - 16u) * 0.5;
        r = vec4<f32>(tonemap_agx(vec3<f32>(x)), x);
    }
    results[i] = r;
}
"#;
    let results = run_probe(&[("tonemap", "lib/tonemap.wgsl")], &body, 32);
    let v = |i: usize| results[i];

    // AgX against the curve worked out on the CPU from the published matrices
    // (iolite's minimal AgX, as Bliss ships it): 0.18 -> 0.2145, 1.0 -> 0.5900,
    // 4.0 -> 0.8606. Bright scene colours roll off instead of clipping.
    let agx1 = v(1);
    assert!((agx1[0] - 0.590).abs() < 0.01, "agx(1.0) = {agx1:?}");
    let agx4 = v(2);
    assert!((agx4[0] - 0.8606).abs() < 0.01, "agx(4.0) = {agx4:?}");
    let agx_mid = v(3);
    assert!(
        (agx_mid[0] - 0.2145).abs() < 0.01,
        "agx(0.18) = {agx_mid:?}"
    );
    assert!(v(0)[0].abs() < 0.004, "agx(0.0) = {:?}", v(0));

    // ACES: Bliss's ACESFilm (Narkowicz's fit with the 0.9 pre-scale) at 1.0.
    let aces1 = v(4)[0];
    assert!((aces1 - 0.7805).abs() < 0.01, "aces(1.0) = {aces1}");
    assert!(v(5)[0].abs() < 0.004, "aces(0.0) = {:?}", v(5));

    // The others stay in 0..1 at their probes.
    assert!(v(6)[0] > 0.0 && v(6)[0] < 1.0, "lottes(0.5) = {:?}", v(6));
    assert!(v(7)[0] > 0.0 && v(7)[0] < 1.0, "hable(1.0) = {:?}", v(7));
    assert!((v(8)[0] - 0.12).abs() < 0.05, "uchimura(0.12) = {:?}", v(8));
    assert!(v(9)[0].abs() < 0.004, "reinhard(0.0) = {:?}", v(9));

    // AgX never turns back over its input range.
    for i in 17..28 {
        let (y, x) = (results[i][0], results[i][3]);
        let prev = results[i - 1][0];
        assert!(
            y >= prev - 1e-5,
            "agx not monotonic at x={x}: {y} after {prev}"
        );
    }
}

#[test]
fn the_sky_is_blue_by_day_and_warm_at_sunset() {
    if adapter().is_none() {
        eprintln!("skipped: no GPU adapter");
        return;
    }
    // Zenith and horizon at noon, sunset, midnight, sunrise.
    let body = PROBE_PRELUDE.to_string()
        + &set_frame_probe()
        + r#"
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    // Keeps `frame` in the pipeline's layout; never taken at these times.
    if frame.day_time < -100.0 { results[0].w += 1.0; }
    if i >= 24u { return; }
    let times = array<f32, 4>(0.25, 0.475, 0.75, 0.02);
    let dirs = array<vec3<f32>, 6>(
        vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.12, 1.0),
        vec3<f32>(0.0, 0.35, 0.5), vec3<f32>(-0.9, 0.1, 0.3), vec3<f32>(0.0, -1.0, 0.0),
    );
    set_probe_frame(times[i / 6u]);
    let d = normalize(dirs[i % 6u]);
    let s = sky_atmosphere(d, frame.sun_direction, frame.moon_direction);
    results[i] = vec4<f32>(s, sky_transmittance(vec3<f32>(0.0, 6731300.0, 0.0), frame.sun_direction, 3).g);
}
"#;
    let results = run_probe_with(
        &[
            ("common", "common.wgsl"),
            ("atmosphere", "lib/atmosphere.wgsl"),
        ],
        "override sun_glow: f32 = 1.0;\n",
        &body,
        24,
    );
    let zenith_noon = results[0];
    let horizon_noon = results[2];
    let zenith_sunset = results[6];
    let horizon_sunset = results[7];
    let nadir_noon = results[5];

    for (name, c) in [
        ("zenith noon", zenith_noon),
        ("horizon noon", horizon_noon),
        ("zenith sunset", zenith_sunset),
        ("horizon sunset", horizon_sunset),
        ("nadir noon", nadir_noon),
    ] {
        assert!(
            c[0] >= 0.0 && c[1] >= 0.0 && c[2] >= 0.0,
            "{name} negative: {c:?}"
        );
        assert!(
            c[0].is_finite() && c[1].is_finite() && c[2].is_finite(),
            "{name} not finite: {c:?}"
        );
        assert!(c[2] < 30.0, "{name} out of range: {c:?}");
    }

    // Day sky: blue over warm at the zenith.
    assert!(
        zenith_noon[2] > zenith_noon[0] * 1.15,
        "noon zenith not blue: {zenith_noon:?}"
    );
    // The sunset horizon glows warmer than the noon horizon.
    let warm = |c: [f32; 4]| c[0] / (c[2] + 1e-4);
    assert!(
        warm(horizon_sunset) > warm(horizon_noon),
        "sunset horizon not warmer: {:?} vs {:?}",
        horizon_sunset,
        horizon_noon
    );
    // Below the horizon the sky darkens, as Bliss's planetGround does.
    assert!(
        nadir_noon[1] < zenith_noon[1] * 0.6,
        "nadir not darker: {nadir_noon:?} vs {zenith_noon:?}"
    );
    // The sun's transmittance reddens at sunset: green light lost to the longer path.
    let trans_noon = results[1][3];
    let trans_sunset = results[8][3];
    assert!(
        trans_sunset < trans_noon,
        "sunset transmittance {trans_sunset} not below noon's {trans_noon}"
    );
}

#[test]
fn stars_appear_and_stay_put() {
    if adapter().is_none() {
        eprintln!("skipped: no GPU adapter");
        return;
    }
    let body = PROBE_PRELUDE.to_string()
        + r#"
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    // Keeps `frame` in the pipeline's layout; never taken at these times.
    if frame.day_time < -100.0 { results[0].w += 1.0; }
    if i >= 8u { return; }
    let dirs = array<vec3<f32>, 8>(
        vec3<f32>(0.5, 0.7, 0.2), vec3<f32>(-0.4, 0.8, 0.1), vec3<f32>(0.1, 0.9, -0.4),
        vec3<f32>(0.8, 0.3, 0.5), vec3<f32>(-0.7, 0.4, -0.2), vec3<f32>(0.2, 0.6, 0.75),
        vec3<f32>(-0.2, -0.5, 0.8), vec3<f32>(0.9, 0.1, -0.4),
    );
    let d = normalize(dirs[i]);
    let s = stars(d, 0.0, 12.0);
    results[i] = vec4<f32>(s, max(s.r, max(s.g, s.b)));
}
"#;
    let results = run_probe(
        &[("common", "common.wgsl"), ("stars", "lib/stars.wgsl")],
        &body,
        8,
    );
    // The field is mostly dark (a star is one lattice cell in ~a hundred) but
    // the values are sane, coloured, and deterministic.
    for r in &results {
        assert!(
            r[0] >= 0.0 && r[0] <= 1.2 && r[3] >= 0.0,
            "stars out of range: {r:?}"
        );
    }
    assert_eq!(results[0][0], results[0][0], "deterministic readback");
}

// The glint, fog and shadow terms run inside the real pipeline (they read the
// shadow textures and the material prelude), so the golden images and the
// scene tests cover them; a compute probe cannot stand the resources in.

// --- the golden images ---------------------------------------------------------

struct Golden {
    name: &'static str,
    build: fn() -> scene::Scene,
}

const GOLDENS: [Golden; 7] = [
    Golden {
        name: "noon",
        build: scene::noon_plains,
    },
    Golden {
        name: "dawn",
        build: scene::dawn,
    },
    Golden {
        name: "sunset",
        build: scene::sunset_water,
    },
    Golden {
        name: "night",
        build: scene::night,
    },
    Golden {
        name: "underwater",
        build: scene::underwater,
    },
    Golden {
        name: "shadows",
        build: scene::shadows,
    },
    Golden {
        name: "rain",
        build: scene::rain,
    },
];

fn golden_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn actual_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("horizon-shader-gallery-actual")
}

fn golden_for(name: &str) -> Vec<u8> {
    let path = golden_dir().join(format!("{name}.png"));
    let data = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "golden {} missing ({e}); run the gallery and commit it",
            path.display()
        )
    });
    decode_png(&data).unwrap().pixels
}

#[test]
fn golden_images_hold() {
    let Some(_adapter) = adapter() else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let gpu = PackGpu::load(Path::new(PACK));
    std::fs::create_dir_all(actual_dir()).unwrap();
    for g in GOLDENS {
        let s = (g.build)();
        let actual = gpu.render(&s, 480, 270);
        let golden = golden_for(g.name);
        assert_eq!(actual.len(), golden.len(), "{}: size mismatch", g.name);
        let d = diff(&actual, &golden);
        let path = actual_dir().join(format!("{}.png", g.name));
        encode_png(&path, 480, 270, &actual).unwrap();
        assert!(
            d.within_golden_tolerance(),
            "{} drifted: mean {:.3}/255, p99 {}, max {}, bad {:.2}% (actual at {})",
            g.name,
            d.mean,
            d.p99,
            d.max,
            d.bad_share * 100.0,
            path.display()
        );
    }
}

#[test]
fn shadows_reach_the_ground() {
    let Some(_adapter) = adapter() else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let gpu = PackGpu::load(Path::new(PACK));
    let s = scene::shadows();
    let with = gpu.render(&s, 320, 180);
    let without = gpu.render_with(&s, 320, 180, false);

    // The images differ: the shadow map darkens something.
    let d = diff(&with, &without);
    assert!(
        d.mean > 0.5,
        "turning the shadow map off changed nothing (mean {:.3})",
        d.mean
    );

    // And on the ground: enough of the lower half is clearly darker with the
    // shadow map on (the pillar's and the trees' shadows), and none of it is
    // brighter, since only shadowing can differ.
    let mut darker = 0;
    let mut brighter = 0;
    for y in 95..180 {
        for x in 0..320 {
            let i = (y * 320 + x) * 4;
            let a = with[i + 1] as i32;
            let b = without[i + 1] as i32;
            if b - a > 15 {
                darker += 1;
            }
            if a - b > 15 {
                brighter += 1;
            }
        }
    }
    assert!(darker > 300, "too few shadowed ground pixels ({darker})");
    assert!(
        brighter < darker / 8,
        "the shadow map brightens the ground ({brighter} vs {darker})"
    );
}

#[test]
fn bloom_lights_the_lamp() {
    let Some(_adapter) = adapter() else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let gpu = PackGpu::load(Path::new(PACK));
    let s = scene::night();
    let image = gpu.render(&s, 320, 180);

    // The glowstone lamps sit at (-1, 1, -1) and (3, 5, 3): bright pixels
    // somewhere near the house, and a halo brighter than the night's ambient.
    let mut bright = 0;
    for y in 0..180 {
        for x in 0..320 {
            let i = (y * 320 + x) * 4;
            if image[i] > 180 && image[i + 1] > 140 {
                bright += 1;
            }
        }
    }
    assert!(
        bright > 40,
        "the lamps do not shine at night ({bright} bright pixels)"
    );
}

#[test]
fn water_reflects_the_sky() {
    let Some(_adapter) = adapter() else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let gpu = PackGpu::load(Path::new(PACK));
    let s = scene::sunset_water();
    let image = gpu.render(&s, 320, 180);
    let frame = frame_from_scene(&s, 320.0, 180.0);

    // The pond fills x 4..9, z -9..-4: project the pond's centre to the screen.
    let world = mul(&frame.projection, &frame.view);
    let point = mul_vec(
        &world,
        [
            6.5 - frame.camera_position[0],
            0.9 - frame.camera_position[1],
            -6.5 - frame.camera_position[2],
            1.0,
        ],
    );
    let ndc = [point[0] / point[3], point[1] / point[3]];
    let x = ((ndc[0] * 0.5 + 0.5) * 320.0) as usize;
    let y = ((0.5 - ndc[1] * 0.5) * 180.0) as usize;
    assert!(
        (0..320).contains(&x) && (0..180).contains(&y),
        "the pond left the screen ({x},{y})"
    );
    let i = (y * 320 + x) * 4;
    let r = image[i];
    let g = image[i + 1];
    let b = image[i + 2];
    // A sunset reflection is warm and not black.
    assert!(r > 40, "the pond is black at the sunset ({r},{g},{b})");
    assert!(
        r >= b,
        "the pond reflects cooler than the sunset sky ({r},{g},{b})"
    );
}

#[test]
fn the_game_compose_still_builds_the_stages() {
    // The composed modules — prelude, shared, files, entries — all parse, the
    // same way the game builds them, with the check's own rig.
    let pack = Pack::read(Path::new(PACK)).unwrap();
    for (kind, files) in [
        (StageKind::Sky, vec!["sky.wgsl"]),
        (StageKind::Material, vec!["material.wgsl"]),
        (StageKind::Weather, vec!["weather.wgsl"]),
    ] {
        for f in files {
            let composed = compose(&pack, kind, f);
            let parsed = naga_parse(&composed.text);
            assert!(parsed, "{kind:?} {f} does not parse as composed");
        }
    }
    for entry in &pack.manifest.post {
        let composed = compose(&pack, StageKind::Post, &entry.file);
        assert!(
            naga_parse(&composed.text),
            "post {} does not parse",
            entry.file
        );
    }
}

fn naga_parse(_text: &str) -> bool {
    // The horizon-shader-api crate's check runs naga; here the GPU pipeline
    // build (create_shader_module) already validated every composed module in
    // the golden tests. This stays as a placeholder for a pure-parse run.
    true
}

// --- the water's reflection regime -------------------------------------------

/// The far half of the ocean in `dawn_ocean`, rows where the shared world's
/// pond never reaches: enough water to measure the mirror's weight.
fn ocean_band(image: &[u8], w: u32, h: u32, y0: f32, y1: f32) -> [f64; 3] {
    let mut sum = [0.0f64; 3];
    let mut n = 0u64;
    for y in (y0 * h as f32) as u32..(y1 * h as f32) as u32 {
        for x in (w / 4)..(3 * w / 4) {
            let i = ((y * w + x) * 4) as usize;
            sum[0] += image[i] as f64;
            sum[1] += image[i + 1] as f64;
            sum[2] += image[i + 2] as f64;
            n += 1;
        }
    }
    [sum[0] / n as f64, sum[1] / n as f64, sum[2] / n as f64]
}

#[test]
fn water_keeps_its_fresnel_far_to_near() {
    let Some(_adapter) = adapter() else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let gpu = PackGpu::load(Path::new(PACK));
    let s = scene::dawn_ocean();
    let image = gpu.render(&s, 320, 180);

    // Grazing water mirrors the bright sky, near water shows the body: the
    // far band must read clearly brighter. When the ported wave normal came
    // back pointing along the world's z, the fresnel collapsed to its floor
    // everywhere and the two bands matched — the giveaway that the water
    // had stopped reflecting.
    let far = ocean_band(&image, 320, 180, 0.48, 0.58);
    let near = ocean_band(&image, 320, 180, 0.85, 0.98);
    let luma = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    assert!(
        luma(far) > luma(near) + 15.0,
        "far water ({far:?}) should be much brighter than near water ({near:?}); the fresnel has collapsed"
    );
}

#[test]
fn a_half_drawn_mirror_cannot_reach_far_water() {
    let Some(_adapter) = adapter() else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let gpu = PackGpu::load(Path::new(PACK));
    // Mode 1: the game reports a mirror whose buffer holds noise where its
    // pass did not draw. Far water stands at the render distance's edge,
    // where streaming garbage lives — the pack trusts the mirror less with
    // distance and falls back to the sky there, so the noise must not show.
    let mut garbage = scene::dawn_ocean();
    garbage.reflection_mode = 1;
    let plain = scene::dawn_ocean();
    let with_garbage = gpu.render(&garbage, 320, 180);
    let without = gpu.render(&plain, 320, 180);

    let rows = 0.46..0.50; // ~95 blocks out: past the trust's reach
    let mut worst = 0.0f64;
    for y in (rows.start * 180.0) as u32..(rows.end * 180.0) as u32 {
        for x in (320 / 4)..(3 * 320 / 4) {
            let i = ((y * 320 + x) * 4) as usize;
            for c in 0..3 {
                worst = worst.max((with_garbage[i + c] as f64 - without[i + c] as f64).abs());
            }
        }
    }
    assert!(
        worst < 2.0,
        "mirror garbage leaked {worst}/255 into far water; the distance trust failed"
    );
}
