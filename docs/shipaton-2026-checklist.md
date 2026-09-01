# Shadows of War — Shipaton 2026

Checklist de elegibilidad, publicación, RevenueCat, monetización y submission.

**Última revisión:** 28 de agosto de 2026  
**Deadline de Shipaton:** 30 de septiembre de 2026, 11:45 p. m. PDT  
**Categoría principal:** Best Game Award  
**Package name Android:** com.shadowsofwar

> Este documento es el registro operativo del proyecto. Se actualiza conforme se confirme cada punto en Play Console, RevenueCat y Devpost. No guardar aquí contraseñas, JSON de service accounts, tokens privados ni secretos.

## 1. Veredicto actual

**Sí se puede intentar participar con Shadows of War.** El proyecto no está descartado por estar actualmente en **Internal testing**.

La diferencia importante es:

- **Internal testing:** distribución privada para testers; no es el lanzamiento público.
- **Production:** primera publicación pública para usuarios de Google Play.

El FAQ de Shipaton permite que un proyecto que existía solamente en web publique su primera versión de tienda durante el evento. Por lo tanto, si Shadows of War nunca estuvo públicamente disponible en Google Play, App Store o Galaxy Store antes del periodo del evento, la ruta de Android sigue siendo viable.

### Estado confirmado

| Requisito | Estado | Qué falta |
|---|---|---|
| Plataforma soportada | ✅ Android | Usar el wrapper Android existente |
| Package name | ✅ com.shadowsofwar | No cambiarlo |
| Android developer verification | ✅ Registered | La captura muestra package registrado y 3 signing keys |
| Track actual | ✅ Internal + Closed en preparación | Play exige Closed testing; la captura muestra 0 testers opt-in |
| Estados Unidos | ⚠️ Seleccionado | Verificar que también esté incluido en Production |
| Primera publicación pública | ⚠️ Por confirmar | Revisar historial de releases; Internal no cuenta como publicación pública |
| Acceso a Production | 🔒 Bloqueado por Google Play | Closed testing está en 3/5; falta enviar la release a revisión y conseguir 12 testers durante 14 días |
| App publicada antes del deadline | ⏳ Pendiente | Primero completar Closed testing y solicitar acceso a Production |
| RevenueCat SDK | 🟡 Código integrado | Falta configurar key/productos y ejecutar compra real de prueba |
| Producto digital en Google Play | ❌/⏳ Pendiente | Crear y activar un producto in-app |
| Entitlement y Offering | 🟡 Pendiente de configurar | Para gems se usa offering/productos; la concesión la hace el webhook |
| Video público | ❌ Pendiente | Video de menos de 2 minutos, en dispositivo real |
| Promo/free trial para judges | ❌ Pendiente | Crear una forma de probar el contenido premium |
| Submission de Devpost | ❌ Pendiente | Completarla antes del deadline |

### Importante sobre ./sow p

./sow p publica los componentes web, servidor, base de datos y relay de Shadows of War. Actualmente no es el botón que publica el AAB Android en Google Play. La publicación Android se termina en Play Console.

### Artefacto Android

El AAB se genera únicamente por el pipeline en:

    /home/bizkit/Github/shadows-of-war/dist/android/com.shadowsofwar.aab

El pipeline consulta el versionCode más alto de todos los tracks de Play y genera el siguiente. No reutilizar un AAB viejo ni editar JSON de metadata junto al AAB. La configuración vigente del wrapper es `compileSdk 36`, `targetSdk 36` y `applicationId com.shadowsofwar`.

Si después hacemos cambios de código para RevenueCat, el pipeline oficial sigue siendo la validación de producción del proyecto según las reglas de este repositorio. No se deben hacer despliegues manuales de infraestructura.

## 2. La ruta más rápida a Production

No hay que cambiar com.shadowsofwar. El package name ya está correcto y cambiarlo crearía otra aplicación en Play Console.

### Paso 0 — Revisar la condición que decide si hay que esperar 14 días

En Play Console abre el **Dashboard** de Shadows of War y busca uno de estos estados:

1. **Apply for production disponible:** la cuenta puede solicitar acceso a Production. Continúa con el setup y la release.
2. **En tu caso:** Play exige un closed test. La pantalla muestra tres requisitos pendientes o incompletos: publicar la release de closed testing, conseguir 12 testers opt-in y mantenerlos durante 14 días.
3. **No necesitas investigar el tipo de cuenta ahora:** la propia pantalla de Play Console ya confirmó cuál es el camino que aplica a esta app.

**Internal testing no sustituye ese closed test.** Internal sirve para QA rápido, pero para esta condición se necesita publicar y mantener un track de **Closed testing**.

### Paso 1 — Completar el setup mínimo de Google Play

En **Dashboard**, resolver todos los pendientes que bloqueen la publicación:

- Store listing: nombre, descripción corta, descripción completa, icono y screenshots.
- App access: explicar si se necesita login y proporcionar instrucciones/cuenta de prueba si aplica.
- Ads declaration.
- Target audience and content.
- Content rating.
- Data safety.
- Privacy policy pública.
- Categoría, tags y datos de contacto.
- Países/regiones: confirmar que Estados Unidos esté seleccionado para Production.
- App signing y upload key.
- Android developer verification: package com.shadowsofwar registrado y signing keys verificadas.
- Precio/distribución: confirmar que la app base sea gratuita si el plan es monetizar dentro del juego.

No avanzar mientras el Dashboard muestre errores rojos. Los warnings se revisan, pero los errores bloquean la release.

### Paso 2A — Estado actual: completar Closed testing

La captura actual muestra **3 of 5 complete**. La release ya fue creada, pero todavía no se ha enviado a revisión y ningún tester ha hecho opt-in.

La única acción inmediata es:

1. Abrir **Preview and confirm the release**.
2. Revisar que el AAB, el nombre de la app y la versión sean correctos.
3. Continuar hasta pulsar **Send the release to Google for review**.
4. Esperar a que la release pase a publicada en **Closed testing**.

Después de que Google publique el closed test:

1. Ir a **Test and release → Testing → Closed testing**.
2. Crear o usar el track inicial de closed testing.
3. Agregar por lo menos 12 cuentas Google reales. Recomiendo invitar 15–20 para tener margen si alguien abandona.
4. Copiar el opt-in link y enviarlo a los testers.
5. Cada tester debe aceptar el opt-in e instalar la app.
6. Los 12 testers deben permanecer opt-in continuamente durante 14 días.
7. Mantener un registro de feedback, bugs y correcciones; Google pide resumirlo en la solicitud de Production.
8. No mover el proyecto a Open testing; no es necesario y puede hacer pública la ficha de prueba antes de tiempo.
9. Al cumplir el periodo, volver al Dashboard y pulsar **Apply for production**.

Si se empieza el 28 de agosto, el periodo de 14 días debería terminar aproximadamente entre el 10 y 11 de septiembre, según cómo Play registre el momento exacto del opt-in. La fecha real válida será la que muestre Play Console.

### Paso 2B — No aplica al estado actual

La captura confirmó que Google sí exige Closed testing para esta app. No usar esta ruta; primero hay que publicar el closed test, conseguir 12 testers opt-in y completar los 14 días.

### Paso 3 — Solicitar Production access, si aparece

En **Dashboard → Apply for production**, responder las tres partes:

- **About your closed test:** cuántos testers participaron, cuánto duró, qué feedback recibiste y qué corregiste.
- **About your app/game:** qué hace Shadows of War, cómo se juega y quién es el público.
- **About your production readiness:** por qué el juego está listo, cómo monitorearás errores y cómo atenderás feedback.

Google indica que la revisión suele tardar siete días o menos, pero puede tardar más. No esperar a la última semana.

### Paso 4 — Crear la primera release pública

Cuando Production esté habilitado:

1. Ir a **Test and release → Production**.
2. Pulsar **Create new release**.
3. Subir el AAB firmado con la misma upload key de la aplicación.
4. Confirmar el número de versión. Debe ser mayor que cualquier versionCode ya usado en Play Console.
5. Usar un release name claro, por ejemplo 0.2.1-shipaton.
6. Agregar release notes en inglés.
7. Confirmar países/regiones y verificar que United States esté incluido.
8. Resolver todos los errores de la pantalla de revisión.
9. Pulsar **Start rollout to production** para la primera publicación.
10. Esperar a que el estado sea **Published/Available**, no **In review**.
11. Guardar la URL pública de Google Play y la fecha/hora de publicación.

Para Shipaton, la app debe estar publicada y descargable desde Estados Unidos antes del deadline. Una submission con la app todavía bajo revisión no cumple el requisito.

## 3. RevenueCat: qué significa realmente “integrarlo”

No basta con crear una cuenta o agregar una dependencia. Shipaton exige una app funcional que use el RevenueCat SDK para operar al menos una compra in-app, compra web o RevenueCat Ads.

Para el camino Android más seguro, la compra debe pasar por **Google Play Billing** y RevenueCat debe administrar el producto, entitlement, offering, compra y restauración.

La integración mínima real es:

1. Producto creado y activo en Google Play.
2. Producto importado/configurado en RevenueCat.
3. Entitlement que representa el contenido desbloqueado.
4. Offering que muestra el producto.
5. SDK RevenueCat instalado en Android.
6. Pantalla o botón de compra funcional.
7. Resultado de compra que desbloquea el contenido.
8. Restauración de compras funcional.
9. Identidad consistente del jugador para que el entitlement no se pierda al cambiar de dispositivo.
10. Compra de prueba verificada en un track de prueba.

### Productos Android propuestos

| Campo | Propuesta |
|---|---|
| Tipo | One-time, consumable |
| Product IDs Google Play | sow_gems_500, sow_gems_1200, sow_gems_2600 |
| Nombre visible | Gem bundles |
| RevenueCat offering | default |
| Qué entrega | Gems para skins y contenido premium del juego |
| Concesión | El servidor entrega las gems; nunca se confía en el cliente |

El Product ID de Google Play debe elegirse con cuidado: un Product ID usado no se debe reutilizar en otra app aunque se borre.

### Orden correcto en RevenueCat y Google Play

1. En Google Play Console, abrir **Monetize → Products → In-app products**.
2. Crear los productos `sow_gems_500`, `sow_gems_1200` y `sow_gems_2600`, definir nombre y precio, y activarlos.
3. En RevenueCat, crear/seleccionar el proyecto y agregar la app Android con package com.shadowsofwar.
4. Conectar Google Play con RevenueCat siguiendo el flujo oficial de permisos/service account.
5. Importar el producto de Google Play.
6. Para gem bundles, RevenueCat registra la compra; el webhook entrega las gems al account ID estable. No usar un entitlement para representar consumibles.
7. Crear el offering `default` y agregar los tres productos.
8. Obtener la **public SDK key** de Android.
9. Configurar el webhook con `SOW_REVENUECAT_WEBHOOK_SECRET`.
10. No compartir la secret API key ni archivos JSON de Google Play.

### Particularidad del Android actual

El Android actual es un wrapper TWA que abre https://shadowsofwar.io/play/. Por eso, instalar el SDK por Gradle no hace automáticamente que el botón de compra web use RevenueCat.

Hay dos caminos:

- **Recomendado para Shipaton:** agregar la compra nativa de RevenueCat en Android y comunicar el resultado al juego web mediante un puente nativo. Es el camino más claro para Google Play Billing.
- **Web Billing:** requiere Stripe/Paddle/Stripe Billing y tiene que revisarse cuidadosamente contra las reglas de pagos de Google Play. No usarlo como atajo dentro de una app Play si el usuario puede comprar bienes digitales desde la app.

La implementación técnica debe incluir, como mínimo, el SDK Android, una acción de compra, identidad estable, comunicación del resultado con la sesión del jugador y el webhook de concesión. La public SDK key sí puede estar en la app; las claves privadas nunca.

## 4. Qué haces tú manualmente y qué puede hacer Codex

### Tareas manuales del propietario

- Confirmar si la cuenta de Play Console es Personal u Organization y cuándo fue creada.
- Revisar si Dashboard exige Closed testing.
- Crear/agregar testers y enviarles el opt-in link.
- Recopilar feedback real durante los 14 días, si aplica.
- Completar Store listing, Data safety, Content rating, App access, Ads y Privacy policy.
- Crear el producto de Google Play y activarlo.
- Crear/conectar el proyecto y la app en RevenueCat.
- Crear productos y offering; el webhook entrega las gems.
- Proporcionar al desarrollo solamente la public SDK key de Android.
- Crear acceso de prueba o promo code para que los jueces puedan probar premium.
- Subir el AAB a la track que corresponda y publicar Production.
- Confirmar que la ficha esté publicada y disponible en Estados Unidos.
- Grabar y publicar el video en YouTube o Vimeo.
- Crear y enviar la submission de Devpost.

### Trabajo técnico del proyecto

- Integrar RevenueCat en el wrapper Android.
- Implementar la acción de compra y el flujo de restore.
- Asociar RevenueCat al usuario autenticado del juego.
- Mostrar el contenido premium únicamente cuando el entitlement esté activo.
- Mantener la tienda universal: líderes por rotación/laurels; skins por gems cuando existan assets originales.
- Preparar el AAB con un versionCode superior al ya subido.
- Preparar release notes, instrucciones de prueba y el guion técnico del video.
- Ejecutar el pipeline oficial del repositorio si se modifican web/backend/relay.

## 5. Modelo universal de tienda

La tienda vive dentro del juego y no depende de CrazyGames, Poki ni de un portal específico:

1. El jugador abre **Store** desde el menú.
2. Los líderes de la rotación semanal se pueden usar gratis; los demás se desbloquean con laurels.
3. Las gems se compran con Google Play + RevenueCat en Android.
4. Las skins originales de SOW se compran con gems.
5. El servidor valida ownership, saldo y concesiones.

La rotación, los líderes, las skins originales y los gem bundles están implementados en el contrato de catálogo. El bridge nativo Android y el webhook están en código; falta configurar claves/productos y validarlos con una compra real.

CrazyGames/Poki son canales de distribución, no la arquitectura de monetización.

## 6. Estrategia para Best Game Award

### Posicionamiento

**Shadows of War turns every match into a war story.**

La idea que debe entender un juez en los primeros segundos: no es solamente un mapa con territorios; cada partida crea una historia de expansión, alianzas, traición y remontada.

### Loop que debe verse

1. Elegir un líder.
2. Reclamar territorio.
3. Administrar recursos.
4. Expandirse y enfrentarse a otros jugadores.
5. Formar una alianza o traicionar en el momento decisivo.
6. Sobrevivir o perder una guerra que se sienta memorable.

### Monetización

El juego base debe ser gratuito y jugable. Los líderes rotan gratis y los demás se desbloquean con laurels; las gems compran skins originales y contenido premium cuando esté disponible. El servidor valida cada concesión.

Después del primer match completado, mostrar una oferta breve con una vista clara de la personalización. No interrumpir el onboarding ni bloquear el primer momento divertido.

### Métricas que conviene registrar

- Installs.
- Usuarios que completan el primer match.
- Tiempo hasta el primer match.
- Retención D1 y D7.
- Paywall views.
- Purchases y conversion rate.
- Revenue reportado por RevenueCat.
- Concesiones de compra y reintentos idempotentes.
- Bugs/crashes y feedback de testers.
- Invitaciones o shares en Discord/redes.

No inventar métricas para Devpost. Si todavía no existen, decir “not yet measured” y explicar qué se está instrumentando.

## 6. Material para la submission de Devpost

La submission debe estar en inglés o incluir traducción al inglés.

### Inspiration — borrador

We wanted to make strategy games feel personal again. In Shadows of War, a match is not just a victory screen: it is a story of expansion, diplomacy, betrayal, and survival. The world map gives every decision a visible consequence, while different leaders create different ways to approach the same war.

### What it does — borrador

Shadows of War is a real-time multiplayer strategy game about conquering and defending territory on a living world map. Players choose a leader, build an economy, expand their borders, form alliances, and fight for control. Eight leaders rotate free each week, while the others can be unlocked with earned laurels; gems are reserved for skins and premium identity.

### How we built it — borrador para actualizar después de la integración

The game client runs through a web/WASM experience backed by a real-time game server. The Android release packages the mobile experience for Google Play. For monetization, the game uses a universal in-game store: leaders rotate for free, other leaders unlock with laurels, and RevenueCat with Google Play Billing sells gem bundles for original skins and premium content. The server validates every grant.

### Challenges we ran into — borrador

The hardest part was connecting a real-time strategy game with a mobile distribution and monetization flow without compromising the game loop. We had to keep the first match accessible, make purchase grants reliable across sessions, and make sure the Android wrapper behaved like a real mobile app rather than simply displaying a website.

### Accomplishments that we're proud of — borrador

We are proud of turning a persistent multiplayer world into a compact mobile experience while preserving the tension of territory control. We are also proud of building a universal store that works independently of any web portal and keeps purchase grants under server control.

### What we learned — borrador para completar con datos reales

We learned that a store launch is a product challenge, not just a packaging step. Testing with real players exposed gaps in onboarding, purchase recovery, and the clarity of the first-match experience. We are tracking those findings and using them to prioritize the smallest changes that make the game easier to understand and more memorable.

### What's next for Shadows of War — borrador

Next we want to improve the first-match experience, add more ways to share war stories, expand cosmetic identity options, and use player feedback to make alliances and diplomacy more meaningful. We also want to keep measuring whether the game is fun before optimizing how it earns money.

### Video obligatorio

- Menos de 2 minutos.
- Subido públicamente a YouTube o Vimeo.
- Grabado mostrando la app funcionando en un dispositivo real.
- Sin música o material con copyright sin permiso.
- Mostrar gameplay, no solamente mockups.
- Mostrar el producto y el flujo de compra/restauración sin exponer secretos.
- Incluir subtítulos o narración en inglés.

### Guion sugerido de 120 segundos

| Tiempo | Qué mostrar |
|---|---|
| 0–8 s | Una invasión, traición o remontada con el texto “Every match becomes a war story.” |
| 8–25 s | Selección de líder y mapa mundial |
| 25–65 s | Economía, expansión, combate y defensa |
| 65–90 s | Alianza, traición y resultado de la partida |
| 90–108 s | Store universal: líderes, gems y skins originales |
| 108–120 s | Compra/restauración RevenueCat, app publicada y CTA “Play your first war.” |

## 7. Registro de evidencias

Completar estos campos conforme se confirme cada punto:

    Play Console app URL:
    Play account type: Personal / Organization
    Play account creation date:
    Highest versionCode already uploaded:
    Current track: Internal testing; Closed testing setup in progress
    Closed test required: Yes
    Closed testing setup: 3 of 5 complete
    Closed test release sent to Google for review: No
    Closed test start:
    Closed test end:
    Number of testers opted in currently: 0
    Number of testers opted in continuously:
    Production access requested:
    Apply for production button: Disabled
    Production access approved:
    First public Production release date/time:
    Production release versionCode:
    United States available in Production: Yes / No

    RevenueCat project:
    RevenueCat Android app/package:
    Google Play connection:
    Product ID:
    Entitlement:
    Offering:
    Public Android SDK key stored in development:
    Test purchase completed:
    Restore purchase completed:
    Promo code/free trial for judges:

    Public demo video URL:
    Devpost draft URL:
    Devpost submission URL:
    Devpost submission date/time:

## 8. Checklist final

### Elegibilidad

- [ ] La primera publicación pública de Shadows of War no ocurrió antes del periodo de Shipaton.
- [ ] La cuenta y el equipo cumplen las reglas generales de Shipaton.
- [ ] La app está publicada en Google Play antes del deadline.
- [ ] La app se puede descargar desde Estados Unidos.
- [ ] La app está disponible sin restricción para jueces hasta el final del judging period.

### Google Play

- [ ] Package confirmado: com.shadowsofwar.
- [x] Android developer verification muestra com.shadowsofwar como Registered.
- [x] Play Console muestra 3 signing keys registradas para el package.
- [ ] Setup del Dashboard completo.
- [ ] Store listing completa en inglés.
- [ ] Privacy policy, Data safety, Content rating, App access y Ads declaration completos.
- [ ] Closed test de 14 días completado si Play lo exige.
- [ ] Production access aprobado si Play lo exige.
- [ ] AAB firmado y con versionCode superior.
- [ ] United States seleccionado en Production.
- [ ] Release publicada; no “Under review”.
- [ ] URL pública guardada.

### RevenueCat

- [ ] Producto de Google Play creado y activo.
- [ ] Google Play conectado a RevenueCat.
- [ ] Producto importado.
- [ ] Entitlement creado y conectado.
- [ ] Offering creado y marcado como default.
- [ ] SDK Android integrado.
- [ ] Compra funciona.
- [ ] Restore funciona.
- [ ] Entitlement persiste con el usuario correcto.
- [ ] No hay secretos en el repositorio, AAB, video ni Devpost.

### Devpost

- [ ] Descripción en inglés.
- [ ] Best Game: gameplay explicado.
- [ ] Best Game: dirección artística explicada.
- [ ] Best Game: monetización y por qué encaja con el género explicadas.
- [ ] Video público menor a 2 minutos.
- [ ] Video grabado en dispositivo real.
- [ ] URL pública de Google Play.
- [ ] Icono 1024×1024.
- [ ] Screenshot 1179×2556 sin marco de dispositivo.
- [ ] Promo code/free trial para judges.
- [ ] Submission enviada antes del 30 de septiembre de 2026 a las 11:45 p. m. PDT.

## 9. Siguiente acción concreta

La siguiente acción manual, y la única por ahora, es abrir la parte de **Closed testing** que dice **Preview and confirm the release** y completar **Send the release to Google for review**. No buscar todavía **Apply for production**: permanecerá deshabilitado hasta que el closed test esté publicado, haya 12 testers opt-in y hayan pasado 14 días.

Después de enviar la release, anotar estos datos:

    Tipo de cuenta: Personal / Organization
    Fecha de creación de la cuenta:
    ¿La release de Closed testing fue enviada a revisión?: Sí / No
    ¿La release de Closed testing está publicada?: Sí / No
    ¿Cuántos testers aparecen como opted-in?:
    ¿Aparece “Apply for production”?: Sí / No

Con esos datos se decide inmediatamente si la ruta es **Production directa** o **Closed testing → 14 días → Apply for production → Production**.

## 10. Fuentes oficiales

- [RevenueCat Shipaton 2026 — Official Rules](https://revenuecat-shipaton-2026.devpost.com/rules)
- [Shipaton FAQ](https://shipaton.com/faq)
- [Shipaton Best Game Award](https://shipaton.com/categories/best-game-award)
- [Google Play — App testing requirements for new personal developer accounts](https://support.google.com/googleplay/android-developer/answer/14151465?hl=en)
- [Google Play — Set up an open, closed, or internal test](https://support.google.com/googleplay/android-developer/answer/9845334?hl=en)
- [Google Play — Prepare and roll out a release](https://support.google.com/googleplay/android-developer/answer/9859348?hl=en)
- [Google Play — Payments policy / billing](https://support.google.com/googleplay/android-developer/answer/10281818?hl=en)
- [RevenueCat — Android SDK installation](https://www.revenuecat.com/docs/getting-started/installation/android)
- [RevenueCat — Google Play product setup](https://www.revenuecat.com/docs/getting-started/entitlements/android-products)
- [RevenueCat — Products, entitlements and offerings](https://www.revenuecat.com/docs/projects/configuring-products)
