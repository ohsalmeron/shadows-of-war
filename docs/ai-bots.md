# AI Bots — diseño, parámetros y la saga de los 24h (ago 2026)

Doc de referencia para todo trabajo futuro en los bots. La moraleja operativa primero:

> **Tres lecciones caras (cada una costó un deploy fallido):**
> 1. Copiar reglas de OpenFront sin su ecología produce lo contrario. El "2× bar" de OF funciona
>    porque sus bots son zero-brain y se quedan chiquitos; las tribus de SoW crecen sin límite.
> 2. Cualquier barra basada en ratio de tropas favorece estructuralmente a la tribu: las tribus
>    viven pegadas a su `max_troops` (nunca gastan), las naciones viven al 30-60% del suyo
>    (las sweep drenan tropas y cada conquista agranda el tope).
> 3. Validar en el Lab ANTES de deploy, y verificar en cuál path de código corre el cambio
>    (el mapa mundo usa `random_spawn=false` → fase Spawning de `tick.rs`, no `spawn_human`).

## La cadena causal de la saga (26-28 ago 2026)

Cada fix destapó la pared siguiente. Orden cronológico:

1. **Bancarrota de iq_points** (causa raíz del "nunca avanzan"): la guerra cobraba 5-10 pts/acción;
   un ghost gastaba 5-10/s ganando ~1.7/s → quebraba → su candado congelaba TODAS las acciones,
   incluso crecer. Fix: expandir a neutral es GRATIS (crecimiento, no guerra), deducciones de
   guerra con `.max(0.0)`, candado de bancarrota eliminado (`combat.rs`).
2. **Trigger imposible**: el gate clásico `troops ≥ max_troops × trigger_ratio` era inalcanzable
   a mitad de partida (el max explota con territorio, las tropas van detrás). Fix: una decisión
   de odds comprometida **es** el trigger (`odds_committed ||` trigger gate).
3. **Odds discipline vs jugadores** (paridad OF `isAttackTooWeak` + `weakest`): en FFA no iniciar
   con menos del 20% de las tropas del objetivo ni contra quien supera tus tropas. Bloqueado +
   neutral libre → expandir (OF: expansión antes que guerra). Bloqueado y encerrado → bankear.
   **Defensa/retaliación siempre exentas. Team games exentos** (en OF `troopSendCap`/
   `isAttackTooWeak` son FFA-only — portarlo a todos los modos fue un bug).
4. **Atrición vs tribus**: se probó OF-parity 2× → matemáticamente insatisfacible (tribu max = ÷1.5
   handicap; nación al cap llega a affordable 1.2× tribe_max < 2×). Luego 1× → el lab volvió a
   congelarse. Final: **sin piso** vs tribus — se ataca siempre la tribu fronteriza con golpe
   `min(4× tribe_troops, affordable)`.
5. **Swing anti-stall**: si el objetivo elegido (más débil) está bloqueado por odds y hay tribu en
   la frontera, se ataca la tribu en vez de bankear (lab W1: IAs con contacto tribal y 0 ataques).
6. **Movilidad**: D1 = sin neutral en frontera → bote a orilla neutral aleatoria (`try_expansion_boat`,
   gratis, todos los tiers). D2 = cercado sin puerto puede lanzar flota (`enclosed && tier != Tribe`).
7. **Cascade (a)**: tribus Vanilla no capturan tiles de jugadores en la cascada de cercado
   (`set_tile_owner`, guard `capturer_is_passive_tribe`).
8. **Spawn por zonas** (OF `teamSpawnArea`): `Team {Red, Blue}` → Rojo mitad izquierda, Azul mitad
   derecha del mapa. Todos los miembros del equipo (incl. el primero y humanos rezagados) caen
   dentro de su zona; piso de 14 tiles entre hogares contra todos; fallback anillo 12..36 → random.
   **El path vivo del mapa mundo es la fase Spawning de `tick.rs`** (world maps: `random_spawn=false`
   → `spawn_human` registra sin posición y regresa). S17/S18 existen para que esto no se vuelva a
   perder.

## Parámetros actuales (fuente: código; actualizar este doc al cambiarlos)

| Parámetro | Ghost | Nation | Tribe (Vanilla) |
|---|---|---|---|
| IQ band | 160-181 | 130-160 | 50-86 |
| Cadencia base (ticks) | 5 | 30 | 100 |
| trigger_ratio | 0.05 | 0.45 | (ignorado, attacks_players=false) |
| reserve_ratio | 0.02 | 0.20 | 0.50 |
| expand_ratio | 0.02 | 0.15 | 0.10 |
| Costos iq (war/build/alliance/send) | 5/5/5/5 | 5/5/5/999 | 10/10/10/999 |
| Contadores reales | ~83-118 | 128 | 420 |

- Neutral expansion: **gratis**; guerra cuesta `attack_cost` con clamp ≥0.
- `max_troops = 10 + tiles^0.625 × 350 + 5000×city_levels`; tribus ÷1.5.
- Ingreso tropas = 250 base + 25×cities + tiles/16 por segundo (tribus ×0.75).
- Ghost fill: 65-92% de `max_players` (`SOW_BOT_FILL_MIN/MAX`).
- Rostros: `spawn_ai(nation_count=128, bot_count=420)` + spawn_scripted (mapa).
- Tierras: tribus nunca inician contra no-tribus; tribu-vs-tribu sí pelean (troops/4).

## El Lab (regresión de comportamiento)

`sow-core/src/intent/nation/bot_lab.rs` — S1-S18 bajo `#[cfg(test)]`:
S1 ghost expande · S2 ghost presiona · S3 tribu Vanilla pasiva-creciente · S4 nación defiende ·
S5 (ignorado, harness de agua) · S7 alguien gana · S9 cluster · S10 mapa mundo real midgame ·
S11 mundo particionado sigue en guerra · S12 bote-TN de isla · S13 zonas de equipo (spawn_human) ·
S14 caza decisiva de tribus · S15 no-suicidio FFA · S16 ecosistema largo (firma de freeze) ·
S17 zonas en fase Spawning (el path vivo) · S18 mapa mundo real + zonas (geografía real).

Regla de oro: **cambios de IA sin Lab verde no se despliegan.** El lab usa el mapa mundo real
(`WORLD_MAP_BYTES`) — geografía, no cajas planas.

## Lección de proceso (por qué falló dos veces)

- Fix sin commit + refactor de hermano = fix perdido (pasó con avatares y con spawn-zones).
- Verificar EN QUÉ path corre el cambio antes de shipearlo: grep del flag (`random_spawn`) y de
  todos los call sites del helper compartido.
- Lab verde contradiciendo prod = condiciones del sim equivocadas (o entidad de tier equivocado —
  `build_lab` mintió naciones como Bot-type una vez; `ai_tier` se resuelve por
  `(player_type, is_ai_controlled)`).
