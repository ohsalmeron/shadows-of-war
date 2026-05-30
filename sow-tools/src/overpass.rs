use reqwest::Client;
use serde_json::Value;
use std::error::Error;

/// Fetches OSM data within the bounding box (south, west, north, east per Overpass convention).
pub async fn fetch_bbox(
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
) -> Result<Value, Box<dyn Error>> {
    let query = format!(
        r#"[out:json][timeout:60];
        (
          way["natural"="water"]({min_lat},{min_lon},{max_lat},{max_lon});
          way["natural"="bay"]({min_lat},{min_lon},{max_lat},{max_lon});
          relation["natural"="water"]({min_lat},{min_lon},{max_lat},{max_lon});
          way["landuse"="water"]({min_lat},{min_lon},{max_lat},{max_lon});
          way["waterway"]({min_lat},{min_lon},{max_lat},{max_lon});
          way["natural"="coastline"]({min_lat},{min_lon},{max_lat},{max_lon});
          way["landuse"]({min_lat},{min_lon},{max_lat},{max_lon});
          way["natural"~"wood|grassland|scrub|bare_rock|sand|heath|wetland"]({min_lat},{min_lon},{max_lat},{max_lon});
          node["place"~"city|town|village|suburb|hamlet"]({min_lat},{min_lon},{max_lat},{max_lon});
        );
        out geom;"#
    );

    let query_single_line = query.replace('\n', " ");
    let client = Client::new();
    let res = client
        .post("https://overpass-api.de/api/interpreter")
        .header("User-Agent", "ShadowsOfWar-sow-tools/1.0")
        .form(&[("data", &query_single_line)])
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await?;
        return Err(format!("Overpass API error: {status} - {text}").into());
    }

    let json: Value = res.json().await?;
    Ok(json)
}
