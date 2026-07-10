use crate::leaders::Leader;
use redb::{Database, TableDefinition, ReadableTable};
use serde::{Deserialize, Serialize};
use redis::Commands;
use std::path::Path;

pub const LEADERS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("leaders");
pub const GEO_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("geo_entities");
pub const PLAYERS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("players");

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LeaderRecord {
    pub name: String,
    pub civilization: String,
    pub perk_description: String,
    pub troop_strength_multiplier: f64,
    pub menu_emoji: String,
    pub filler_rgb: [f32; 3],
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeoEntityRecord {
    pub name: String,
    pub kind: String,
    pub era: String,
    pub region: String,
    pub lat: f32,
    pub lon: f32,
    pub flag: String,
}

/// Automatically initialize and build the redb metadata database file if it does not exist
pub fn init_database<P: AsRef<Path>>(path: P) -> Result<Database, Box<dyn std::error::Error>> {
    let exists = path.as_ref().exists();
    if let Some(parent) = path.as_ref().parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let db = Database::create(path)?;
    if !exists {
        log::info!("Initializing new Redb metadata database from static game data...");
        let write_txn = db.begin_write()?;
        
        {
            let mut leaders_table = write_txn.open_table(LEADERS_TABLE)?;
            for leader in &Leader::ALL {
                let rec = LeaderRecord {
                    name: leader.name().to_string(),
                    civilization: leader.civilization().name().to_string(),
                    perk_description: leader.perk_description().to_string(),
                    troop_strength_multiplier: leader.troop_strength_multiplier(),
                    menu_emoji: leader.menu_emoji().to_string(),
                    filler_rgb: leader.filler_rgb(),
                };
                let bytes = bincode::serialize(&rec)?;
                let slug_key = format!("leader:{}", leader.name().to_lowercase().replace(' ', "_"));
                leaders_table.insert(slug_key.as_str(), bytes.as_slice())?;
            }
        }

        {
            let mut geo_table = write_txn.open_table(GEO_TABLE)?;
            for entity in crate::geo_entities::all() {
                let rec = GeoEntityRecord {
                    name: entity.name.to_string(),
                    kind: format!("{:?}", entity.kind),
                    era: format!("{:?}", entity.era),
                    region: format!("{:?}", entity.region),
                    lat: entity.lat,
                    lon: entity.lon,
                    flag: entity.flag.to_string(),
                };
                let bytes = bincode::serialize(&rec)?;
                let slug_key = format!("geo:{}", entity.name.to_lowercase().replace(' ', "_"));
                geo_table.insert(slug_key.as_str(), bytes.as_slice())?;
            }
        }

        write_txn.commit()?;
        log::info!("Redb metadata database successfully populated and committed!");
    }
    Ok(db)
}

pub fn get_leader_by_name(db: &Database, name: &str) -> Option<LeaderRecord> {
    let slug_key = format!("leader:{}", name.to_lowercase().replace(' ', "_"));
    let read_txn = db.begin_read().ok()?;
    let table = read_txn.open_table(LEADERS_TABLE).ok()?;
    let val_bytes = table.get(slug_key.as_str()).ok()??;
    bincode::deserialize(val_bytes.value()).ok()
}

pub fn get_geo_entity_by_name(db: &Database, name: &str) -> Option<GeoEntityRecord> {
    let slug_key = format!("geo:{}", name.to_lowercase().replace(' ', "_"));
    let read_txn = db.begin_read().ok()?;
    let table = read_txn.open_table(GEO_TABLE).ok()?;
    let val_bytes = table.get(slug_key.as_str()).ok()??;
    bincode::deserialize(val_bytes.value()).ok()
}

/// Seed Valkey's RAM database on startup from the persistent Redb file
pub fn seed_valkey_from_redb(db: &Database, valkey_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = redis::Client::open(valkey_url)?;
    let mut con = client.get_connection()?;
    let read_txn = db.begin_read()?;
    
    let mut seeded_count = 0;

    // 1. Seed Leaders
    if let Ok(table) = read_txn.open_table(LEADERS_TABLE) {
        for item in table.iter()? {
            let (key, val) = item?;
            let key_str = key.value();
            let valkey_key = format!("sow:{}", key_str);
            let _: () = con.set(&valkey_key, val.value())?;
            seeded_count += 1;
        }
    }

    // 2. Seed Geo Entities
    if let Ok(table) = read_txn.open_table(GEO_TABLE) {
        for item in table.iter()? {
            let (key, val) = item?;
            let key_str = key.value();
            let valkey_key = format!("sow:{}", key_str);
            let _: () = con.set(&valkey_key, val.value())?;
            seeded_count += 1;
        }
    }

    // 3. Seed Player Accounts and their platform identity mappings
    if let Ok(table) = read_txn.open_table(PLAYERS_TABLE) {
        for item in table.iter()? {
            let (key, val) = item?;
            let account_id = key.value();
            
            // Write player account record to Valkey
            let valkey_key = format!("sow:player:account:{}", account_id);
            let val_str = std::str::from_utf8(val.value())?;
            let _: () = con.set(&valkey_key, val_str)?;
            seeded_count += 1;

            // Map each linked identity back to the account ID in Valkey
            if let Ok(account) = serde_json::from_slice::<crate::db::PlayerAccount>(val.value()) {
                for identity in &account.linked_identities {
                    let identity_key = format!("sow:player:identity:{}:{}", identity.provider, identity.external_id);
                    let _: () = con.set(&identity_key, account_id)?;
                    seeded_count += 1;
                }
            }
        }
    }

    log::info!("Successfully seeded Valkey with {} records from REDB!", seeded_count);
    Ok(())
}
