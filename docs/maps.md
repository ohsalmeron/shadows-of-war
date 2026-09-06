# Maps, thumbnails, and mobile terrain budgets

Audited 2026-09-05. This document is the source of truth for the map and
thumbnail pipeline. The approved 16:9 framing is integrated into the official
web packaging path; the playable map grid remains independent.

## Two outputs with independent resolutions

`map.bin` is the playable grid: dimensions, packed terrain, spawns, and optional
geographic bounds. `map.bin.br` is its compressed transport form. A thumbnail
is a display image; its aspect ratio and resolution do not determine grid size.

Using a large world image to author a thumbnail does not require loading that
image as playable terrain or increasing any existing map's cell count. Changes
to thumbnails alone must preserve existing `map.bin` and `map.bin.br` hashes.
Creating a new map deliberately produces a new grid, constrained by a gameplay
budget chosen independently of the thumbnail frame.

## Visually accepted framing

Keep the regional view recognizable and fill a 16:9 image edge to edge. Extend
the source view with the actual neighboring terrain or sea where more coverage
is needed. Do not turn a regional map into an isolated rectangle surrounded by
an artificial water border.

- Retain the original regional source rectangle; enlarge its shorter dimension
  to reach 16:9. Do not center-crop that rectangle to square or to 16:9.
- For a portrait/narrow region, retain its full height and extend sideways.
- For a region wider than 16:9, retain its full width and extend vertically
  using source geography, without inserting blank top/bottom bars.
- Do not add a fixed percentage margin around the land, mask away neighboring
  countries, or flood-fill connected continents to choose the framing. That
  rejected experiment made Europe and Africa show almost the same continents.
- Land may continue across the **outer edge of the image**, as it does in a
  regional map. The rejected defect was an interior rectangular cut where
  existing land suddenly became synthetic sea. A check requiring zero land
  pixels at the image edge would enforce the wrong design.

In regional source-pixel coordinates, for original size `(w, h)`:

```text
frame_w = max(w, h * 16 / 9)
frame_h = max(h, w * 9 / 16)
left = (w - frame_w) / 2
top  = (h - frame_h) / 2
frame = [left, top, left + frame_w, top + frame_h]
```

Every original regional coordinate remains inside this expanded frame. Sample
its continuation from the larger source and resize once to the output image.
These framing calculations do not write gameplay dimensions.

## Reference source and provenance

The production thumbnail source is vendored as an authoring-only input at
`assets/map_sources/giantworldmap.png`. It is not copied into `dist/web`, the
release root, or Android. Its manifest is
`assets/map_sources/thumbnail_frames.json`.

- OpenFront checkout commit: `f51f165b947a92cc683ade3a72c3800300e86e61`.
- Image dimensions: **4110×1948 = 8,006,280 source pixels**.
- Image SHA-256: `f12ddca2fcdd795f13900ba5061da86408f9fa5a4b56e15f162e356c7bf6be19`.
- Regional sources: `../games/openfrontio/map-generator/assets/maps/<name>/image.png`
  and `info.json` under that pinned OpenFront `map-generator` directory.

The separate `../games/MapGenerator` checkout is research material. Its file
with the same `giantworldmap` name has different bytes; do not silently switch
between checkouts or assume same filenames mean identical source revisions.

The manifest stores one deterministic source rectangle per geographic map. It
uses the giant-world equirectangular coordinates and expands the playable
region to 16:9 without a square crop. World-wrap frames may cross the source's
left/right edge; the renderer wraps longitude. A frame that extends beyond the
source vertically is filled with ocean so no land is cut. Fictional Pangaea
has no Earth frame and therefore uses the complete `map.bin` preview as a
no-crop fallback.

The renderer in `sow-map/src/thumbnail.rs` reconstructs the OpenFront terrain
palette and shoreline/depth water once at build time, samples the frame, and
writes exactly one lossless **512×288 WebP** at
`maps/<name>/thumbnail.webp`. It never writes a second square thumbnail.

## Current generation and packaging code

| Responsibility | Existing implementation |
|---|---|
| Limits and dimension helpers | `sow-core/src/maps.rs` |
| PNG classification/downsampling | `sow-map/src/image_pipeline.rs` |
| `image-map` authoring | `sow-tools/src/image_map.rs`, `sow-tools/src/main.rs` |
| OpenFront import | `sow-tools/src/openfront_import.rs` |
| Grid export, compression, catalog refresh | `sow-tools/src/exporter.rs` |
| Palette, terrain preview, source framing, WebP output | `sow-map/src/thumbnail.rs` |
| Staged thumbnail regeneration | `sow-dist/src/main.rs::refresh_map_thumbnails` |

`./sow l` and `./sow p` copy maps into the web output and then run:

```text
staged map.bin + thumbnail_frames.json -> source frame -> thumbnail.webp
```

When a map is absent from the frame manifest, the pipeline falls back to its
complete terrain preview fitted into 16:9 without cropping. OpenFront imports
and exporters also write the same 16:9 shape. The main-menu lobby card uses
the same 16:9 aspect ratio and the existing single thumbnail URL.

`assets/maps/SOURCES.toml` records the playable recipe for every map: origin
hash and revision, target dimensions, pipeline settings, and hashes for
`map.bin`, `map.bin.br`, and `thumbnail.webp`. East Anglia is explicitly marked
as an existing-bin snapshot because its original authoring image is absent;
that map is not reproducible until its source is recovered. Pangaea's playable
bin was regenerated from the pinned source while its existing thumbnail hash
was preserved; East Anglia preserved both its existing bin and thumbnail.

All authoring routes now call `sow-map::image_pipeline::generate_from_rgba`.
The former parallel `sow-map/src/generator.rs` path was removed, so no second
terrain generator can silently produce different map bins.

`./sow l` is local visual preview; `./sow p` is the official Web/backend/maps
release path. Android publishing remains separate. Do not invent additional
`./sow` subcommands or manually copy thumbnails into production.

## Measured grid sizes and enforcement gaps

Headers in the current local map library after the canonical regeneration:

| Asset | Dimensions | Cells / source pixels | Role |
|---|---:|---:|---|
| Pangaea | 1000×1000 | 1,000,000 | Existing playable grid |
| World | 1408×704 | 991,232 | Existing playable grid |
| Europe | 1312×756 | 991,872 | Existing playable grid |
| East Anglia | 896×504 | 451,584 | Existing playable grid |
| giantworldmap PNG | 4110×1948 | 8,006,280 | Authoring reference image |

All 15 local grids are at or below 1,000,000 cells. Pangaea has 420,205 land
cells; MENA has 763,406 land cells in a 992,640-cell grid. Similar total sizes
do not guarantee similar simulation workloads. Pangaea's acceptable behavior
on slower phones is owner-reported; this documentation update ran no device
benchmark and did not reproduce the reported giant-map crash.

Current code is not a uniform hard gate:

- `MAX_MAP_PIXELS = 1_000_000` in `sow-core/src/maps.rs` is the total-cell cap.
- `MAX_MAP_AXIS = 1_000` is additionally used by the `image-map` path's
  `mobile_safe_dims`. That path downsizes proportionally and aligns to four.
- `import-openfront` uses a proportional total-cell cap, without the same
  per-axis limit. This helps explain why existing grids can have a side above
  1,000 while staying within the total budget.
- The editor rejects a grid over the total budget. `image_pipeline::generate_from_rgba`
  applies the mobile-safe clamp when `target_dims` is absent; explicit
  `target_dims` are caller-owned and must already satisfy the map budget.
- `image_pipeline` processes the full-resolution source before downsampling.
  Thus a large authoring source can cost build-machine memory even when the
  exported grid is small. Do not move that authoring work onto the phone.

Compressed download size is not the runtime terrain budget. Gameplay expands
the terrain and uses additional simulation/render state. Water cells count
toward the grid too; increasing both dimensions twofold creates four times as
many cells. Bots, land distribution, updates, rendering, and device capability
also affect performance. A one-million-cell cap is not a universal FPS guarantee.

## Stable workflow for new maps

Keep a reviewed, versioned recipe per map: source identity/hash, playable region,
target grid budget/dimensions, spawn inputs, and thumbnail frame. Keep source
images in `assets/map_sources/`, not the shipped game bundle. For geographic
maps, add a frame to `thumbnail_frames.json`; for fictional maps, let the
`map.bin` no-crop fallback render the complete source. Do not create a second
thumbnail asset for a different aspect ratio.

Generate two independent outputs from that recipe:

```text
approved source + playable region + cell budget -> map.bin + map.bin.br + catalog
approved source + expanded 16:9 visual frame    -> thumbnail.webp
```

The thumbnail's extra surrounding geography is visual context, not additional
playable territory. Regenerating thumbnails must not implicitly run terrain
export, restamp geographic bounds, rescale spawns, or change the catalog's grid
dimensions. For new maps, verify those gameplay outputs explicitly.

Use the existing Pangaea cell count as the current **upper bound** for new
playable grids, with an explicit lower per-map target when possible. The
thumbnail source does not participate in that budget. Never regenerate or
rescale an existing `map.bin` just to change its thumbnail; that would change
gameplay data unnecessarily.

For lobby delivery, the implementation uses **512×288**. An RGBA decode is
about 0.56 MiB per image; the 4110×1948 authoring source is never delivered to
the client. A higher-resolution reference does not require a larger delivered
thumbnail or playable grid.

Before calling the workflow stable, verify:

- Pinned source and generator settings reproduce identical artifact hashes in
  the supported toolchain; record dependencies that affect sampling/encoding.
- Original regional coverage fits the expanded 16:9 frame; no synthetic water
  border, square crop, or whole-continent selection returns. Source coverage
  and geographic alignment are valid; missing coverage is reported explicitly.
- All maps, including portrait, wide, world-wrap, and fictional cases, receive
  visual review; the three-map experiment alone is insufficient.
- Thumbnail-only runs leave gameplay hashes identical and produce one final
  thumbnail URL per map. The large source stays out of packaged client assets.
- Every new grid meets its chosen cell budget; spawns and land detail survive
  downsampling. Run a comparable local Android match against Pangaea using the
  same bot/player load, measuring load time, memory, frame/tick behavior, and
  crashes before claiming performance on low-end devices.
- Use the official pipeline for the eventual production implementation and
  release, after the local visual review. Documentation does not activate it.
