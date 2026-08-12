# RPT-048 — VIS-04: qué ve el operador y qué no debe ver

**Tema:** Diseño del tablero de vigilancia, con cuatro canales que todavía no sirve nadie
**Nº de reporte:** 048
**Fecha:** 12 de agosto de 2026
**Área designada:** Interfaz
**Entidad:** PremosCorp
**Estado:** **Diseño. Pendiente de ratificación.**

- **Depende de:** RPT-004 (puente y ventana), RPT-019 (sucesos y condiciones), RPT-036 (rechazo con motivo), RPT-046, RPT-047
- **Aborda:** VIS-04

---

## 1. La restricción que define la pantalla

De los seis canales, **dos responden y cuatro rechazan con motivo**:

| Canal | Hoy |
|---|---|
| `obtener-condiciones` | Responde |
| `consultar-alertas` | Responde |
| `obtener-inventario` | «declarado y aún no tiene manejador» |
| `obtener-estado-boveda` | Ídem |
| `obtener-estado-agente` | Ídem |
| `consultar-sandbox` | Ídem |

Eso no es un impedimento para diseñar: **es el requisito más importante**.

Una interfaz que pinte inventario vacío donde hay un rechazo le está diciendo al
operador «no hay dispositivos en este segmento». Es mentira, y es la mentira
concreta que RPT-036 §6 existe para impedir: «no hay nada» y «esto no lo sirve
nadie todavía» no son lo mismo.

**Cada panel debe tener tres estados visuales, no dos**: con datos, vacío de
verdad, y no servido — este último **mostrando el motivo que llegó por el cable**.
Es el tercer estado de RPT-006 §4 llevado a la pantalla, y si no se construye
ahora habrá que reconstruir todo cuando lleguen los manejadores.

## 2. Qué se ve primero

El orden no es estético. Un panel de vigilancia se mira dos segundos, y lo que
esté arriba es lo único que se lee de verdad.

1. **`capturaNoDisponible`**, sola y ocupando el ancho. Mientras esté activa,
   **todo lo demás de la pantalla es de antes**. Un tablero que la muestre como
   una fila más entre nueve invita a leer el resto como si fuera actual.
2. **Manipulación** — `inventarioSuprimido`, `inventarioNoVerifica`. Alguien tocó
   el almacén.
3. **Sucesos** — amenazas incontenibles desde ALM-01, lo más reciente arriba.
4. **Las demás condiciones**, agrupadas y sin ordenar por gravedad inventada.

## 3. Tres cosas que la interfaz nunca debe hacer

**Presentar `accionAdministrativa` como manipulación.** «Hay que reemitir el
inventario» y «alguien borró el inventario» se arreglan de forma opuesta.
`Condiciones::hay_manipulacion()` ya separa las dos familias y la pantalla debe
respetar esa separación, no reinventarla comparando cadenas.

**Pintar un campo ausente como `false`.** El puesto de diagnóstico distingue
`undefined` de `false` y esa decisión de una línea es la que demostró, el día que
añadimos la novena condición, que el contrato había cruzado entero. Un panel que
las colapse dice «todo bien» exactamente igual cuando el agente dejó de mandar un
campo.

**Ofrecer contención.** RPT-004 §6.2 lo prohíbe y hay pruebas que lo verifican en
los dos lenguajes. La interfaz observa; la contención se decide en `guardian-cc`
con telemetría real. Un botón aquí trasladaría al operador una decisión que el
producto toma con más información que él.

## 4. El histórico archivado no es ausencia

`consultar-alertas` devuelve `{ primerDisponible, sucesos }` (PA-74). Si
`primerDisponible > 1`, hay asientos anteriores **archivados en disco** y esta
respuesta no los incluye.

La pantalla debe decirlo con esas palabras. Presentar la lista sin más es cómo un
operador concluye que un incidente no ocurrió, y es la razón entera por la que
ese envoltorio existe.

## 5. Fatiga de alertas

RPT-019 §2 separó sucesos de condiciones para no inundar ALM-01 con la misma
noticia. La interfaz puede deshacer ese trabajo en un día: un aviso que aparece
cada dos segundos deja de leerse en una semana.

Las condiciones **son estados, no notificaciones**. Se muestran mientras duran y
no se anuncian de nuevo en cada refresco. Sólo la **transición** merece llamar la
atención, y es la misma regla que `EMISIBLES` aplica al syslog.

## 6. Lo que hace falta decidir y no decido yo

**La cadencia de refresco.** El puesto de diagnóstico consulta cada dos segundos
porque era cómodo. El agente atiende al final de cada vuelta (~500 ms), así que
refrescar más a menudo no aporta datos nuevos — sólo latencia percibida (PA-83,
sin medir). Y en un panel que alguien deja abierto todo el día, dos segundos son
43 000 consultas por jornada.

**Si VIS-04 debe funcionar sin agente.** Hoy la pantalla sin agente es una lista
de errores. Un tablero de vigilancia que no puede decir «no encuentro al sensor»
de forma legible para alguien que no sabe qué es un socket no está terminado.

## 7. Lo que propongo ratificar

1. Tres estados por panel —datos, vacío, no servido con motivo— desde la primera
   línea de código (§1).
2. El orden del §2, con `capturaNoDisponible` fuera de la lista.
3. Las tres prohibiciones del §3, con prueba para la tercera.
4. `primerDisponible > 1` presentado como histórico archivado (§4).
5. Condiciones como estado y no como notificación (§5).
6. El §6 abierto y decidido por producto.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| **PA-92** | Cadencia de refresco de VIS-04, sin medir (relacionado con PA-83) |
| **PA-93** | Qué muestra VIS-04 cuando no encuentra al agente, para alguien que no es técnico |

---

*Reporte Nº 48 — Diseño de VIS-04 · PremosCorp · 12 de agosto de 2026*
