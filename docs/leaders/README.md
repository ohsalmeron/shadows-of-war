# Leaders roster

Complete AI art dossier for **1270** leaders, split by macro region.
Each regional file lists leaders **oldest → newest** (global IDs unchanged).

## Regions

| Region | Count | File |
|--------|------:|------|
| Africa | 57 | [africa.md](africa.md) |
| Australia & Oceania | 1 | [australia-oceania.md](australia-oceania.md) |
| Central Asia | 14 | [central-asia.md](central-asia.md) |
| East Asia | 250 | [east-asia.md](east-asia.md) |
| Western Europe | 252 | [western-europe.md](western-europe.md) |
| Eastern Europe | 214 | [eastern-europe.md](eastern-europe.md) |
| Nordic & Scandinavia | 103 | [nordic-scandinavia.md](nordic-scandinavia.md) |
| Middle East & North Africa | 114 | [middle-east-north-africa.md](middle-east-north-africa.md) |
| North America | 15 | [north-america.md](north-america.md) |
| South America | 4 | [south-america.md](south-america.md) |
| South Asia | 239 | [south-asia.md](south-asia.md) |
| South East Asia | 7 | [south-east-asia.md](south-east-asia.md) |

## Sort rules

- Explicit years: first year (BCE as negative).
- Century ranges: start century (e.g. "5th–4th Century BCE" → -500).
- `Historical Era` and unparseable dates sort to the end of a region.
- Tie-breakers: end year, global ID, name.

## Regenerate

```bash
python3 scripts/split_leaders_roster.py
```

Canonical source: [roster-source.md](roster-source.md). Mapping: [regions.toml](regions.toml).

## Limitations

- Many entries share dynasty-wide date strings; reign-level order needs a future enrichment pass.
- Known source duplicates (e.g. Tamerlane) are preserved.
