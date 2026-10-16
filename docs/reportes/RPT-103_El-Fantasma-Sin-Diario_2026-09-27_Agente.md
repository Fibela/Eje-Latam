# RPT-103 — El fantasma sin diario

**Tema:** Veintiuna condiciones observadas sobre hardware real, y un silencio que no dejó rastro porque el agente no contaba lo que descartaba
**Nº de reporte:** 103
**Fecha:** 27 de septiembre de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** Cierra PA-158. Deja el silencio de `obtener-inventario` abierto y **sin clasificar**, ahora medible

- **Depende de:** RPT-102 (PA-156 y el panel en veintiuna), RPT-084 (atender a mitad de vuelta), RPT-081 (declarado no es cableado)
- **Verificado:** 636 pruebas de Rust, 138 de TypeScript, `clippy -D warnings` limpio, `xtask verificar` limpio
- **Evidencia:** dos capturas de VIS-04 sobre `eje-prueba`, 03:40 y 03:48 del 27 de septiembre

---

## 1. Lo observado, que era el objetivo

VIS-04 pinta **veintiuna condiciones** sobre el sensor real, con el centinela v4 y el binario de la fase 3 instalados en RPT-102. La novena fila dice:

> **IDENTIDAD REVOCADA: hay certificado que la anula y este sensor sigue usándola — no**

PA-156 llegó al operador. La cabecera dice *«Este sensor no informa a ninguna sala»* en atención, que es lo correcto para un sensor sin colector, y el panel **Sensor** de RPT-091 aparece sin que nadie haya parcheado el HTML a mano.

**Y una predicción derribada.** Se anunció que el primer arranque encendería `accionAdministrativa`. La última fila dice **no**. Un sensor recién instalado sin inventario no es una anomalía administrativa, y el diario ya lo decía al anunciar sólo `sinColector`: la lectura de más fue del analista, no del sensor.

## 2. El fantasma

El 19 de septiembre, con el agente llevando **cincuenta y un minutos** en marcha, VIS-04 informó:

```
ErrorEnlace: [sin-respuesta] el agente no respondió a «obtener-inventario» en 5000 ms
```

No es un rechazo. Es silencio, que es distinto y peor.

Lo que se descartó mirando el código, y no suponiendo:

- **El canal está declarado `servido = true`** en `contrato-ipc.toml`.
- **El manejador existe** desde RPT-090: `Canal::ObtenerInventario` responde los nodos, o un `Err` con motivo cuando aún no hay primera vuelta. **Las dos ramas devuelven algo.**
- **`obtener-condiciones` sí respondió** en esa misma sesión —el panel pintó las veintiuna—, así que el socket y el marco funcionaban.

Así que no es PA-135. No es un canal declarado y sin cablear.

Dos ejecuciones el 27 de septiembre, con ocho minutos entre ellas, **no lo reprodujeron**. Es intermitente.

> **Y el experimento estaba mal diseñado.** Se planteó un lanzamiento A «inmediatamente después de arrancar el agente» para pillar la primera vuelta con el inventario en `None`. Pero el agente ya llevaba rato en marcha cuando se lanzó, así que A y B fueron el mismo escenario y la discriminación no existió. Revisando además el día 19 con cuidado, allí tampoco era primera vuelta. La hipótesis de la carrera de arranque estaba mal desde el principio.

## 3. La hipótesis, con mecanismo y sin demostrar

`servicio.rs` tiene dos piezas que juntas explican un silencio intermitente:

```rust
pub const PLAZO_CONEXION: Duration = Duration::from_millis(250);
```

```rust
// Un fallo de una conexion no puede tumbar el ciclo. Se ignora y se
// pasa a la siguiente [...]
let _ = servir(flujo, atiende);
```

El plazo corre **desde que el agente acepta**. Si el cliente conectó pero aún no ha escrito cuando el agente llega a `read_exact`, el plazo vence a los 250 ms, `servir` devuelve `Err`, y ese `Err` se descarta. Conexión cerrada sin una línea en ninguna parte.

Electron arrancando abre cuatro conexiones —una por petición, porque el marco no lleva identificador de correlación (PA-80)— mientras crea la ventana y carga el renderizador. Que el bucle de Node tarde más de un cuarto de segundo en poner los bytes de una de ellas es normal en una máquina virtual. La que pierde la carrera muere muda, y a los cinco segundos la consola dice lo que dijo.

**Esto no se da por demostrado.** Es un mecanismo plausible, no una medición. Lo que sí se hizo es volverlo medible.

## 4. PA-158 — lo que sí se arregló

El problema de fondo no era el fallo: era que **cuando volviera a pasar tendríamos la misma información que la primera vez, ninguna.**

`atender` pasa a devolver `Atencion` —aceptadas, caídas, y el motivo de la primera— en lugar de un `usize`. Cambiar el tipo de retorno obliga a cada sitio de llamada a enterarse, que es la correa de transmisión de siempre.

Tres decisiones de diseño, con su porqué:

**Se acumula a lo largo de la vuelta y se dice una sola vez.** `atender_pendientes` se llama una vez por trama, hasta doscientas por vuelta; una línea por caída inundaría el diario y expulsaría lo que importa, que es el mismo defecto con otro disfraz.

**Se guarda un motivo, el primero, no todos.** La primera caída de una racha suele ser la causa y las siguientes su consecuencia.

**Y no se calla en modo demonio**, al contrario que las consultas que salen bien. Ésa es la distinción que importa: las que funcionan son constantes y silenciarlas es correcto; una caída es rara, y es exactamente lo que alguien irá a buscar en el diario cuando la consola diga que el sensor no contesta.

La línea que saldrá la próxima vez:

```
!! 1 conexion(es) aceptadas y cerradas SIN RESPUESTA en esta vuelta.
   Primera causa: Resource temporarily unavailable. Quien pregunto se quedo esperando.
```

Con eso, cotejar la hora con el `[sin-respuesta]` de la consola confirma o descarta la hipótesis de §3 en una línea.

**Y es defecto por sí solo, no sólo una molestia para depurar.** En una planta, «la consola no responde» era indistinguible de «el agente está caído», y las dos cosas llaman a personas distintas.

**Lo que NO se tocó:** los 250 ms. Subir el plazo a ciegas cambiaría un defecto por otro, y el comentario del código tiene razón en que un cliente lento no puede apagar la vigilancia. Primero medir.

## 5. El agujero que encontró leer, no ejecutar

Las pruebas de PA-158 nacieron en verde. Antes de perturbarlas se leyeron **juntas**, y ahí apareció el problema:

- `una_conexion_que_no_escribe_se_cuenta_como_caida` exige `caidas == 1`.
- `sin_conexiones_no_hay_caidas` exige `caidas == 0` sin aceptar ninguna conexión.

**Un `atender` roto que contara toda conexión aceptada como caída pasaba las dos.** La primera porque espera una caída; la segunda porque sin `accept` el bucle rompe antes del contador. Ninguna miraba el caso bueno.

Se añadió la tercera, `una_conexion_bien_atendida_no_cuenta_como_caida`, con un cliente que escribe una petición válida y recibe respuesta. Contar caídas donde no las hay es peor que no contarlas: un diario que avisa siempre se aprende a ignorar, y entonces el aviso de verdad pasa desapercibido (PA-45).

**Esto no lo dice ningún comando.** Las tres pruebas pasaban. Lo que faltaba era preguntarse *«¿qué versión rota del código sobreviviría a esto?»*, y esa pregunta se hace a mano, leyendo.

## 6. Las perturbaciones

Dos tandas, cada una rompiendo una cosa distinta, con la predicción escrita antes:

| Perturbación | Qué cae | Qué sigue verde |
|---|---|---|
| **R-A** — `let _ = servir(...)`, el código de ayer | `una_conexion_que_no_escribe` → `left: 0, right: 1` | las otras cinco |
| **R-B** — contar toda conexión como caída | `una_conexion_bien_atendida` → `left: 1, right: 0` | las otras cinco, **incluida la del cliente mudo** |

Que en R-B la prueba del cliente mudo se quedara verde es la demostración literal de §5: sin la tercera prueba, esa perturbación habría pasado entera y el agente habría llenado el diario de avisos falsos.

## 7. Tres errores de método, del mismo día

**Se archivó una captura rancia como evidencia de hoy.** `~/vis04.png` era la captura del 8 de septiembre y entró en `docs/evidencia/` con el nombre y la fecha del 19. Hash idéntico a `RPT-095`. Dentro de seis meses alguien habría abierto ese fichero, contado dieciséis condiciones en una imagen que dice tener veintiuna, y no habría tenido forma de saber cuál de las dos cosas era mentira. **De aquí sale la regla, ya sellada por perturbación:** ninguna evidencia entra sin comprobar que su hash no coincide con el de ninguna anterior. Una captura que se repite es, por definición, una captura que no se tomó.

**Se escribió una cadena `if let ... && ...` sin mirar el MSRV.** El proyecto fija 1.85 en `rust-toolchain.toml` y esa sintaxis es posterior. Era código de perturbación y no llegó a commitearse, pero es el error de siempre: escribir de memoria en vez de mirar lo que el propio proyecto declara.

**Y un comando de verificación que devolvía lo mismo en dos casos opuestos.** El filtro propuesto para leer la salida de `cargo test` no incluía `^error`, así que devolvió vacío cuando la compilación falló — indistinguible de «no rompió nada». Es exactamente la clase de instrumento que este proyecto persigue dentro del producto, algo que informa de éxito y entrega otra cosa, puesto por el analista en la línea de órdenes.

## 8. Recuento

```
Rust        : 636 pruebas, 0 fallos
TypeScript  : 138 pruebas, 0 fallos
clippy      : limpio con -D warnings
xtask verificar : limpio
```

**Tablero,** leído de `cargo xtask tablero`:

```
Identificadores : 160
Cerrados        : 109
Parciales       :   6
Abiertos        :  45
Pendientes      :  51
```

## 9. Lo siguiente

1. **Desplegar el agente con PA-158 en `eje-prueba`** y reproducir el arranque de VIS-04 varias veces hasta que el fantasma vuelva. Con el diario hablando, la hipótesis de §3 se confirma o se descarta.
2. **PA-157**, que sigue siendo la decisión de fondo: el tablero canta la identidad revocada y `aceptar_configuracion` sigue obedeciéndola.
3. **La barrera de evidencia duplicada dentro de `conformidad.rs`.** La comprobación está sellada por perturbación y todavía vive en una línea de órdenes, no en `xtask`.
4. **Los fragmentos de custodia**, que siguen sin existir en disco y bloquean el simulacro de rotación. Hasta resolverlo, `operativaRevocada` sólo se puede observar apagada.
