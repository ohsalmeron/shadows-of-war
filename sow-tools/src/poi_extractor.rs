use serde_json::Value;

pub struct POISpawn {
    pub name: String,
    pub x: u32,
    pub y: u32,
}

const PLACE_TYPES: &[&str] = &[
    "city", "town", "village", "suburb", "hamlet", "neighbourhood",
];

pub fn extract_bots(
    data: &Value,
    min_lon: f64,
    _min_lat: f64,
    _max_lon: f64,
    max_lat: f64,
    scale: f64,
    map_width: u32,
    map_height: u32,
) -> Vec<POISpawn> {
    let mut spawns = Vec::new();

    let Some(elements) = data.get("elements").and_then(|e| e.as_array()) else {
        return spawns;
    };

    for element in elements {
        if element.get("type").and_then(|t| t.as_str()) != Some("node") {
            continue;
        }
        let tags = element.get("tags");
        let place = tags.and_then(|t| t.get("place")).and_then(|p| p.as_str());
        let name = tags
            .and_then(|t| t.get("name"))
            .and_then(|n| n.as_str())
            .or_else(|| {
                tags.and_then(|t| {
                    t.get("name:en")
                        .and_then(|n| n.as_str())
                        .or_else(|| t.get("name:es").and_then(|n| n.as_str()))
                })
            });

        let Some(place_type) = place else {
            continue;
        };
        if !PLACE_TYPES.contains(&place_type) {
            continue;
        }
        let Some(poi_name) = name else {
            continue;
        };
        if poi_name.trim().is_empty() {
            continue;
        }

        let (Some(lat), Some(lon)) = (
            element.get("lat").and_then(|v| v.as_f64()),
            element.get("lon").and_then(|v| v.as_f64()),
        ) else {
            continue;
        };

        let x = ((lon - min_lon) * scale).round() as u32;
        let y = ((max_lat - lat) * scale).round() as u32;
        if x >= map_width || y >= map_height {
            continue;
        }

        spawns.push(POISpawn {
            name: poi_name.to_string(),
            x,
            y,
        });
    }

    spawns.sort_by(|a, b| a.name.cmp(&b.name));
    spawns.dedup_by(|a, b| a.name == b.name && a.x == b.x && a.y == b.y);

    if spawns.len() > 64 {
        spawns.truncate(64);
    }

    spawns
}
