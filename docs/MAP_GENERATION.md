# Map generation

Code: `src/map/mod.rs`, `Map::generate(size, candidates_per_point, seed)`.

## Inputs

| Size | Raster | Territories | Cells per territory (avg) |
|---|---|---|---|
| Small | 192 × 120 | 80 | 288 |
| Medium | 256 × 160 | 150 | 273 |
| Large | 352 × 220 | 300 | 258 |

`candidates_per_point = 10` (`map.candidates_per_point`).

## Algorithm

1. **Seed points (best-candidate sampling).** For each territory, draw 10 random cells; skip
   cells already used by a seed; keep the one whose squared distance to the nearest existing seed
   is largest. The first seed has no competitors, so its first free candidate wins. If all 10
   candidates are used cells (only on very dense maps), take the next free cell after a random
   start. Result: well-spread, organic-looking points on distinct cells.
2. **Assign cells (raster Voronoi).** Every cell goes to the seed with the smallest integer
   squared distance. Ties go to the lower territory id. Brute force: cells × territories
   (23 M distance checks for Large, about 16 ms in release).
3. **Repair connectivity.** Breadth-first search from every seed over cells of the same id marks
   what each territory reaches. Any unreached cell (a rare tie artefact) is handed to an adjacent
   reached cell's territory, repeated until every cell is reached. Afterwards every territory is
   one 4-connected blob containing its own seed, so no territory is empty or split.
4. **Adjacency.** Each cell is compared with its right and lower neighbour. Different ids form a
   pair `(min, max)`. Pairs are sorted and deduplicated, then stored both ways in a CSR layout
   (`neighbor_offsets`, `neighbor_list`). Each list is sorted, has no duplicates, never contains
   the territory itself, and is symmetric by construction.
5. **Stats.** Cell count (`area`) and coordinate sums (for `centroid`) per territory.

## Seed behaviour

- The seed is a `u64`. All randomness comes from `core::Rng` (SplitMix64, defined in this
  repository), and all geometry is integer maths. The same seed and size always give the same map,
  on any machine and with any dependency versions.
- Match setup (neutral troops, start positions) uses a separate stream, `seed ^ 0x5EED0F5E70000000`,
  so changing setup rules never changes the map for a seed.
- `Map::content_hash()` (FNV-1a over raster and adjacency) identifies a map in tests.

## Neighbour rules

- Two territories are neighbours when at least one cell of one shares an edge with a cell of the
  other. Diagonal-only contact does **not** count.
- The map border is not a territory; edge territories simply have fewer neighbours.
- With 2+ territories every territory has at least one neighbour, and the whole graph is connected
  (the raster is connected and every cell belongs to a territory).

## Isolated or empty territories

- Empty: impossible. Each seed's own cell has distance 0 to it and no other seed shares the cell.
- Split: removed by the repair pass (step 3).
- Isolated: impossible for 2+ territories (see above).

## How it is tested (`tests/map_tests.rs`)

- Symmetry on all three sizes with four seeds.
- Lists sorted, unique, without self, never empty.
- Same seed → identical cells and hash; different seed → different hash.
- Territory count and cell count match the config; areas sum to the cell count.
- Every territory is one connected blob containing its seed (flood fill, two seeds × three sizes).
- The territory graph is connected.
- A dense 4×3 map with 12 territories gives one cell each; 13 territories is an error.

## Fallbacks

- If generation gets too slow for bigger maps: bucket the seeds in a coarse grid and search only
  nearby buckets in step 2.
