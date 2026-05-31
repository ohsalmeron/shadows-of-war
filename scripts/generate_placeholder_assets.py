#!/usr/bin/env python3
"""Generate themed placeholder assets for Shadows of War (no TBD gray boxes)."""

from __future__ import annotations

import random
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

SKIP_REL = {
    "sow-client/assets/factory.svg",
    "sow-client/assets/port.svg",
    "sow-client/assets/defense_post.svg",
    "sow-client/assets/atombomb.png",
    "sow-ui/assets/ui/sow-splash-desktop.webp",
    "sow-ui/assets/ui/sow-splash-mobile.webp",
}

MIN_FINAL_PORTRAIT_BYTES = 50_000

PLACEHOLDER_AVATAR_KEYS = frozenset({
    "richard_the_lionheart",
    "vercingetorix",
    "boudica",
    "lady_six_sky",
    "leonidas",
    "napoleon",
    "null",
})

LEADER_FILLER_RGB: dict[str, tuple[int, int, int]] = {
    "caesar": (191, 38, 46),
    "cleopatra": (217, 166, 38),
    "ragnar": (38, 89, 166),
    "sun_tzu": (38, 140, 107),
    "alexander": (56, 115, 199),
    "genghis_khan": (140, 107, 56),
    "richard_the_lionheart": (184, 46, 38),
    "vercingetorix": (71, 133, 56),
    "boudica": (224, 107, 31),
    "lady_six_sky": (31, 148, 133),
    "leonidas": (158, 107, 56),
    "napoleon": (46, 71, 173),
    "null": (64, 64, 96),
}

LEADER_INITIALS: dict[str, str] = {
    "richard_the_lionheart": "R",
    "vercingetorix": "V",
    "boudica": "B",
    "lady_six_sky": "6",
    "leonidas": "L",
    "napoleon": "N",
}

SVG_THEMES: dict[str, str] = {
    "city": "#5a6a8a",
    "missile_silo": "#6a5a4a",
    "trade_ship": "#4a7a9a",
    "transport_ship": "#3a6a8a",
    "battleship": "#2a4a6a",
    "star": "#c8a030",
    "handshake": "#8a9a6a",
    "back": "#7a8a9a",
}

try:
    from PIL import Image, ImageDraw, ImageFont

    HAS_PIL = True
except ImportError:
    HAS_PIL = False

DARK = (18, 22, 38)
MID = (36, 48, 72)
ACCENT = (180, 150, 60)


def png_rgb(size: int, base: tuple[int, int, int]) -> bytes:
    if HAS_PIL:
        img = Image.new("RGBA", (size, size))
        draw = ImageDraw.Draw(img)
        for y in range(size):
            t = y / max(size - 1, 1)
            r = int(base[0] * (1 - t) + base[0] * 0.6 * t)
            g = int(base[1] * (1 - t) + base[1] * 0.6 * t)
            b = int(base[2] * (1 - t) + base[2] * 0.6 * t)
            draw.line([(0, y), (size, y)], fill=(r, g, b, 255))
        cx, cy = size // 2, size // 2
        draw.ellipse([cx - 14, cy - 14, cx + 14, cy + 14], fill=(220, 200, 120, 255))
        import io

        buf = io.BytesIO()
        img.save(buf, format="PNG")
        return buf.getvalue()

    def chunk(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    raw = b""
    row = b"\x00" + bytes(base) * size
    for _ in range(size):
        raw += row
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def gradient_canvas(width: int, height: int, top: tuple[int, int, int], bottom: tuple[int, int, int]):
    if not HAS_PIL:
        raise SystemExit("Pillow required: pip install Pillow")
    img = Image.new("RGBA", (width, height))
    draw = ImageDraw.Draw(img)
    for y in range(height):
        t = y / max(height - 1, 1)
        color = (
            int(top[0] * (1 - t) + bottom[0] * t),
            int(top[1] * (1 - t) + bottom[1] * t),
            int(top[2] * (1 - t) + bottom[2] * t),
            255,
        )
        draw.line([(0, y), (width, y)], fill=color)
    return img, draw


def webp_panel(width: int, height: int, accent: tuple[int, int, int] | None = None) -> bytes:
    accent = accent or ACCENT
    img, draw = gradient_canvas(width, height, DARK, MID)
    draw.rectangle([0, 0, width - 1, height - 1], outline=(*accent, 180), width=2)
    rng = random.Random(width * height)
    for _ in range(max(40, (width * height) // 8000)):
        x, y = rng.randint(0, width - 1), rng.randint(0, height - 1)
        draw.point((x, y), fill=(accent[0], accent[1], accent[2], rng.randint(20, 50)))
    import io

    buf = io.BytesIO()
    img.save(buf, format="WEBP", quality=85)
    return buf.getvalue()


def webp_loader_bar(width: int, height: int, filled: bool) -> bytes:
    img, draw = gradient_canvas(width, height, (12, 16, 28), (28, 36, 56))
    bar_h = height // 3
    y0 = (height - bar_h) // 2
    draw.rounded_rectangle([24, y0, width - 24, y0 + bar_h], radius=8, fill=(40, 52, 78, 255))
    if filled:
        draw.rounded_rectangle([24, y0, width - 24, y0 + bar_h], radius=8, fill=(140, 110, 40, 255))
    import io

    buf = io.BytesIO()
    img.save(buf, format="WEBP", quality=85)
    return buf.getvalue()


def webp_leader_portrait(width: int, height: int, leader_key: str) -> bytes:
    r, g, b = LEADER_FILLER_RGB.get(leader_key, LEADER_FILLER_RGB["null"])
    top = (max(0, r - 40), max(0, g - 40), max(0, b - 40))
    bottom = (min(255, r + 20), min(255, g + 20), min(255, b + 20))
    img, draw = gradient_canvas(width, height, top, bottom)
    margin = min(width, height) // 16
    draw.rectangle(
        [margin, margin, width - margin, height - margin],
        outline=(220, 190, 90, 220),
        width=max(3, min(width, height) // 120),
    )
    initial = LEADER_INITIALS.get(leader_key, leader_key[:1].upper())
    font_size = min(width, height) // 3
    try:
        font = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSerif-Bold.ttf", font_size)
    except OSError:
        font = ImageFont.load_default()
    bbox = draw.textbbox((0, 0), initial, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    draw.text(
        ((width - tw) // 2, (height - th) // 2 - th // 4),
        initial,
        fill=(240, 220, 160, 255),
        font=font,
    )
    import io

    buf = io.BytesIO()
    img.save(buf, format="WEBP", quality=88)
    return buf.getvalue()


def webp_avatar(leader_key: str) -> bytes:
    r, g, b = LEADER_FILLER_RGB.get(leader_key, LEADER_FILLER_RGB["null"])
    img, draw = gradient_canvas(256, 256, (r, g, b), (max(0, r - 50), max(0, g - 50), max(0, b - 50)))
    draw.ellipse([32, 32, 224, 224], outline=(220, 190, 90, 200), width=4)
    initial = LEADER_INITIALS.get(leader_key, leader_key[:1].upper())
    try:
        font = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSerif-Bold.ttf", 96)
    except OSError:
        font = ImageFont.load_default()
    bbox = draw.textbbox((0, 0), initial, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    draw.text(((256 - tw) // 2, (256 - th) // 2), initial, fill=(255, 240, 200, 255), font=font)
    import io

    buf = io.BytesIO()
    img.save(buf, format="WEBP", quality=85)
    return buf.getvalue()


def svg_icon(name: str, size: int = 64) -> str:
    color = SVG_THEMES.get(name.replace(".svg", ""), "#506080")
    stem = name.replace(".svg", "")
    s = size
    cx, cy = s // 2, s // 2
    shapes = {
        "city": f'<rect x="{cx-12}" y="{cy-4}" width="24" height="20" fill="{color}"/><rect x="{cx-18}" y="{cy-8}" width="10" height="24" fill="{color}"/><rect x="{cx+8}" y="{cy-12}" width="10" height="28" fill="{color}"/>',
        "missile_silo": f'<ellipse cx="{cx}" cy="{cy+8}" rx="16" ry="6" fill="#333"/><rect x="{cx-8}" y="{cy-16}" width="16" height="24" rx="4" fill="{color}"/><polygon points="{cx},{cy-22} {cx+6},{cy-14} {cx-6},{cy-14}" fill="#aaa"/>',
        "trade_ship": f'<path d="M{cx-20},{cy+8} L{cx+20},{cy+8} L{cx+12},{cy-4} L{cx-12},{cy-4}Z" fill="{color}"/><rect x="{cx-2}" y="{cy-18}" width="4" height="14" fill="#888"/>',
        "transport_ship": f'<rect x="{cx-18}" y="{cy-2}" width="36" height="12" rx="2" fill="{color}"/><rect x="{cx-4}" y="{cy-16}" width="8" height="14" fill="#666"/>',
        "battleship": f'<rect x="{cx-22}" y="{cy}" width="44" height="10" fill="{color}"/><rect x="{cx-6}" y="{cy-14}" width="12" height="14" fill="#555"/><circle cx="{cx+14}" cy="{cy+5}" r="3" fill="#888"/>',
        "star": f'<polygon points="{cx},{cy-16} {cx+4},{cy-4} {cx+16},{cy-4} {cx+6},{cy+4} {cx+10},{cy+16} {cx},{cy+8} {cx-10},{cy+16} {cx-6},{cy+4} {cx-16},{cy-4} {cx-4},{cy-4}" fill="{color}"/>',
        "handshake": f'<path d="M{cx-18},{cy} Q{cx-8},{cy-8} {cx},{cy} Q{cx+8},{cy+8} {cx+18},{cy}" stroke="{color}" stroke-width="4" fill="none"/>',
        "back": f'<polygon points="{cx+10},{cy-12} {cx-10},{cy} {cx+10},{cy+12}" fill="{color}"/>',
    }
    body = shapes.get(stem, f'<circle cx="{cx}" cy="{cy}" r="14" fill="{color}"/>')
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}">
  <rect width="{s}" height="{s}" fill="#1a2030" rx="6"/>
  {body}
</svg>
"""


def leader_bg(leader_key: str) -> tuple[int, int, int, int]:
    r, g, b = LEADER_FILLER_RGB.get(leader_key, LEADER_FILLER_RGB["null"])
    return (r, g, b, 255)


def should_skip_existing(path: Path) -> bool:
    rel = path.relative_to(ROOT).as_posix()
    if rel in SKIP_REL:
        return True
    if not path.is_file():
        return False
    if rel.startswith("sow-ui/assets/ui/leaders/"):
        return path.stat().st_size >= MIN_FINAL_PORTRAIT_BYTES
    if rel.startswith("sow-ui/assets/avatars/"):
        leader_key = path.stem
        if leader_key not in PLACEHOLDER_AVATAR_KEYS:
            return True
    return False


def write(path: Path, data: bytes | str) -> None:
    rel = path.relative_to(ROOT).as_posix()
    if should_skip_existing(path):
        print(f"  skip {rel}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, str):
        path.write_text(data, encoding="utf-8")
    else:
        path.write_bytes(data)
    print(f"  {rel}")


def main() -> None:
    client = ROOT / "sow-client" / "assets"
    ui = ROOT / "sow-ui" / "assets"

    print("sow-client SVG/PNG")
    for name in [
        "city.svg",
        "factory.svg",
        "port.svg",
        "defense_post.svg",
        "missile_silo.svg",
        "trade_ship.svg",
        "transport_ship.svg",
        "battleship.svg",
        "star.webp",
        "handshake.svg",
        "back.svg",
    ]:
        write(client / name, svg_icon(name))

    for name, rgb in [("atombomb.png", (180, 60, 40)), ("sam_missile.png", (80, 100, 120))]:
        write(client / name, png_rgb(64, rgb))

    for name in ["request.webp", "handshake.webp", "betray.webp", "nameplate.webp"]:
        write(client / name, webp_panel(128, 128))

    print("sow-ui avatars (placeholder leaders only)")
    for name in sorted(PLACEHOLDER_AVATAR_KEYS):
        write(ui / "avatars" / f"{name}.webp", webp_avatar(name))

    print("sow-ui loader bars")
    write(ui / "ui" / "loader_empty.webp", webp_loader_bar(2064, 512, False))
    write(ui / "ui" / "loader_full.webp", webp_loader_bar(2064, 512, True))

    print("sow-ui hud panels")
    for name in ["hud_controls", "hud_battle_log", "hud_logs"]:
        write(ui / "ui" / "hud" / f"{name}.webp", webp_panel(256, 256))

    print("sow-ui leader portraits (regenerate small placeholders)")
    for name in [
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
    ]:
        write(ui / "ui" / "leaders" / f"{name}_desktop.webp", webp_leader_portrait(1920, 1080, name))
        write(ui / "ui" / "leaders" / f"{name}_mobile.webp", webp_leader_portrait(1080, 1920, name))

    print("Done.")


if __name__ == "__main__":
    main()
