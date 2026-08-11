# RPT-033 — Anclaje del extremo de la cadena

**Tema:** Que cortar la cola de la evidencia deje rastro
**Nº de reporte:** 033
**Fecha:** 6 de agosto de 2026
**Área designada:** Evidencia
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-57

- **Depende de:** RPT-029 §2.1 (el hueco, escrito como tal), RPT-011 (`Centinela`, mismo mecanismo)
- **Cierra:** PA-57

---

## 1. El hueco estaba escrito y probado como tal

RPT-029 §2.1 no lo escondió:

> Alterar el **último** asiento sin cambiar longitudes produce un registro que se reconstruye y verifica consigo mismo. Lo que cambia es su extremo, así que **sólo lo detecta quien conserve el extremo anterior** — y hoy nadie lo conserva.

La prueba de entonces afirmaba exactamente eso: que la cadena sigue siendo coherente y que el extremo difiere. Ahora hay quien lo conserva.

## 2. El ancla vive fuera del registro, y tiene la misma limitación que el centinela

Un fichero aparte con el número del último asiento y su resumen propio. Si viviera dentro, alterarlo sería la misma operación que alterar lo que ancla, y no comprobaría nada.

Y hereda la limitación que `Centinela` ya documenta para la secuencia del inventario, que conviene repetir en lugar de dejarla en otro reporte: **si el ancla vive en el mismo almacén que el atacante controla, puede actualizar los dos de forma coherente.** Lo que se consigue no es impedir la manipulación. Es que **no sea silenciosa** — y eso es lo mismo que se consiguió en PA-27, con las mismas palabras y por la misma razón.

## 3. Tres desenlaces, y el del medio es el que evita la falsa alarma

| Cotejo | Qué ocurrió |
|---|---|
| `Conforme` | el prefijo anclado está intacto y el registro no lo excede |
| `SinAnclar` | hay asientos posteriores al ancla y el prefijo cuadra |
| `Truncado` | el registro es más corto de lo que el ancla cubre |
| `Alterado` | el asiento anclado está y su resumen no es el que el ancla dice |

`SinAnclar` es lo que deja un corte de energía entre las dos escrituras. Colapsarlo en violación haría que **cada apagón en el momento justo pareciera un ataque**, que es la fatiga contra la que lleva media sesión escribiéndose.

## 4. El registro se escribe antes que el ancla, y el orden decide qué falsa alarma se produce

- **Ancla primero**: quedaría cubriendo asientos que no están en disco → se lee como `Truncado`, o sea «alguien cortó la evidencia». Respuesta a incidente por un corte de luz.
- **Registro primero**: quedan asientos que el ancla no cubre → `SinAnclar`, que es un estado propio y no una acusación.

**La evidencia real pesa más que su cobertura.** Una falsa alarma de manipulación cuesta más que una cola sin anclar.

## 5. Dos ausencias que no significan lo mismo

**Ancla ausente con registro vacío** es el primer arranque. No hay extremo que anclar, y fabricarlo con el resumen génesis haría indistinguible «vacío» de «con un asiento borrado».

**Ancla ausente con asientos dentro** es manipulación. Es justo lo que haría quien pretende cortar la cola: primero desactivar la comprobación.

Y un **ancla corrupta no se degrada a ausente**, por el mismo argumento que el centinela corrupto de RPT-017: corromper treinta bytes sería la vía para desactivar la comprobación por la puerta de atrás.

## 6. El fallo que encontré cableándolo

`apartar` movía el registro dañado y **dejaba el ancla atrás**. El efecto: el arranque siguiente encontraría un ancla que cubre asientos que ya no están en su sitio, lo leería como truncamiento, y lo haría **en cada arranque posterior, para siempre** — una acusación permanente por un incidente ya archivado.

Ahora se aparta con él, y se conserva en lugar de borrarse porque también es evidencia: dice cuál era el extremo antes de que alguien tocara nada.

Apareció al preguntarme qué pasa en el segundo arranque después de una violación, que es una pregunta que ninguna de las pruebas de RPT-030 hacía.

## 7. Lo que sigue sin resolverse

1. **El ancla es rebobinable por quien controle el almacén** (§2). La protección completa exige un contador monótono en TPM o elemento seguro, que no está disponible en todos los destinos. Es la misma frontera que PA-28 para el inventario, y se anota junto a él.
2. **No se firma.** Un ancla firmada por la clave del cliente resistiría a quien tenga escritura pero no la clave. Es posible con lo que ya hay y añade una verificación criptográfica a cada arranque; se registra como **PA-64** en lugar de hacerlo por impulso.
3. **PA-59 sigue abierto**: no hay retención ni rotación.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-64** | **Firmar el ancla con la clave del cliente.** Resistiría a quien tenga escritura en el almacén pero no la clave | Detección de manipulación por quien controla el disco |
| PA-57 | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 33 — Anclaje del extremo de la cadena · PremosCorp · 6 de agosto de 2026*
