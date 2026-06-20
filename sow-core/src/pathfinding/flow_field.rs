#[derive(Clone)]
pub struct FlowField {
    pub target: u32,
    pub width: u32,
    pub height: u32,
    pub directions: Vec<u8>, // 0-7: N, NE, E, SE, S, SW, W, NW. 255: Unreachable/Obstacle
}

impl FlowField {
    pub fn new(width: u32, height: u32, target: u32) -> Self {
        Self {
            target,
            width,
            height,
            directions: vec![255; (width * height) as usize],
        }
    }

    pub fn compute_from_target(&mut self, map: &crate::map::GameMap) {
        let n = (self.width * self.height) as usize;
        let mut distances = vec![u32::MAX; n];
        let mut queue = std::collections::VecDeque::new();

        let tx = self.target % self.width;
        let ty = self.target / self.width;

        distances[self.target as usize] = 0;
        self.directions[self.target as usize] = 6; // Reached
        queue.push_back((tx, ty));

        while let Some((cx, cy)) = queue.pop_front() {
            let curr_idx = (cy * self.width + cx) as usize;
            let current_dist = distances[curr_idx];

            let is_odd = (cy % 2) != 0;
            let deltas = if is_odd {
                [
                    (1, 0),  // East (0)
                    (-1, 0), // West (1)
                    (0, -1), // Northwest (2)
                    (1, -1), // Northeast (3)
                    (0, 1),  // Southwest (4)
                    (1, 1),  // Southeast (5)
                ]
            } else {
                [
                    (1, 0),   // East (0)
                    (-1, 0),  // West (1)
                    (-1, -1), // Northwest (2)
                    (0, -1),  // Northeast (3)
                    (-1, 1),  // Southwest (4)
                    (0, 1),   // Southeast (5)
                ]
            };

            for (i, delta) in deltas.iter().enumerate() {
                let nx = cx as i32 + delta.0;
                let ny = cy as i32 + delta.1;

                if nx >= 0 && nx < self.width as i32 && ny >= 0 && ny < self.height as i32 {
                    let n_idx = (ny as u32 * self.width + nx as u32) as usize;

                    let b = map.terrain[n_idx].as_byte();
                    let is_land = (b & (1 << 7)) != 0;
                    if is_land {
                        continue;
                    }

                    if distances[n_idx] > current_dist + 1 {
                        distances[n_idx] = current_dist + 1;
                        let opp = match i {
                            0 => 1,
                            1 => 0,
                            2 => 5,
                            3 => 4,
                            4 => 3,
                            5 => 2,
                            _ => 6,
                        };
                        self.directions[n_idx] = opp as u8;
                        queue.push_back((nx as u32, ny as u32));
                    }
                }
            }
        }
    }
}

#[derive(Default, Clone)]
pub struct FlowFieldCache {
    pub fields: std::collections::HashMap<u32, FlowField>,
    pub access_order: std::collections::VecDeque<u32>,
}

impl FlowFieldCache {
    pub fn get_or_compute(&mut self, target: u32, map: &crate::map::GameMap) -> &FlowField {
        if !self.fields.contains_key(&target) {
            if self.fields.len() >= 8 {
                if let Some(oldest) = self.access_order.pop_front() {
                    self.fields.remove(&oldest);
                }
            }
            let mut field = FlowField::new(map.width, map.height, target);
            field.compute_from_target(map);
            self.fields.insert(target, field);
            self.access_order.push_back(target);
        } else {
            if let Some(pos) = self.access_order.iter().position(|&x| x == target) {
                self.access_order.remove(pos);
            }
            self.access_order.push_back(target);
        }
        self.fields.get(&target).unwrap()
    }
}
