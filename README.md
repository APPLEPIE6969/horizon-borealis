<p align="center">
  <h1 align="center">Borealis</h1>
  <p align="center">A shader pack for <a href="https://github.com/horizon-rust/horizon">Horizon</a>'s shader API, with the pack toolchain and a golden-image gallery in one small workspace.</p>
</p>

<p align="center">
  <a href="https://github.com/APPLEPIE6969/horizon-borealis/actions/workflows/ci.yml"><img src="https://github.com/APPLEPIE6969/horizon-borealis/actions/workflows/ci.yml/badge.svg?branch=main" alt="checks"></a>
  <img src="https://img.shields.io/badge/shader%20API-horizon--shaders.toml%20%2B%20WGSL-2A2470" alt="shader API">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-3B2F7E" alt="MIT license"></a>
</p>

---

Borealis is an Enhanced-grade shader pack written in WGSL for Horizon's shader
API (`horizon-shaders.toml` + WGSL): a physical single-scattering sky, soft sun
and moon shadows that keep the colour of whatever they pass through, rippling
and reflecting water, coloured block light, bloom, and a filmic grade with a
choice of tone curves. The shader code is this repository's own
work, with credited MIT ports from [Photon Shaders](https://github.com/sixthsurge/photon)
(see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)) — the full provenance of
every part is documented in [the pack's README](crates/horizon-shader-api/packs/borealis/README.md).

## Install in Horizon

1. Download `borealis-0.1.1.hshader` from the
   [latest release](https://github.com/APPLEPIE6969/horizon-borealis/releases/latest).
2. Drop it into the `shaderpacks` folder of your Horizon game folder — the folder
   that holds `options.txt` (create `shaderpacks` if it isn't there). If you launch
   with `--game-dir`, or `--player-dir` with several players in one game folder,
   the folder that holds *that* `options.txt` is the one that counts.
3. In game: **Options → Horizon Settings → Shader Packs** and pick **Borealis**.

Settings live on the pack's four pages (Lighting, Water, Sky & atmosphere,
Post-processing). What you change is saved next to the pack as
`borealis-0.1.1.hshader.txt`; keep that file beside the `.hshader` when you
upgrade and your settings carry over. Developers can also install the pack as a
plain folder — the one with `horizon-shaders.toml` in it — instead of a
`.hshader`; Horizon lists both, Horizon-format packs first.

## Layout

| Path | What it is |
|---|---|
| [`crates/horizon-shader-api/packs/borealis/`](crates/horizon-shader-api/packs/borealis/) | **The pack**: `sky.wgsl`, `material.wgsl`, `weather.wgsl`, the post chain (`post/`), shared includes (`lib/`), settings (`horizon-shaders.toml`), licence and provenance |
| [`crates/horizon-shader-api/`](crates/horizon-shader-api/) | The pack toolchain (Horizon's MIT crate, kept under its own licence): the manifest format, the WGSL parser/validator and the `horizon-shader` CLI (`new` / `check` / `pack`) |
| [`crates/horizon-shader-gallery/`](crates/horizon-shader-gallery/) | Headless wgpu renderer for the pack: the scene gallery and the frozen golden-image tests |
| [`crates/horizon-shader-api/packs/borealis/README.md`](crates/horizon-shader-api/packs/borealis/README.md) | Options, provenance and maintainer notes for the pack |

The pack format itself (manifest schema, stage pipeline, the check's budgets)
is documented in [`docs/api/shaders-reference.md`](docs/api/shaders-reference.md),
generated from the toolchain crate (a test fails when the file drifts);
`cargo doc --workspace --no-deps` renders the crate-level docs.

## Validation

Every push runs the `checks` workflow (`.github/workflows/ci.yml`):

| Job | What it does |
|---|---|
| `verify` | `cargo fmt` + `clippy --workspace --all-targets -D warnings`, then tests for the toolchain and the gallery harness (SDK, templates, packaging, WGSL probes) plus doc tests |
| `cross` | `clippy + test` on `windows-latest` and `macos-latest` |
| `gpu-golden` | Renders the gallery scenes (noon, sunset, night, underwater, shadows, rain) on pinned SwiftShader (chrome-headless-shell 156.0.8078.80) and compares against the frozen golden images |
| `shader-validation` | `horizon-shader check` on the pack, then packages borealis twice and requires byte-identical `.hshader` output |
| `doc` | `cargo doc --workspace --no-deps` |
| `msrv` | `cargo check` on the locked tree's real floor (rustc 1.95) |
| `deny` | `cargo deny` (licenses, bans, sources) |

## Working on the pack

```sh
# validate (the same check the game, launcher and registry run)
cargo run -p horizon-shader-api --bin horizon-shader -- check crates/horizon-shader-api/packs/borealis --json

# package (reproducible: two runs are byte-identical)
cargo run -p horizon-shader-api --bin horizon-shader -- pack crates/horizon-shader-api/packs/borealis --out borealis.hshader

# render the gallery / run the golden-image tests (needs a Vulkan adapter; SwiftShader works)
cargo test -p horizon-shader-gallery
```

## Licence

The pack, the gallery and the CI are MIT — see [LICENSE](LICENSE). The pack
toolchain crate is [Horizon](https://github.com/horizon-rust/horizon)'s MIT
code and keeps its own licence at
[`crates/horizon-shader-api/LICENSE`](crates/horizon-shader-api/LICENSE).
Ported WGSL portions carry their upstream licences (Photon Shaders by
SixthSurge, the minimal AgX curve by Benjamin Wrensch), collected in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

> This pack targets the look of Minecraft: Java Edition but contains no
> Minecraft code, assets or data, and is not an official Minecraft product,
> nor approved by or associated with Mojang or Microsoft.
