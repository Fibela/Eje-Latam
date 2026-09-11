# RPT-097 — Una regla en dos idiomas

**Tema:** PA-149. Qué es «manipulación» deja de estar escrito dos veces, y por qué la cabecera **no** se deriva
**Nº de reporte:** 097
**Fecha:** 10 de septiembre de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** **Cerrado**, y comprobado al revés. `eje-ipc` 30, TypeScript 132

- **Depende de:** RPT-096 §5 (donde se descubrió), RPT-091 §4 (la cadena de paridad), RPT-048 §2 (la cascada de la cabecera)
- **Aborda:** PA-149 (cerrado)

---

## 1. El defecto, en una frase

«Qué condiciones significan que alguien tocó el almacén» estaba escrito a mano
**dos veces y en dos idiomas** —`hay_manipulacion()` en Rust y la cascada de
`componerCabecera` en TypeScript— y nada las comparaba. Ayer divergieron: la de
Rust pasó a cuatro y la de TypeScript se quedó en dos.

Lo encontró **una captura de pantalla**, con 176 pruebas de Rust, 128 de
TypeScript y 105 de xtask en verde.

## 2. Lo que se hizo, y lo que deliberadamente no

**El hecho pasa al contrato.** Cada campo de `Condiciones` declara
`manipulacion = true|false`. `hay_manipulacion()` deja de enumerar y cruza
`CAMPOS_CONDICIONES` con `enumerar()`. Los dos lados dejan de ser dos listas y
pasan a ser dos lectores de la misma declaración.

**La cascada de la cabecera NO se deriva, y eso fue una decisión.** La propuesta
inicial era renderizar sobre las claves del objeto `Condiciones`. Habría destruido
las dos cosas que hacen útil a la cabecera:

- **el orden**, que *es* la prioridad y cuya documentación dice que no es
  negociable;
- **la prosa**, que es lo que le dice al operador qué hacer.

Dieciséis frases genéricas generadas sobre las claves serían peores que no tener
cabecera. Lo que estaba duplicado no era la cascada: era **un booleano por
condición**.

Así que la cascada se queda escrita a mano y se sujeta con una prueba que lee el
contrato y exige que cada manipulación ocupe cabecera. El texto sigue siendo
humano; la omisión deja de poder embarcarse.

## 3. Se declara siempre, también cuando es `false`

Doce de las dieciséis llevan `manipulacion = false` explícito, y eso no es ruido.

Si «ausente» valiera por `false`, añadir una condición y olvidar la clave la
dejaría fuera de la manipulación **en silencio** — que es exactamente el defecto
que esta columna viene a cerrar, reintroducido por la puerta de atrás. La prueba
de paridad cuenta las dieciséis y falla si alguna no declara.

Tres de esos `false` llevan su motivo escrito al lado, porque el nombre invita a
lo contrario:

| Condición | Por qué `false` |
|---|---|
| `configuracionNoVerifica` | Una firma rota apunta a manipulación, pero una máquina ajena o una clave rotada dan la misma condición |
| `registroSaturado` | Tan urgente como la manipulación **sin serlo**: nadie tocó nada |
| `recuperacionNoAprovisionada` | Falta una ceremonia. Subirla pondría en crítica a todos los sensores recién instalados |

## 4. La prueba de la prueba

Una barrera que no se ha visto ponerse roja no es una barrera. Se desactivó a
mano la rama de `recuperacionNoVerifica` —dejando la cascada como estaba ayer— y:

```
not ok 5 - toda condición de manipulación ocupa la cabecera de VIS-04
    'recuperacionNoVerifica' es manipulación en el contrato y la cabecera
    no la presenta como crítica. La cascada de componerCabecera no la cubre.
```

Cayeron dos suites: la barrera nueva y la prueba específica de ayer. El mensaje
**nombra la condición**, que es la diferencia entre un fallo que se arregla en un
minuto y uno que se pasa media hora buscando.

## 5. Casi meto el defecto dentro de su propio arreglo

La prueba nueva necesita un objeto `Condiciones` con las dieciséis apagadas.
Escribirlo a mano habría sido **una lista de dieciséis nombres dentro del arreglo
del problema de las listas de dieciséis nombres**, y habría quedado atrás en
cuanto el contrato ganara una condición.

Se deriva con `Object.fromEntries` sobre `CAMPOS_CONDICIONES`. Se anota porque el
reflejo de escribirla a mano estaba ahí, en el mismo fichero donde se estaba
arreglando ese reflejo.

## 6. Dónde queda el límite

Esto no elimina la clase de defecto: la elimina **para esta regla**. Siguen
existiendo hechos declarados una sola vez en un solo lenguaje —el segundo campo
de `EMISIBLES`, por ejemplo, que dice qué gravedad lleva cada condición al SIEM y
que **no** es lo mismo que manipulación—. Ese no ha divergido nunca porque sólo
tiene un lector.

La regla que sale de aquí: **un hecho que dos lenguajes necesitan se declara en
el contrato, no en los dos.** Y si por alguna razón no puede, entonces necesita
una prueba que lea de un lado y compruebe el otro — no un comentario que afirme
que están de acuerdo.

## 7. Una nota de método, la cuarta de la semana

Al cerrar esta fila volví a partirla en dos —la nueva y una «histórico»—, por
cuarta vez en cinco días. Las cuatro las vi al releer. `identificador_de` las
ignora, así que no llegan al recuento, pero ensucian el documento y el tiempo que
cuesta encontrarlas no lo paga nadie más que yo.

Dejo escrito el arreglo: **una fila se reemplaza entera, no se parte.**

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| PA-149 | **Cerrado.** §2 y §4 |
| PA-146b | El certificado de rotación. El único rojo |
| PA-145, PA-147, PA-148 | Sin cambios |
| PA-142 | `diagnostico.js` sigue ciego |

---

*Reporte Nº 97 — Una regla en dos idiomas · PremosCorp · 10 de septiembre de 2026*
