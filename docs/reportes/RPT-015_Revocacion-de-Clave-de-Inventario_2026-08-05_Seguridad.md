# RPT-015 — Revocación de la Clave de Inventario (Diseño)

**Tema:** Invalidación y rotación de la clave del administrador del cliente
**Nº de reporte:** 015
**Fecha:** 5 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** Canónico — diseño ratificado e implementado

- **Depende de:** RPT-011 (custodia y `DominioClave`), RPT-012 (frescura y centinela), RPT-013 (formato en disco)
- **Cierra:** PA-31, PA-32, PA-33
- **Enmienda:** RPT-012, cuya regla «el centinela nunca retrocede» resultó ser explotable (§6.1)

---

## 1. Por qué es un reporte de diseño

Las cuatro decisiones de este documento son de **custodia**: quién guarda qué llave, dónde vive, y quién puede usarla bajo presión. El código que las implemente es la parte fácil y sale sola una vez acordadas. Escribirlo antes de acordarlas produciría un mecanismo que hay que rehacer.

## 2. El agujero

La autoridad del inventario descansa por completo en una clave. Si se filtra, el atacante emite secuencia N+1 con el contenido que quiera. **La secuencia monótona de RPT-012 no ayuda**: quien tiene la clave firma secuencias crecientes igual que el legítimo.

Puede marcar un equipo comprometido como soporte vital para volverlo incontenible, o desmarcar uno crítico para que se contenga.

### 2.1 Qué tan grande es, exactamente

Menos de lo que parece, y conviene tenerlo medido antes de diseñar la respuesta.

| Lo que intenta | Qué lo frena hoy |
|---|---|
| Volver un equipo incontenible | Nada. **Funciona.** |
| Provocar contención indebida marcando de menos | La inferencia de RPT-009: si la huella sugiere criticidad, sale `ConflictoEntreFuentes` → `Ambiguo` → humano |
| Lo anterior en perfil OT | El perfil OT nunca ejecuta contención automática (RPT-008 §5) |

El hueco real es **un equipo crítico, en segmento corporativo, que la huella no reconozca**. Serio, pero acotado — y acotado por asimetrías que no se diseñaron para esto.

La consecuencia práctica: **la revocación es importante, no urgente**. Eso permite diseñarla bien en lugar de deprisa.

## 3. Decisión 1 — la revocación no puede ser total

Es el punto que más fácil se hace mal.

Un certificado que diga «la clave K queda invalidada» invalida **todo lo que K firmó**, incluidos los inventarios legítimos de antes del compromiso. El agente se quedaría sin inventario. Y sin inventario no hay marcados, y sin marcados los equipos críticos dejan de estar protegidos — la misma dirección peligrosa que la escritura atómica de RPT-014 §1 existe para evitar.

Revocar de golpe sería **provocarnos la pérdida de protección que el atacante buscaba**.

El certificado lleva una **secuencia de corte**:

> Los inventarios firmados por K con secuencia **mayor que S** dejan de valer. Los de secuencia ≤ S siguen siendo válidos.

El cliente sabe cuál fue su última emisión buena. Todo lo que el atacante firmó después cae; lo anterior sobrevive.

### 3.1 El caso incómodo

Si el atacante ya empujó N+1 y el agente lo aceptó, el centinela avanzó a N+1 y el inventario bueno (N) ya no está en disco. La revocación con corte en N invalida lo que hay y no queda nada.

Por eso el paquete de revocación **incluye un inventario fresco firmado por la clave nueva**. No es un adorno: sin él, revocar deja al agente sin marcados.

## 4. Decisión 2 — quién firma la revocación

No puede ser:

- **la clave operativa**, porque el atacante la tiene;
- **la de PremosCorp**, porque `DominioClave` existe desde RPT-011 §4 precisamente para que el proveedor no pueda declarar qué equipos del cliente son críticos. Reutilizarla aquí «porque es la que ya está desplegada» destruiría esa frontera por comodidad.

Hace falta un tercer dominio: `DominioClave::ClienteRecuperacion`. Custodia del cliente, **fuera de línea**, y usada sólo para esto.

### 4.1 La regresión infinita, y dónde se corta

La clave de recuperación también puede comprometerse. Y su revocación necesitaría una cuarta clave, y así.

La cadena se corta por decisión, no por criptografía: **existe una raíz que se confía porque está fuera de línea y casi nunca se usa**. Si esa cae, el remedio es reaprovisionar el agente presencialmente. Escribirlo evita que alguien intente resolverlo con más niveles.

## 5. Decisión 3 — el estado de revocación es más fácil que la frescura

Aquí hay una asimetría a favor, y merece aprovecharse.

| | Frescura (PA-28) | Revocación |
|---|---|---|
| Qué guarda | contador monótono | conjunto de claves revocadas |
| Cómo cambia | sube | **sólo crece** |
| Si se pierde | el atacante revierte y no se nota | se olvida una revocación |
| Gravedad de perderlo | degrada por debajo del estado actual | degrada **al** estado actual |
| ¿Se puede reponer? | no: la secuencia buena ya pasó | **sí: se vuelve a presentar el certificado** |

Un olvido de revocación deja el sistema como está hoy, que ya sabemos vivir. Y el cliente conserva el certificado, así que reponerlo es volver a presentarlo.

**Consecuencia práctica:** la revocación **no** necesita el ancla de hardware de PA-28. Un fichero de revocaciones en el almacén local, que sólo se añade y nunca se vacía, es suficiente. Que PA-28 siga abierto no bloquea esto.

## 6. Decisión 4 — la secuencia es global, no por clave

Si la clave nueva empezara su numeración en cero, rotar la clave sería un camino de reversión: el atacante presenta un inventario viejo con secuencia baja firmado por la clave nueva, y el centinela lo acepta.

**La secuencia pertenece al inventario, no a la clave.** Tras rotar, la numeración continúa donde estaba.

### 6.1 Enmienda — el centinela **sí** retrocede ante una revocación

La regla «el centinela nunca retrocede» de RPT-012 abre un bloqueo permanente, y lo descubrí al plantear la implementación de este reporte, no al escribirlo.

Un atacante con la clave operativa emite un inventario con secuencia `u64::MAX`. El agente lo acepta —la firma es válida y la secuencia sube— y el centinela queda en el máximo. **A partir de ahí ningún inventario legítimo puede tener secuencia mayor.** El cliente queda bloqueado para siempre, y la revocación con corte no lo arregla: el corte invalida lo firmado por la clave vieja, pero el centinela sigue arriba y rechaza también al sucesor.

El resultado es **peor que el compromiso**: el atacante pierde la clave al revocarse, y a cambio deja el inventario congelado de forma irreversible. Un ataque de un solo mensaje.

Corrección: **un certificado de revocación válido reinicia el centinela a `hasta_secuencia`.**

Es la única operación autorizada a bajar la marca de agua, y es segura porque exige la clave de recuperación fuera de línea — precisamente la que el atacante no tiene. Sin ella, el «nunca retrocede» se convierte en el arma.

La formulación correcta es entonces:

> El centinela sólo retrocede por un certificado firmado con la clave de recuperación, y sólo hasta la secuencia de corte que ese certificado declara.

Nótese que esto no reabre la reversión de RPT-012: un atacante que quisiera usar el mecanismo necesitaría la clave de recuperación, y con ella no le haría falta.

## 7. La ventana, y por qué no se cierra con diseño

Entre el compromiso y la llegada de la revocación al agente, el atacante manda. En un producto Local-First **no hay garantía de red**: un agente en una planta aislada puede tardar semanas.

Ninguna decisión de este reporte acorta esa ventana. Lo que la hace tolerable es lo del §2.1 — que un compromiso de clave sirva para poco. Conviene tenerlo presente al valorar cuánto invertir en la mecánica de entrega frente a en mantener esa acotación.

## 8. Forma propuesta

```
CertificadoRevocacion
  identificador_revocado   resumen de la clave publica comprometida
  hasta_secuencia          u64, corte del §3
  identificador_sucesora   resumen de la clave que la sustituye
  emitido_en               u64
  firma                    hibrida, por la clave de recuperacion
```

Y un registro local que sólo crece. La verificación de RPT-012 gana un eslabón, el **sexto**:

> La clave que firma no está revocada para esta secuencia.

Se comprueba junto al dominio de clave, **antes** que la frescura y la firma, por el mismo motivo que allí: una clave revocada no debe llegar a gastar ciclos criptográficos.

## 8.1 Custodia ratificada, con dos salvedades

PA-32 se resuelve con reparto de secreto 2-de-3: fragmento en poder del responsable de seguridad del cliente, fragmento en operaciones de TI, y copia en custodia bancaria. La clave se genera en máquina aislada durante el aprovisionamiento y sólo se reconstruye fuera de línea para firmar.

Dos observaciones sobre el esquema, ninguna bloqueante:

**Dos de los tres fragmentos viven dentro de la misma organización.** Quien haya comprometido al cliente lo bastante para robar la clave operativa puede estar en posición de alcanzar los fragmentos A y B, que son los dos internos. El de custodia bancaria es el único fuera. El umbral efectivo frente a un compromiso profundo o a un interno es menor que 2-de-3 nominal. Si se quiere el umbral real, conviene que dos de los tres custodios sean externos a la organización, o subir a 3-de-5.

**El certificado prefirmado de emergencia no es implementable.** Un certificado debe nombrar `hasta_secuencia` y `identificador_sucesora`; ninguno de los dos se conoce antes del incidente —la secuencia de corte depende de cuándo ocurrió y la clave sucesora todavía no existe—. Un certificado prefirmado sólo podría revocar sin sustituir, que es exactamente la revocación total que el §3 descarta. Se retira la alternativa.

## 9. Lo que este diseño **no** resuelve

1. **Cómo llega el certificado al agente.** Fichero copiado a mano, canal de gestión, medio extraíble. Es PA-31 y es operativo.
2. **Dónde vive la clave de recuperación.** Caja fuerte, HSM, reparto entre custodios. Es PA-32 y es la decisión más delicada, porque una clave de recuperación que nadie encuentra cuando hace falta equivale a no tenerla.
3. **La detección del compromiso.** Todo esto empieza cuando alguien se entera. Nada en el producto lo detecta.
4. **La rotación programada.** Este diseño cubre la rotación **por compromiso**. La rotación periódica preventiva usa la misma mecánica, pero su cadencia es otra decisión.

El punto 3 es el que más pesa: el mecanismo completo se activa por un hecho que ocurre fuera del sistema.

## 10. Verificación

`crates/guardian-cc` pasa de 77 a **88 pruebas**; el workspace, de 188 a **199**. Clippy con `-D warnings` limpio.

La central es `el_bloqueo_por_secuencia_maxima_se_recupera_con_el_certificado`, que recorre los cuatro pasos del §6.1 y **comprueba que el bloqueo es real antes de intentar salir de él**. Sin ese paso intermedio, la prueba pasaría igual con un mecanismo que no bloquease nunca.

Tres decisiones aparecieron al implementar, no al diseñar:

**El registro conserva el corte más bajo.** Si la misma clave se revoca dos veces con cortes distintos, gana el menor. Un corte posterior más alto aflojaría una revocación existente, y una revocación que se puede aflojar no es una revocación.

**Un certificado que se declara a sí mismo como sucesora se rechaza.** Dejaría al cliente sin autoridad ninguna sobre su inventario.

**La prueba de recuperación necesitó dos semillas de clave.** La primera versión usaba la misma clave para el sucesor, con lo que habría fallado por `ClaveRevocada` y el diagnóstico habría apuntado al código en lugar de a la prueba. Ahora se comprueba además que **la clave vieja sigue bloqueada tras la rotación**, que es la mitad que se olvida.

### 10.1 Lo que «inexpugnable» no significa

El mecanismo es sólido **frente a los ataques modelados**. No es lo mismo que inexpugnable, y la diferencia importa porque quien lea el reporte dentro de un año decidirá en qué invertir a partir de ella:

- La clave de recuperación puede robarse. El §4.1 corta la regresión por decisión, no por criptografía.
- La ventana entre compromiso y llegada del certificado sigue siendo **ilimitada** en un agente sin red (§7).
- **Nada detecta el compromiso** (§9.3). Todo el mecanismo se activa por un hecho que ocurre fuera del sistema.
- El registro de revocaciones vive en el almacén local. Perderlo devuelve al estado previo —eso es el §5— pero un atacante que controle el almacén puede borrarlo y ganar tiempo hasta que alguien vuelva a presentar el certificado.

## 11. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| ~~**PA-31**~~ | ~~Entrega del certificado de revocación~~ | ✅ Viaja con el paquete de inventario; el agente no lo busca en ningún servidor |
| ~~**PA-32**~~ | ~~Custodia de la clave de recuperación~~ | ✅ 2-de-3 fuera de línea, con la salvedad de localidad del §8.1 |
| ~~**PA-33**~~ | ~~Reinicio del centinela ante certificado válido~~ | ✅ Implementado y probado (§10) |
| **PA-34** | **Persistencia del registro de revocaciones.** Hoy `RegistroRevocaciones` vive en memoria y nadie lo escribe a disco. Sin eso, cada arranque olvida las revocaciones | Uso real de la revocación |

PA-34 es consecuencia directa de implementar: el diseño hablaba de «un fichero de revocaciones en el almacén local» y lo que existe es una estructura en memoria. `disco.rs` ya tiene la escritura atómica que hace falta, pero nadie la usa para esto.

---

*Reporte Nº 15 — Revocación de la Clave de Inventario (Diseño) · PremosCorp · 5 de agosto de 2026*
