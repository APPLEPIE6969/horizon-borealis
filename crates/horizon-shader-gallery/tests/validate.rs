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
        7,
        "sky, material, weather and 4 post passes: {labels:?}"
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

// The released game rejects any manifest field it doesn't know: Horizon
// 0.2.97's "unknown field" error names exactly the fields below. `vertex`
// (geometric waving) exists only on upstream main and ships in no release, so
// a manifest that uses it loads nowhere but a toolchain build — the trap the
// 0.1.0 pack fell into. Pin the pack to the released set here; widen it only
// when a game release actually ships the new field.
#[test]
fn uses_only_fields_the_released_game_accepts() {
    const RELEASED_GAME_FIELDS: &[&str] = &[
        "schema",
        "id",
        "name",
        "version",
        "api",
        "description",
        "authors",
        "license",
        "shared",
        "sky",
        "material",
        "weather",
        "post",
        "options",
        "pages",
        "tags",
        "materials",
        "textures",
        "lights",
        "budget",
        "reflection",
        "shadow",
    ];
    let text = std::fs::read_to_string(Path::new(PACK).join("horizon-shaders.toml")).unwrap();
    let table = text.parse::<toml::Table>().unwrap();
    let unknown: Vec<_> = table
        .keys()
        .filter(|k| !RELEASED_GAME_FIELDS.contains(&k.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "fields the released game rejects (main-only or misspelled): {unknown:?}"
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
