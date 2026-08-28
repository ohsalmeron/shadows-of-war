# F-Stack TX Wedge — el bug silencioso (ago 2026)

Historia, causa raíz y instrumental del wedge de TX del relay. Doc de referencia para
cualquier trabajo futuro en `fstack-bridge/` o el relay.

## El bug en una frase

Dentro de f-stack (stack TCP en userspace sobre DPDK), la función que arma el timer de
**retransmisión** — `callout_when` — era un **stub no-op** en el árbol desplegado: TCP
asume que un paquete perdido se retransmite vía timer; sin timer armado, **un solo paquete
perdido = conexión congelada para siempre** (`so_snd` full, writes en EAGAIN eterno) y
**cero errores en ningún log**.

## Timeline (fechas locales del supervisor, UTC-6)

| Fecha | Evento |
|---|---|
| ~1 ago | `libfstack.a` compilado/desplegado en Azure — ya traía el stub (viene del port) |
| 18 ago | Primer reporte de freezes del supervisor; atribuido (mal) a bundle viejo |
| 20 ago | Freezes persisten; logs del relay sanos → la capa WS era **ciega** al wedge (sin contadores) |
| 21 ago | **Causa raíz confirmada en host**: `nm` sobre el `libfstack.a` desplegado mostró `callout_when` como stub (un símbolo; desensamblado verificado). El knob del supervisor (timer de envío) cambiaba la duración del freeze, no su existencia — validó que la falla vivía en la cadena de drenaje TCP |
| 21-22 ago | **Fix real**: `callout_when` implementado en el fork (commit `52fa8f9ae`), `libfstack.a` recompilado en Azure, relay recompilado. Prueba viva: `[BOOT] fstack=52fa8f9ae666` |
| 22-23 ago | Knob de write-timeout a 15000ms, TCP_NODELAY, e instrumental del bridge (commit `e3fa033`) |
| 26 ago | TCP_NODELAY verificado vivo (ed/txd 1:1) |

## El instrumental del bridge (por qué existe)

Un wedge silencioso es indetectable sin contadores. `fstack-bridge/src/bridge.rs`:

- `Cmd::Send { fd, generation, buf, tx_pending }` — el comando carga un contador compartido
  (`Arc<AtomicUsize>`) que la capa WS puede leer.
- `PendingSend` — cola por conexión de bytes no enviados: `bytes`, `first_stalled_at`
  (Instant del stall), y actualización del contador para la capa de arriba.
- `TX_STALL_TIMEOUT_SECS = 10` — un stall que supera esto es un wedge candidate.
- `MAX_CONN_PENDING_BYTES = 128 * 1024` — tope de bytes pendientes por conexión.
- Knob en prod: `SOW_WS_WRITE_TIMEOUT_MS=15000` (registrado en el manifest del relay).

Esta instrumentación es la que convirtió "se congela y no hay nada en los logs" en datos
(bytes pendientes, duración de stall) — sin ella el wedge era indetectable por diseño.

## Lecciones

1. **Los stubs deben paniquear, no devolver 0.** El stub de `callout_when` existió meses
   callado. Un stub que paniquea al usarse se detecta el día uno.
2. **Un stall silencioso requiere contadores ANTES del incidente** — después del incidente
   no hay nada que leer.
3. `strings(1)` sobre binarios release NO sirve para forense de fuentes en este proyecto
   (los literales del `log` crate no sobreviven — verificado con control). Usar
   `BUILD_EPOCH`/`[SERVER-BOOT]` y hashes de release.
4. El knob que mueve la duración del síntoma (no su existencia) localiza la falla: la
   sensibilidad del knob apuntó a la cadena de drenaje antes de cualquier evidencia binaria.

## Estado del fork

- Fix `callout_when` = commit `52fa8f9ae` en NUESTRO fork (no upstream). Fork ~439 commits
  adelantado de `origin/master` (la mayoría ya upstreamed a F-Stack `dev`).
- Azure corre NUESTRO fork — `[BOOT] fstack=52fa8f9ae…` es la prueba de runtime.
- Los stubs restantes del port (`ff_stub_14_extra.c`) deben paniquear — seguimiento pendiente.
