# Map thumbnails

Map thumbnails have one canonical generation path:

```text
maps/<name>/map.bin
        ↓
sow_map::terrain_preview_image
        ↓
maps/<name>/thumbnail.webp
```

The thumbnail is regenerated while `./sow l` and `./sow p` package the web
release. Existing `thumbnail.webp` files are treated as build output; no
second thumbnail or manually copied OpenFront thumbnail is used.

The current output is a 512×512 preview. Aspect-ratio-aware presentation in
the lobby is a separate UI concern; the generator must remain the single
source of the map's visual style.
