<p align="center">
  <h1 align="center">Borealis</h1>
  <p align="center">A shader pack for <a href="https://github.com/horizon-rust/horizon">Horizon</a>'s own shader API, validated by a golden-image gallery.</p>
</p>

<p align="center">
  <a href="https://github.com/APPLEPIE6969/horizon-borealis/actions/workflows/ci.yml"><img src="https://github.com/APPLEPIE6969/horizon-borealis/actions/workflows/ci.yml/badge.svg?branch=main" alt="checks"></a>
  <img src="https://img.shields.io/badge/shader%20API-horizon--shaders.toml%20%2B%20WGSL-2A2470" alt="shader API">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-3B2F7E" alt="MIT license"></a>
</p>

---

Borealis is an Enhanced-grade shader pack written for Horizon's shader API
(`horizon-shaders.toml` + WGSL): a physical single-scattering sky, soft sun and
moon shadows that keep the colour of whatever they pass through, waving plants,
rolling and reflecting water, coloured block light, bloom, and a filmic grade
with a choice of tone curves. Parts are ported from
[Photon Shaders](https://github.com/sixthsurge/photon) (MIT); the rest is
original MIT work. The full provenance of every ported part is documented in
[the pack's own README](crates/horizon-shader-api/packs/borealis/README.md).

## Where the pack lives

| Path | What it is |
|---|---|
| [`crates/horizon-shader-api/packs/borealis/`](crates/horizon-shader-api/packs/borealis/) | **The pack**: `sky.wgsl`, `material.wgsl`, `weather.wgsl`, the post chain (`post/`), shared includes (`lib/`), settings (`horizon-shaders.toml`), licence and provenance |
| [`crates/horizon-shader-api/`](crates/horizon-shader-api/) | The pack format: parser, validator and the `horizon-shader` CLI (`check` / `pack`) |
| [`crates/horizon-shader-gallery/`](crates/horizon-shader-gallery/) | Headless wgpu renderer for the packs: the scene gallery and the frozen golden-image tests |
| [`docs/api/shaders.md`](docs/api/shaders.md) · [`docs/api/shaders-reference.md`](docs/api/shaders-reference.md) | The pack format and its reference |
| [`docs/shaders.md`](docs/shaders.md) | How the pipeline stages run in the game |

## Why this repository contains the whole Horizon workspace

The pack format, its validator, the `.hshader` packager and the gallery are
crates of the [Horizon workspace](https://github.com/horizon-rust/horizon)
(MIT), and the golden images are only meaningful against the exact renderer
the game ships. The workspace is therefore checked in verbatim so the pack is
always validated against the real toolchain, not a partial copy: the pack and
the gallery are this repository's deliverable, everything around them is the
harness they are tested inside.

## Validation

Every push runs the `checks` workflow (`.github/workflows/ci.yml`):

| Job | What it does |
|---|---|
| `verify` | `cargo fmt` + `clippy --workspace --all-targets -D warnings`, the full Rust test suite, doc tests, and the workspace's Python tooling tests |
| `cross` | `clippy + test` on `windows-latest` and `macos-latest` |
| `gpu-golden` | Renders the gallery scenes on pinned SwiftShader (chrome-headless-shell 156.0.8078.80) and compares against the frozen golden images |
| `shader-validation` | `horizon-shader check` on every pack, then packages borealis twice and requires byte-identical `.hshader` output |
| `doc` | `cargo doc --workspace --no-deps` |
| `msrv` | `cargo check` on the declared per-crate MSRV (1.95) |
| `deny` | `cargo deny` (licenses, bans, sources) |

## Working on the pack

```sh
# validate (what the game, launcher and registry run)
cargo run -p horizon-shader-api --bin horizon-shader -- check crates/horizon-shader-api/packs/borealis --json

# package (reproducible: two runs are byte-identical)
cargo run -p horizon-shader-api --bin horizon-shader -- pack crates/horizon-shader-api/packs/borealis --out borealis.hshader

# render the gallery / run the golden-image tests (needs a Vulkan adapter; SwiftShader works)
cargo test -p horizon-shader-gallery
```

## Licence

MIT — see [LICENSE](LICENSE). Ported portions carry their upstream licences:
Photon Shaders by SixthSurge (MIT, reproduced in
[the pack's `LICENSE`](crates/horizon-shader-api/packs/borealis/LICENSE)), the
minimal AgX curve by Benjamin Wrensch (MIT), and the published tone-curve fits
credited in the pack README. [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)
collects the workspace-wide notices.

> Not an official Minecraft product, nor approved by or associated with Mojang
> or Microsoft. The upstream Horizon project contains no Minecraft code,
> assets or data.
