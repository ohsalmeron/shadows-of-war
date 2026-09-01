use crate::leaders::Leader;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const FREE_ROTATION_SIZE: usize = 8;
pub const ROTATION_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;
// ponytail: one balancing constant until real retention data exists; move pricing to live config then.
pub const LEADER_UNLOCK_COST_LAURELS: u64 = 500;

const GEM_BUNDLES: [(&str, u64); 3] = [
    ("sow_gems_500", 500),
    ("sow_gems_1200", 1_200),
    ("sow_gems_2600", 2_600),
];

// Original SOW cosmetic catalog. The style id is an internal renderer hint;
// the public id is the only value persisted in player ownership.
const SKINS: [(&str, &str, &str, u64, u8); 3] = [
    ("ember_vein", "Ember Vein", "gameplay/skins/ember_vein.svg", 300, 1),
    ("storm_grid", "Storm Grid", "gameplay/skins/storm_grid.svg", 450, 2),
    ("royal_lattice", "Royal Lattice", "gameplay/skins/royal_lattice.svg", 600, 3),
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LeaderOffer {
    pub id: String,
    pub name: String,
    pub civilization: String,
    pub perk: String,
    pub cost_laurels: u64,
    pub free_rotation: bool,
    pub owned: bool,
    pub available: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SkinOffer {
    pub id: String,
    pub leader_id: String,
    pub name: String,
    pub asset_path: String,
    pub cost_gems: u64,
    pub owned: bool,
    pub style: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GemBundle {
    pub id: String,
    pub gems: u64,
    pub product_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StoreCatalog {
    pub rotation_period: u64,
    pub free_leaders: Vec<String>,
    pub leaders: Vec<LeaderOffer>,
    pub skins: Vec<SkinOffer>,
    pub gem_bundles: Vec<GemBundle>,
    pub laurels: u64,
    pub gems: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaderResolution {
    pub requested: Leader,
    pub resolved: Leader,
    pub requested_available: bool,
    pub used_fallback: bool,
}

pub fn current_rotation_period() -> u64 {
    rotation_period(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}

pub const fn rotation_period(unix_seconds: u64) -> u64 {
    unix_seconds / ROTATION_PERIOD_SECS
}

pub fn rotation_for_period(period: u64) -> Vec<Leader> {
    let start = (period as usize * FREE_ROTATION_SIZE) % Leader::ALL.len();
    (0..FREE_ROTATION_SIZE)
        .map(|offset| Leader::ALL[(start + offset) % Leader::ALL.len()])
        .collect()
}

pub fn current_leader_rotation() -> Vec<Leader> {
    rotation_for_period(current_rotation_period())
}

pub fn leader_id(leader: Leader) -> &'static str {
    match leader {
        Leader::Caesar => "caesar",
        Leader::Cleopatra => "cleopatra",
        Leader::Ragnar => "ragnar",
        Leader::SunTzu => "sun_tzu",
        Leader::Alexander => "alexander",
        Leader::GenghisKhan => "genghis_khan",
        Leader::RichardTheLionheart => "richard_the_lionheart",
        Leader::Vercingetorix => "vercingetorix",
        Leader::Boudica => "boudica",
        Leader::LadySixSky => "lady_six_sky",
        Leader::Leonidas => "leonidas",
        Leader::Napoleon => "napoleon",
    }
}

pub fn leader_wire_id(leader: Leader) -> &'static str {
    match leader {
        Leader::Caesar => "Caesar",
        Leader::Cleopatra => "Cleopatra",
        Leader::Ragnar => "Ragnar",
        Leader::SunTzu => "SunTzu",
        Leader::Alexander => "Alexander",
        Leader::GenghisKhan => "GenghisKhan",
        Leader::RichardTheLionheart => "RichardTheLionheart",
        Leader::Vercingetorix => "Vercingetorix",
        Leader::Boudica => "Boudica",
        Leader::LadySixSky => "LadySixSky",
        Leader::Leonidas => "Leonidas",
        Leader::Napoleon => "Napoleon",
    }
}

pub fn leader_from_id(value: &str) -> Option<Leader> {
    let normalized: String = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect();
    Leader::ALL
        .into_iter()
        .find(|leader| {
            [
                leader_id(*leader),
                leader_wire_id(*leader),
                leader.name(),
            ]
            .into_iter()
            .map(|candidate| {
                candidate
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .map(|character| character.to_ascii_lowercase())
                    .collect::<String>()
            })
            .any(|candidate| candidate == normalized)
        })
}

pub fn assigned_leader_for_account(account_id: &str, period: u64) -> Leader {
    let hash = account_id.bytes().fold(period, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u64)
    });
    rotation_for_period(period)[(hash as usize) % FREE_ROTATION_SIZE]
}

pub fn leader_available(leader: Leader, owned: &BTreeSet<String>, period: u64) -> bool {
    owned.contains(leader_id(leader)) || rotation_for_period(period).contains(&leader)
}

pub fn gem_bundles() -> Vec<GemBundle> {
    GEM_BUNDLES
        .into_iter()
        .map(|(product_id, gems)| GemBundle {
            id: product_id.to_string(),
            gems,
            product_id: product_id.to_string(),
        })
        .collect()
}

pub fn gem_amount_for_product(product_id: &str) -> Option<u64> {
    GEM_BUNDLES
        .into_iter()
        .find_map(|(known_id, gems)| (known_id == product_id).then_some(gems))
}

pub fn skins() -> Vec<SkinOffer> {
    SKINS
        .into_iter()
        .map(|(id, name, asset_path, cost_gems, style)| SkinOffer {
            id: id.to_string(),
            leader_id: "all".to_string(),
            name: name.to_string(),
            asset_path: asset_path.to_string(),
            cost_gems,
            owned: false,
            style,
        })
        .collect()
}

pub fn skin_by_id(skin_id: &str) -> Option<SkinOffer> {
    skins().into_iter().find(|skin| skin.id == skin_id)
}

pub fn skin_style_for_id(skin_id: Option<&str>) -> u8 {
    skin_id
        .and_then(skin_by_id)
        .map(|skin| skin.style)
        .unwrap_or(0)
}

pub fn resolve_leader(
    requested: Option<&str>,
    account_id: &str,
    owned: &BTreeSet<String>,
    period: u64,
) -> LeaderResolution {
    let fallback = assigned_leader_for_account(account_id, period);
    let requested = requested.and_then(leader_from_id).unwrap_or(fallback);
    let requested_available = leader_available(requested, owned, period);
    let resolved = if requested_available { requested } else { fallback };
    LeaderResolution {
        requested,
        resolved,
        requested_available,
        used_fallback: resolved != requested,
    }
}

pub fn catalog_for_profile(
    owned_leaders: &BTreeSet<String>,
    owned_skins: &BTreeSet<String>,
    laurels: u64,
    gems: u64,
    period: u64,
) -> StoreCatalog {
    let free = rotation_for_period(period);
    let leaders = Leader::ALL
        .into_iter()
        .map(|leader| {
            let owned = owned_leaders.contains(leader_id(leader));
            let free_rotation = free.contains(&leader);
            LeaderOffer {
                id: leader_id(leader).to_string(),
                name: leader.name().to_string(),
                civilization: leader.civilization().name().to_string(),
                perk: leader.perk_description().to_string(),
                cost_laurels: LEADER_UNLOCK_COST_LAURELS,
                free_rotation,
                owned,
                available: free_rotation || owned,
            }
        })
        .collect();

    let skins = skins()
        .into_iter()
        .map(|mut skin| {
            skin.owned = owned_skins.contains(&skin.id);
            skin
        })
        .collect();
    StoreCatalog {
        rotation_period: period,
        free_leaders: free.into_iter().map(|leader| leader_id(leader).to_string()).collect(),
        leaders,
        skins,
        gem_bundles: gem_bundles(),
        laurels,
        gems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_is_eight_and_deterministic() {
        assert_eq!(rotation_for_period(7).len(), FREE_ROTATION_SIZE);
        assert_eq!(rotation_for_period(7), rotation_for_period(7));
        assert_ne!(rotation_for_period(7), rotation_for_period(8));
    }

    #[test]
    fn locked_leader_falls_back_to_account_assignment() {
        let owned = BTreeSet::new();
        // period 7 rotation = Boudica, LadySixSky, Leonidas, Napoleon, Caesar, Cleopatra, Ragnar, SunTzu
        // Alexander is locked at period 7, so it must fallback
        let resolution = resolve_leader(Some("alexander"), "account", &owned, 7);
        assert!(!resolution.requested_available);
        assert!(resolution.used_fallback);
        assert!(rotation_for_period(7).contains(&resolution.resolved));
    }

    #[test]
    fn gem_products_have_authoritative_amounts() {
        assert_eq!(gem_bundles().len(), 3);
        assert_eq!(gem_amount_for_product("sow_gems_1200"), Some(1_200));
        assert_eq!(gem_amount_for_product("unknown"), None);
    }

    #[test]
    fn skins_are_original_and_have_stable_prices() {
        assert_eq!(skins().len(), 3);
        assert_eq!(skin_by_id("storm_grid").map(|skin| skin.cost_gems), Some(450));
        assert_eq!(skin_style_for_id(Some("royal_lattice")), 3);
        assert_eq!(skin_style_for_id(Some("missing")), 0);
    }
}
