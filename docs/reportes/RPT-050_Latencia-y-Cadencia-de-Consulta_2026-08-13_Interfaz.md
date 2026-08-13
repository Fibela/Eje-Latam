# RPT-050 — Latencia medida: el cable no era el problema

**Tema:** Cuánto cuesta consultar al agente, y por qué la respuesta cambió la pregunta
**Nº de reporte:** 050
**Fecha:** 13 de agosto de 2026
**Área designada:** Interfaz
**Entidad:** PremosCorp
**Estado:** **Medido e implementado.** Cierra PA-83, PA-92, PA-98 y PA-99

- **Depende de:** RPT-034 (ciclo del agente), RPT-045 (cable), RPT-049 (cota por bytes)
- **Aborda:** PA-83, PA-92

---

## 1. Qué se midió y cómo

Veinte peticiones consecutivas por canal, sobre el agente real en WSL, con el
cliente del producto (`enlace.ts` compilado). La medida separa dos cosas que el
total mezcla:

- **Espera**: de escribir la petición al primer byte de vuelta. Es el agente, no
  el cable.
- **Transferencia**: del primer trozo al último, reensamblado incluido.

Separarlas era la mitad del trabajo. Medir sólo el total habría llevado a
optimizar la parte que no importa.

## 2. Los números

| | `obtener-condiciones` | `consultar-alertas` | factor |
|---|---|---|---|
| Carga | 251 B | 1 038 213 B | **×4 136** |
| Transferencia (mediana) | 0,3 ms | 1,8 ms | ×6 |
| Espera (mediana) | 500,2 ms | 540,7 ms | +40 ms |
| Trozos | 1 | 16–18 | |

## 3. El cable no era el problema

Multiplicar la carga por cuatro mil cuesta **millón y medio de microsegundos**.
El acumulador de RPT-045 reensambla un megabyte en diecisiete trozos y no se
nota.

Toda la preocupación que llevó a escribir aquel acumulador con tanto cuidado
estaba justificada por **corrección**, no por rendimiento. Es una distinción que
conviene no perder: la pieza tenía que estar bien, y además resulta ser barata.

## 4. Lo que sí cuesta: 40 ms del hilo del sensor

La espera subió 40 ms con el megabyte. **Eso no es el cable: es el agente
serializando** antes de mandar el primer byte.

Y el agente tiene **un solo hilo** (RPT-034 §3). Esos 40 ms no salen de la nada:
salen del tiempo que debería estar observando la red.

Un panel que consulte el histórico entero cada dos segundos le está quitando al
sensor un **2% de su vigilancia** para reenviarle lo que el panel ya tiene. En un
producto cuya única promesa es mirar, eso no es un detalle de rendimiento.

## 5. El sondeo secuencial paga siempre el caso pésimo

La espera se clava en 500 ms, no en 250.

Se predijo 250 suponiendo llegadas al azar dentro de una ventana de 500. Pero
estas peticiones no llegan al azar: **cada una sale justo después de que la
anterior fuera atendida**, es decir, justo después de que el agente terminara de
atender. Aterriza al principio de una vuelta nueva y espera la vuelta entera.

Un cliente que consulta en bucle nunca ve la media. Ve siempre el máximo.

Esto no se deduce leyendo el código: sale de medir con el patrón de acceso real.

## 6. Respuesta a PA-92 — la cadencia

**Por debajo de 500 ms es desperdicio.** El agente atiende al final de cada
vuelta; pedir más rápido devuelve lo mismo.

**Dos segundos es razonable** *si se usa cursor*. Sin él, son 40 ms de sensor
robados cada dos segundos, indefinidamente.

**Cuando el agente dice `hayMas`, no se espera.** Hay cola pendiente y esperar
sólo la alarga. `esperaSugerida` devuelve 0 en ese caso.

## 7. PA-98 — el cursor, y el hueco que abre

`desdeAsiento` existe desde RPT-019 y **nadie lo usaba**: la consola pedía
siempre desde cero. Otro mecanismo correcto que nadie llamaba.

La bitácora (`bitacora.ts`) lleva la marca de agua y acumula entre consultas.
Pero el cursor introduce un peligro propio:

> Si la marca está en el asiento 100, el agente rota y `primerDisponible` pasa a
> 5000, pedir «desde 100» devuelve desde el 5000 — y los asientos 101 a 4999
> desaparecen **sin que nada lo diga**.

El cursor convierte un hueco visible en uno silencioso: el error de PA-74,
creado por la optimización que lo evitaba. De ahí `huboSalto`, con tres pruebas
que marcan sus bordes:

- Un panel recién abierto sobre histórico archivado **no** es un salto — llamarlo
  así acusaría de pérdida a quien acaba de abrir la ventana.
- La continuidad exacta (se tenía el 100, ahora lo más antiguo es el 101)
  tampoco: un «mayor o igual» daría falso positivo en cada rotación limpia.
- El salto es **pegajoso**. No se cierra porque la siguiente consulta vaya bien:
  sigue habiendo alertas que ese panel nunca mostró.

## 8. El régimen permanente, medido

Veinte consultas consecutivas con cursor, sobre 300 asientos sembrados:

| Vuelta | Bytes | Trozos | Espera |
|---|---|---|---|
| 1 | 1 038 213 | 16 | 248,8 ms |
| 2 | 217 778 | 4 | 509,4 ms |
| 3–20 | **55** | **1** | ~500,2 ms |

Bitácora final: **300 sucesos, marca en 300, sin salto.**

Las dos primeras vueltas drenan el histórico —256 asientos y luego los 44 que no
cabían, con `hayMas` guiando la segunda— y a partir de la tercera el refresco son
**55 bytes en un trozo**.

**La espera vuelve a 500,2 ms**, la misma que `obtener-condiciones`. Los 40 ms de
serialización del §4 desaparecen: no eran un coste del canal, eran el precio de
reenviar lo que el cliente ya tenía.

La predicción del §6 queda confirmada con la cifra exacta. **PA-99 cerrado.**

### 8.1. Lo que sigue sin medirse

**El coste desde el sensor.** Los 40 ms se infieren de la espera vista *desde el
cliente*. Nadie ha medido cuánta observación pierde el agente, que es la cifra
que de verdad importa en un producto cuya promesa es mirar.

Que la espera baje 40 ms es consistente con haberle devuelto ese tiempo, pero
consistente no es lo mismo que medido. Queda como **PA-100**, y se cierra
instrumentando el ciclo del agente, no al cliente.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-83~~ | ✅ Medido |
| ~~PA-92~~ | ✅ Contestado con números: ≥500 ms, 2 s con cursor, 0 con `hayMas` |
| ~~PA-98~~ | ✅ Bitácora con cursor y detección de salto |
| ~~PA-99~~ | ✅ Régimen medido: 55 bytes, 1 trozo, 500,2 ms |
| **PA-100** | El coste en el sensor sigue infiriéndose desde el cliente |

---

*Reporte Nº 50 — Latencia y cadencia de consulta · PremosCorp · 13 de agosto de 2026*
