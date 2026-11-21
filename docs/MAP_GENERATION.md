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
5. **Style** (`--map`):

   | Style | Change |
   |---|---|
   | continents | as above (default) |
   | archipelago | 70% of the land, 3× the noise cells (many islands), islands down to 30 tiles |
   | pangaea | 90% of the land, 0.6× the cells, wider edge falloff, only the biggest landmass kept |
   | lakes | 130% of the land; a second, finer noise field turns low land into lakes |

6. **Rivers.** Per 400 000 tiles: 6 (continents), 2 (archipelago), 9 (pangaea), 4 (lakes). Each starts
   at the highest of 9 random land tiles and steps to the lowest unvisited neighbour until it
   reaches water; walks that get stuck, run past half the map or are shorter than 25 tiles are
   dropped. River tiles are water (ocean when they reach the sea), so boats can use them and land
   attacks need a boat to cross, as on upstream's maps.
7. **Peaks.** The highest 0.6% of land (`terrain.peak_share`) becomes impassable.
8. **Cleanup.** Land components under 60 tiles sink. Enclosed lakes under 40 tiles fill in as plains.
9. **Water kinds.** Water connected to the map edge is ocean; other water is lake. A tile with a
   4-neighbour of the other kind (land/water) gets the shoreline bit.
10. **Biomes** (colours only, never rules). Generated maps are drawn as if they spanned 70° N to
    70° S. Latitude (wavy, ±9°) and a moisture noise with a dry belt near 25° and wet tropics pick
    snow, tundra, taiga, temperate, forest, steppe, desert or jungle; high ground near the poles is
    snow.

## Real-Earth maps

`world`, `europe`, `asia`, `africa`, `americas`, `british-isles` come from **Natural Earth** 1:50m
land and lakes (public domain). `cargo run --release --bin earth_import -- --land ne_50m_land.shp
--lakes ne_50m_lakes.shp` reads the shapefiles (own minimal reader), projects each map's bounds
equirectangularly (sizes chosen so a tile is about as wide as it is tall in the middle of the map),
fills polygons with even-odd scanlines at tile centres, cuts out lakes and writes
`assets/maps/<map>.lkmap`: a land mask, run-length encoded (84 KB for all six). The files are built
into the game. At match start the mask gets noise heights (the source has none), the same cleanup
and water kinds as generated maps (islands under 12 tiles, lakes under 20), and biomes from the
real latitudes. The shapefiles themselves are not in the repository.

## Seed behaviour

The seed feeds the ported sfc32 `PseudoRandom`, which draws one hash salt per octave. Everything
else is integer hashing, `+ - * /`, `sqrt` (correctly rounded by IEEE 754) and `detmath`, so the
same seed and size give the same map on every machine.

## Tests

`src/engine_lib/terrain.rs`: same seed → same bytes; different seed → different bytes; land share
near target; map edge is ocean; shoreline bits match neighbours; plains, highland and mountain all
occur. `src/engine_lib/earth.rs`: mask round trip, corrupt files rejected, every shipped map
builds. `tests/engine_p11.rs`: styles differ as described, peaks are impassable and not counted,
rivers reach the sea, biomes follow latitude, real places are land or sea, every map plays, a ship
crosses the Atlantic in under 50 ms.
