# RPT-065 — Manual de comandos

**Tema:** Glosario ejecutable del proceso de creación
**Nº de reporte:** 065
**Fecha:** 14 de agosto de 2026
**Área designada:** Método
**Entidad:** PremosCorp
**Estado:** Emitido. Abre PA-119

- **Depende de:** RPT-003 §9.4 y §9.5 (las verificaciones y por qué son `xtask`), RPT-006 §4 (tres estados), RPT-060 (el tablero abandonado), RPT-064 §6 (evidencia del proceso equivocado)
- **Aborda:** petición del equipo de pruebas, 14-ago-2026
- **Acuña:** PA-119

---

## 1. Qué se pidió y qué se entregó

El equipo de pruebas pidió *«un estilo de glosario o un instructivo de todos los
comandos usados y por usar y para qué sirven en todo el entorno del proceso de
creación»*.

Se entrega [`docs/Comandos.md`](../Comandos.md): trece secciones, ordenadas por
fase —entorno, compilación, verificaciones, `xtask`, interfaz, ejecución,
empaquetado, observación, pruebas de fuego pendientes— y no por herramienta.

**Se ordenó por fase a propósito.** Una lista alfabética de comandos responde
«¿qué hace esto?», que es la pregunta fácil. La difícil es «¿qué toca ahora?», y
sólo la contesta el orden del proceso.

## 2. Cada comando lleva lo que no puede afirmar

Es la única decisión de forma que merece explicación. La tabla de §3 del manual
tiene tres columnas y la tercera es **«qué no afirma»**:

| Comando | Qué afirma | Qué **no** afirma |
|---|---|---|
| `cargo test --workspace` | Pasan las pruebas registradas | Que estén todas registradas |
| `cargo xtask verificar` | No hay marcadores de implementación inconclusa | Que lo implementado sea correcto |
| `cargo deny check` | Ninguna dependencia contamina la frontera | Nada sobre el código propio |

Sin esa columna, un manual de comandos es una lista de cosas que salen en verde,
y este proyecto lleva dos días encontrando defectos **debajo** de verdes
legítimos: la suite seguía en verde con dos pruebas menos (PA-73), la frontera de
licencia seguía en verde sin que nadie supiera si la barrera funcionaba
(`verificar:frontera:negativa`), y el tablero seguía contando 76 de 115 y
presentándolo como el total (PA-108).

## 3. Los tres códigos de salida quedan escritos

El manual dice en §4.2 que `probar-instalador` devuelve **3** —ni 0 ni 1— cuando
no puede comprobar. Queda escrito porque es la información que se pierde primero:
quien automatice esto en un guion escribirá `if [ $? -ne 0 ]` y convertirá «no se
sabe» en «falló», o —peor— `|| true` y lo convertirá en «pasó».

Es RPT-006 §4 aplicado a un valor de salida de shell.

## 4. La higiene de observación es parte del manual, no un anexo

§10 recoge las tres formas en que se leyó evidencia del proceso equivocado en
media hora (RPT-064 §6) y la regla de la predicción escrita.

Van en el manual de comandos y no en un reporte aparte porque **no son lecciones,
son argumentos de línea de órdenes**: `&&` en lugar de `;`, `--no-pager` siempre,
y comprobar el PID antes de leer un diario. Una lección se lee una vez; un
argumento se teclea.

## 5. Lo que el manual admite de sí mismo

§13 lo dice sin adornos: es una lista escrita a mano de comandos que viven en
otros ficheros.

Esa es **exactamente** la forma del defecto de RPT-060. El tablero era un índice
que nadie derivaba, se quedó atrás y siguió pareciendo el total. Un manual de
comandos tiene la misma anatomía: `xtask` puede ganar una orden mañana, `verificar`
puede dejar de encadenar `probar`, y el documento seguirá leyéndose como completo.

La diferencia con el tablero es que aquí se dice **antes** de que ocurra, y en el
propio documento, no en un reporte que quien lee el manual nunca abrirá.

## 6. PA-119 — paridad entre el manual y lo que existe

Se acuña el punto en lugar de resolverlo hoy, porque tiene la forma de las
barreras que ya funcionan y merece construirse con ellas delante, no deprisa.

Lo que tendría que probar, en las dos direcciones:

- Toda orden que `xtask` despacha aparece en `docs/Comandos.md` §4.1.
- Todo `cargo xtask <orden>` citado en `docs/` es una orden que `xtask` acepta.

La segunda dirección es la que caza el caso peor: un comando **documentado que ya
no existe** manda a alguien a teclear algo que falla, y lo hará justamente en la
sesión en la que menos tiempo hay para averiguar por qué.

Es la misma desigualdad de `cobertura` y la misma barrera de PA-108, aplicadas a
un tercer índice. Tres veces el mismo patrón en una semana sugiere que lo general
no es el tablero ni el manual: es que **todo índice escrito a mano de cosas que
viven en el código necesita un lector que lo derive**.

## 7. Lo que el manual dejó anotado y no estaba en ningún sitio

Al recorrer el árbol para escribirlo aparecieron dos cosas que ningún reporte
recogía:

- **`--grupo-ipc` toma un número y no un nombre** (`$(id -g)`, no `$(id -gn)`).
  Estaba implícito en PA-84, que promete aceptar el nombre, pero no escrito como
  advertencia de uso en ninguna parte.
- **`cargo build --offline` no se usa** porque oculta que falta una dependencia
  hasta el día del despliegue. Era criterio tácito.

Ninguna de las dos es un hallazgo. Se anotan porque escribir el manual las hizo
visibles, y ese es el argumento de por qué escribirlo valía la pena aunque nadie
lo hubiera pedido.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| PA-119 | Paridad entre `docs/Comandos.md` y las órdenes que `xtask` acepta (§6) |
| PA-117 | Sus dos comprobaciones quedan escritas en §9 del manual, listas para ejecutar |
| PA-84 | `--grupo-ipc` aceptaría un nombre de grupo y no un número |

---

*Reporte Nº 65 — Manual de comandos · PremosCorp · 14 de agosto de 2026*
