#!/usr/bin/env python3
"""Split docs/leaders.md into regional markdown files sorted by era."""

from __future__ import annotations

import re
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs" / "leaders.md"
OUT_DIR = ROOT / "docs" / "leaders"
SOURCE_ARCHIVE = OUT_DIR / "roster-source.md"
REGIONS_TOML = OUT_DIR / "regions.toml"
STUB = ROOT / "docs" / "leaders.md"

HEADER_RE = re.compile(r"^### (\d+)\. (.+)$")
ORDINAL_CENTURY = re.compile(
    r"(\d+)(?:st|nd|rd|th)\s*[–-]\s*(\d+)(?:st|nd|rd|th)?\s+Century\s+(BCE|CE)",
    re.I,
)
SINGLE_CENTURY = re.compile(r"(\d+)(?:st|nd|rd|th)\s+Century\s+(BCE|CE)", re.I)
YEAR_RANGE = re.compile(
    r"(\d+)\s*[–-]\s*(\d+)\s*(BCE|CE)?", re.I
)
SINGLE_YEAR = re.compile(r"^(\d+)\s*(BCE|CE)?$", re.I)

REGION_TITLES = {
    "africa": "Africa",
    "australia-oceania": "Australia & Oceania",
    "central-asia": "Central Asia",
    "east-asia": "East Asia",
    "western-europe": "Western Europe",
    "eastern-europe": "Eastern Europe",
    "nordic-scandinavia": "Nordic & Scandinavia",
    "middle-east-north-africa": "Middle East & North Africa",
    "north-america": "North America",
    "south-america": "South America",
    "south-asia": "South Asia",
    "south-east-asia": "South East Asia",
}


@dataclass
class Leader:
    lid: int
    title_line: str
    hero: str
    empire: str
    dates: str
    name: str
    faction: str
    culture: str
    region: str = ""
    sort_year: int = 9998
    sort_year_end: int = 9998


def load_regions() -> dict:
    with REGIONS_TOML.open("rb") as f:
        return tomllib.load(f)


def parse_sort_years(dates: str) -> tuple[int, int]:
    if not dates or dates.strip() == "Historical Era":
        return 9999, 9999

    text = dates.strip()

    m = ORDINAL_CENTURY.search(text)
    if m:
        start_c, end_c, era = int(m.group(1)), int(m.group(2)), m.group(3).upper()
        y = -(start_c * 100) if era == "BCE" else (start_c - 1) * 100 + 1
        y_end = -(end_c * 100) if era == "BCE" else end_c * 100
        return y, y_end

    m = SINGLE_CENTURY.search(text)
    if m:
        c, era = int(m.group(1)), m.group(2).upper()
        y = -(c * 100) if era == "BCE" else (c - 1) * 100 + 1
        return y, y

    m = YEAR_RANGE.search(text)
    if m:
        a, b, era = int(m.group(1)), int(m.group(2)), (m.group(3) or "CE").upper()
        if era == "BCE":
            return -a, -b
        return a, b

    m = SINGLE_YEAR.match(text)
    if m:
        y, era = int(m.group(1)), (m.group(2) or "CE").upper()
        return (-y, -y) if era == "BCE" else (y, y)

    return 9998, 9998


def classify(leader: Leader, cfg: dict) -> str:
    lid = str(leader.lid)
    if lid in cfg.get("original_id", {}):
        return cfg["original_id"][lid]

    india = cfg.get("india_faction_override", {})
    if leader.culture == india.get("when_culture", ""):
        for fac in india.get("factions", []):
            if leader.faction == fac or fac in leader.faction:
                return "south-asia"

    for key in ("faction", "culture"):
        table = cfg.get(key, {})
        val = leader.faction if key == "faction" else leader.culture
        if val in table:
            return table[val]

    for substr, region in cfg.get("faction_contains", {}).items():
        if substr in leader.faction or substr in leader.culture:
            return region

    middle = leader.faction if leader.culture else leader.faction
    for kw, region in cfg.get("empire_keyword", {}).items():
        if kw in middle or kw in leader.culture or kw in leader.name:
            return region

    return ""


def parse_source(text: str) -> list[Leader]:
    leaders: list[Leader] = []
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        m = HEADER_RE.match(lines[i])
        if not m:
            i += 1
            continue

        title_line = lines[i]
        lid = int(m.group(1))
        rest = m.group(2)
        fields = [f.strip() for f in rest.split(" | ")]
        name = fields[0]
        dates = fields[-1]

        if len(fields) == 3:
            faction = fields[1]
            culture = ""
        elif len(fields) >= 4:
            faction = fields[1]
            culture = fields[2]
        else:
            faction = fields[1] if len(fields) > 1 else ""
            culture = ""

        hero = ""
        empire = ""
        i += 1
        while i < len(lines) and not HEADER_RE.match(lines[i]):
            if lines[i].startswith("- **Hero:**"):
                hero = lines[i]
            elif lines[i].startswith("- **Empire:**"):
                empire = lines[i]
            i += 1

        leaders.append(
            Leader(
                lid=lid,
                title_line=title_line,
                hero=hero,
                empire=empire,
                dates=dates,
                name=name,
                faction=faction,
                culture=culture,
            )
        )
    return leaders


def sort_key(leader: Leader) -> tuple:
    return (leader.sort_year, leader.sort_year_end, leader.lid, leader.name.lower())


def render_leader(leader: Leader) -> str:
    return f"{leader.title_line}\n{leader.hero}\n{leader.empire}\n"


def write_region_file(region: str, leaders: list[Leader]) -> None:
    title = REGION_TITLES[region]
    path = OUT_DIR / f"{region}.md"
    body = [
        f"# {title}",
        "",
        f"{len(leaders)} leaders, sorted oldest → newest. Global roster IDs preserved.",
        "",
        "---",
        "",
    ]
    for leader in leaders:
        body.append(render_leader(leader))
    path.write_text("\n".join(body), encoding="utf-8")


def write_readme(cfg: dict, counts: dict[str, int]) -> None:
    total = sum(counts.values())
    lines = [
        "# Leaders roster",
        "",
        f"Complete AI art dossier for **{total}** leaders, split by macro region.",
        "Each regional file lists leaders **oldest → newest** (global IDs unchanged).",
        "",
        "## Regions",
        "",
        "| Region | Count | File |",
        "|--------|------:|------|",
    ]
    for region in cfg["meta"]["regions"]:
        title = REGION_TITLES[region]
        n = counts.get(region, 0)
        lines.append(f"| {title} | {n} | [{region}.md]({region}.md) |")
    lines.extend(
        [
            "",
            "## Sort rules",
            "",
            "- Explicit years: first year (BCE as negative).",
            '- Century ranges: start century (e.g. "5th–4th Century BCE" → -500).',
            '- `Historical Era` and unparseable dates sort to the end of a region.',
            "- Tie-breakers: end year, global ID, name.",
            "",
            "## Regenerate",
            "",
            "```bash",
            "python3 scripts/split_leaders_roster.py",
            "```",
            "",
            "Canonical source: [roster-source.md](roster-source.md). Mapping: [regions.toml](regions.toml).",
            "",
            "## Limitations",
            "",
            "- Many entries share dynasty-wide date strings; reign-level order needs a future enrichment pass.",
            "- Known source duplicates (e.g. Tamerlane) are preserved.",
        ]
    )
    (OUT_DIR / "README.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_stub() -> None:
    STUB.write_text(
        "# Leaders roster (moved)\n\n"
        "The roster now lives in [leaders/README.md](leaders/README.md) "
        "(12 regional files, chronological order).\n\n"
        "Regenerate from `leaders/roster-source.md`: `python3 scripts/split_leaders_roster.py`\n",
        encoding="utf-8",
    )


def resolve_source_text() -> str:
    if SOURCE_ARCHIVE.exists():
        return SOURCE_ARCHIVE.read_text(encoding="utf-8")
    if SOURCE.exists() and "### 1." in SOURCE.read_text(encoding="utf-8"):
        return SOURCE.read_text(encoding="utf-8")
    print("error: no roster source (leaders.md or leaders/roster-source.md)", file=sys.stderr)
    sys.exit(1)


def main() -> int:
    cfg = load_regions()
    regions: list[str] = cfg["meta"]["regions"]
    text = resolve_source_text()
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if not SOURCE_ARCHIVE.exists() and "### 1." in text:
        SOURCE_ARCHIVE.write_text(text, encoding="utf-8")
    leaders = parse_source(text)

    unclassified: list[Leader] = []
    for leader in leaders:
        sy, sy_end = parse_sort_years(leader.dates)
        leader.sort_year = sy
        leader.sort_year_end = sy_end
        leader.region = classify(leader, cfg)
        if not leader.region:
            unclassified.append(leader)

    if unclassified:
        print("unclassified leaders:", file=sys.stderr)
        for u in unclassified[:20]:
            print(
                f"  #{u.lid} {u.name} | {u.faction} | {u.culture} | {u.dates}",
                file=sys.stderr,
            )
        print(f"  ... total {len(unclassified)}", file=sys.stderr)
        return 1

    by_region: dict[str, list[Leader]] = {r: [] for r in regions}
    for leader in leaders:
        by_region[leader.region].append(leader)

    counts: dict[str, int] = {}
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for region in regions:
        bucket = sorted(by_region[region], key=sort_key)
        counts[region] = len(bucket)
        write_region_file(region, bucket)

    write_readme(cfg, counts)
    write_stub()

    ids = [l.lid for l in leaders]
    if len(ids) != len(set(ids)):
        print("error: duplicate IDs in source", file=sys.stderr)
        return 1
    if len(leaders) != 1270:
        print(f"warning: expected 1270 leaders, got {len(leaders)}", file=sys.stderr)

    print(f"Wrote {len(leaders)} leaders across {len(regions)} regions in {OUT_DIR}")
    for region in regions:
        print(f"  {region}: {counts[region]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
