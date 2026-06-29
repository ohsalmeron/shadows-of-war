use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfChar {
    pub id: u32,
    pub index: u32,
    pub char: String,
    pub width: u32,
    pub height: u32,
    pub xoffset: i32,
    pub yoffset: i32,
    pub xadvance: u32,
    pub chnl: u32,
    pub x: u32,
    pub y: u32,
    pub page: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfInfo {
    pub size: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfCommon {
    #[serde(rename = "lineHeight")]
    pub line_height: u32,
    pub base: u32,
    #[serde(rename = "scaleW")]
    pub scale_w: u32,
    #[serde(rename = "scaleH")]
    pub scale_h: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfDistanceField {
    #[serde(rename = "fieldType")]
    pub field_type: String,
    #[serde(rename = "distanceRange")]
    pub distance_range: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfKerning {
    pub first: u32,
    pub second: u32,
    pub amount: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfAtlas {
    pub pages: Vec<String>,
    pub chars: Vec<MsdfChar>,
    pub info: MsdfInfo,
    pub common: MsdfCommon,
    #[serde(rename = "distanceField")]
    pub distance_field: MsdfDistanceField,
    pub kernings: Vec<MsdfKerning>,
}

pub struct FontAtlas {
    pub atlas: MsdfAtlas,
    pub char_map: HashMap<char, MsdfChar>,
    pub kerning_map: HashMap<(char, char), i32>,
}

impl FontAtlas {
    pub fn load_static() -> Self {
        let json_str = include_str!("../../../assets/static/fonts/msdf-atlas.json");
        let atlas: MsdfAtlas = serde_json::from_str(json_str).expect("Failed to parse MSDF atlas JSON");
        let mut char_map = HashMap::new();
        for c in &atlas.chars {
            if let Some(first_char) = c.char.chars().next() {
                char_map.insert(first_char, c.clone());
            }
        }
        let mut kerning_map = HashMap::new();
        let char_by_id: HashMap<u32, char> = atlas.chars.iter()
            .filter_map(|c| c.char.chars().next().map(|ch| (c.id, ch)))
            .collect();
        for k in &atlas.kernings {
            if let (Some(&c1), Some(&c2)) = (char_by_id.get(&k.first), char_by_id.get(&k.second)) {
                kerning_map.insert((c1, c2), k.amount);
            }
        }
        Self {
            atlas,
            char_map,
            kerning_map,
        }
    }
}
