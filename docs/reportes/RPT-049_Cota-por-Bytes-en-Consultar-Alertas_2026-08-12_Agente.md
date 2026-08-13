# RPT-049 — La cota que contaba elementos y debía contar bytes

**Tema:** Un canal de alertas que quedaba inservible para siempre, y la prueba que decía cubrirlo
**Nº de reporte:** 049
**Fecha:** 12 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado por observación.** Cierra PA-95, PA-96 y la deuda de RPT-046 §11.1

---

## 1. El defecto

`consultar-alertas` acotaba la respuesta a 256 asientos. No miraba cuántos bytes
ocupaban.

Con detalles largos, esos 256 pasan del millón de bytes que `LONGITUD_MAXIMA_MARCO`
permite, y `enmarcar` rechaza la respuesta **entera**. Y como el cliente vuelve a
pedir desde el mismo asiento, el rechazo se repite en cada consulta: el canal de
alertas queda **inservible de forma permanente**, sin degradarse ni avisar.

Observado en campo con `cargo xtask sembrar`:

```text
← 49 bytes: carga de 1071690 bytes; el maximo es 1048576
```

Las alertas existían, verificaban y estaban en disco. Nadie podía llegar a ellas.

No hace falta una siembra para provocarlo: basta un detalle largo de verdad —una
ruta, un mensaje de fabricante, un nombre de dispositivo— repetido unas cuantas
veces.

## 2. La prueba que decía cubrirlo

Existía `una_consulta_no_devuelve_mas_de_lo_que_cabe_en_un_marco`.

**Contaba elementos.** Su nombre afirmaba exactamente la propiedad que el agente
incumplía, y por eso nadie la volvió a mirar durante meses. Es la clase de prueba
más peligrosa que hay: la que hace creer que algo está cubierto porque se llama
como si lo estuviera.

Se renombra a `una_consulta_no_devuelve_mas_asientos_de_los_que_marca_la_cota`,
que es lo que sí comprueba, y se añade
`una_respuesta_con_detalles_largos_sigue_cabiendo_en_un_marco`, que **compone y
enmarca la respuesta de verdad**. Contar elementos no habría detectado nada.

## 3. `hayMas` — acotar sin decirlo es peor

`primerDisponible` cubría el lado antiguo: «lo de antes está archivado». Nadie
cubría el nuevo. Quien recibía 256 sucesos no tenía forma de saber si eran todos
o el principio de dos mil, salvo adivinar por el tamaño del lote.

Y arreglar el §1 sin esto habría cambiado un rechazo ruidoso por **una lista
silenciosamente incompleta**, que es peor: el operador la lee como el histórico
entero.

Novena entrada del contrato, en sus tres sitios. `componerSucesos` de RPT-048 §4
tiene que consumirla: hoy presentaría 256 de 2000 como si fueran todo.

## 4. Dos decisiones sobre la cota

**El margen es holgado a propósito.** 8 KB reservados para un envoltorio que
ocupa cien bytes. Los dos errores no cuestan lo mismo: quedarse corto devuelve el
canal a inservible; pasarse entrega unas alertas menos y `hayMas` lo dice.

Medido después: la respuesta real ocupó **1 038 209** bytes sobre un techo de
1 048 576 — **10 367 de holgura**. La constante deja de ser una corazonada.

**Se mide serializando**, no estimando. Una cuenta por longitud de cadena se
equivoca con acentos, comillas y escapes, y fallaría justo con los detalles
raros, que son los que importan.

## 5. La fragmentación, por fin observada

```text
→ 41 bytes
← 65536 ← 65536 ← 65536 ← 59200 ← 65536 … ← 1093     (17 trozos)
Clase : respuesta
```

Prefijo declarado `0x000fd781` = 1 038 209. Los diecisiete trozos suman
1 038 213 = 4 + 1 038 209.

El acumulador de RPT-045 —la pieza que más preocupaba al escribir el cliente,
con sus pruebas del prefijo partido por la mitad y el marco byte a byte— llevaba
semanas verde **por construcción**. Hoy trabajó: reensambló un marco de un
megabyte desde pedazos de 64 KB que el núcleo entregó como quiso.

Cierra RPT-046 §11.1.

## 6. Cómo apareció

No lo encontró una prueba. Apareció **fabricando datos para probar otra cosa**:
la herramienta de siembra existía para ejercitar la fragmentación, y de camino
destapó un defecto que dejaba un canal muerto.

Es el mismo patrón de toda la semana: los hallazgos salen al mover cosas y al
usarlas, no al leerlas. Y dos veces hoy el sistema tenía razón contra nosotros
—el registro sembrado sin ancla apartado como manipulación, y este rechazo— antes
de que nosotros la tuviéramos contra el sistema.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-95~~ | ✅ Absorbido por PA-96: `hayMas` |
| ~~PA-96~~ | ✅ Cerrado, verificado por observación |
| **PA-97** | `componerSucesos` (RPT-048 §4) no lee `hayMas` todavía |
| PA-83 | La latencia sigue sin medir, y ahora hay respuestas de un megabyte |

---

*Reporte Nº 49 — La cota que contaba elementos · PremosCorp · 12 de agosto de 2026*
