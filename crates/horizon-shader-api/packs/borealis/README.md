# Borealis

An [Enhanced](https://modrinth.com/shader)-grade shader pack written for
Horizon's own shader API (`horizon-shaders.toml` + WGSL): a physical sky, soft
sun and moon shadows that keep the colour of whatever they pass through,
rippling and reflecting water, colored block light, bloom and a filmic grade
with a choice of tone curves.

Check it with `horizon-shader check crates/horizon-shader-api/packs/borealis`.

## Where each part comes from

| Part | Origin |
|---|---|
| Stars (`lib/stars.wgsl`) | Ported from [Photon Shaders](https://github.com/sixthsurge/photon) by SixthSurge (`shaders/include/sky/stars.glsl`, with its `hash4` and `blackbody`): coloured stars by blackbody temperature, with twinkle, under Photon's license (reproduced in the pack's `LICENSE`), which permits redistributing modified portions. |
| Water waves (the water parts of `lib/waves.wgsl`) | Ported from Photon's Gerstner wave stack (`shaders/include/surface/water_normal.glsl`, credited by Photon to Belmu in #snippets, modified), under the same license. |
| AgX tone curve (`lib/tonemap.wgsl`) | The [minimal AgX](https://iolite-engine.com/blog_posts/minimal_agx_implementation) by Benjamin Wrensch (iolite-engine), MIT. |
| The physical sky (`lib/atmosphere.wgsl`) | This pack's own WGSL implementation of the standard single-scattering atmosphere (Rayleigh + Mie + ozone, the Hillaire-2020-style approach and the standard Earth constants). |
| Terrain lighting, shadows, fog, the material stage, the post chain, weather | This pack's own work, MIT. The shadow bias follows the technique Emin and Gri573 described publicly for the Complementary lineage. The other tone curves are the published ACES (Narkowicz), Lottes, Hable, Uchimura and Reinhard fits. |

The overall look aims at what the well-known "Enhanced" packs (Bliss, Photon,
Complementary) do with Minecraft's lighting; the pack began as a study of
Bliss's pipeline before settling on these sources. Nothing from a pack whose
license forbids redistribution was kept: Bliss ships as "all rights
reserved", so nothing of its code survives here.

## Compatibility

The pack uses only manifest fields a released game accepts — pinned by a test
(`uses_only_fields_the_released_game_accepts`). Horizon's `vertex` stage
(geometric waving of plants and water, Bliss-style) exists on upstream main but
ships in no release yet, so 0.1.1 leaves it out: water keeps its fragment-side
ripples, plants stay still. When a game release ships the stage, re-add
`[vertex]` (the code is in the v0.1.0 tag), the `wavy_plants` option and the
waving entries, and widen the test's field set.

## Options

The pack's settings screen has four pages: Lighting (shadows, the strength of
each kind of light, specular), Water (waves, reflections,
clarity), Sky & atmosphere (sun glow, stars, fog) and Post-processing (bloom,
exposure, tone mapping, saturation, contrast, vignette, night desaturation).

## Notes for maintainers

- `noise.png` is a generated 512×512 tiling value-noise texture (its blue
  channel feeds the water waves' phase offsets).
- The goldens include a `dawn` scene: a new world starts at sunrise, so the
  first minutes of play run the sun low — the regime most likely to regress
  unseen (0.1.2's low-sun sky lift and the water-reflection fallback fix both
  came out of a player's first-morning screenshot).
- The gallery and the golden images live in `crates/horizon-shader-gallery`:
  `cargo run -p horizon-shader-gallery --bin gallery -- crates/horizon-shader-api/packs/borealis <dir>`
  renders the gallery; `cargo test -p horizon-shader-gallery` checks the pack
  against the committed goldens (needs a GPU adapter — a software one, such as
  SwiftShader, is fine).
