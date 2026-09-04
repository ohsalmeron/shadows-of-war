# Google Play Console — takeover de Shadows of War

**Última auditoría:** 2026-09-02  
**Paquete:** `com.shadowsofwar`  
**Proyecto Google Cloud:** `worldofunreal`  
**Cuenta de servicio:** `play-deploy@worldofunreal.iam.gserviceaccount.com`

## Objetivo

Este documento permite que otro agente continúe la operación de Google Play Console y la automatización de releases sin volver a pedir autorizaciones ni repetir investigación.

La regla operativa es simple:

- El agente inspecciona y automatiza todo lo que tenga una API o CLI soportada.
- El navegador se usa únicamente cuando Google no expone esa operación por API o exige una decisión/confirmación del propietario.
- Para cambios de Shadows of War, `./sow p` es el pipeline oficial de web/backend/infra y `./sow a` el pipeline explícito de Android/Play. No se reemplazan con `scp`, `rsync`, SSH manual, symlinks ni reinicios manuales.

## Respuesta inmediata: países de los testers

Los cinco países son mercados normales de Google Play y no hay una prohibición general para usarlos en testing o producción:

| País | Código Play |
|---|---|
| Indonesia | `ID` |
| Kenya | `KE` |
| Nigeria | `NG` |
| Filipinas | `PH` |
| Vietnam | `VN` |

La causa más probable de **“This app isn’t available in your region”** es una de estas:

1. El track **Closed testing — Alpha** no incluye el país del tester.
2. El tester está inscrito en **Internal testing**, no en Alpha. Internal puede funcionar desde cualquier país, pero no cuenta para el requisito de producción.
3. El tester está incluido en la lista, pero todavía no hizo el **opt-in** en Alpha.
4. El país de Google Play de su cuenta no es el país físico donde está conectado. Google usa el país de Play de la cuenta.
5. El primer release o un cambio reciente todavía está propagándose. Google advierte que puede tardar varias horas.

## Único paso manual que probablemente falta ahora

En Play Console:

```text
Shadows of War
→ Test and release
→ Testing
→ Closed testing
→ Alpha
→ Manage track
→ Countries / regions
```

Si aparece sincronizado con producción:

```text
Unsync countries/regions
→ Edit countries
→ agregar Indonesia, Kenya, Nigeria, Philippines y Vietnam
→ Save / Confirm
```

Después se comparte el enlace de Alpha:

```text
https://play.google.com/apps/testing/com.shadowsofwar
```

Cada tester debe abrirlo con la cuenta que está en la lista y pulsar **Join the test**. Si esa cuenta está en Internal, primero debe salir de Internal y luego entrar a Alpha. Una vez inscrito en Alpha, las versiones nuevas del mismo track se actualizan sin repetir el opt-in.

El cambio de países de un **closed testing track** no está expuesto como una operación de escritura en la API REST pública de Google Play. La API sí permite leer la disponibilidad. Por eso este paso concreto se hace en el navegador, salvo que Google cambie la API.

## Estado verificado de Play Console

Estado observado en las últimas capturas/auditorías; debe revalidarse antes de afirmar que cambió:

- Package name: `com.shadowsofwar`.
- Track closed activo: `Alpha`.
- Último release observado: `0.1.2`, version code `24`, actualizado el 31 de agosto de 2026.
- Alpha mostraba 31 países/regiones.
- Lista de testers de Alpha: `Alpha testers`, 26 usuarios observados.
- En una captura anterior: 0 testers opted-in. El número actual debe leerse en Play Console; no se debe asumir.
- Internal testing está activo, pero sus testers no cuentan para el requisito del closed test.
- Production todavía no estaba habilitado en la última evidencia.
- El requisito mostrado por Play Console era: release de closed testing publicado, al menos 12 testers opted-in y al menos 14 días de prueba cerrada.

## Cómo funciona el requisito de 12 testers y 14 días

No son 12 instalaciones aisladas ni 12 cuentas autorizadas. Para que cuenten:

1. La cuenta debe estar en la lista de Alpha.
2. Debe abrir el enlace de Alpha con esa cuenta.
3. Debe pulsar **Join the test / Opt in**.
4. Debe permanecer inscrita durante el periodo requerido.

Una actualización de version code dentro del mismo Alpha no reinicia por sí sola el opt-in. Los testers permanecen en el track y reciben la actualización cuando está disponible.

Internal testing sirve para pruebas rápidas en cualquier país, pero esos opt-ins no satisfacen el requisito de producción. Open testing tampoco sustituye el requisito de closed testing que muestra esta cuenta.

## Qué puede automatizar otro agente

### Lectura y verificación de Play Console

Con la cuenta de servicio ya activa, el agente puede:

- Leer tracks, releases y version codes.
- Verificar qué release está activo en Alpha.
- Leer la disponibilidad de países del track.
- Verificar que el AAB tenga package name y version code correctos.
- Detectar si un version code ya fue usado.
- Crear/validar releases mediante el pipeline existente cuando el pipeline lo soporte.
- Ejecutar verificaciones post-release sin tocar servidores manualmente.
- Preparar la solicitud de acceso a producción y registrar qué requisito falta.

### Lo que no debe automatizar a ciegas

- Cambiar países del closed track si la API pública no lo soporta.
- Aplicar a producción sin que Play Console muestre el botón habilitado.
- Crear productos Play ignorando el estado del payments profile.
- Reusar un version code.
- Subir archivos `.json` junto con el `.aab`. El JSON de service account nunca es un artefacto de la app.
- Imprimir claves, tokens o el contenido del service-account JSON.

## API oficial de Google Play

La API relevante es **Google Play Android Publisher API v3**, no una API secreta de la interfaz web.

Scope OAuth requerido:

```text
https://www.googleapis.com/auth/androidpublisher
```

La clave local existente es:

```text
/home/bizkit/.config/shadows-of-war/google-play-service-account.json
```

No copiarla al repositorio, no pegar su contenido en chat y no incluirla en un release.

### Endpoints útiles de lectura

La API trabaja con un `editId` temporal. Crear un edit tiene efecto en Play Console; un agente debe crear uno solo cuando vaya a ejecutar una operación concreta y debe cerrarlo correctamente.

```text
POST /androidpublisher/v3/applications/com.shadowsofwar/edits
GET  /androidpublisher/v3/applications/com.shadowsofwar/edits/{editId}/tracks
GET  /androidpublisher/v3/applications/com.shadowsofwar/edits/{editId}/countryAvailability/alpha
```

El resultado de `countryAvailability` contiene, entre otros:

```text
syncWithProduction
countries[].countryCode
restOfWorld
```

La API de tracks conoce releases y `countryTargeting`, pero Google documenta ese campo para releases **inProgress de production**; no es una vía documentada para editar los países de un closed testing track.

### Validación segura de version codes

El repo ya tiene soporte para revisar tracks con Fastlane. La ruta de Fastlane existente es:

```text
/home/bizkit/.local/share/gem/ruby/3.4.0/bin/fastlane
```

Ejemplo de lectura, no de deploy:

```sh
/home/bizkit/.local/share/gem/ruby/3.4.0/bin/fastlane \
  run google_play_track_version_codes \
  package_name:"com.shadowsofwar" \
  track:"alpha" \
  json_key:"/home/bizkit/.config/shadows-of-war/google-play-service-account.json"
```

El script del repo es:

```text
/home/bizkit/Github/shadows-of-war/scripts/android-release.sh
```

Para producción Web/backend/infra de Shadows of War se usa:

```sh
cd /home/bizkit/Github/shadows-of-war
./sow p
```

Para Android/Google Play se usa únicamente `./sow a` (o `./sow android`).
La interfaz completa es `./sow p` para Web/backend/infra, `./sow a` para
Android/Play, `./sow l`/`./sow local` para preview y `./sow native` para el
cliente nativo.

## Reglas para hacer opt-in sin confusión

Para cada tester finalista:

```text
1. Agregar su correo a Alpha testers.
2. Compartir https://play.google.com/apps/testing/com.shadowsofwar
3. El tester abre el enlace con esa misma cuenta de Google Play.
4. Pulsa Join the test.
5. Instala Shadows of War desde Google Play.
6. Confirmar el opt-in desde Alpha → Testers / estadísticas de testers.
```

No se necesita una invitación automática por correo si el tester ya está en la lista; el enlace manual es suficiente. Sí se necesita que la cuenta esté en la lista y que el tester pulse opt-in.

## Diagnóstico específico de “no disponible en mi región”

El agente debe comprobar en este orden:

```text
1. Alpha → Countries / regions: confirmar ID, KE, NG, PH y VN.
2. Alpha → Testers: confirmar que el correo exacto está en Alpha testers.
3. Confirmar que el tester no sigue inscrito en Internal.
4. Confirmar que abrió el enlace con la cuenta correcta.
5. Confirmar el país de Google Play de esa cuenta.
6. Esperar propagación solo después de que los puntos 1–5 sean correctos.
```

No usar VPN como supuesto de corrección: el país de Play de la cuenta es el dato principal.

## Monetización: relación entre Play, RevenueCat y el pipeline

### Google Play

Los productos de compra única se crean en Google Play y luego se registran/vinculan en RevenueCat. RevenueCat no puede inventar un producto que todavía no existe en Play.

La API antigua `inappproducts` está deprecada para este caso. La API actual usa `oneTimeProducts:batchUpdate`, pero la cuenta tuvo este bloqueo documentado:

```text
FAILED_PRECONDITION:
Cannot manage a one-time product without first registering a payments profile
for the developer account.
```

El payments profile ya fue iniciado/enrolado, pero la captura del 1 de septiembre mostraba **Verification pending**. Hasta que Google termine esa verificación, un intento de crear productos Play puede seguir fallando. No es un problema de RevenueCat ni de versionado.

### RevenueCat

CLI ya autenticado; no pedir login otra vez:

```text
profile: default
project: projf6bc119d
```

Apps existentes:

- Test Store: `app5cbcb3e77b`.
- App Store: `app44fef55941`, bundle `games.shadowsofwar.app`.
- Play Store: `appc659dd11dd`, package `com.shadowsofwar`.
- Stripe Web Billing: `app5ab74d80c4`, configurada para el Stripe de Shadows of War.

Productos Play registrados actualmente en RevenueCat:

```text
sow_gems_500
sow_gems_1200
sow_gems_2600
```

Offering `default` existente:

```text
$rc_monthly  → sow_gems_500
$rc_annual   → sow_gems_1200
$rc_lifetime → sow_gems_2600
```

Los nombres `$rc_monthly`, `$rc_annual` y `$rc_lifetime` son IDs de package heredados; el catálogo real son bundles consumibles de gems, no suscripciones.

No hay un entitlement asociado al offering de lanzamiento. No se debe crear ni
adjuntar `shadows_of_war_pro` a estos productos: son bundles consumibles de
gemas y el backend los entrega mediante el webhook de compra única.

Comandos de lectura del CLI:

```sh
npx --yes @revenuecat/cli apps list \
  --project-id projf6bc119d --json

npx --yes @revenuecat/cli products list \
  --project-id projf6bc119d --json

npx --yes @revenuecat/cli offerings list \
  --project-id projf6bc119d --json
```

Si una orden del CLI cambia entre versiones, descubrir el schema en vez de adivinar flags:

```sh
npx --yes @revenuecat/cli commands --schemas --json
```

RevenueCat CLI puede administrar la configuración de RevenueCat, pero no sustituye Play Console para habilitar países, completar verificaciones financieras ni publicar la app.

### Web

RevenueCat Web Billing está configurado mediante el Purchase Link de producción y Stripe; el cliente Web debe usar ese enlace. Eso es independiente del release Android.

## Checklist de takeover para otro agente

```text
[ ] Leer este documento completo.
[ ] Confirmar repo, branch y git status sin modificar trabajo ajeno.
[ ] Verificar que la autenticación existente funciona sin imprimir secretos.
[ ] Leer tracks, release activo, version code y country availability.
[ ] Leer testers de Alpha y separar “en lista” de “opted in”.
[ ] Confirmar ID/KE/NG/PH/VN en Alpha.
[ ] Si faltan, indicar al propietario el único paso manual de Countries / regions.
[ ] No pedir re-login de RevenueCat si el perfil default sigue autenticado.
[ ] No crear productos Play hasta que payments profile esté verificado.
[ ] Para cambios del cliente Web/backend/infra usar `./sow p`; para Android/Play usar `./sow a`.
[ ] Verificar el resultado en Play Console/API y documentar fecha, track y version code.
```

## Registro de evidencia

Actualizar esta tabla en cada takeover:

| Fecha/hora | Track | Version code | Países relevantes | Usuarios en lista | Opted-in | Estado |
|---|---|---:|---|---:|---:|---|
| 2026-09-02 | Alpha | 24 observado | 31 observados; ID/KE/NG/PH/VN por confirmar | 26 observado | Releer en Console | Requiere verificación |

## Fuentes oficiales

- [Google Play: distribuir releases a países específicos](https://support.google.com/googleplay/android-developer/answer/7550024?hl=en)
- [Google Play: configurar testing abierto, cerrado o interno](https://support.google.com/googleplay/android-developer/answer/9845334?hl=en)
- [Android Publisher API: leer disponibilidad por país](https://developers.google.com/android-publisher/api-ref/rest/v3/edits.countryavailability/get)
- [Android Publisher API: tracks y releases](https://developers.google.com/android-publisher/api-ref/rest/v3/edits.tracks)
- [RevenueCat Web Billing](https://www.revenuecat.com/docs/web/web-billing/overview)
- [RevenueCat Web Purchase Links](https://www.revenuecat.com/docs/web/web-billing/web-purchase-links)

## Prompt corto para entregar a otro agente

```text
Toma el takeover de Google Play Console para Shadows of War.
Lee primero docs/play-console-takeover.md.
Package: com.shadowsofwar.
Track de requisito: closed testing / Alpha.
Investiga y reporta primero: disponibilidad de ID, KE, NG, PH y VN; testers en lista versus opted-in; release y version code activos.
Usa la API/CLI existente y no vuelvas a pedir autenticación.
No imprimas secretos.
El cambio de países de Alpha probablemente requiere Play Console web porque la API pública solo expone lectura.
No hagas deploy manual: para cambios del repo usa únicamente ./sow p (web/backend) y ./sow a (Android/Play).
Reporta exactamente qué puede hacer el agente y qué único paso manual queda.
```
