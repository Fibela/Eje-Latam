# RPT-032 — Emisión por syslog

**Tema:** Que la alerta llegue al SIEM del cliente
**Nº de reporte:** 032
**Fecha:** 6 de agosto de 2026
**Área designada:** Comunicación
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-42

- **Depende de:** RPT-031 (diseño ratificado), RPT-028 (manejadores)
- **Cierra:** PA-42
- **Amplía:** `Condiciones` de cinco campos a seis

---

## 1. El marco cuenta octetos, y eso cierra una inyección

RFC 6587 admite dos formas de delimitar mensajes de syslog sobre TCP: contar octetos o terminar en salto de línea. Aquí se cuenta, y no por elegancia.

Con delimitación por salto de línea, **un salto dentro del mensaje inyecta una entrada de syslog completa**. Quien controle cualquier texto que acabe en el mensaje podría escribir entradas falsas en el SIEM del cliente, **atribuidas al agente**. Hoy ese texto lo escribe sólo nuestro código; mañana podría incluir el nombre de un dispositivo.

Contar octetos elimina la clase entera. Y aun así se sanean los caracteres de control, porque una defensa que depende de que nadie cambie el marco es una defensa que caduca. Hay una prueba para cada mitad.

## 2. La marca de tiempo es la del suceso

RFC 5424 admite `-` en el campo de tiempo, y el colector estampa entonces su hora de recepción. Se rechaza: **se perdería cuándo ocurrió**, que es el dato que importa cuando la alerta se investiga días después.

Eso obliga a formatear una fecha, y se hace a mano con el algoritmo civil-from-days en lugar de traer una dependencia. Veinte líneas frente a superficie añadida a un binario que corre con privilegios de captura. Está probado contra fechas conocidas —epoch, un 29 de febrero, un instante anterior a la epoca— porque el cálculo ingenuo falla exactamente ahí.

Un reloj anterior a 1970 produce una fecha correcta y absurda. Si aparece, el reloj del sensor está mal y **eso es en sí mismo una noticia**; lo que no puede es entrar en pánico.

## 3. Se emite la transición, no la condición

Las condiciones son verdaderas hasta que alguien interviene. Emitirlas en cada ciclo inundaría el SIEM con la misma noticia, que es el defecto que RPT-019 §2 evitó al no anexarlas a ALM-01.

`anterior = None` es el primer ciclo: sale lo que esté **activo**, y nada de lo apagado. Emitir «todo normal» al arrancar sería ruido puro.

Y la vuelta a la normalidad **sí se emite**, como informativa. Un operador que vio una alerta necesita saber que se resolvió; si no, la única forma de enterarse es preguntar.

## 4. La condición que no se puede emitir

`Condiciones` gana `salidaNoDisponible`, declarado en los seis sitios. Y es **la única que no viaja por syslog**: emitirla exigiría el canal que acaba de fallar.

Si estuviera en la lista de emisibles, el agente intentaría enviar por un socket roto la noticia de que el socket está roto. Hay una prueba que lo fija, `la_condicion_de_salida_caida_no_se_emite_por_la_salida`, porque es el tipo de cosa que alguien «arregla» añadiendo el campo que falta.

Viaja sólo por IPC. VIS-04 es lo único que puede saber que el SIEM no se está enterando de nada.

### 4.1 El orden es circular y se resuelve rellenando después

`condiciones()` devuelve `salida_no_disponible: false` y quien llama lo fija tras emitir. Emitir necesita las condiciones y la condición necesita el resultado de emitir; pedirlo a la función sería circular.

Que ese campo no se emita nunca es lo que hace el orden seguro: las transiciones que se calculan no dependen del valor que aún no se conoce.

## 5. El estado anterior se actualiza aunque el envío falle

Si no, al recuperarse el colector se reemitirían transiciones ya pasadas como si fueran nuevas, y **el operador vería un incidente que no ocurrió**.

La contrapartida es que una transición perdida durante la caída no se recupera. Es aceptable: la alerta sigue en ALM-01 y el canal de consulta la sirve. La alternativa —una cola de pendientes— es el agotamiento de memoria de RPT-018 §6 con otro nombre.

## 6. El despacho sólo emite, literalmente

`DespachoTcp` no tiene método que lea, del mismo modo que `eje-captura` no tiene método que envíe. RPT-031 §2 exige que la interfaz de gestión sea de emisión pura, y aquí eso es una propiedad del tipo y no una convención.

La conexión se abre en cada envío y se cierra al soltarse. Mantenerla dejaría un descriptor vivo hacia la red de gestión entre alerta y alerta —horas o días— y para un volumen de alertas raras y graves el coste de reconectar es irrelevante.

El plazo de escritura es tan necesario como el de conexión: **un colector que acepta y no lee dejaría al agente bloqueado indefinidamente**, y con él la observación detenida. Tres segundos.

## 7. Lo que sigue sin resolverse

1. **Sin `--syslog` no sale nada, y se dice.** Un agente que no emite y no lo anuncia parece uno que emite y nadie escucha.
2. **PA-61 sigue 🔴.** Todo esto no sirve en un sensor con una sola tarjeta de red: la que vigila suele ser receive-only.
3. **TLS** (PA-62) y **autenticidad de la línea** (PA-63) siguen abiertos. Quien esté en la red de gestión puede inyectar alertas falsas al SIEM haciéndose pasar por el agente.
4. **El agente sigue siendo un recorrido.** Emite una vez, al final. En un servicio la transición se detectaría en cuanto ocurre; hoy se detecta al terminar (PA-41).
5. **No hay reintento.** Se intenta una vez por ciclo. Con un recorrido efímero eso es todo el reintento que cabe; con un servicio habrá que decidir la cadencia.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| PA-42 | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 32 — Emisión por syslog · PremosCorp · 6 de agosto de 2026*
