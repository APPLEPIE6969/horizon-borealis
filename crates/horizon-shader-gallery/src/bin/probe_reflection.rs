//! Throwaway probe: the reflection-path experiment. The game reports a
//! mirror the harness never did — and its buffer may hold garbage where its
//! mirror pass did not draw (chunks still streaming). Mode 0 is the shipped
//! default (no mirror), 1 feeds the pack a half-garbage mirror buffer, 2 is
//! a fully drawn mirror. Far-water garbage surviving into mode 1's output
//! means the pack propagates unvalidated mirror content. Run manually.

use horizon_shader_gallery::harness::{PackGpu, encode_png};
use horizon_shader_gallery::scene;

fn main() {
    let gpu = PackGpu::load(std::path::Path::new(
        "crates/horizon-shader-api/packs/borealis",
    ));
    std::fs::create_dir_all("/tmp/probe").unwrap();
    for (mode, name) in [(0u8, "off"), (1, "garbage"), (2, "clean")] {
        let mut s = scene::dawn_ocean();
        s.reflection_mode = mode;
        let pixels = gpu.render(&s, 480, 270);
        let path = format!("/tmp/probe/reflection_{name}.png");
        encode_png(std::path::Path::new(&path), 480, 270, &pixels).unwrap();
        println!("rendered {path}");
    }
}
