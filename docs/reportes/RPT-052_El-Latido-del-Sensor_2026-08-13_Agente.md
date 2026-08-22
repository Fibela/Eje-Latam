# RPT-052 — El latido: convertir el silencio en observación

**Tema:** PA-104. Que la sala pueda distinguir «no pasa nada» de «no hay sensor»
**Nº de reporte:** 052
**Fecha:** 13 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Ratificado e implementado en RPT-053.** Los cinco puntos del §7
siguen vigentes, incluido el quinto: PA-104 **no** se cierra con la emisión.

- **Depende de:** RPT-038 (testigo externo), RPT-047 (degradación), RPT-051 (opción D)
- **Aborda:** PA-104. Condiciona **PA-107** (empaquetado dual; este reporte lo
  llamó PA-84 por error, ver RPT-053 §9)

---

## 1. El hueco que abre la decisión

RPT-051 eligió la opción D. El operador de sala lee del colector, y
`salidaNoDisponible` **no viaja por syslog** — emitirla exigiría el canal que
acaba de fallar.

Desde la sala, un sensor cuyo enlace se cortó y un sensor con la red en calma
producen **exactamente el mismo dato: ninguno**.

## 2. La forma de la solución no es un canal más

Añadir un aviso de «me caí» es imposible por construcción: quien se cayó no
puede avisar. La solución es simétrica: **el colector debe saber qué esperaba
recibir**, para que su ausencia sea una observación en lugar de un vacío.

Es la regla de RPT-006 §4 aplicada al perímetro entre el sensor y la sala.

## 3. Qué lleva el latido, y por qué no sólo «estoy vivo»

Un latido que dice «existo» obliga a la sala a preguntar lo demás por otro
camino que no tiene. Así que el latido lleva **el estado**:

- Las **condiciones vigentes**. Un sensor vivo y ciego (`capturaNoDisponible`)
  debe verse desde la sala, y hoy no se ve.
- El **extremo del registro y su número de asiento** — exactamente lo que
  RPT-038 ya envía como sello al testigo externo.

Esto último no es un añadido: es la observación de que **PA-64 y PA-104 son el
mismo mecanismo visto desde dos sitios**. El sello periódico ya es un latido; lo
que le falta es cadencia garantizada y que alguien vigile su ausencia.

Propongo no construir un canal nuevo, sino **darle cadencia al sello y adjuntarle
las condiciones**.

## 4. Lo que hace inútil un latido mal hecho

**Si sólo se emite cuando hay cambios, no es un latido.** Tiene que salir
igualmente en un sensor tranquilo, que es precisamente el caso indistinguible.

**Si se puede repetir, no prueba nada.** Un atacante que silencie el sensor y
reproduzca su último latido mantiene la sala en verde. El número de asiento es
monótono (RPT-039 §3) y el extremo cambia con él: un latido con el mismo par
repetido N veces es sospechoso, y la sala debe poder decirlo.

**Si se apaga cuando el sensor se degrada, miente.** Un agente ciego sigue
latiendo — con `capturaNoDisponible` a cierto. Esa es la diferencia entre un
sensor que se apagó y uno que dejó de ver, y son dos llamadas distintas.

## 5. Cuántos intervalos, y por qué no lo decido

Pocos: un corte de red de diez segundos levanta a la sala de madrugada por nada,
y la fatiga de alertas se paga una sola vez.

Muchos: un sensor muerto pasa desapercibido ese tiempo, y ese tiempo es
exactamente lo que no se está vigilando en un hospital.

**No hay una cifra correcta sin medir la red del cliente.** Es PA-41 con otro
nombre: la cadencia sigue sin medirse. Lo que sí propongo es que el número sea
**configuración firmada y no una constante**, y que el propio latido declare su
intervalo, para que el colector no tenga que suponerlo.

## 6. El riesgo principal, y no es técnico

**El lado del colector no existe.**

Podemos emitir el latido esta semana. Nadie ha construido lo que se da cuenta de
su ausencia. Y un latido que nadie vigila es un mecanismo correcto que nadie
llama — el defecto dominante de este proyecto, contado nueve veces en el
histórico.

Emitir sin vigilar sería **peor que no emitir**: daría por resuelto PA-104 y
dejaría a la sala igual de ciega, con la sensación de estar cubierta.

Por eso este reporte propone que PA-104 **no se dé por cerrado con la emisión**.
Se cierra cuando alguien apaga un sensor y la sala se entera.

## 7. Lo que propongo ratificar

1. El latido **es el sello de RPT-038 con cadencia garantizada**, no un canal
   nuevo (§3).
2. Lleva las condiciones vigentes, no sólo señal de vida (§3).
3. Se emite en un sensor tranquilo y en uno degradado; sólo calla si el sensor
   calla (§4).
4. El intervalo es configuración firmada, viaja en el propio latido, y su valor
   sale de medir (§5).
5. **PA-104 se cierra por observación, no por implementación** (§6).

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-105~~ | ✅ Detector de referencia en RPT-057 |
| PA-41 | La cadencia sigue sin medirse; el intervalo depende de ello |
| PA-79 | El intervalo es el primer parámetro que exige configuración firmada |

---

*Reporte Nº 52 — El latido del sensor · PremosCorp · 13 de agosto de 2026*
