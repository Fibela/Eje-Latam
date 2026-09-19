# RPT-101 — Romper para saber

**Tema:** Fase 3 de la identidad operativa, y los tres puntos que sólo aparecieron al romper lo que ya estaba en verde
**Nº de reporte:** 101
**Fecha:** 14 de septiembre de 2026
**Área designada:** Método
**Entidad:** PremosCorp
**Estado:** Cierra PA-146b (fase 3), PA-153, PA-154 y PA-155. Deja una pregunta de diseño abierta

- **Depende de:** RPT-099 (centinela v4 y `EstadoOperativa`), RPT-097 (la barrera de PA-149), RPT-096 (el par de recuperación)
- **Verificado:** 632 pruebas de Rust, 136 de TypeScript, `cargo clippy -D warnings` limpio, `cargo fmt` sin cambios

---

## 1. Lo que se cableó

`EstadoOperativa` existía desde RPT-099 y **no lo consumía nadie**: se calculaba en el arranque
y moría ahí. Eso es la forma canónica del defecto dominante de este proyecto —productor sin
consumidor— y la fase 3 consistía en darle el consumidor.

Lo que llegó al otro extremo:

| Eslabón | Qué cambió |
|---|---|
| `contrato-ipc.toml` | cuatro campos con su `manipulacion` declarada |
| `crates/eje-ipc/src/mensajes.rs` | `Condiciones` de dieciséis a **veinte** |
| `crates/eje-agente/src/alertas.rs` | `condiciones(…)` recibe `EstadoOperativa` |
| `ciclo.rs`, `main.rs`, `salida.rs` | constructor, `match` de arranque, `EMISIBLES` de 14 a 18 |
| `puente.ts`, `vis04.ts`, `diagnostico.js`, `cabecera.ts` | tipos, filas y dos ramas de cabecera |

### Por qué son cuatro condiciones y no tres

La clave de recuperación tiene tres estados presentables; la operativa tiene **uno más**, y
es el que da sentido a todo el trabajo de PA-146b: **se puede rotar legítimamente.**

El mismo hecho en el disco —un `.pub` distinto del anclado— es secuestro sin certificado y
rotación con él. `operativaRotada` **no es manipulación**, y ponerla en crítica enseñaría al
operador a ignorar la crítica justo después de cada ceremonia bien hecha: la fatiga de
alertas de PA-45 con el peor disparador posible.

Y es **condición, no estado transitorio**, porque el agente no reescribe su propia ancla. El
sensor se queda en «rotada» hasta que un administrador vuelva a anclar, y durante ese rato la
sala tiene que poder contar cuántos sensores quedaron a medias de la ceremonia.

### El único dato bueno del cableado

**TypeScript pasó en verde a la primera, 132 sobre 132.** La barrera de PA-149 leyó del
contrato las dos manipulaciones nuevas —`operativaSuprimida` y `operativaNoVerifica`— y
encontró sus ramas de cabecera ya escritas. Nadie le dijo que esas condiciones existían.

Eso era exactamente lo que se compró en RPT-097. Pero pasar en verde no demuestra nada, y el
resto del reporte trata de eso.

---

## 2. Las perturbaciones

Tres barreras nuevas, ninguna vista en rojo. Se rompió lo que cada una dice comprobar.

### P1 — quitar la rama `operativaNoVerifica` de la cascada

**Predicción escrita antes:** dos fallos, la barrera del contrato y la prueba directa de
`cabecera.prueba.ts`.

**Resultado:** un fallo.

```
not ok 5 - toda condición de manipulación ocupa la cabecera de VIS-04
    'operativaNoVerifica' es manipulación en el contrato y la cabecera
    no la presenta como crítica. La cascada de componerCabecera no la cubre.
# pass 131  # fail 1
```

La barrera funciona y funciona por el camino correcto: se puso roja nombrando una condición
que se añadió **después** de escribirla, sacada del contrato y no de ninguna lista propia.

La mitad que falló de la predicción abre §4.

### P3 — un sexto caso en `EstadoOperativa`

Esto **no era una confirmación, era una pregunta**. La doctrina dice que un `match` sin
comodín es correa de transmisión: crece el dominio, el compilador obliga a decidir. Pero
`alertas.rs` no usaba `match`, usaba `matches!`.

**Predicción:** el compilador para en `main.rs` y **calla en `alertas.rs`**.

**Resultado:** exactamente eso, y una sola línea de error.

```
error[E0004]: non-exhaustive patterns: `EstadoOperativa::Inventada` not covered
   --> crates/eje-agente/src/main.rs:863:11
```

Abre PA-153, y es lo más valioso del día.

---

## 3. PA-153 — el compilador paraba en el sitio equivocado

`matches!` da exclusión mutua. No da exhaustividad. Las siete condiciones de las dos claves
se derivaban así:

```rust
operativa_no_aprovisionada: matches!(operativa, EstadoOperativa::NoAprovisionada),
operativa_suprimida:        matches!(operativa, EstadoOperativa::Suprimida),
operativa_no_verifica:      matches!(operativa, EstadoOperativa::NoVerifica),
operativa_rotada:           matches!(operativa, EstadoOperativa::Rotada),
```

Con un sexto estado, los cuatro dan `false` y **el tablero pinta calma** para algo que nadie
ha decidido cómo presentar. Ni una prueba lo vería: no hay prueba para un estado que todavía
no existe.

Lo que hace a este punto distinto de un descuido es **dónde sí paraba el compilador**. Paraba
en `main.rs`, en el `match` que escribe una línea en el diario del sensor. Quien reciba ese
error añade el brazo que falta, vuelve a compilar, ve verde y da el dominio por cubierto —
con la pantalla mintiendo.

> Un `match` sin comodín en el sitio menos importante es peor que ninguno: da la señal de
> que el dominio está sujeto, y la da desde donde no lo sujeta.

**Arreglo.** Dos `match` sin comodín que devuelven tuplas, uno por enum. La exclusión mutua se
conserva —cada brazo tiene como mucho un `true`, y eso se lee de un vistazo— y el dominio ya
no puede crecer sin que alguien decida en `alertas.rs`, que es donde se decide lo que el
operador ve.

---

## 4. PA-154 — lo encontró que mi predicción fallara

Anuncié dos fallos en P1 y hubo uno. La prueba directa que creía haber escrito ayer **no
existe**: la fase 3 añadió los cuatro campos al literal `CALMA` de `cabecera.prueba.ts` —lo
justo para que compilara— y delegó el resto en la barrera del contrato.

La barrera del contrato exige `urgencia === "critica"` y título no vacío. **Acepta que dos
condiciones distintas titulen exactamente igual.** Es laxa por construcción, y tiene que
serlo: es genérica y no puede saber qué prosa corresponde a qué condición.

El par de recuperación tenía `sabotaje y secuestro no pueden titular igual` desde RPT-096,
escrita porque nos quemamos. El par operativo no tenía nada equivalente, y nadie lo notó
porque la barrera genérica lo daba por cubierto.

**Arreglo.** Cuatro pruebas: la sustituida ocupa cabecera y no la tapa una de atención; la
operativa manda sobre la de recuperación cuando las dos están encendidas —lo que el sensor
obedece ahora pesa más que el remedio que falta para mañana—; borrada y sustituida titulan
distinto; y las dos que no son manipulación callan.

**Lo general que deja:** derivar del contrato quita el trabajo de mantener listas paralelas,
no el de escribir la prosa. Una barrera derivada dice *que* hay que cubrir algo; no puede
decir *cómo*. Confundir las dos cosas es lo que pasó aquí.

---

## 5. PA-155 — clippy tenía razón en el fondo, no en la forma

Con la fase 3, `condiciones` llegó a nueve argumentos y `too_many_arguments` se disparó.

La lectura cómoda es que es una regla de estilo y se silencia con `#[allow]`. La lectura
correcta es mirar qué había en esos nueve:

```rust
self.captura_no_disponible,   // bool
self.emisor.is_none(),        // bool
self.escucha_no_disponible,   // bool
```

**Tres `bool` contiguos.** Transponer dos compila, pasa las 632 pruebas, y pinta la condición
equivocada: el tablero diría que el sensor no admite consultas cuando lo que pasa es que no
está vigilando la red. Son dos averías distintas con dos remedios distintos, y el operador
actuaría sobre la que no es.

`#[allow]` habría callado el aviso dejando el agujero — la misma forma que el `PA_151` que se
rechazó en RPT-098 §6: adaptar la verdad al instrumento en vez de al revés.

**Arreglo.** `HechosDeFuera`, registro de tres campos con su documentación, desestructurado
exhaustivamente dentro de `condiciones` para que un cuarto hecho de fuera rompa ahí y no se
cuele apagado. Siete argumentos, y en la llamada los tres van con su nombre.

---

## 6. Dos errores de método, anotados porque se repiten

### 6.1 Busqué la llamada por cómo se escribe

Tres vueltas de compilación se fueron en encontrar sitios de `Ciclo::nuevo`. Mi búsqueda era
`Ciclo::nuevo(` y uno de los cuatro sitios es `Ciclo::<DespachoEspia>::nuevo(`. La misma
llamada, y mi índice sólo tenía una forma.

Es el decimotercer índice escrito a mano de la serie y el primero que se esconde detrás de la
sintaxis del lenguaje. **No abre PA:** aquí el compilador es la barrera, falló ruidosamente y
señaló la línea exacta. El índice a mano peligroso es el que se queda corto y sigue
pareciendo el total; éste se quedó corto y el sistema lo dijo. La lección es sobre cómo
busco, no sobre una barrera que falte.

### 6.2 Metí un registro entre un bloque `///` y su función

Al insertar `HechosDeFuera` justo encima de `#[must_use] pub fn condiciones`, el bloque de
documentación de la función —cuatro párrafos de por qué `salida_no_disponible` se rellena
después— **se reasignó en silencio al registro nuevo**, y `condiciones` se quedó sin nada.

Lo cazó `missing_docs`, que este proyecto tiene en `-D warnings`. Sin ese lint se habría
commiteado tal cual: código correcto, pruebas verdes, y el razonamiento de una función
colgando de otra cosa.

### 6.3 Una comprobación que no comprobaba nada

Pedí `cargo fmt --all && git diff --stat` para ver cuánto había reformateado `fmt`. No podía
decirlo: el árbol ya estaba sucio con mis propias ediciones, así que el diff era contra HEAD y
casi todo era mío. Se dio por no preguntada. Queda anotado porque una comprobación que no
puede responder la pregunta que dice responder es peor que ninguna — invita a leer un número
como si significara algo.

---

## 7. Una pregunta de diseño que no se cierra hoy

`inventarioSuprimido` e `inventarioNoVerifica` **comparten rama y comparten título** en
`componerCabecera`:

```ts
if (condiciones.inventarioSuprimido || condiciones.inventarioNoVerifica) {
  // "La evidencia de este equipo no verifica"
```

Para las dos claves decidimos lo contrario: borrar y sustituir piden remedios distintos y no
pueden titular igual. Para el inventario decidimos que sí, hace mucho, y nadie ha vuelto a
mirarlo.

**No se abre PA.** Puede ser deliberado y correcto —en los dos casos la respuesta es «responder
como incidente»— y abrir un punto para cada asimetría que uno se encuentra es cómo se llena el
tablero de ruido. Queda escrito aquí para que quien lo mire de verdad lo encuentre planteado.

---

## 8. Recuento

```
Rust        : 632 pruebas, 0 fallos
TypeScript  : 136 pruebas, 0 fallos   (132 antes de PA-154)
clippy      : limpio con -D warnings
fmt         : sin cambios
```

**Tablero,** leído de `cargo xtask tablero` y no sumado a mano:

```
Identificadores : 157
Cerrados        : 107
Parciales       :   6
Abiertos        :  44
Pendientes      :  50
```

> **Este apartado llevaba «155 / 105 / 50», calculado sumando tres a las cifras de RPT-100.**
> Las tres estaban mal. Los cerrados eran 107 y no 105 porque cerré cuatro cosas y no tres
> —PA-146b sale de 🔵 Parcial— y los identificadores son 157 y no 155, dos más de los que
> explica mi aritmética. **Esa diferencia de dos no se ha reconciliado**, y reconciliarla es
> justamente el trabajo de PA-152: mientras el recuento salga de un comando y el triaje se
> escriba a mano, las dos fuentes divergen y la de a mano pierde. Se deja escrito en vez de
> arreglado a ojo, que es como se llegó aquí.

> **ERRATA, 16-sep-2026 (RPT-102).** La diferencia de dos **sí** se reconcilió, y la causa no
> tenía nada de particular: la aritmética partía de las cifras de RPT-100 y **no contaba las
> filas que yo mismo había añadido en la sesión**. El 16 de septiembre volvió a pasar lo mismo
> con una fila y se identificó el mecanismo. No es que las dos fuentes divergieran: es que una
> de ellas era yo sumando de memoria. La conclusión de arriba —que el recuento salga del
> comando— sigue valiendo; el «no se ha reconciliado» era prudencia mal colocada.

Tres puntos abiertos y cerrados el mismo día, y **ninguno lo encontró una prueba en verde**:
uno lo encontró una perturbación, otro lo encontró que fallara una predicción escrita, y el
tercero un lint que casi se silencia. Las 768 pruebas de este repositorio no habrían visto
ninguno de los tres.

## 9. Lo siguiente

1. Formalizar la asimetría entre `configuracion.rs` e `inventario.rs` respecto del corte de
   revocación — `configuracion.rs` no consulta revocaciones, así que rotar la clave operativa
   deja `agente.conf.firmado` sin verificar hasta que se reemita.
2. El simulacro de rotación en cuatro pasos sobre WSL y `eje-prueba`, con las veinte
   condiciones observadas en pantalla.
3. **Las cuatro pruebas de PA-154 pasaron en verde a la primera**, igual que le pasó a la
   barrera de PA-149. Están sin probar. Perturbarlas antes de fiarse de ellas.
