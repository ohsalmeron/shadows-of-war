#[path = "combat_fleets.rs"]
mod combat_fleets;
#[path = "combat_tiles.rs"]
mod combat_tiles;

#[cfg(test)]
mod tests {
    use crate::execution::fractional_extra_tiles_milli;

    #[test]
    fn fractional_extra_tile_milli_threshold_is_stable() {
        assert_eq!(fractional_extra_tiles_milli(12.499, 498), 1);
        assert_eq!(fractional_extra_tiles_milli(12.499, 499), 0);
        assert_eq!(fractional_extra_tiles_milli(7.0009, 0), 0);
        assert_eq!(fractional_extra_tiles_milli(3.999_999_999, 998), 1);
        assert_eq!(fractional_extra_tiles_milli(3.999_999_999, 999), 0);
    }
}
