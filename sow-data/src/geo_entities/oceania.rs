//! Oceanian geo entities. Coordinates are approximate historical heartlands.

use super::{GeoEntity, geo_entity as e};

pub static OCEANIA: &[GeoEntity] = &[
    // --- Countries (modern) ---
    e!(Oceania: "Australia", Country, Modern, -25.3, 134.0, "au"),
    e!(Oceania: "New Zealand", Country, Modern, -41.3, 174.8, "nz"),
    e!(Oceania: "Papua New Guinea", Country, Modern, -6.3, 146.0, "pg"),
    e!(Oceania: "Fiji", Country, Modern, -17.8, 178.0, "fj"),
    e!(Oceania: "Solomon Islands", Country, Modern, -9.6, 160.2, "sb"),
    e!(Oceania: "Vanuatu", Country, Modern, -16.6, 168.3, "vu"),
    e!(Oceania: "Samoa", Country, Modern, -13.8, -172.1, "ws"),
    e!(Oceania: "Tonga", Country, Modern, -21.2, -175.2, "to"),
    e!(Oceania: "Palau", Country, Modern, 7.5, 134.6, "pw"),
    e!(Oceania: "Micronesia", Country, Modern, 6.9, 158.2, "fm"),
    e!(Oceania: "Marshall Islands", Country, Modern, 7.1, 171.2, "mh"),
    e!(Oceania: "Kiribati", Country, Modern, 1.35, 173.0, "ki"),
    e!(Oceania: "New Caledonia", Country, Modern, -21.3, 165.5, "nc"),
    // --- Tribes & peoples ---
    e!(Oceania: "Maori", Tribe, Medieval, -38.7, 176.1),
    e!(Oceania: "Yolngu", Tribe, Ancient, -12.5, 135.0),
    e!(Oceania: "Noongar", Tribe, Ancient, -32.5, 116.5),
    e!(Oceania: "Arrernte", Tribe, Ancient, -23.7, 133.9),
    e!(Oceania: "Palawa", Tribe, Ancient, -42.0, 146.5),
    e!(Oceania: "Chamorro", Tribe, Medieval, 13.45, 144.75),
    e!(Oceania: "Papuans", Tribe, Ancient, -5.5, 141.0),
    e!(Oceania: "Tahitians", Tribe, Medieval, -17.65, -149.42),
    e!(Oceania: "Marquesans", Tribe, Medieval, -9.78, -139.08),
    e!(Oceania: "Rapa Nui", Tribe, Medieval, -27.11, -109.35),
    // --- City-states ---
    e!(Oceania: "Nan Madol", CityState, Medieval, 6.84, 158.33),
    // --- Kingdoms ---
    e!(Oceania: "Kingdom of Hawaii", Kingdom, Modern, 21.31, -157.86),
    e!(Oceania: "Kingdom of Tahiti", Kingdom, Modern, -17.53, -149.56),
    // --- Empires ---
    e!(Oceania: "Tui Tonga Empire", Empire, Medieval, -21.14, -175.20),
    // --- Phase A tribes ---
    e!(Oceania: "Wiradjuri", Tribe, Ancient, -33.5, 147.5),
    e!(Oceania: "Kamilaroi", Tribe, Ancient, -30.5, 150.0),
    e!(Oceania: "Pitjantjatjara", Tribe, Ancient, -26.0, 132.0),
    e!(Oceania: "Warlpiri", Tribe, Ancient, -20.5, 132.0),
    e!(Oceania: "Gunditjmara", Tribe, Ancient, -38.2, 141.8),
    e!(Oceania: "Wurundjeri", Tribe, Ancient, -37.7, 145.0),
    e!(Oceania: "Tiwi", Tribe, Ancient, -11.6, 130.8),
    e!(Oceania: "Kanak", Tribe, Medieval, -21.0, 165.0),
    e!(Oceania: "Tolai", Tribe, EarlyModern, -4.3, 152.2),
    e!(Oceania: "Huli", Tribe, Ancient, -6.0, 142.9),
    e!(Oceania: "Asmat", Tribe, EarlyModern, -5.5, 138.5),
    e!(Oceania: "Dani", Tribe, Ancient, -4.0, 138.9),
    e!(Oceania: "Motu", Tribe, EarlyModern, -9.5, 147.2),
    e!(Oceania: "Trobrianders", Tribe, EarlyModern, -8.6, 151.0),
    e!(Oceania: "Hawaiians", Tribe, Medieval, 20.8, -156.3),
    e!(Oceania: "Moriori", Tribe, Medieval, -44.0, -176.5),
    e!(Oceania: "Lapita", Tribe, Ancient, -15.5, 167.0),
    e!(Oceania: "Ngapuhi", Tribe, EarlyModern, -35.4, 173.8),
    e!(Oceania: "Tainui", Tribe, EarlyModern, -37.8, 175.3),
    // --- Phase B nations & city-states ---
    e!(Oceania: "Kingdom of Maui", Kingdom, EarlyModern, 20.8, -156.33),
    e!(Oceania: "Kingdom of Oahu", Kingdom, EarlyModern, 21.5, -158.0),
    e!(Oceania: "Kubuna Confederacy", Kingdom, Modern, -18.0, 178.5),
    e!(Oceania: "Levuka", CityState, Modern, -17.68, 178.83),
];
