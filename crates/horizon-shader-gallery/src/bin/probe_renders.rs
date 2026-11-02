//! Throwaway probe: render the world under the candidate in-game regimes
//! (clear/rain x noon/morning) so the pixel numbers can be compared with the
//! screenshot the user took. Not a golden source; run manually.

use horizon_shader_gallery::harness::PackGpu;
use horizon_shader_gallery::scene::{SkyParams, world};

fn probe(name: &'static str, sky: SkyParams) {
    let gpu = PackGpu::load(std::path::Path::new(
        "crates/horizon-shader-api/packs/borealis",
    ));
    let s = world(name, sky, [10.0, 7.5, 13.0], [-1.0, 2.0, -1.0]);
    let pixels = gpu.render(&s, 480, 270);
    let dir = "/tmp/probe";
    std::fs::create_dir_all(dir).unwrap();
    let path = format!("{dir}/{name}.png");
    horizon_shader_gallery::harness::encode_png(std::path::Path::new(&path), 480, 270, &pixels)
        .unwrap();
    println!("rendered {path}");
}

fn main() {
    let noon = 0.25f32;
    // A fresh world starts at day_time 0 (sunrise); the tutorial toast implies
    // early game. sun.y = sin(0.045*TAU) ~ 0.28. Also try right at 0.02 (~0.125).
    let morning = 0.045f32;
    let dawn = 0.02f32;
    probe(
        "clear_noon",
        SkyParams {
            day_time: noon,
            ..Default::default()
        },
    );
    probe(
        "clear_morning",
        SkyParams {
            day_time: morning,
            ..Default::default()
        },
    );
    probe(
        "clear_dawn",
        SkyParams {
            day_time: dawn,
            ..Default::default()
        },
    );
    probe(
        "rain_noon",
        SkyParams {
            day_time: noon,
            rain: 0.9,
            ..Default::default()
        },
    );
    probe(
        "rain_morning",
        SkyParams {
            day_time: morning,
            rain: 0.9,
            ..Default::default()
        },
    );
    probe(
        "rain_dawn",
        SkyParams {
            day_time: dawn,
            rain: 0.9,
            ..Default::default()
        },
    );
}
