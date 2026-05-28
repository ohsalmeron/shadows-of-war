#[cfg(test)]
mod alliance_lifecycle_tests {
    use crate::diplomacy::{AllianceProposal, ALLIANCE_REQUEST_TTL_TICKS};
    use crate::engine::SowEngine;
    use crate::game::{GamePhase, GameState};
    use crate::protocol::{GameplayIntent, StampedIntent};
    use crate::water_components::WaterComponents;

    fn minimal_engine() -> SowEngine {
        let mut game = GameState::new(1, 4, 4, crate::game_config::GameConfig::default());
        game.phase = GamePhase::Playing;
        SowEngine::new(game, WaterComponents::default())
    }

    #[test]
    fn proposal_expires_and_sets_cooldown() {
        let mut engine = minimal_engine();
        engine.state.tick = 0;
        engine.push_alliance_proposal(1, 2);
        engine.state.tick = ALLIANCE_REQUEST_TTL_TICKS as u64 + 1;
        engine.prune_alliance_diplomacy();
        assert!(engine.alliances_proposed.is_empty());
        assert!(engine.alliance_request_cooldown_until.contains_key(&(1, 2)));
        assert!(!engine.can_send_alliance_request(1, 2));
        let until = *engine.alliance_request_cooldown_until.get(&(1, 2)).unwrap();
        engine.state.tick = until as u64 + 1;
        engine.prune_alliance_diplomacy();
        assert!(engine.can_send_alliance_request(1, 2));
    }

    #[test]
    fn reject_marks_cooldown() {
        let mut engine = minimal_engine();
        engine.push_alliance_proposal(1, 2);
        let stamped = StampedIntent {
            player_id: 2,
            intent: GameplayIntent::RejectAlliance { target_player: 1 },
        };
        engine.apply_stamped_intent(&stamped, 0);
        assert!(!engine.can_send_alliance_request(1, 2));
    }

    #[test]
    fn proposal_records_created_tick() {
        let mut engine = minimal_engine();
        engine.state.tick = 42;
        engine.push_alliance_proposal(3, 4);
        assert_eq!(
            engine.alliances_proposed[0],
            AllianceProposal {
                proposer: 3,
                target: 4,
                created_tick: 42,
            }
        );
    }
}
