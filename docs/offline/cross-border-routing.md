# Cross-border routing: Moldova → Romania

Status: research note for global maps v1. No code. 2026-11-17.

## Why one `graph.bin` per country cannot route today

`Graph` is one CSR with one `u32` id space: `nodes`, CSR `offsets` (n+1),
`edges` grouped by `from`, and one interned `names` table. The builder assigns
node ids in way order; `Node.osm_id` is the only cross-file identity. `route()`
takes one `&Graph` and `LocalRouter` loads one path, so two country files are
never opened together.

Concatenation must shift B's endpoints by `n_A` and B's `name` ids by
`names_A.len()` (except `NO_NAME`), then rebuild `offsets`. `Graph::load`
checks nondecreasing `from`, but an aliased graph can pass it: the failure is
silent wrong nodes and street names, not a rejected file. Duplicate names only
cost table bytes; a missed name offset resolves the wrong street. Fields are
private, so a merge needs a crate API.

There is also no seam. Geofabrik keeps crossing ways complete, so both PBFs
usually share OSM nodes and the Moldovan graph can include a fringe of Romanian
road. Two `Graph`s still have no joining edge, and a naive merge duplicates
every crossing way. A dead-end stub at the river, a border node present in only
one extract, or a node missing from one snapshot leaves no join; `load_pbf`
breaks the chain at a missing node rather than guessing.

Near the border this fails silently. `nearest_node` scans all nodes and
`MAX_SNAP_M` is 25 km, so a Romanian destination within 25 km of a Moldovan node
(Iași is ~18 km from the Prut) snaps and returns a route to the fringe edge.
The UI draws a plausible line and no error.

## Options

| Option | Gain | Cost |
|---|---|---|
| (a) Merge PBFs before build | True connectivity; no router change | Dedupe duplicate objects; MD+RO ≈450 MiB; MD+RO+UA ≈1.35 GiB |
| (b) Runtime concat + border stitch | Keeps per-country downloads | New graph/route API, offsets, border index, multi-graph A*, maneuver stitching; memory = both graphs |
| (c) Europe / macro-region | No seam | Europe ~36 GiB graph; a South-East Europe region ~2.5 GiB, over the 2 GiB cap |
| (d) Online fallback | Global coverage | Breaks the no-network invariant; CSP, privacy, attribution, hosting; excluded from v1 |

Sizes: Moldova 1.49M nodes / 3.12M edges / 105.7 MiB; Romania ~350 MiB and
~4.9M nodes (1.09x PBF and Moldova node density). A* uses 25 B/node, so MD+RO
adds ~160 MB of arrays to ~450 MiB; MD+RO+UA peaks near 1.9 GB. Moldova loads
in 0.27 s on desktop; a merged pack is estimated seconds on a phone. Memory,
not query time, is the blocker.

## Recommendation

**v1: do not stitch.** Keep one active country graph and make the boundary
honest: ship a simplified country polygon (a few KB, from OSM admin boundaries;
the Geofabrik `.poly` is a buffered clipping boundary, not the border) next to
`graph.bin`, and reject endpoints outside it before snapping. A bbox is not
enough — Moldova's bbox corner is in Romania and Ukraine. Outside → 404
`OUTSIDE_COVERAGE`: "Outside the offline area — download Romania to route
there." Never draw a partial route. Switch packs only when both endpoints are
in one installed country; otherwise say cross-border routing is unavailable.
Keep `GRAPH_FORMAT_VERSION` 1 and the `Router` seam.

**v2, cheapest: a merged MD+RO macro-region as its own catalog region.** This
is option (a) for two countries, not Europe. Build from a combined polygon cut
from `europe-latest`, or dedupe the two PBFs using
`osmium cat --clean=duplicate`. Existing `build`/`route` work unchanged; the
app picks the macro pack when both endpoints are inside it, otherwise the
country pack. A cross-border query loads ~450 MiB — the same bytes as both
country graphs — with no seam code. Add `ro-md` to `regions.json`/catalog; add
Ukraine only after a device memory measurement.

Two-graph A* with border handoff is the fallback only if per-country packs must
stay the sole unit. It needs an `osm_id` border index (complete-way overlap can
give thousands of shared nodes), offsetting, a combined view, path/maneuver
stitching, and a rule for multiple crossings. It saves no memory and breaks
when packs have different dates.

## Tests and fixtures

- Synthetic A (osm 1,2,3) + B (osm 3,4,5) sharing node 3: route 1→5 crosses
  once, contains 3 once, names resolve after offset, `NO_NAME` preserved.
- Broken seam: no shared `osm_id`, nearest nodes 300 m apart → `NoPath`, never
  a synthetic straight line.
- Near-border: only A installed, destination 10 km into B but within 25 km of
  node 3 → `OutsideCoverage`, not a route ending at node 3.
- One-way crossing: forward succeeds, reverse `NoPath`.
- Merge: zero duplicate ids after dedupe; shared coordinates equal; offsets
  monotonic, endpoints and name ids in range.

## Top 3 risks

1. **Silent partial routes.** 25 km snap + bbox-only coverage returns a
   border-truncated route as success. Fix in v1 with the polygon gate.
2. **Phone memory.** MD+RO ~450 MiB graph + ~160 MB arrays; +UA ~1.9 GB peak.
   Low-end Android will OOM/ANR; measure, then mmap or compact the graph.
3. **Seam integrity.** Packs from different dates can move or lose a shared
   node; a stitched route can jump, duplicate a road, or refuse to connect.
   Build from one snapshot or pin both packs to one date.
