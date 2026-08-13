# RPT-051 — Dónde vive la consola

**Tema:** PA-77. Qué cuesta cada opción de despliegue, con lo medido en dos semanas
**Nº de reporte:** 051
**Fecha:** 13 de agosto de 2026
**Área designada:** Producto
**Entidad:** PremosCorp
**Estado:** **Documento de decisión. No recomienda: expone.**

- **Depende de:** RPT-002 §9.3 (transporte), RPT-046 (Electron), RPT-047 (degradación), RPT-050 (latencia)
- **Aborda:** PA-77. Condiciona PA-79, PA-84 y el empaquetado

---

## 0. Qué es esto y qué no

Este reporte **no propone una opción**. La decisión es de producto y de seguridad,
no de ingeniería, y quien la tome debe poder hacerlo con las cifras delante.

Lo que aporta es que ya no hay que imaginar nada: durante dos semanas se midió
qué exige la ventana, cuánto cuesta una consulta, qué pasa cuando el sensor no
puede capturar y cómo se comporta el socket con dos usuarios distintos.

## 1. Lo que hoy es cierto, medido

| Hecho | Fuente |
|---|---|
| Electron exige NSS, CUPS, ALSA, GBM y una docena más — cientos de MB | RPT-046, observado al instalar en WSL |
| El agente captura tramas: corre como **root** | `CAP_NET_RAW`, observado |
| La consola **no** debe correr como root; el socket lo permite con grupo `0660` | PA-82, verificado |
| Una consulta en régimen cuesta 55 bytes y ~500 ms, dominados por el ciclo | RPT-050 §8 |
| Sin captura, el agente sigue sirviendo el porqué por el socket | RPT-047, verificado |
| **No existe transporte de red.** RPT-002 §9.3 eliminó el WebSocket local | Decisión ratificada |

## 2. Las cuatro opciones

### A — Consola en el sensor

El operador se sienta en el equipo que vigila, o entra por escritorio remoto.

**Lo que cuesta:** varios cientos de megas de bibliotecas gráficas en una máquina
cuyo trabajo es mirar una red hospitalaria. Cada una es superficie que alguien
tendrá que justificar en una auditoría, y ninguna tiene que ver con vigilar.

**Lo que compra:** funciona hoy. El socket local, el grupo `0660` y la
degradación de PA-81 están construidos y verificados para exactamente esto.

### B — Consola remota hablando con el sensor

La consola en el puesto del operador; el agente expone su socket por red.

**Lo que cuesta, y es más de lo que parece:** RPT-002 §9.3 eliminó el WebSocket
local **por diseño**, porque un servicio alcanzable desde otra máquina es otro
modelo de amenaza. Esta opción exige autenticación, cifrado de transporte e
identidad de cliente — **ninguna de las tres existe**, y las tres son trabajo de
semanas, no de días.

Además reabre una decisión ratificada. Si se elige, hay que decir explícitamente
que RPT-002 §9.3 se revisa, no darlo por hecho.

**Lo que compra:** un operador vigilando varios sensores desde su sitio.

### C — Consola remota leyendo del colector

La consola no habla con el sensor: lee del SIEM al que el agente ya emite por
syslog.

**Lo que cuesta:** VIS-04 cambia de fuente de datos. `consultar-alertas`,
`primerDisponible`, `hayMas`, la bitácora con cursor — todo eso es la forma del
socket, no la del colector. Se conserva la lógica de presentación; se rehace el
suministro.

Y hay un límite duro: **`salidaNoDisponible` no viaja por syslog**, porque
emitirla exigiría el canal que acaba de fallar. Una consola que sólo lee del
colector **no puede saber que el colector está caído**. Es el punto ciego exacto
que esa condición existe para cubrir.

**Lo que compra:** ni una biblioteca gráfica en el sensor, ni un puerto abierto.

### D — Las dos cosas

Sensor headless por omisión; consola local instalable aparte para diagnóstico en
sitio; consola de operación leyendo del colector.

**Lo que cuesta:** dos productos que mantener y dos caminos de datos que probar.
El coste no es el código —la lógica ya está separada— sino que **todo lo que se
construya hay que verificarlo dos veces**, y este proyecto ha aprendido lo que
cuesta lo que no se ejercita.

## 3. La pregunta que decide, y no es técnica

**¿Quién mira el tablero, y desde dónde?**

Un técnico que va a la planta y necesita saber por qué el sensor no captura
quiere A. Un operador de seguridad vigilando quince hospitales desde una sala
quiere C. Los dos existen y no son el mismo producto.

Todo lo demás —el empaquetado, la configuración firmada, el grupo del socket—
se deduce de esa respuesta. Ninguno se puede decidir antes.

## 4. Lo que este reporte pide

No una opción. **Un enunciado de quién es el usuario de VIS-04**, con nombre y
sitio. Con eso, PA-79, PA-84 y el empaquetado dejan de estar bloqueados.

Y si la respuesta es «los dos», que se diga como decisión y no como aplazamiento:
la opción D es legítima y cuesta el doble de verificación, que es un precio que
alguien tiene que aceptar a sabiendas.

## 5. Lo que no cambia, se elija lo que se elija

La capa base —máquina de estados, cabecera, sucesos, bitácora, traducción de
fallos— es lógica pura y **no depende del transporte**. Está probada sin ventana,
sin agente y sin escritorio.

Esa separación no se hizo previendo esta decisión, pero es lo que permite tomarla
tarde sin tirar nada.

---

*Reporte Nº 51 — Dónde vive la consola · PremosCorp · 13 de agosto de 2026*
