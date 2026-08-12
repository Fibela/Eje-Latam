# RPT-046 — Instanciación de Electron: el primer consumidor real

**Tema:** Ventana, preload y enlace con el agente. Dos colisiones estructurales y una lección repetida
**Nº de reporte:** 046
**Fecha:** 9 de agosto de 2026
**Área designada:** Interfaz
**Entidad:** PremosCorp
**Estado:** **Implementado.** 66 pruebas en verde, 12 suites. Abre PA-79 y PA-80

- **Depende de:** RPT-004 (seguridad de la ventana y puente IPC), RPT-045 (cliente de cable), RPT-036 (rechazo con motivo)
- **Aborda:** la instanciación de VIS pedida al cerrar RPT-045
- **Abre:** PA-79, PA-80

---

## 1. Lo que había que evitar, y estuvo a punto de pasar

`seguridad-ventana.ts` declara `PREFERENCIAS_SEGURIDAD` congelada desde RPT-004 §6.1,
y tres pruebas la verifican. La forma natural de instanciar la ventana era escribir
`new BrowserWindow({ contextIsolation: true, ... })` con literales.

Eso habría dejado las tres pruebas verdes verificando **una constante que nadie
usa**. Es el patrón que este proyecto ya conoce: el mecanismo existe, es correcto,
y no lo llama nadie.

Y aquí tiene un agravante propio: **Electron ignora en silencio las claves de
`webPreferences` que no conoce y acepta cualquier subconjunto**. Pasar la mitad
del objeto crea una ventana insegura sin que nada proteste — ni el compilador, ni
Electron, ni las pruebas que miran la constante.

De ahí `opcionesDeVentana`, que compone las preferencias **enteras** desde la
constante, y dos pruebas en direcciones opuestas: que están todas las declaradas,
y que no aparece ninguna que no lo esté. La segunda es la que caza a quien añada
`webSecurity: false` en la creación.

## 2. Primera colisión: el sandbox obliga a CommonJS

`sandbox: true` está ratificado. **Electron no admite preloads en módulos ES bajo
sandbox**, y `proceso-principal` es `"type": "module"`.

No hay negociación posible: el sandbox manda. El preload pasa a `preload.cts`, que
`tsc` emite como `preload.cjs`. Con `verbatimModuleSyntax` activo eso obliga
además a `import electron = require("electron")`.

Se anota porque es invisible en revisión de código y sólo se manifiesta al
arrancar con escritorio. La prueba que lo sostiene lee el directorio y comprueba
que existe `preload.cts` y **no** existe `preload.ts`.

## 3. Segunda colisión: el preload es el cuarto sitio del contrato

Un preload sandboxeado no puede cargar `@eje/vision-base`, que es ESM. La lista de
canales **no puede llegar ahí en tiempo de ejecución**: hay que escribirla a mano.

Eso es exactamente lo que PA-20 existe para impedir, y no hay forma de evitarlo
con el sandbox activo. Lo que sí hay es atadura: una prueba lee el fuente del
preload y lo compara con `CANALES_PERMITIDOS`. Mismo mecanismo que PA-75 usa con
`puente.ts`, por el mismo motivo — lo que no se compara, diverge.

Seis pruebas, incluida una que exige que el canal sea un **literal** y no una
variable: ésa es la forma que tendría el pasamanos genérico que RPT-004 §6.2
prohíbe, y no se detecta mirando la lista de canales.

## 4. Una conexión por petición, porque el formato no admite otra cosa

El marco de `eje-ipc` **no lleva identificador de correlación**. Una respuesta no
dice a qué petición contesta.

Sobre una conexión persistente con dos peticiones en vuelo no hay forma de saber
cuál es cuál, y el emparejamiento erróneo **no falla**: devuelve el inventario de
otro momento, o las condiciones de otra consulta, en silencio. Es la clase de
defecto que en pantalla parece «datos raros» y se diagnostica como problema del
agente.

Mientras el contrato no tenga correlación, una conexión por petición es la única
forma correcta. Es más cara y da igual: son consultas de interfaz.

Queda como **PA-80**: si alguna vez hace falta concurrencia real, lo que hay que
añadir es un identificador en el marco, no un `Map` en el cliente.

## 5. Las tres formas de no obtener respuesta

Es lo que más pruebas tiene del enlace, y es deliberado. Hoy las tres acaban en la
misma pantalla:

| Qué pasó | Qué significa | Qué se hace |
|---|---|---|
| Vencimiento | El agente está vivo y atascado | Mirar el agente |
| Cierre limpio sin responder | El agente estaba y colgó | Mirar por qué colgó |
| El conducto no abre | El agente no está corriendo | Arrancarlo |

Colapsarlas en un `Error` genérico las hace indistinguibles, y se arreglan de tres
maneras distintas. El caso de cierre a media respuesta menciona además los bytes
que faltaban, que distingue «no mandó nada» de «se cortó a la mitad».

El vencimiento no es adorno: sin él, un agente que acepta la conexión y no
contesta deja la interfaz cargando para siempre. Eso es peor que un error — el
operador ve una pantalla que carga y no sabe que el sensor no está respondiendo.

## 6. La lección repetida

La suite falló en `se expone por contextBridge y no asignando a window`. El
comentario del propio preload citaba `window.eje = ...` como ejemplo de lo que no
hay que hacer, y la prueba se leyó a sí misma.

El equipo propuso borrar el ejemplo del comentario. **Se rechazó**: eso hace pasar
la suite debilitando el artefacto, y deja el defecto intacto.

Porque el fallo ocurrió en la dirección inofensiva. La peligrosa es la simétrica:
un comentario que cite `ipcRenderer.invoke("obtener-inventario")` haría pasar la
paridad **con el método real borrado**. Falso negativo silencioso, en la única
barrera que protege el cuarto sitio donde vive el contrato.

El arreglo es un lexer —`sinComentarios`— aplicado a **las tres** inspecciones, no
sólo a la que falló.

Y hay que decir lo incómodo: **esto ya estaba resuelto**. `solo_codigo` en
`xtask/src/cobertura.rs` hace lo mismo, por lo mismo, desde PA-73, hace tres días.
Se repitió en otro lenguaje sin recordarlo. La conclusión operativa es que toda
prueba que inspeccione fuentes empieza por quitar comentarios, y eso queda escrito
en los dos sitios.

El comentario del preload **se conserva**, con una prueba que lo exige: documenta
la decisión y es el fixture que demuestra que el lexer funciona. Si alguien lo
borra «para que pase la suite», la prueba dice que ése no era el arreglo.

## 7. Dos pruebas que se corrigieron por débiles

`es CommonJS` afirmaba que `".cts".endsWith(".cts")`. Cierta, y no verifica nada —
el mismo tipo de prueba descartada en RPT-044 §7. Se sustituyó por una que lee el
directorio.

Se anota porque la anterior habría sobrevivido a cualquier revisión: pasa, tiene
buen nombre y toca el tema correcto.

## 8. Lo que sigue sin poder probarse

**Que los dos procesos se hablen.** Sigue siendo PA-78 y sigue bloqueado por
PA-40. Todo lo de este reporte se prueba con un conducto de mentira; nadie ha
visto un byte cruzar un socket de verdad.

**Que la ventana se abra.** `fabricar` —el adaptador de `BrowserWindow`— y
`arrancar` no tienen prueba: son las diez líneas que necesitan un escritorio. Es
deliberado que sean diez y no cien.

**No existe la vista.** `montarVentanaPrincipal` carga `vista/indice.html`, que no
está escrito. Arrancará y fallará al cargarlo. No se improvisa aquí porque VIS-04
tiene diseño propio.

## 9. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-79** | `RUTA_SOCKET` está fijada en el código con una variable de entorno como escape. Debe salir de configuración firmada | Que el entorno decida a qué agente habla la consola |
| **PA-80** | El marco no lleva identificador de correlación; por eso hay una conexión por petición | Concurrencia real sobre el puente |
| PA-77 | Si VIS es co-ubicado y el sensor es headless, Electron no es el componente que corre ahí | Decidir el despliegue |
| PA-78 | Nadie ha visto a los dos procesos hablarse | PA-40 |

## 10. Excepción registrada en el guardián de fronteras

`electron` es `devDependency` y el código de producción lo importa, lo que choca
con `sin-devdependencies-en-produccion`. La excepción es real: el empaquetador
incrusta el runtime, y declararlo en `dependencies` lo duplicaría —unos 200 MB—
dentro del instalador.

Se acotó **por nombre**, no por patrón. Un patrón dejaría pasar la siguiente
devDependency por descuido, que es justo lo que esa regla existe para impedir.

## 11. PA-78 cerrado: los dos procesos se hablaron

El mismo día. `eje-agente` sobre WSL, socket en `/tmp/eje/agente.sock`, y
`enlace.ts` compilado —el que va en el producto, no un cliente de ocasión— al
otro extremo.

| Canal | Petición | Respuesta | Resultado |
|---|---|---|---|
| `obtener-condiciones` | 25 B | 223 B = 4 + 1 + 218 | Las ocho condiciones, `accionAdministrativa: true` |
| `obtener-inventario` | 24 B | 87 B = 4 + 1 + 82 | Rechazo **con motivo íntegro** |
| `consultar-alertas` | 41 B | 40 B = 4 + 1 + 35 | `{"primerDisponible":1,"sucesos":[]}` |

**PA-74 está cableado hasta el socket**, no sólo declarado. Y `primerDisponible`
vale 1, que es lo correcto: el asiento más antiguo que sobrevive es el 1. Un 0 no
describiría ningún asiento.

**Nada se rompió en el primer contacto**, y eso es mérito de `vectores-ipc.json`.
Ésa era la apuesta entera de RPT-045: que este momento fuese aburrido. Lo fue.

### 11.1. Lo que la conversación real NO ejercitó

Se escribe porque el riesgo de una integración exitosa es darla por completa.

**La trampa multibyte.** El motivo del rechazo son 82 caracteres y 82 bytes:
ASCII puro. El agente escribe «esta» y «aun» sin tilde. El caso que justificó los
vectores —recortar por bytes y no por unidades UTF-16— no ha cruzado el cable
nunca, y hoy no puede.

**La fragmentación.** 223, 87 y 40 bytes, los tres de una pieza. El acumulador no
ha acumulado jamás. El prefijo partido por la mitad y el marco a trozos siguen
verdes por construcción y sin observar. Hace falta una respuesta que no quepa en
un trozo, y sólo `consultar-alertas` con histórico puede darla.

**La latencia.** El agente atiende al final de cada vuelta, con vueltas de ~500 ms.
Eso es una lectura del código, **no una medida**: el script de diagnóstico no
cronometra. Queda como **PA-83**, sin cifra.

### 11.2. Dos diagnósticos equivocados, y de dónde salieron

El agente pareció morirse solo dos veces. Se sospechó del bucle de `main.rs`. El
bucle está bien: con `--ciclos 0` la condición de salida no se evalúa nunca.

Lo que pasaba es que había **una sola terminal**: para lanzar el cliente se
interrumpía el agente. Se anota porque los dos equipos buscaron un defecto en
Rust durante dos rondas donde había una consola compartida, y porque el rastro
—`ECONNREFUSED` sobre un socket que existe— era la pista y nadie la leyó hasta
que el enlace la nombró.

Eso mismo produjo la cuarta causa de fallo del §5: **el fichero del socket existe
y no hay nadie detrás**. Comprobar que el fichero está no lo detecta. Es el caso
más frecuente en campo y estaba colapsado en «el conducto falló».

### 11.3. Una prueba que pasaba por casualidad

`pedir` llamaba a `unref()` sobre el temporizador de vencimiento. Suena
prudente: que un temporizador no mantenga vivo el proceso. Es exactamente al
revés — mientras una petición está en vuelo el proceso **debe** seguir vivo,
porque eso es lo que se está esperando.

Con `unref()`, si no queda otra asa viva el bucle de eventos se vacía y la
promesa no se resuelve ni se rechaza. Node lo llama «Promise resolution is still
pending but the event loop has already resolved», y **depende de si por
casualidad hay otro trabajo asíncrono**.

Las seis pruebas afectadas pasaron en verde tres veces antes de fallar. Eso es
peor que una prueba que falla: la siguiente vez se archiva como incidencia del
entorno.

Dos cosas para la próxima:

- `unref()` no hacía falta. Todos los caminos de salida llaman a `clearTimeout`,
  así que el temporizador nunca sobrevive a la petición. La precaución no
  protegía de nada y rompía algo.
- Node contabiliza esto como **`cancelled`, no como `fail`**. La línea de resumen
  decía `fail 0`. Quien mire sólo esa cifra da la suite por buena.

### 11.4. PA-82 cerrado por observación, y la ventana abierta

```text
srw-rw---- 1 0 1000  /tmp/eje/agente.sock
```

Propietario `root`, grupo del operador, modo `0660`. El agente captura con
privilegios; la consola conecta sin ellos. Es lo que RPT-002 §9.3 llamaba
«socket Unix con ACL» y hasta hoy no existía.

La ventana de Electron arrancó y pintó las nueve condiciones, con
`accionAdministrativa` en rojo — el mismo `true` que el agente llevaba toda la
tarde imprimiendo en su bucle, llegando por fin a una pantalla.

Los **dos saltos** funcionaron a la vez: renderer → preload → proceso principal
→ socket → agente, y vuelta.

**Y un detalle que vale tanto como el cierre.** El puesto de diagnóstico
distingue `undefined` de `false` y pinta «AUSENTE EN LA RESPUESTA» en rojo. La
fila `capturaNoDisponible` mostró **`no`**, no «ausente»: el noveno campo
añadido ese mismo día cruzó el cable entero, serializado por Rust y recibido por
TypeScript, sin que nadie lo comprobara a mano.

Un panel que pintara los campos ausentes como «no» habría dicho que todo iba
bien exactamente igual. Esa distinción de una línea es la diferencia entre
observar y suponer.

## 12. Puntos abiertos añadidos tras la observación

| ID | Punto | Prioridad |
|---|---|---|
| **PA-81** | Un fallo de captura mata el proceso y con él la escucha. El momento en que más falta hace la consola es cuando el agente no está | Alta |
| **PA-82** | El socket se crea en `0600`. RPT-002 §9.3 autorizó «socket Unix **con ACL**», y `0600` no es una ACL: obliga a que consola y agente corran como el mismo usuario | Alta |
| **PA-83** | La latencia de atención está acotada por la vuelta y no se ha medido | Media |
| ~~PA-78~~ | ✅ **Cerrado por observación**, no por construcción | — |

---

*Reporte Nº 46 — Instanciación de Electron · PremosCorp · 9 de agosto de 2026*
