//! Episode 1 — Boudica's Rebellion on the `boudica` map (896×504; x→east, y→south).
//!
//! The roster (who/where/role/iq + player spawn) lives in ONE place:
//! `assets/campaign/boudica.json`, authored visually with `tools/campaign-editor` (`./sow m`).
//! That file is the single source of truth — embedded here as the compiled-in default so web /
//! offline always has it, and overridden at runtime on native by the same file on disk (export →
//! relaunch, no recompile). There is deliberately no second, hand-written Rust roster to drift.

/// The committed roster, embedded so it ships in every build (incl. wasm, which can't read files).
/// Exactly the bytes the native runtime override reads — one source, no divergence.
const DEFAULT_ROSTER: &str = include_str!("../../../assets/campaign/boudica.json");

/// Last-ditch spawn if even the embedded roster fails to parse (a test guards that it never does).
const FALLBACK_SPAWN: (u32, u32) = (720, 180);

/// The episode roster + player spawn the tutorial uses. Resolution order (one path, no confusion):
/// 1. native runtime override — `$SOW_CAMPAIGN_ROSTER` or `assets/campaign/boudica.json` on disk
///    (the editor loop: Export → relaunch, no recompile);
/// 2. the embedded committed default (same file, compiled in) — web and installed builds;
/// 3. an empty roster at `FALLBACK_SPAWN` (only if the embedded JSON is somehow corrupt).
pub fn roster() -> (Vec<super::Faction>, (u32, u32)) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let env_path = std::env::var("SOW_CAMPAIGN_ROSTER").ok();
        let candidates = env_path
            .as_deref()
            .into_iter()
            .chain(["assets/campaign/boudica.json"]);
        for path in candidates {
            if let Some(loaded) = super::load_roster_json(path) {
                log::info!(
                    "campaign: roster from file '{}' ({} factions, spawn {:?})",
                    path,
                    loaded.0.len(),
                    loaded.1
                );
                return loaded;
            }
        }
    }
    super::parse_roster(DEFAULT_ROSTER).unwrap_or_else(|| {
        log::error!("campaign: embedded boudica.json failed to parse — using empty roster");
        (Vec::new(), FALLBACK_SPAWN)
    })
}

#[cfg(test)]
mod tests {
    // Poka-yoke: the embedded roster (the single source of truth) must always parse and look sane.
    // If the committed JSON is broken, the build fails here instead of the campaign silently dying.
    #[test]
    fn embedded_roster_parses_and_is_sane() {
        let (factions, spawn) = crate::campaign::parse_roster(super::DEFAULT_ROSTER)
            .expect("embedded assets/campaign/boudica.json must parse");
        assert!(
            factions.len() >= 10,
            "roster looks too small ({} factions)",
            factions.len()
        );
        assert!(spawn.0 > 0 && spawn.1 > 0, "player spawn unset");
    }
}
