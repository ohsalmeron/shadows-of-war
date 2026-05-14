# Shadows of War Design Journal

## Core Philosophy
We are shifting from an instructional RTS to a combinatorial RTS, drawing inspiration from deckbuilders and civilization builders.

### 1. Hex Topology Foundation
The map layout has transitioned from a square array to an odd-r hex grid. This provides equidistant neighbors, eliminating diagonal pathfinding issues and creating organic borders for troops to expand into.

### 2. Troops are Chips, Policies are Multipliers
Standard games use linear logic where more units win. Shadows of War is now combinatorial. 
* Base Value: The troop count on a hex.
* Multipliers: Civilization-style leaders, unique buildings, and policies.
* Execution: A small group of troops combined with a potent defense multiplier can hold off massive armies.

### 3. The Powerless Reversal
Losing map control will no longer mean a slow, agonizing defeat. Cornered players gain access to extreme abilities.
* Desperation Policies: Reaching a critically low hex count unlocks unique mechanics, such as an Insurgency state that multiplies troop strength on core territory.
* Scorched Earth: A dying player can sacrifice their final hexes to spawn a massive, uncap-restricted elite horde for one final push.

### 4. Combinatorial Diplomacy
Diplomacy is a mechanical tool.
* Shared Multipliers: Alliances provide shared mechanical buffs, making cooperation mechanically rewarding.
* The Traitor Gambit: Betrayal acts as a game-breaking card. A cornered player can trigger a betrayal to instantly seize control of allied border hexes, making them dangerous prey.

### 5. Design for Discovery
The game balance relies on chaotic, high-impact modifiers rather than strict unit counters. Players are encouraged to discover overpowered synergies. When a player finds a game-breaking combo, they take total ownership of the strategy.

### 6. Networking & Synchronization Architecture
* **Binary vs JSON**: The engine relies on `bincode` serialization rather than JSON over WebSockets. While JSON is highly debuggable, RTS data (tile states, intent enums, tick IDs) consists of dense numeric structures. Binary serialization yields massive performance gains in high-throughput environments (60Hz turn data) and drastically reduces payload sizes, avoiding CPU parsing bottlenecks on the WASM client.
* **Strict Type Envelopes**: Raw binary is opaque and unforgiving. Without explicitly tagged enums (`ServerMessage`), a dropped byte or an un-versioned struct update causes silent deserialization failures. We enforce all network traffic to be wrapped in a strongly typed Rust enum to guarantee schema discipline and correct routing.
