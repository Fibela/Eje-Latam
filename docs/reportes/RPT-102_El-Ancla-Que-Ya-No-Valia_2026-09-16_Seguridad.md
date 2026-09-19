# RPT-102 — El ancla que ya no valía

**Tema:** La identidad revocada que el tablero presentaba como sana, y la barrera de ayer cazándola sin que nadie se lo pidiera
**Nº de reporte:** 102
**Fecha:** 16 de septiembre de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** Cierra PA-156. Abre PA-157. Deja el simulacro de `eje-prueba` bloqueado por una causa medida

- **Depende de:** RPT-099 (centinela v4 y `EstadoOperativa`), RPT-101 (PA-153 y PA-154), RPT-015 (certificados de revocación)
- **Verificado:** 633 pruebas de Rust, 138 de TypeScript, `clippy -D warnings` limpio, `xtask verificar` limpio

---

## 1. El punto ciego

```rust
if presente == anclada {
    return Self::Anclada;
}
```

Cinco líneas, y la palabra que faltaba es **«y ningún certificado la revoca»**.

`EstadoOperativa::evaluar` declaraba sana la identidad en cuanto la clave del disco coincidía con el ancla del centinela. El escenario que eso deja pasar:

1. Se compromete la clave operativa de un sensor.
2. Se revoca con dos custodios y se declara sucesora. El certificado va firmado por la autoridad de recuperación y se anota en el propio almacén del sensor.
3. Reanclar es un acto administrativo, así que el ancla sigue nombrando a la clave vieja. Y la vieja sigue en el disco.

Resultado:

| Componente | Qué dice |
|---|---|
| `aceptar_inventario` | rechaza por encima del corte — **el único que se entera** |
| `aceptar_configuracion` | acepta: no consulta revocaciones |
| `EstadoOperativa` | `Anclada` |
| **VIS-04** | **sensor sano** |

Hay un certificado firmado dentro de la propia máquina diciendo que esa clave no vale, y el panel no lo mencionaba.

## 2. Se midió antes de arreglarlo

El equipo propuso montar el escenario y llamar a `evaluar`. Eso no habría demostrado nada: **`evaluar` es una función pura** y el riesgo nunca fue que calculara mal, sino que quien la llama no le pasara las revocaciones. Es el defecto dominante del proyecto —productor sin consumidor— y por eso una unitaria verde sobre `evaluar` habría convivido perfectamente con un agente ciego.

La prueba se montó sobre un almacén real en disco y entra por `arrancar_con_almacen`, que es la puerta que usa el agente:

```
thread 'pruebas::una_clave_anclada_y_revocada_no_puede_leerse_como_sana' panicked
  left: Anclada
 right: Anclada
```

Lleva una asserción anti-vacuidad delante: si el ancla del centinela no fuera la clave del disco, el escenario no se habría montado y la prueba habría pasado por la puerta de atrás con `NoAprovisionada`. Pasó, así que el fallo es el real.

**Y se aprendió algo de paso:** el temor de que `arrancar_con_almacen` cortocircuitara en `Supresion` sin llegar a evaluar la clave operativa era infundado. Escrito porque se predijo como modo de fallo probable y no ocurrió.

## 3. El sexto estado

`Revocada` es el espejo exacto de `Rotada`, y esa simetría es lo que le da el nombre:

| | Clave en disco | Ancla | Qué pasó | Remedio |
|---|---|---|---|---|
| `Rotada` | la sucesora | la vieja | avanzó el sensor, se quedó atrás el ancla | reanclar |
| **`Revocada`** | la vieja | la vieja (revocada) | **avanzó la revocación, se quedó atrás el sensor** | completar la rotación aquí |

Tres decisiones, con su motivo:

**Es un caso nuevo y no un matiz de `Anclada`.** `presente == anclada` es literalmente la definición de `Anclada` hoy; no cabe dentro.

**No es manipulación.** Nadie tocó este almacén. `manipulacion` significa «alguien entró aquí» y manda a respuesta a incidentes; esto manda a quien despliega, a terminar una ceremonia. Marcarlo como manipulación llamaría al equipo equivocado — que es el mismo error que la fatiga de alertas de PA-45, sólo que al revés.

**Pero en el tablero va en crítica,** con el precedente exacto de `registroSaturado`: tan grave como la manipulación sin serlo. El sensor está obedeciendo inventarios y configuración firmados por una clave que dos custodios declararon muerta, y por debajo del corte `aceptar_inventario` los sigue aceptando. Si la revocación se emitió porque robaron la clave, el ladrón firma y el equipo obedece.

Veintiuna condiciones.

## 4. La barrera de ayer se validó sola

Esto es lo más importante del reporte y no estaba planeado.

Ayer (RPT-101 §3) se cambiaron siete `matches!` por dos `match` sin comodín en `alertas.rs`, con el argumento de que el compilador paraba en el sitio equivocado: en `main.rs`, que escribe el diario, y no donde se decide lo que ve el operador. El argumento era correcto pero estaba demostrado con una perturbación fabricada — un sexto estado inventado y retirado a los diez minutos.

Hoy el sexto estado se añadió **de verdad, por un motivo independiente**, y:

```
error[E0004]: non-exhaustive patterns: `EstadoOperativa::Revocada` not covered
   --> crates/eje-agente/src/alertas.rs:718:15
```

Paró en `alertas.rs`. `main.rs` ni apareció, porque la biblioteca falla antes y su binario no llega a compilarse.

**Con los `matches!` de anteayer, este cambio habría compilado en silencio** y `operativaRevocada` habría salido apagada: tablero en calma con una identidad revocada, que es exactamente el defecto que se estaba arreglando. Es la primera barrera de este proyecto que demuestra su valor por el uso y no por un ataque a propósito, y eso vale más que las perturbaciones que la precedieron.

## 5. Y la lección de PA-154 se aplicó el mismo día

`operativaRevocada` declara `manipulacion = false`. La barrera «toda condición de manipulación ocupa la cabecera de VIS-04» **no la recorre**: para esta condición no comprueba absolutamente nada.

Hace dos días eso habría pasado desapercibido —fue exactamente el defecto de PA-154—, así que las dos pruebas de cabecera se escribieron a mano **antes de compilar**, y se perturbaron después.

Tres tandas, cinco asserciones observadas en rojo:

| Perturbación | Qué cae | Barrera de PA-149 |
|---|---|---|
| **Q-A** rama retirada | *ocupa la cabecera* → `urgencia` | 🟢 verde |
| **Q-B** rama subida por encima de las de inventario | *la manipulación manda* → `/evidencia/` | 🟢 verde |
| **Q-C** detalle con «Tratar el equipo como comprometido» | *ocupa la cabecera* → bucle de frases | 🟢 verde |

Q-B es el hallazgo. Subir una condición que no es manipulación por encima de dos que sí lo son deja a un sensor con el almacén tocado titulando sobre una ceremonia pendiente — y la barrera genérica pasa sin inmutarse, porque comprueba que cada manipulación produzca *alguna* cabecera crítica, y la producía: la equivocada.

> **Una barrera derivada puede verificar que algo se dice. No puede verificar que se diga lo correcto.**

Esa frase es PA-154 en su forma general, y hoy tiene tres mediciones detrás en vez de un argumento.

## 6. Lo que este reporte NO cierra

**El tablero ya lo dice. El agente sigue obedeciendo.**

`aceptar_configuracion` no consulta revocaciones, así que un `agente.conf.firmado` firmado por la clave muerta sigue verificando. Lo que se arregló hoy es que el operador lo vea; no que el sensor lo rechace.

Y **no se decide hoy porque la respuesta obvia puede ser la mala**: si la configuración se rechazara, el sensor caería a los parámetros de la línea de órdenes —`configuracionSinFirmar`—, y de eso ya dice RPT-074 que «quien controle el arranque puede alargar la ventana de silencio que la sala vigila». Cambiar un fichero firmado por una clave muerta por ningún fichero firmado en absoluto no es evidentemente mejor.

Queda como **PA-157**. Lo que sí está resuelto es de dónde sale el dato: el hecho binario ya lo calcula el arranque y sólo hay que pasárselo — que es literalmente lo que le pasaba a `EstadoOperativa` hasta anteayer.

## 7. El simulacro sigue bloqueado, y ahora se sabe por qué

Se iba a correr la rotación entera sobre `eje-prueba`. La inspección previa —exigida antes de ejecutar nada— lo impidió, y encontró tres cosas:

1. **El centinela de la VM es v3**, no v4. `xxd` lo confirma sobre los bytes: `454a 452d 4345 4e31 0003`. Sin ancla operativa, `evaluar` devuelve `NoAprovisionada` y **el escenario de PA-156 no es reproducible ahí**.
2. **El agente desplegado es del 9 de septiembre**, anterior a la fase 3. Pinta dieciséis condiciones. Una captura de control de veintiuna no existe todavía.
3. **No hay fragmentos de custodia.** Ni un `.frg` en disco. Sin dos fragmentos `revocar` devuelve `Uso` y no hace nada, y generar unos nuevos produciría otra clave de recuperación que el centinela de la VM no reconoce.

Además, el plan que se propuso llevaba `--fragmento1` y `--fragmento2` —las banderas se llaman `--fragmento-uno` y `--fragmento-dos`—, omitía `--sucesora`, que es obligatorio, y omitía `--corte`, que sin pasar vale cero y habría revocado todo por accidente.

**Nada de eso se descubrió razonando: se descubrió midiendo hashes y bytes antes de ejecutar.**

## 8. Errores de método

**Una prueba que busca una palabra prohíbe hablar del tema.** La primera versión de la prueba de encuadre decía `doesNotMatch(detalle, /ataque|comprometid/i)` y se puso roja contra la cascada correcta: el detalle dice «**No** es un ataque a esta máquina», y una expresión regular no distingue una afirmación de su negación. Se reescribió contra las frases que usan de verdad las ramas de manipulación (`responder como incidente`, `tratar el equipo como comprometido`) y se le añadió una asserción positiva sobre la rotación. **Quedó más estricta, no menos** — lo único que dejó de prohibir es explicar por qué no aplica, que es justo lo que ese detalle existe para hacer.

**Se inventó un identificador.** Se escribió `cabecera_titulo_de_manipulacion(...)`, una función que no existe, en lugar de comprobar contra el texto real de la rama. Se corrigió antes de compilar, pero el proyecto tiene esa regla escrita desde hace meses y se saltó igual.

**La barrida de prosa de la fase 3 se dejó cuatro sitios.** `vista.prueba.ts` seguía diciendo «dieciséis» en tres comentarios y un nombre de prueba, y `cabecera.prueba.ts` en dos más — dos días después de que el panel tuviera veinte. No rompe nada y no hay barrera que lo cace: es el defecto de PA-152 fuera del tablero.

## 9. Recuento

```
Rust        : 633 pruebas, 0 fallos
TypeScript  : 138 pruebas, 0 fallos
clippy      : limpio con -D warnings
xtask verificar : limpio
```

**Tablero,** leído de `cargo xtask tablero`:

```
Identificadores : 159
Cerrados        : 108
Parciales       :   6
Abiertos        :  45
Pendientes      :  51
```

> La predicción decía 158 y salió 159. La causa, esta vez sí identificada: la aritmética
> partía del cierre de anoche y **no contaba la fila de PA-156, añadida esta misma mañana**.
> Es el mismo mecanismo que produjo la diferencia de dos en RPT-101 §8, que allí se dejó como
> «no reconciliada». Erratado. No divergían dos fuentes: una de las dos era alguien sumando de
> memoria.

## 10. Lo siguiente

1. **PA-157**: decidir si `aceptar_configuracion` rechaza, y contra qué cae el sensor si lo hace.
2. **Desbloquear `eje-prueba`**: decidir entre reaprovisionar la identidad de recuperación entera —ceremonia completa, el ancla cambia con ella— o aceptar que PA-156 se cerró por medición en WSL y que la observación en máquina real espera a la siguiente ventana de despliegue.
3. Migrar el centinela de la VM a v4 y desplegar el agente de la fase 3, **antes** de cualquier simulacro. Con el binario de hoy sobre el almacén de la VM sin migrar, el agente no arranca degradado: falla con `CentinelaCorrupto`.
4. Tarea pendiente de RPT-101: buscar más `match` sin comodín puestos en el sitio secundario. Hoy apareció uno gratis — `EstadoRecuperacion::es_manipulacion` tenía el mismo `matches!` y se corrigió al chocar con él, no por buscarlo.
