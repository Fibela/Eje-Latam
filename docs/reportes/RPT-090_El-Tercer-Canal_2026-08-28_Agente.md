# RPT-090 — El tercer canal

**Tema:** PA-138b. `obtener-inventario` pasa a servirse, y el traductor que el compilador vigila
**Nº de reporte:** 090
**Fecha:** 28 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Cerrado por observación en máquina real.** md5 `8b547ec…` comprobado en los dos lados

- **Depende de:** RPT-087 (dónde leer), RPT-089 (qué decir), RPT-084 (atender a mitad de vuelta)
- **Aborda:** PA-138b (cerrado)

---

## 1. Lo entregado

Tercero de los seis canales cableado. `obtener-inventario` responde `lista<NodoInventario>`
con lo que el agente observa, y el contrato pasa a `servido = true`.

| Pieza | Dónde |
|---|---|
| `clase_en_el_cable` | `eje-agente/src/inventario.rs`, `match` de once brazos **sin comodín** |
| `segmento_en_el_cable` | Igual, tres brazos. Mismo patrón que `perfil_en_el_cable` (RPT-081) |
| `clasificar_nodo` | `ciclo.rs`, las tres fuentes por dispositivo |
| Composición | Una vez por vuelta, en `Resultado`, como las condiciones |
| Manejador | `servicio.rs`, con `Option<&[NodoInventario]>` y rechazo en la primera vuelta |

## 2. Dos decisiones que no eran mecánicas

**El inventario sale del volátil entero, no de lo visto en esta vuelta.** El bucle del
ciclo recorre `vistos` —lo que habló en los últimos quinientos milisegundos—. Reutilizarlo
habría dado una lista que se vacía sola cuando un equipo calla medio segundo. El
inventario es lo que hay en la red.

**Si la fuente declarativa falla, el nodo sale como `ambiguaEvidenciaNoVerificable`**, no
como si no tuviera marcado. RPT-010: una firma inválida o una inclusión no probada indican
manipulación del inventario. Leerlas como ausencia borraría la acusación justo en el camino
del sensor a la pantalla.

## 3. El compilador cazó lo que ninguna prueba habría cazado

Al primer intento de compilar el traductor:

```
error[E0004]: non-exhaustive patterns:
  `Clasificacion::Ambiguo { motivo: MotivoAmbiguedad::EvidenciaNoVerificable }` not covered
```

**Un quinto motivo de ambigüedad que no habíamos visto**, y precisamente el que significa
manipulación. Con un brazo `_ =>` habría compilado a la primera y ese caso habría salido al
cable como el valor más parecido, en silencio.

El `match` sin comodín se ganó el sueldo en su primer día. Es el argumento entero a favor de
escribir la traducción como una correa rígida en vez de como un mapeo cómodo.

## 4. Dos barreras que cambiaron de sentido solas — y una que estaba anclada

Al poner `servido = true`, dos pruebas existentes se pusieron rojas. Las dos tenían razón.

**`cada_manejador_responde_con_la_forma_que_el_manifiesto_declara`** asumía que toda
respuesta es un objeto. Nunca se había servido un canal que devolviera una lista. Ahora
desenvuelve `lista<X>`, compara el primer elemento contra los campos de `X`, y **falla si
la lista viene vacía** — una lista vacía no comprueba ninguna forma.

**`un_canal_sin_manejador_se_rechaza_con_motivo_y_no_con_lista_vacia`** usaba
`obtener-inventario` **como ejemplo** de canal sin manejador. Era un índice escrito a mano
disfrazado de prueba: el día que ese canal se cableara, pasaría a afirmar algo falso.

Ahora recorre todos los que el contrato declara `servido = false`, y lleva una aserción
anti-vacuidad que dice en voz alta qué hacer el día que no quede ninguno: **retirarla a
propósito, no dejarla comprobando nada.**

## 5. Cuatro errores de edición, ninguno de diseño

Se anotan porque tienen causa común y volverán:

| Qué | Cómo se cazó |
|---|---|
| El brazo del `match` sin `#[cfg(test)]` que sí tenía la variante | `E0220`, con un mensaje que apunta lejos de la causa |
| Un lector de fuentes que se comía su propio módulo de pruebas | Su propio fallo |
| `clasificar_nodo` insertado **entre el doc de `vuelta` y su firma** | `missing_docs` con `-D warnings` |
| El `Vec` movido antes de prestarlo | El préstamo |

La causa es la misma en las cuatro: **editar por sustitución de texto en lugar de sobre la
estructura.** Ninguna la habría visto una revisión a ojo; las cuatro las vio el compilador
en segundos.

### 5.1 Y una prueba que sobraba

Escribí una que leía su propio fichero para comprobar que el reloj interno del almacén no
viaja al cable. Falló por estar mal delimitada, y el arreglo correcto no era delimitarla
mejor: **era no haberla escrito.** `NodoInventario` tiene cinco campos y ninguno es ése; la
cadena `contrato ↔ CAMPOS_* ↔ struct` ya lo garantiza con desestructuración exhaustiva.

Los lectores de texto valen para lo que el compilador no ve —`vis04.js`, `docs/Comandos.md`,
el manifiesto— y no para lo que sí ve. Para eso está el tipo.

## 6. La corrida, en la VM

```
OK  obtener-inventario  (393 ms, 178 bytes)
[{"direccionEnlace":"00:00:00:00:00:00",
  "clase":"ambiguaSegmentoPuedeAlojarCriticos",
  "declaracionSegmento":"noDeclarado",
  "vistoEnSegmentoCritico":true,
  "protocolosObservados":[]}]
```

Cuatro canales con datos, dos con rechazo por contrato, los seis por debajo de 430 ms.

**La predicción acertó entera, incluida la clase.** Ninguna `declarada*`: esa MAC no está
en ningún marcado firmado, y el agente dice «no hay marcado y el segmento no está
declarado» en lugar de inventarse una clasificación. Es el punto de todo el bloque, visto
en una máquina.

### 6.1 Dos cosas que la corrida enseñó y no había previsto

**`declaracionSegmento: noDeclarado` con `vistoEnSegmentoCritico: true`** parece una
contradicción y no lo es: `NoDeclarado` se trata como `PuedeAlojarCriticos` (RPT-018 §6),
así que la marca se anota. El cable lo dice por separado y **deja ver la regla en lugar de
esconderla** — con un campo colapsado, el operador vería una sola cosa y no sabría por qué.

**MAC toda a ceros y `protocolosObservados: []`** es lo que produce `lo`. Nada que
arreglar: es el dato honesto de una interfaz de bucle.

### 6.2 El `scp` dijo que había fallado, y no

```
scp.exe: dest open "/tmp/eje-agente": Failure
scp.exe: failed to upload file
```

Y el md5 del destino era el del binario nuevo. El fichero se escribió y el fallo llegó
después, al cerrar. **Se anota porque invierte el sesgo habitual**: hasta hoy el riesgo era
dar por bueno un artefacto viejo, y aquí era descartar una corrida válida.

En este caso la corrida se detuvo igualmente para pedir el md5 — y se detuvo porque el
bloque de órdenes que se dio **no lo incluía**, error de quien lo escribió. La regla no es
«comprobar cuando algo parece raro»: es comprobar siempre.

## 7. Lo que sigue faltando

`resumirRespaldo` sigue sin consumidor: `vis04.js` no pinta inventario. Eso es PA-78
mitad B, y ahora hay tres canales con datos que enseñar en lugar de dos.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| PA-138b | **Cerrado.** §6 |
| PA-78 | Mitad B: que el operador lo vea. Ahora hay tres canales con datos que enseñar |
| PA-142 | Los ficheros del renderer siguen ciegos |

---

*Reporte Nº 90 — El tercer canal · PremosCorp · 28 de agosto de 2026*
