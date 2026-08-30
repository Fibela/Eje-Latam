# RPT-088 — El campo sin productor

**Tema:** PA-139. `Postura` no se amplía: se retira. El cable pasa a llevar evidencia
**Nº de reporte:** 088
**Fecha:** 28 de agosto de 2026
**Área designada:** Visión
**Entidad:** PremosCorp
**Estado:** **Cerrado.** `guardian-cc` 173, `eje-ipc` 29, TypeScript 128

- **Depende de:** RPT-087 (la evidencia que lo desbloqueó), RPT-006 §4, RPT-020 (el agente no contiene), RPT-081 (el patrón de tipo espejo en el cable)
- **Aborda:** PA-139 (cerrado). Desbloquea PA-138b

> **ERRATA — corregido por RPT-089 el mismo dia.** El `ClaseConocida` que describe §5
> tenia `inferidaSoporteVital` e `inferidaSeguridadFuncional`, y `clasificar` nunca
> devuelve eso: ante huella sin marcado declara ambiguedad. Ademas le faltaba
> `declaradaNoCritica`, el unico estado que permite accion automatica. El enumerado
> vigente es el de RPT-089 §3. Lo demas de este reporte se mantiene.

---

## 1. La pregunta cambió al mirar

PA-139 se abrió como «`Postura` necesita una cuarta variante para *no se sabe*». Antes de
escribirla se buscó quién la produce:

```
crates/eje-ipc/src/pruebas.rs:408:  postura: Postura::Conforme,
```

**Ésa es su única aparición en todo Rust, y es un dato de prueba.** En TypeScript sí tenía
consumidor: `resumirPostura` hacía `switch` sobre ella en VIS-04.

Un tipo con consumidor y sin productor es la clase de defecto dominante del proyecto,
ahora a nivel de campo. Añadirle una variante habría sido poner una cuarta pieza a un
mecanismo que nadie invoca — PA-135 otra vez, un nivel más abajo.

## 2. Lo que el agente sabe, y que no es una postura

Tras RPT-087: `Indicio` por dispositivo, `DeclaracionSegmento` del segmento donde se le
vio, y la marca de segmento crítico. Eso no es un juicio: **son las evidencias con las que
se forma uno.**

De ahí la decisión, que es la del reporte entero: **el agente es testigo y VIS-04 es quien
lee.** El sensor no emite `conforme` ni `anomalo`. Entrega hechos y la capa de
presentación compone la lectura — donde cambiar la regla no exige recompilar el sensor ni
volver a desplegar en planta.

## 3. Tres propuestas del equipo

**`identificador` fuera — aceptada.** Era la MAC serializada, la misma cadena que
`direccionEnlace`. Un campo que finge ser una abstracción y es el mismo dato.

**`ConocimientoClase` — aceptada la intención, rechazada la forma.** El dominio ya tiene
`Indicio`, con la distinción escrita en su propio comentario:

```rust
/// **No** significa «no es critico». Significa que esta fuente no aporta.
SinIndicio,
/// Se distingue de SinIndicio porque colapsarlas repetiria el defecto
/// que RPT-006 §4 documenta.
Indeterminado,
```

La propuesta reinventaba ese tipo y **perdía su tercer estado**: `Ninguna` colapsaba «esta
fuente no aporta» con «no se pudo consultar». Además `ClaseSugerida` no existe —la
inferencia produce `ClaseExcluida`— y faltaba el cuarto caso que el dominio ya nombra:
`MotivoAmbiguedad::ConflictoEntreFuentes`.

**`Contenido` = marca pegajosa — rechazada, tercera vez con la misma premisa.** El
pegajoso es resistencia a la expulsión (RPT-018 §6) y **el agente no suprime tráfico de
nadie** (RPT-020).

## 4. El nombre hace el trabajo que el comentario no hace

Dos decisiones de forma, y las dos por el mismo motivo.

**La clase lleva su procedencia dentro.** `declaradaSoporteVital` frente a
`inferidaSoporteVital`, en un solo valor y no en dos campos. Con dos, alguien lee la clase
y no mira el respaldo; y no es lo mismo que lo jure el administrador con su firma a que el
agente lo suponga por haber visto tráfico HL7. **Bloquear un equipo inferido es gestión de
red; bloquear uno declarado de soporte vital es riesgo humano.** Con un solo valor, la
lectura equivocada **no se puede escribir**.

**`vistoEnSegmentoCritico`, no `marcaPegajosa`.** Se leyó como contención tres veces
seguidas. Cuando un nombre invita al mismo error tres veces, el arreglo es el nombre — no
un comentario que nadie leerá.

### 4.1 Y no existe `inferidaCaminoDeGestion`

`Protocolo::sugiere` sale de Modbus, DNP3, HL7 y BACnet, y ninguno apunta ahí: un camino
de gestión se declara, no se deduce del tráfico. Una variante para un caso imposible
invitaría a rellenarla.

Lo sujeta `ninguna_inferencia_apunta_al_camino_de_gestion`, con un `match` exhaustivo que
deja de compilar si se añade un protocolo. Sin ella, añadir mañana uno que sí lo sugiera
dejaría el caso sin variante en el cable y alguien lo mapearía a otra cosa.

## 5. El contrato, antes y después

| Antes | Después |
|---|---|
| `identificador` (texto) | *(retirado: era `direccionEnlace`)* |
| `direccionEnlace` (texto) | `direccionEnlace` (texto) |
| `clase`: plc\|camara\|medico\|estacion\|desconocido | `clase`: ocho valores **con procedencia dentro** |
| `postura`: conforme\|anomalo\|contenido | *(retirado: sin productor)* |
| — | `declaracionSegmento`: lo que declaró el administrador |
| — | `vistoEnSegmentoCritico` (booleano) |
| — | `protocolosObservados` (lista&lt;texto&gt;) |

En TypeScript, `resumirPostura` pasa a `resumirRespaldo`: cuenta el inventario por
**calidad del respaldo** —declarados, inferidos, en conflicto, sin indicio,
indeterminados—, y las cinco cifras suman siempre el total.

Las pruebas de paridad no necesitaron tocarse: cotejaron el cambio solas en los dos lados.

## 6. Lo que queda, dicho sin adornos

`resumirRespaldo` **todavía no la llama nadie**, igual que no llamaba nadie a
`resumirPostura`. No es un descuido nuevo: el canal sigue `servido = false` y `vis04.js`
no pinta inventario. Se cierra con PA-138b y con la mitad B de PA-78, y se anota aquí para
que no vuelva a descubrirse dentro de un mes como se descubrió PA-97.

## 7. La misma trampa, esta vez la pisé yo

Escribí `Protocolo::clase_sugerida` en cinco sitios —código, contrato y dos reportes—. El
método se llama `sugiere()`.

Es el identificador inventado que llevo cuatro veces rechazando: `ClaseSugerida`,
`Corporativo`, `PerfilSegmento::Aislado`, los canales `ping` y `estadisticas`. **El patrón
no es de quien propone: es de quien escribe de memoria en lugar de mirar.** Hoy fui yo, y
sólo apareció al compilar.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| PA-139 | **Cerrado.** §2 y §5 |
| PA-138b | **Desbloqueado.** Falta la traducción a `ClaseConocida`, el mapeo de segmento al cable, el manejador y `servido = true` |
| PA-78 | Mitad B sigue esperando un escritorio, y ahora también que VIS-04 pinte el inventario |

---

*Reporte Nº 88 — El campo sin productor · PremosCorp · 28 de agosto de 2026*
