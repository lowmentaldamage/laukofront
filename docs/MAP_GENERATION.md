# Map generation

laukofront generates its own tile maps (upstream OpenFront ships hand-made maps; see D20).
Code: `src/engine_lib/terrain.rs`, `generate(&TerrainParams, seed)`. Preview tool:
`cargo run --release --bin map_preview -- --width 1200 --height 750 --seed 7 --out map.bmp`.

## Tile format (ported from upstream `GameMapImpl`)

One terrain byte per tile: bit 7 land, bit 6 shoreline, bit 5 ocean, bits 0–4 magnitude.
Magnitude < 10 Plains, < 20 Highland, < 31 Mountain, 31 Impassable (not generated yet).
Ownership lives in a separate `u16` per tile (owner small id in bits 0–11).

## Sizes

| Size | Tiles | Generation (release, dev PC) |
|---|---|---|
| Small | 800 × 500 | 31 ms |
| Medium | 1200 × 750 | 71 ms |
| Large | 1600 × 1000 | 121 ms |

## Algorithm

1. **Noise.** Fractal value noise: an integer hash (murmur3 finaliser) gives a value at each
   lattice point; smoothstep interpolation between them; 6 octaves, each at twice the frequency
   and half the weight. The first octave has 4 cells across the shorter side.
2. **Edge falloff.** Elevation minus `0.45 × smoothstep` of a distance that blends square and round
   distance to the map centre, over the outer 45% band. Continents get rounded corners and stay off
   the frame. The outermost tile ring is always water.
3. **Sea level.** A 16 384-bin histogram picks the level so 50% of tiles are land.
4. **Magnitude.** Height above sea level, normalised to 0..1, then `pow(h, 1.6) × 31`, capped at 30
   (most land is plains; mountains form the peaks). `pow` is the deterministic `detmath::pow`.
5. **Cleanup.** Land components under 60 tiles sink. Enclosed lakes under 40 tiles fill in as plains.
6. **Water kinds.** Water connected to the map edge is ocean; other water is lake. A tile with a
   4-neighbour of the other kind (land/water) gets the shoreline bit.

## Seed behaviour

The seed feeds the ported sfc32 `PseudoRandom`, which draws one hash salt per octave. Everything
else is integer hashing, `+ - * /`, `sqrt` (correctly rounded by IEEE 754) and `detmath`, so the
same seed and size give the same map on every machine.

## Tests (`src/engine_lib/terrain.rs`)

Same seed → same bytes; different seed → different bytes; land share near target; map edge is
ocean; shoreline bits match neighbours; plains, highland and mountain all occur.
