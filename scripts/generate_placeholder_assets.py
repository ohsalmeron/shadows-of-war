#!/usr/bin/env python3
"""Generate gray TBD placeholder assets for Shadows of War (replace AI/OpenFront art)."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Never overwrite restored original / AI art (see assets/SOURCES.toml).
SKIP_REL = {
    "sow-client/assets/factory.svg",
    "sow-client/assets/port.svg",
    "sow-client/assets/defense_post.svg",
    "sow-client/assets/atombomb.png",
    "sow-ui/assets/ui/sow-splash-desktop.webp",
    "sow-ui/assets/ui/sow-splash-mobile.webp",
    *(f"sow-ui/assets/avatars/{n}.webp" for n in [
        "caesar", "cleopatra", "ragnar", "sun_tzu", "alexander", "genghis_khan",
        "richard_the_lionheart", "vercingetorix", "boudica", "lady_six_sky",
        "leonidas", "napoleon", "null",
    ]),
    *(f"sow-ui/assets/ui/leaders/{n}_{form}.webp"
      for n in ("alexander", "caesar", "cleopatra", "genghis_khan", "ragnar", "sun_tzu")
      for form in ("desktop", "mobile")),
}

try:
    from PIL import Image, ImageDraw, ImageFont

    HAS_PIL = True
except ImportError:
    HAS_PIL = False

BG = (64, 64, 96, 255)
FG = (180, 180, 200, 255)


def png_rgb(size: int, label: str) -> bytes:
    if HAS_PIL:
        img = Image.new("RGBA", (size, size), BG)
        draw = ImageDraw.Draw(img)
        draw.text((size // 2, size // 2), label, fill=FG, anchor="mm")
        import io

        buf = io.BytesIO()
        img.save(buf, format="PNG")
        return buf.getvalue()

    # Minimal valid RGB PNG without Pillow
    def chunk(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    raw = b""
    row = b"\x00" + bytes(BG[:3]) * size
    for _ in range(size):
        raw += row
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def webp_rgba(width: int, height: int, label: str) -> bytes:
    if not HAS_PIL:
        raise SystemExit("Pillow required for WebP placeholders: pip install Pillow")
    img = Image.new("RGBA", (width, height), BG)
    draw = ImageDraw.Draw(img)
    draw.text((width // 2, height // 2), label, fill=FG, anchor="mm")
    import io

    buf = io.BytesIO()
    img.save(buf, format="WEBP", quality=80)
    return buf.getvalue()


def svg_placeholder(label: str, size: int = 64) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}">
  <rect width="{size}" height="{size}" fill="#404060"/>
  <text x="{size // 2}" y="{size // 2 + 4}" text-anchor="middle" fill="#b4b4c8" font-family="sans-serif" font-size="10">{label}</text>
</svg>
"""


def write(path: Path, data: bytes | str) -> None:
    rel = path.relative_to(ROOT).as_posix()
    if rel in SKIP_REL:
        print(f"  skip {rel}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, str):
        path.write_text(data, encoding="utf-8")
    else:
        path.write_bytes(data)
    print(f"  {path.relative_to(ROOT)}")


def main() -> None:
    client = ROOT / "sow-client" / "assets"
    ui = ROOT / "sow-ui" / "assets"

    print("sow-client SVG/PNG (sprite atlas uses subset)")
    svgs = [
        "city.svg",
        "factory.svg",
        "port.svg",
        "defense_post.svg",
        "missile_silo.svg",
        "trade_ship.svg",
        "transport_ship.svg",
        "battleship.svg",
        "star.svg",
        "handshake.svg",
        "back.svg",
    ]
    for name in svgs:
        label = name.replace(".svg", "").replace("_", " ")[:8].upper()
        write(client / name, svg_placeholder(label))

    pngs = ["atombomb.png", "sam_missile.png"]
    for name in pngs:
        label = name.replace(".png", "")[:6].upper()
        write(client / name, png_rgb(64, label))

    webps_client = ["request.webp", "handshake.webp", "betray.webp", "nameplate.webp"]
    for name in webps_client:
        write(client / name, webp_rgba(128, 128, "TBD"))

    print("sow-ui avatars")
    avatars = [
        "caesar",
        "cleopatra",
        "ragnar",
        "sun_tzu",
        "alexander",
        "genghis_khan",
        "richard_the_lionheart",
        "vercingetorix",
        "boudica",
        "lady_six_sky",
        "leonidas",
        "napoleon",
        "null",
    ]
    for name in avatars:
        write(ui / "avatars" / f"{name}.webp", webp_rgba(256, 256, name[:8].upper()))

    print("sow-ui splash / loader")
    write(ui / "ui" / "sow-splash-desktop.webp", webp_rgba(1920, 1080, "SOW"))
    write(ui / "ui" / "sow-splash-mobile.webp", webp_rgba(1080, 1920, "SOW"))
    write(ui / "ui" / "loader_empty.webp", webp_rgba(2064, 512, "LOAD"))
    write(ui / "ui" / "loader_full.webp", webp_rgba(2064, 512, "LOAD"))

    print("sow-ui hud")
    for name in ["hud_controls", "hud_battle_log", "hud_logs"]:
        write(ui / "ui" / "hud" / f"{name}.webp", webp_rgba(128, 128, "HUD"))

    print("sow-ui leader portraits")
    leaders = [
        "caesar",
        "cleopatra",
        "ragnar",
        "sun_tzu",
        "alexander",
        "genghis_khan",
        "richard_the_lionheart",
        "vercingetorix",
        "boudica",
        "lady_six_sky",
        "leonidas",
        "napoleon",
    ]
    for name in leaders:
        write(ui / "ui" / "leaders" / f"{name}_desktop.webp", webp_rgba(1920, 1080, name[:10]))
        write(ui / "ui" / "leaders" / f"{name}_mobile.webp", webp_rgba(1080, 1920, name[:10]))

    print("Done.")


if __name__ == "__main__":
    main()
