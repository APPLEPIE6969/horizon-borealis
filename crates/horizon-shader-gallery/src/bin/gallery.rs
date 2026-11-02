//! Renders the gallery: every crafted scene through a Horizon shader pack, as
//! PNGs, for eyeballing the look. `cargo run -p horizon-shader-gallery --bin
//! gallery -- <pack dir> <output dir> [scale]`.

use horizon_shader_gallery::harness::{PackGpu, encode_png};
use horizon_shader_gallery::scene;

fn main() {
    let mut args = std::env::args().skip(1);
    let pack_dir = args
        .next()
        .unwrap_or_else(|| "crates/horizon-shader-api/packs/borealis".to_string());
    let out_dir = args.next().unwrap_or_else(|| "gallery".to_string());
    let scale: u32 = args.next().map(|s| s.parse().unwrap()).unwrap_or(2);

    let gpu = PackGpu::load(std::path::Path::new(&pack_dir));
    let scenes = [
        scene::noon_plains(),
        scene::dawn(),
        scene::sunset_water(),
        scene::night(),
        scene::underwater(),
        scene::shadows(),
        scene::rain(),
    ];
    let w = 480 * scale;
    let h = 270 * scale;
    std::fs::create_dir_all(&out_dir).unwrap();
    for s in &scenes {
        let t0 = std::time::Instant::now();
        let pixels = gpu.render(s, w, h);
        let path = std::path::Path::new(&out_dir).join(format!("{}.png", s.name));
        encode_png(&path, w, h, &pixels).unwrap();
        println!(
            "{} -> {} ({}x{}, {:?})",
            s.name,
            path.display(),
            w,
            h,
            t0.elapsed()
        );
    }
}
