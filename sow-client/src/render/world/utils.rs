
pub(crate) fn get_building_icon_size(zoom_scaled: f32) -> f32 {
    let size = if zoom_scaled < 10.0 {
        zoom_scaled * 2.0
    } else {
        zoom_scaled * 1.6
    };
    (size * 4.0).clamp(44.0, 384.0)
}

pub(crate) fn get_level_str(level: u8) -> &'static str {
    match level {
        0 => "0",
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        8 => "8",
        9 => "9",
        10 => "10",
        11 => "11",
        12 => "12",
        13 => "13",
        14 => "14",
        15 => "15",
        16 => "16",
        17 => "17",
        18 => "18",
        19 => "19",
        20 => "20",
        21 => "21",
        22 => "22",
        23 => "23",
        24 => "24",
        25 => "25",
        26 => "26",
        27 => "27",
        28 => "28",
        29 => "29",
        30 => "30",
        _ => "99+",
    }
}

pub(crate) fn get_train_gold_str(segment_idx: usize) -> &'static str {
    match segment_idx {
        0 => "🪙250",
        1 => "🪙500",
        2 => "🪙750",
        3 => "🪙1.0k",
        4 => "🪙1.2k",
        5 => "🪙1.5k",
        6 => "🪙1.7k",
        7 => "🪙2.0k",
        8 => "🪙2.2k",
        9 => "🪙2.5k",
        10 => "🪙2.7k",
        11 => "🪙3.0k",
        12 => "🪙3.2k",
        13 => "🪙3.5k",
        14 => "🪙3.7k",
        15 => "🪙4.0k",
        16 => "🪙4.2k",
        17 => "🪙4.5k",
        18 => "🪙4.7k",
        19 => "🪙5.0k",
        20 => "🪙5.2k",
        21 => "🪙5.5k",
        22 => "🪙5.7k",
        23 => "🪙6.0k",
        24 => "🪙6.2k",
        25 => "🪙6.5k",
        26 => "🪙6.7k",
        27 => "🪙7.0k",
        28 => "🪙7.2k",
        29 => "🪙7.5k",
        30 => "🪙7.7k",
        31 => "🪙8.0k",
        32 => "🪙8.2k",
        33 => "🪙8.5k",
        34 => "🪙8.7k",
        35 => "🪙9.0k",
        36 => "🪙9.2k",
        37 => "🪙9.5k",
        38 => "🪙9.7k",
        39 => "🪙10.0k",
        _ => "🪙10k+",
    }
}



#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RailType {
    Vertical,
    Horizontal,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub struct RailTile {
    pub tile_idx: u32,
    pub rail_type: RailType,
}

pub(crate) fn get_railroad_rects(rail_type: RailType) -> &'static [[i32; 4]] {
    match rail_type {
        RailType::Vertical => &[
            [-1, -1, 1, 2],
            [1, -1, 1, 2],
            [0, 0, 1, 1],
        ],
        RailType::Horizontal => &[
            [-1, -1, 2, 1],
            [-1, 1, 2, 1],
            [-1, 0, 1, 1],
        ],
        RailType::TopRight => &[
            [-1, -1, 1, 1],
            [0, -1, 1, 2],
            [1, -1, 1, 3],
        ],
        RailType::TopLeft => &[
            [-1, -1, 1, 3],
            [0, -1, 1, 2],
            [1, -1, 1, 1],
        ],
        RailType::BottomRight => &[
            [-1, 1, 1, 1],
            [0, 0, 1, 2],
            [1, -1, 1, 3],
        ],
        RailType::BottomLeft => &[
            [-1, -1, 1, 3],
            [0, 0, 1, 2],
            [1, 1, 1, 1],
        ],
    }
}

pub(crate) fn get_bridge_rects(rail_type: RailType) -> &'static [[i32; 4]] {
    match rail_type {
        RailType::Vertical => &[
            [-2, -1, 1, 3],
            [2, -1, 1, 3],
        ],
        RailType::Horizontal => &[
            [-1, -2, 3, 1],
            [-1, 2, 3, 1],
            [-1, 3, 1, 1],
            [1, 3, 1, 1],
        ],
        RailType::TopRight => &[
            [-2, -2, 1, 2],
            [-1, 0, 1, 1],
            [0, 1, 1, 1],
            [1, 2, 2, 1],
            [2, -2, 1, 1],
        ],
        RailType::TopLeft => &[
            [-2, -2, 1, 1],
            [-2, 2, 2, 1],
            [0, 1, 1, 1],
            [1, 0, 1, 1],
            [2, -2, 1, 2],
        ],
        RailType::BottomRight => &[
            [-2, 1, 1, 2],
            [-1, 0, 1, 1],
            [0, -1, 1, 1],
            [1, -2, 2, 1],
            [2, 2, 1, 1],
        ],
        RailType::BottomLeft => &[
            [-2, -2, 2, 1],
            [0, -1, 1, 1],
            [1, 0, 1, 1],
            [2, 1, 1, 2],
            [-2, 2, 1, 1],
        ],
    }
}

pub(crate) fn compute_direction(w: u32, prev: u32, current: u32, next: u32) -> RailType {
    let x1 = (prev % w) as i32;
    let y1 = (prev / w) as i32;
    let x2 = (current % w) as i32;
    let y2 = (current / w) as i32;
    let x3 = (next % w) as i32;
    let y3 = (next / w) as i32;

    let dx1 = x2 - x1;
    let dy1 = y2 - y1;
    let dx2 = x3 - x2;
    let dy2 = y3 - y2;

    if dx1 == dx2 && dy1 == dy2 {
        if dx1 != 0 {
            return RailType::Horizontal;
        }
        if dy1 != 0 {
            return RailType::Vertical;
        }
    }

    if (dx1 == 0 && dx2 != 0) || (dx1 != 0 && dx2 == 0) {
        if dx1 == 0 && dx2 == 1 && dy1 == -1 {
            return RailType::BottomRight;
        }
        if dx1 == 0 && dx2 == -1 && dy1 == -1 {
            return RailType::BottomLeft;
        }
        if dx1 == 0 && dx2 == 1 && dy1 == 1 {
            return RailType::TopRight;
        }
        if dx1 == 0 && dx2 == -1 && dy1 == 1 {
            return RailType::TopLeft;
        }

        if dx1 == 1 && dx2 == 0 && dy2 == -1 {
            return RailType::TopLeft;
        }
        if dx1 == -1 && dx2 == 0 && dy2 == -1 {
            return RailType::TopRight;
        }
        if dx1 == 1 && dx2 == 0 && dy2 == 1 {
            return RailType::BottomLeft;
        }
        if dx1 == -1 && dx2 == 0 && dy2 == 1 {
            return RailType::BottomRight;
        }
    }

    RailType::Vertical
}

pub(crate) fn compute_extremity_direction(w: u32, tile: u32, next: u32) -> RailType {
    let x = (tile % w) as i32;
    let y = (tile / w) as i32;
    let next_x = (next % w) as i32;
    let next_y = (next / w) as i32;

    let dx = next_x - x;
    let dy = next_y - y;

    if dx == 0 && dy == 0 {
        return RailType::Vertical;
    }

    if dx == 0 {
        RailType::Vertical
    } else if dy == 0 {
        RailType::Horizontal
    } else {
        RailType::Vertical
    }
}

pub(crate) fn compute_rail_tiles(w: u32, tiles: &[u32]) -> Vec<RailTile> {
    if tiles.is_empty() {
        return Vec::new();
    }
    if tiles.len() == 1 {
        return vec![RailTile {
            tile_idx: tiles[0],
            rail_type: RailType::Vertical,
        }];
    }
    let mut rail_tiles = Vec::with_capacity(tiles.len());
    rail_tiles.push(RailTile {
        tile_idx: tiles[0],
        rail_type: compute_extremity_direction(w, tiles[0], tiles[1]),
    });
    for i in 1..tiles.len() - 1 {
        let direction = compute_direction(w, tiles[i - 1], tiles[i], tiles[i + 1]);
        rail_tiles.push(RailTile {
            tile_idx: tiles[i],
            rail_type: direction,
        });
    }
    rail_tiles.push(RailTile {
        tile_idx: tiles[tiles.len() - 1],
        rail_type: compute_extremity_direction(w, tiles[tiles.len() - 1], tiles[tiles.len() - 2]),
    });
    rail_tiles
}

