fn main() {
    let gpu = horizon_shader_gallery::harness::PackGpu::load(std::path::Path::new(
        "crates/horizon-shader-api/packs/borealis",
    ));
    let dir = "crates/horizon-shader-gallery/tests/golden";
    std::fs::create_dir_all(dir).unwrap();
    for build in [
        (
            "noon",
            horizon_shader_gallery::scene::noon_plains
                as fn() -> horizon_shader_gallery::scene::Scene,
        ),
        ("sunset", horizon_shader_gallery::scene::sunset_water),
        ("dawn", horizon_shader_gallery::scene::dawn),
        ("night", horizon_shader_gallery::scene::night),
        ("underwater", horizon_shader_gallery::scene::underwater),
        ("shadows", horizon_shader_gallery::scene::shadows),
        ("rain", horizon_shader_gallery::scene::rain),
    ] {
        let s = build.1();
        let pixels = gpu.render(&s, 480, 270);
        let path = format!("{dir}/{}.png", build.0);
        horizon_shader_gallery::harness::encode_png(std::path::Path::new(&path), 480, 270, &pixels)
            .unwrap();
        println!("frozen {}", path);
    }
}
