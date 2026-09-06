mod parse;

pub use parse::{
    CoastlineGeometry, MapBBox, build_landmass_from_coastlines, extract_coastlines, map_dimensions,
    stamp_water_polygons,
};
