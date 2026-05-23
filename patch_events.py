import sys

with open("sow-core/src/engine.rs", "r") as f:
    content = f.read()

events_enum = """
#[derive(Debug, Clone)]
pub enum AiEvent {
    IncomeTick(u16),
    UnderAttack { target: u16, attacker: u16, tile: u32 },
    BuildingCompleted { owner: u16, tile: u32, kind: crate::game::BuildingKind },
}
"""

if "pub enum AiEvent" not in content:
    content = content.replace("pub struct PlacementScratch {", events_enum + "\npub struct PlacementScratch {")

content = content.replace(
    "pub ai_round_robin: usize,",
    "pub ai_round_robin: usize,\n    pub ai_events: std::collections::VecDeque<AiEvent>,"
)

content = content.replace(
    "ai_round_robin: 0,",
    "ai_round_robin: 0,\n            ai_events: std::collections::VecDeque::new(),"
)

with open("sow-core/src/engine.rs", "w") as f:
    f.write(content)
