# Third-party notices

Everything in this repository is original work except the pieces below, each
of which keeps its upstream license.

| What | Where | Origin and license |
|---|---|---|
| The shader-pack format and toolchain | `crates/horizon-shader-api` | [Horizon](https://github.com/horizon-rust/horizon)'s shader-API crate (the manifest format, the WGSL parser/checker and the `horizon-shader` tool), used unmodified under MIT — see [`crates/horizon-shader-api/LICENSE`](crates/horizon-shader-api/LICENSE). The Borealis pack's WGSL is not Horizon code. |
| Stars | `crates/horizon-shader-api/packs/borealis/lib/stars.wgsl` | Ported from [Photon Shaders](https://github.com/sixthsurge/photon) by SixthSurge (`shaders/include/sky/stars.glsl`, with its `hash4` and `blackbody`), MIT — reproduced in [the pack's `LICENSE`](crates/horizon-shader-api/packs/borealis/LICENSE). |
| Water waves | the water parts of `crates/horizon-shader-api/packs/borealis/lib/waves.wgsl` | Ported from Photon's Gerstner wave stack (`shaders/include/surface/water_normal.glsl`, credited by Photon to Belmu), MIT. |
| AgX tone curve | `crates/horizon-shader-api/packs/borealis/lib/tonemap.wgsl` | The [minimal AgX](https://iolite-engine.com/blog_posts/minimal_agx_implementation) by Benjamin Wrensch (iolite-engine), MIT. |
| Other tone curves | `crates/horizon-shader-api/packs/borealis/lib/tonemap.wgsl` | The published ACES (Narkowicz), Lottes, Hable, Uchimura and Reinhard fits — credited in [the pack's README](crates/horizon-shader-api/packs/borealis/README.md). |

Rust dependencies come from crates.io (`naga`, `wgpu`, `serde`, …); CI runs
`cargo deny` over their licenses on every push.

This pack targets Minecraft: Java Edition's look but contains no Minecraft
code, assets or data, and is not an official Minecraft product, nor approved
by or associated with Mojang or Microsoft.
