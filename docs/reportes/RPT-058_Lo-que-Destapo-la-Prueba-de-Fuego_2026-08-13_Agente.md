# RPT-058 — Lo que destapó la prueba de fuego

**Tema:** El sensor se llamaba `lo`. Identidad, el séptimo sitio del contrato, y `enumerar()`
**Nº de reporte:** 058
**Fecha:** 13 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Corregido y verificado.** Cierra PA-114. Abre PA-113

- **Depende de:** RPT-057 (el vigía), RPT-055 (la décima condición), RPT-038 (el testigo)
- **Aborda:** PA-114

---

## 1. Tres defectos que ninguna prueba vio

Los tres estaban en verde. Los tres aparecieron al ejecutar el sistema completo
contra un colector real, y **ninguno lo habría encontrado una prueba unitaria**,
porque cada pieza aislada hacía exactamente lo que decía.

## 2. El sensor se identificaba con el nombre de la interfaz

```
LINEA BASE  lo: primer latido (numero 1).
NUNCA VISTO  LapTap-AF: esta en el censo y no ha dicho nada.
```

El `Emisor` se construía con `&opciones.interfaz`, así que el campo `HOSTNAME` de
RFC 5424 —**la identidad del sensor en la sala**— llevaba `lo`.

La consecuencia no es cosmética:

- **Dos sensores de dos hospitales distintos con la interfaz `eth0` son el mismo
  sensor para el vigía, y el latido de uno tapa la muerte del otro.** Es
  exactamente el fallo que PA-104 existe para impedir, introducido por el campo
  que lo identifica.
- Afecta también a los sellos de RPT-038: las series de dos máquinas se
  entrelazarían en una, y la detección de manipulación cotejaría extremos de
  sensores distintos.

**Y la ejecución parecía correcta.** `AUSENTE` a los 34 segundos, `VUELVE`,
`REVISAR` con el contador de 65 a 1 — todo lo esperado. El único síntoma era un
`NUNCA VISTO` que se leía como «el censo funciona».

La identidad sale ahora de `/proc/sys/kernel/hostname`, con `--nombre` para
forzarla. Si no se puede leer **no se sustituye por algo plausible**: sale
`SIN-NOMBRE-DE-MAQUINA` con tres líneas de aviso en el arranque. Caer hacia un
nombre que parece un nombre es cómo se llegó aquí.

Queda abierto que **dos agentes en la misma máquina** —un sensor por segmento—
volverían a compartir identidad. La clave correcta es máquina + interfaz. Es
**PA-113**, y se anota en vez de arreglarse a medias.

## 3. El séptimo sitio: el resumen del propio agente

La consola imprimía **siete de diez** condiciones. Faltaban `capturaNoDisponible`
—la más grave, la que RPT-047 existe para cubrir—, `evidenciaEnRiesgo` y
`sinColector`.

Las seis superficies del contrato estaban inventariadas y probadas. Ésta no
estaba en la lista de nadie: es el texto que lee la persona que está delante del
sensor, y un resumen que omite una condición activa dice que todo va bien
exactamente igual que uno que la muestra apagada.

## 4. `enumerar()`: los diez nombres en un sitio

La corrección no fue añadir tres líneas al `println!`. Eso habría dejado el
octavo sitio esperando.

`Condiciones::enumerar()` devuelve los diez pares `(identificador, valor)` con
desestructuración **exhaustiva y sin `..`**. De ahí se derivan ahora:

- el resumen por pantalla del agente,
- `valor_de` en `salida.rs`, que tenía su propio `match` — y ese `match` ya se
  había quedado dos condiciones por detrás del contrato en PA-91,
- la barrera de PA-91, **que hasta hoy repetía a mano la lista que vigilaba**.

Esto último es lo que más importa. Una barrera escrita por la misma mano que la
cosa vigilada comprueba que dos copias de la misma equivocación coinciden.

## 5. `VUELVE` donde correspondía `APARECE`

El vigía juntaba en una sola lista a los ausentes y a los que nunca habían
hablado. Al aparecer el primer latido de un sensor del censo, imprimía:

```
VUELVE  LapTap-AF: vuelve a latir.
```

**No volvió: apareció.** Volver de una ausencia y aparecer por primera vez tienen
la misma forma —estaba en la lista de los que faltan y ya no está— y son dos
noticias distintas: una dice que algo se recuperó, la otra que una instalación
terminó. El mismo colapso de estados de siempre, esta vez en la frase que lee la
sala.

## 6. Lo que esto dice del método

Tres defectos, tres formas del mismo error: **una diferencia real presentada como
si no lo fuera**. La interfaz por la máquina, siete condiciones por diez, aparecer
por volver.

Y una lección sobre las pruebas: las tres piezas estaban probadas. Lo que no
estaba probado era el sistema hablando consigo mismo, y por eso RPT-052 §6 exigió
cerrar PA-104 **por observación** en lugar de por implementación. Esa exigencia,
escrita antes de tener nada, es lo que hizo que se ejecutara la prueba de fuego —
y la prueba de fuego es lo que encontró esto.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-114~~ | ✅ `enumerar()` es el único sitio donde se escriben los diez nombres |
| ~~PA-113~~ | ✅ Identidad compuesta `(máquina, interfaz)`, RPT-059 |
| PA-79 | El nombre del sensor es el segundo parámetro que pide configuración firmada, después del intervalo |
| PA-103 | La rama `noServido` del panel sigue sin ejecutarse nunca |

---

*Reporte Nº 58 — Lo que destapó la prueba de fuego · PremosCorp · 13 de agosto de 2026*
