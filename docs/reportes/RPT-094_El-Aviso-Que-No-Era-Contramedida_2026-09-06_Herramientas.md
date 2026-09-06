# RPT-094 — El aviso que no era contramedida

**Tema:** PA-53. La frase de paso deja de verse, y el tercer estado que impide mentir sobre ello
**Nº de reporte:** 094
**Fecha:** 6 de septiembre de 2026
**Área designada:** Herramientas
**Entidad:** PremosCorp
**Estado:** **Cerrado.** Rojo del tablero retirado

- **Depende de:** RPT-026 §5 (donde nació), RPT-092 (donde costó una credencial), RPT-082 (PA-134, la lección de testabilidad que se reutiliza aquí), RPT-018 §2 (el patrón de `unsafe` acotado)
- **Aborda:** PA-53 (cerrado)

---

## 1. Treinta y nueve días avisando de lo mismo

PA-53 se abrió el 6 de agosto con RPT-026. Durante treinta y nueve días la herramienta
imprimió, cada vez que pidió una frase:

```
AVISO: se vera al teclearla; no la use delante de nadie (PA-53).
```

El 31 de agosto ese aviso se imprimió, se leyó, se entendió, y **la frase se tecleó
igual** — porque no había otra forma de darla. Quedó en pantalla, en el historial y en el
registro de la sesión, y hubo que darla por comprometida y rotar la identidad entera del
sensor de la VM de PA-78 (RPT-092).

La conclusión que cierra este punto no es técnica: **un aviso que nombra un peligro sin
ofrecer salida no reduce el riesgo. Traslada la culpa del diseño a quien lo usa.**

## 2. Lo entregado

`crates/eje-manifiesto/src/eco.rs`. `apagar()` quita la bandera `ECHO` de la entrada
estándar con `termios` y devuelve un `Guardia` que la restaura en `Drop`.

**`Drop`, y no una llamada al final.** Una llamada al final no ocurre si se sale antes por
error, y una terminal que se queda muda tras un fallo es un incidente peor que el eco: el
operador no ve lo que escribe y no sabe por qué. Con `Drop`, la restauración la garantiza
el compilador y no la memoria de quien escribe la siguiente función.

Y `TCSAFLUSH` en lugar de `TCSANOW`: descarta lo ya tecleado antes del cambio, para que
nada escrito antes del aviso entre en la frase. Es el segundo filo de PA-134, ahora cerrado
por el otro lado.

## 3. Tres estados, y el tercero es el que importa

Lo natural era escribir dos: apagado o no. Con dos, un descriptor que no es terminal cae en
«no apagado» junto a una terminal donde el apagado falló, y la herramienta acaba diciendo
«no se verá» sobre una pantalla donde sí se ve.

**Eso sería peor que el defecto que este reporte arregla.** Hoy quien teclea sabe que se le
ve. Con dos estados creería que no.

| Estado | Qué imprime `pedir_frase` |
|---|---|
| `Apagado` | «No se verá al teclearla» |
| `NoEsTerminal` | Nada. Es un guion, no hay pantalla, y el ruido que siempre aparece deja de leerse |
| `NoSePudoApagar` | El aviso de siempre, ampliado con el motivo |

RPT-006 §4 otra vez, y van muchas. Empieza a no ser una regla que se aplica: es la forma
que tiene el dominio.

## 4. Coste de suministro: cero

`libc` ya era dependencia directa de `eje-captura` y `Cargo.lock` ya la resolvía a
`0.2.189`. Añadirla a `eje-manifiesto` **no mete un solo paquete nuevo en el árbol**, no
toca `CONFORMIDAD.lock` —que sigue el subárbol de `motor-pqc`— y no amplía la superficie
auditada.

Se comprobó antes de proponerlo, no después. Es la diferencia entre «creo que es barato» y
«es barato».

## 5. `unsafe`, y una frase que dejó de ser cierta

`eje-captura::linux` declaraba en su cabecera ser **«el único módulo del workspace donde se
admite `unsafe`»**, con una lista de siete llamadas enumeradas a propósito.

Al añadir el segundo, esa frase pasaba a ser falsa. Se corrigió en el mismo cambio, y el
módulo nuevo lleva su propia lista de tres —`isatty`, `tcgetattr`, `tcsetattr`— por el mismo
motivo por el que la lleva el otro: **para que ampliarla cueste una revisión.**

Es exactamente el defecto que este proyecto lleva noventa reportes cazando —prosa que
describe un estado que ya no existe—, y esta vez se vio al escribirlo en lugar de un mes
después.

## 6. Lo que se puede probar y lo que no

No se puede observar el apagado real sin una terminal, y el arnés hereda una tty al correr
las pruebas a mano y no la hereda en integración continua. Una prueba sobre `stdin` daría
resultados distintos según quién la ejecute: eso no comprueba nada, informa del sitio.

La salida es la misma que dio RPT-082 a `leer_frase`: **parametrizar**. `apagar_en(descriptor)`
permite probar la rama de «esto no es una terminal» de forma determinista, sobre `/dev/null`
y sobre un descriptor inválido. Tres pruebas nuevas, `eje-manifiesto` pasa de 39 a 42.

Lo que queda sin cubrir se dice en voz alta: **que el eco vuelva de verdad tras un error no
tiene prueba automática.** Sujetarlo exigiría un pseudoterminal y más `unsafe` del que el
arreglo entero tiene. Lo sujeta el tipo —`Drop` no es opcional— y eso se considera
suficiente aquí, pero no se presenta como una prueba que no existe.

## 7. Un error de edición, dos veces el mismo día

Al marcar PA-78 y PA-53 como cerrados partí cada fila en dos —la nueva y una «(histórico)»—
para conservar el texto anterior. Eso **mete un identificador duplicado en el tablero**,
que `identificador_de` extrae igual y que sólo no rompió la cuenta porque
`un_identificador_repetido_solo_cuenta_una_vez` existe desde hace semanas.

Las dos veces lo vi al releer, no al escribir. Se anota porque la barrera que lo habría
cazado antes que yo —una que exija una fila por identificador— **no existe**, y hoy el
trabajo lo hizo una prueba escrita para otra cosa.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| PA-53 | **Cerrado.** §2 y §3 |
| PA-146 | **Abierto, crítico.** El rojo que queda: la rotación de identidad sigue sin procedimiento |
| PA-145 | `--anterior` inexistente rebobina la serie en silencio |
| PA-147 | La plantilla del instalador contradice al código |
| PA-148 | La cabecera de VIS-04 no dice qué sensor es |

---

*Reporte Nº 94 — El aviso que no era contramedida · PremosCorp · 6 de septiembre de 2026*
