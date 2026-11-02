//! The pack passes Horizon's own check, and its manifest and options hold
//! together. These run everywhere (no GPU needed).

use horizon_shader_api::{Pack, check};
use std::path::Path;

const PACK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../horizon-shader-api/packs/borealis"
);

fn pack() -> Pack {
    Pack::read(Path::new(PACK)).unwrap()
}

#[test]
fn the_pack_passes_the_check() {
    let report = check(&pack())
        .unwrap_or_else(|problems| panic!("borealis failed the check: {problems:#?}"));
    // Every stage the pack declares, checked and within budget.
    let labels: Vec<_> = report.stages.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(
        labels.len(),
        8,
        "sky, vertex, material, weather and 4 post passes: {labels:?}"
    );
    assert!(
        report.stages.iter().all(|s| s.expressions <= 20_000),
        "expressions over budget"
    );
    assert!(
        report.stages.iter().all(|s| s.texture_reads <= 64),
        "texture reads over budget"
    );
}

#[test]
fn the_manifest_holds_together() {
    let pack = pack();
    assert_eq!(pack.manifest.id, "borealis");
    assert_eq!(pack.manifest.api, "0.1");
    // The options the pages name all exist, and every option is on a page.
    let option_ids: std::collections::HashSet<_> = pack
        .manifest
        .options
        .iter()
        .map(|o| o.id.as_str())
        .collect();
    assert!(!option_ids.is_empty());
    for page in &pack.manifest.pages {
        for item in &page.items {
            match item {
                horizon_shader_api::PageItem::Page(i) => {
                    assert!(
                        *i < pack.manifest.pages.len(),
                        "page index {i} out of range"
                    );
                }
                horizon_shader_api::PageItem::Option(i) => {
                    assert!(
                        *i < pack.manifest.options.len(),
                        "option index {i} out of range"
                    );
                }
                horizon_shader_api::PageItem::Empty => {}
            }
        }
    }
    // Every option sits on some page, so the settings screen reaches it.
    assert!(
        pack.manifest.options.iter().enumerate().all(|(i, _)| pack
            .manifest
            .pages
            .iter()
            .any(|p| p.items.contains(&horizon_shader_api::PageItem::Option(i)))),
        "an option is on no page"
    );
    // The shader stages reference what the manifest declares.
    assert!(
        pack.source_bytes("noise.png").is_some(),
        "the noise texture ships with the pack"
    );
    assert!(
        pack.manifest.shadow.is_some(),
        "the pack asks for a shadow map"
    );
    assert!(
        pack.manifest.reflection.is_some(),
        "the pack asks for the water mirror"
    );
}

#[test]
fn the_pack_builds_as_an_hshader() {
    // The deterministic zip the launcher installs, built without writing.
    let pack = pack();
    let sha = pack.sha256().unwrap();
    assert_eq!(sha.len(), 64);
    let again = pack.sha256().unwrap();
    assert_eq!(sha, again, "the package's hash is stable");
}
