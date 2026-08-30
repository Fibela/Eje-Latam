# RPT-087 — El almacén que sabía contar

**Tema:** PA-138a. Enumerar lo observado sin tocar el contrato, y el tercer estado que apareció al hacerlo
**Nº de reporte:** 087
**Fecha:** 28 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Cerrado.** `guardian-cc` 167 → 172 pruebas

- **Depende de:** RPT-081 §5 y §6 (donde se descubrió el bloqueo), RPT-018 §6 (la partición volátil/pegajoso), RPT-020 (el agente no contiene)
- **Aborda:** PA-138a (cerrado). Parte PA-138. Desbloquea PA-139

---

## 1. La auditoría antes del diseño

Hoy el tablero mintió dos veces (RPT-084 §7.3, RPT-085 §1), así que PA-138 y PA-139 se
releyeron contra el código antes de escribir nada. Las tres declaraciones —contrato,
manejador y tablero— decían lo mismo entre sí y lo mismo que el código:

```rust
pub fn volatiles(&self) -> usize { self.volatil.len() }
pub fn pegajosos(&self) -> usize { self.pegajoso.len() }
```

Sólo cuentan. **No son fantasmas.**

## 2. Pero las dos filas se esperaban la una a la otra

PA-138 necesita a PA-139: sin postura para «no se sabe», el productor mentiría sobre cada
equipo visto sin marcado. Y PA-139 decía *«no antes de PA-138»*, para hacerse una vez y
con evidencia.

Se rompe partiendo PA-138, como se partió PA-14:

- **PA-138a** — enumerar. No toca el contrato. **Produce la evidencia.**
- **PA-139** — cuarta postura y taxonomía, una vez, con esa evidencia delante.
- **PA-138b** — el productor, ya con dónde leer y qué decir.

## 3. Tres propuestas del equipo, dos aceptadas con matiz y una rechazada

**«El pegajoso son las marcas de contención» — falso.** El agente no contiene nada
(RPT-020). El pegajoso es resistencia a la expulsión para equipos vistos en segmento
crítico. Derivar contención de ahí sería inventar dato justo en el campo que este punto se
abrió para proteger. Hay una prueba dedicada a que nadie lo lea así dentro de un año.

La conclusión sí se sostiene —el volátil es el origen— pero por otra razón: **el pegajoso
guarda sólo direcciones**. De un equipo que estuviera únicamente ahí no se podría decir ni
qué protocolos habló ni en qué segmento se le vio.

**`identificador` = MAC serializada — de acuerdo, con una consecuencia.** Entonces
`identificador` y `direccionEnlace` son la misma cadena, y el contrato lleva un campo que
finge ser una abstracción. Se retira en PA-139, que ya abre esa superficie: un cambio en
vez de dos.

**La taxonomía propuesta — rechazada.** Se propuso `SoporteVital | SeguridadFuncional |
Corporativo | NoClasificado`. `ClaseExcluida` es `SoporteVital | SeguridadFuncional |
CaminoDeGestion`: `Corporativo` es un `PerfilSegmento`, no una clase, y `CaminoDeGestion`
desaparecía. Es el patrón de identificador inventado, parado por cuarta vez.

Y hay algo por debajo que decide PA-139, no esto: la clase puede venir del **marcado
firmado** o de `Protocolo::clase_sugerida`, que es una **inferencia** —tanto que existe
`un_marcado_no_critico_contradicho_por_la_huella_es_ambiguo`—. Colapsar las dos
procedencias en un enumerado plano presenta una sospecha como una declaración.

## 4. Lo entregado

`VistaNodo` lleva **sólo lo que el almacén sabe**: dirección, protocolos observados,
declaración de segmento, reloj de la última observación, y si es pegajoso. Ni `clase` ni
`postura`.

`inventario()` ordena por dirección. `HashMap` no promete orden, y un inventario que se
reordena solo entre dos consultas hace parpadear la pantalla del operador y arruina
comparar una vuelta con la siguiente.

### 4.1 El tercer estado, que no estaba en ninguna fila

Un pegajoso **ya expulsado del volátil** no cabe en la lista: eso lo afirmaría presente. Ni
puede desaparecer: eso lo daría por inexistente. De él se sabe que estuvo en segmento
crítico y **no se sabe si sigue**.

Sale por `pegajosos_no_observados()`, aparte, para que quien componga la respuesta pueda
decir las dos cosas por separado. Es RPT-006 §4 apareciendo por sí solo al mirar el dato
real — no se buscó, apareció.

## 5. Lo que queda para PA-138b

| Campo | Fuente hoy |
|---|---|
| `direccionEnlace` | `VistaNodo::direccion`, honesta |
| `identificador` | Sería la misma cadena. **Sobra uno de los dos** |
| `clase` | Dos procedencias que no valen lo mismo. **PA-139** |
| `postura` | Sin valor para «no se sabe». **PA-139** |

El canal sigue `servido = false`, y su motivo en `contrato-ipc.toml` se corrigió: ya no
dice que el almacén no enumera, porque desde hoy enumera.

## 6. Puntos abiertos

| ID | Punto |
|---|---|
| PA-138a | **Cerrado.** §4 |
| PA-138b | Abierto, bloqueado por PA-139. §5 |
| PA-139 | **Desbloqueado.** Ya tiene la evidencia, y dos preguntas más: procedencia de `clase`, y si `identificador` sobra |

---

*Reporte Nº 87 — El almacén que sabía contar · PremosCorp · 28 de agosto de 2026*
