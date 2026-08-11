# RPT-042 — La paridad pasa de validar esquemas a validar usos

**Tema:** Que un registro declarado en el manifiesto tenga que aparecer en la firma que lo sirve
**Nº de reporte:** 042
**Fecha:** 8 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-75 en la frontera TypeScript; abre PA-76

- **Depende de:** RPT-035 (protocolo), RPT-041 (respuesta de alertas)
- **Cierra:** PA-75 (lado TypeScript)
- **Abre:** PA-76

---

## 1. Lo que la barrera anterior no miraba

PA-20 comprueba que los **campos** de cada registro coincidan entre
`contrato-ipc.toml` y el código, en Rust y en TypeScript, incluido el orden. Es
una barrera buena y ha cazado divergencias reales esta misma semana.

Lo que no hacía: comprobar que alguien **use** el registro declarado.

El caso: `RespuestaAlertas` se añadió al manifiesto, a `mensajes.rs` y a
`puente.ts` con sus dos campos. Las dos pruebas de paridad pasaron. Y la firma
del contrato seguía diciendo `Promise<readonly SucesoAlerta[]>`.

## 2. Y había un segundo hueco, peor

Al construir la barrera apareció que **el manifiesto seguía declarando la forma
vieja**:

```toml
canal = "consultar-alertas"
direccion = "respuesta"
forma = "lista<SucesoAlerta>"
```

Se cambió el struct de Rust, el tipo de TypeScript, los campos de ambos lados y
la firma del puente — y no la declaración de la respuesta del canal, que es la
fuente de verdad. La prueba `el manifiesto declara la forma de cada canal` pasó
porque comprueba que el canal **aparezca**, no que su forma corresponda con nada.

Dos piezas sin cablear en el mismo cambio, una de ellas en el documento que
gobierna a las demás.

## 3. La barrera es derivada, no escrita a mano

- Del manifiesto se extrae `canal → forma de respuesta`, leyendo los bloques
  `[[mensaje]]` con `direccion = "respuesta"`.
- De `puente.ts` se extrae `método → tipo devuelto`, leyendo el fuente real de la
  interfaz `PuenteEje`.
- El canal se traduce a método (`consultar-alertas` → `consultarAlertas`) y la
  forma a tipo (`lista<X>` → `readonly X[]`).
- Cada canal permitido debe cuadrar.

**No hay tabla que mantener.** Si mañana se añade un canal, la prueba exige su
método y su tipo sin que nadie la actualice — y una tabla que hay que mantener
habría sido el mismo problema con otro nombre.

Con esto puesto, el fallo de hoy habría saltado **dos veces**: al cambiar la firma
sin el manifiesto y al cambiar el manifiesto sin la firma.

## 4. Dos decisiones de implementación

**Se ancla en `import.meta.url`, no en `process.cwd()`.** El directorio de trabajo
depende de desde dónde se invoque npm, y esa fragilidad ya costó una verificación
de TypeScript que no llegó a ejecutarse. La ubicación del módulo no cambia con
quién lo lance.

**Si no se puede leer `puente.ts`, se lanza con la ruta completa.** Devolver una
cadena vacía habría producido un mapa vacío y —con un descuido más— una prueba en
verde que no comprueba nada. Es la trampa de PA-73 dentro de la herramienta hecha
para evitarla.

## 5. La barrera nunca se ha puesto en rojo, y eso importa

Falló dos veces mientras la escribía: por tipos y por una ruta mal contada. Pero
**la divergencia real ya estaba corregida cuando la prueba existió**, así que
jamás la vio.

Este proyecto tiene precedente propio: `probar-frontera.mjs` existe porque un
guardián que nunca ha detectado nada es un guardián sin probar.

Se añade por eso una prueba que ejercita la traducción y la comparación con datos
fabricados, incluido el par concreto que se nos escapó hoy —
`lista<SucesoAlerta>` frente a `RespuestaAlertas`. No sustituye a un ensayo
negativo completo, pero sí garantiza que la comparación distingue un array de su
envoltorio, que es donde el fallo vivía.

## 6. PA-76 — el mismo agujero en el otro extremo del cable

`servicio.rs` serializa lo que quiera y nadie comprueba que sea la forma que el
manifiesto declara para ese canal. La barrera de este reporte cubre TypeScript
porque es donde el fallo ocurrió y donde vive el contrato de la interfaz; **el
lado Rust queda igual de descubierto que estaba esta mañana**.

Se registra como PA-76 en lugar de dejarlo en esta prosa, por el motivo de
siempre: media hora con identificador se hace.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| ~~PA-75~~ | — | ✅ **Cerrado en la frontera TypeScript** |
| **PA-76** | **La paridad de uso no cubre Rust.** `servicio.rs` puede serializar una forma distinta de la declarada sin que nada lo note | Que el agente responda algo que el manifiesto no describe |

---

*Reporte Nº 42 — La paridad pasa de validar esquemas a validar usos · PremosCorp · 8 de agosto de 2026*
