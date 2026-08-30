# RPT-085 — El punto que ya estaba hecho

**Tema:** PA-97 cerrado por re-verificación. Un punto abierto que llevaba semanas resuelto, y la ceguera que lo permitió
**Nº de reporte:** 085
**Fecha:** 28 de agosto de 2026
**Área designada:** Visión
**Entidad:** PremosCorp
**Estado:** **Cerrado.** 128/128 en TypeScript

- **Depende de:** RPT-050 (que lo arregló sin que la fila se enterara), RPT-049 §7 (donde nació), RPT-084 §8 (la cifra que me hizo mirar)
- **Aborda:** PA-97 (cerrado). Abre PA-142

---

## 1. El punto decía «todavía no» sobre algo que ya era

PA-97: *«`componerSucesos` no lee `hayMas` todavía»*. Lo lee. Y el bucle entero está
cableado:

```js
const respuesta = await window.eje.consultarAlertas(bitacora.desdeAsiento);
bitacora = incorporar(bitacora, respuesta);
pintarSucesos(componerSucesos({ ...respuesta, sucesos: bitacora.sucesos }));
setTimeout(() => void refrescar(), esperaSugerida(bitacora));
```

`vis04.html` es la vista por omisión —`indice.html` sólo sale con
`EJE_VISTA=diagnostico`—, y `esperaSugerida` devuelve 0 ms cuando hay cola. Lo cableó
RPT-050 y la fila del tablero nunca se actualizó.

## 2. Y el megabyte tampoco era un defecto

RPT-084 §8 anotó que `consultar-alertas` devolvió **1 038 208 bytes** con `hayMas: true`,
y lo presenté como la cifra que ascendía PA-97 de nota a defecto.

No lo era. Quien preguntó fue `conversar.mjs`, que pide siempre `desdeAsiento: 0`. Un
megabyte con `hayMas: true` es **la respuesta correcta a pedir la primera página**. La
consola no lo pide así: pide desde su marca.

De ahí salió la recomendación de ayer —«PA-97 primero, tiene cifra»—, apoyada en una fila
caduca y en una lectura mía que no distinguía el cliente de prueba del cliente real.

## 3. Por qué la fila pudo mentir durante semanas

`vis04.js` y `diagnostico.js` son ficheros del renderer. **No los compila `tsc`, no los
cruza `dependency-cruiser` y ninguna prueba los ejecuta.** Todo lo que se sabe de ellos se
sabe leyendo su texto.

Un fichero así se puede romper en silencio. Lo que este episodio añade es que también se
puede **arreglar en silencio**, y que eso no es inofensivo: un punto abierto que ya está
hecho dirige el trabajo hacia donde no hace falta. Estuve a punto de reescribir algo que
funcionaba.

### 3.1 La pregunta del equipo: ¿un analizador de sintaxis en lugar de expresiones regulares?

Es la mejora obvia, y por eso conviene decir por qué no es la buena. Un AST hace robusta
la lectura del texto, pero **sigue leyendo texto**: mantiene a `vis04.js` fuera del
compilador y fuera del grafo de dependencias, y deja las garantías donde están hoy —en
pruebas que hay que acordarse de escribir.

La salida es que el fichero deje de ser ciego: escribirlo en TypeScript y emitirlo. `tsc`
y `dependency-cruiser` pasarían a cubrirlo, y buena parte de las barreras textuales
—incluidas las dos de §4— se podrían **retirar** en lugar de endurecer. Queda como PA-142.

Mientras tanto la expresión regular se queda, con una ventaja que el equipo señaló bien:
si el formateador la rompe, el fallo es **ruidoso**. Es peor herramienta que un AST y
mejor situación que la de ayer, que era ninguna.

## 4. La barrera que faltaba

Dos pruebas nuevas en `vista.prueba.ts`, y ninguna comprueba que las funciones existan
—eso ya lo hacen `bitacora.prueba.ts` y `sucesos.prueba.ts`—. Comprueban que la vista
**las llame**:

1. **Pide desde la marca y no desde el principio.** Con un literal, la consola traería la
   primera página en cada vuelta y no avanzaría nunca.
2. **La bitácora se actualiza ANTES de decidir cuándo volver a preguntar.** Al revés,
   `esperaSugerida` leería el `hayMas` de la vuelta *anterior*: con cola pendiente
   esperaría dos segundos en lugar de preguntar ya, y la cola se vaciaría a un asiento
   cada dos segundos.

La segunda es la que vale. **No revienta: sólo va lento**, que en un panel de operador es
peor que un fallo — nadie abre una incidencia porque las alertas lleguen despacio.

## 5. Lo que este punto enseña sobre el tablero

Hoy el tablero falló en las dos direcciones en el mismo día:

- **RPT-084 §7.3:** PA-136 cerrado en la prosa y **fuera del recuento**, por un marcador
  que el lector no reconoce. Lo cazó `cargo xtask tablero`.
- **Aquí:** PA-97 abierto en el tablero y **cerrado en el código** desde hace semanas. No
  lo cazó nada; apareció porque fui a arreglarlo.

El primero tiene barrera. El segundo no la tiene y **no puede tenerla**: ninguna
herramienta sabe si una frase en prosa sigue describiendo el código. Lo único que lo
sujeta es releer el punto antes de trabajarlo, que es lo que se hizo.

## 6. Puntos abiertos

| ID | Punto |
|---|---|
| PA-97 | **Cerrado por re-verificación**, no por construcción. §1 y §4 |
| PA-142 | **Nuevo.** Los ficheros del renderer son ciegos al compilador y al grafo. §3.1 |
| PA-121 | Sigue siendo lo siguiente de este bloque: `cargo xtask conformidad` |

---

*Reporte Nº 85 — El punto que ya estaba hecho · PremosCorp · 28 de agosto de 2026*
