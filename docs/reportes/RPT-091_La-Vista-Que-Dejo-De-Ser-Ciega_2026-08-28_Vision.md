# RPT-091 — La vista que dejó de ser ciega

**Tema:** PA-142. `vis04` entra en el ciclo de TypeScript, y los tres canales mudos se pintan
**Nº de reporte:** 091
**Fecha:** 28 de agosto de 2026
**Área designada:** Visión
**Entidad:** PremosCorp
**Estado:** **Parcial.** TypeScript 128 pruebas. `diagnostico.js` sigue ciego

- **Depende de:** RPT-090 (el tercer canal, que dio qué pintar), RPT-088 (`resumirRespaldo`), RPT-085 (PA-97, de donde salió este punto)
- **Aborda:** PA-142 (parcial). Avanza PA-78 mitad B

---

## 1. El orden importaba, y era el contrario del cómodo

Había dos trabajos: pintar los canales que nadie enseñaba, y meter el fichero de la
vista en el ciclo de compilación. Lo cómodo era pintar primero —se ve el resultado— y
compilar después.

Se hizo al revés, y por una razón concreta: **pintar primero es escribir código nuevo
sin comprobación de tipos contra un contrato que acababa de cambiar.** `NodoInventario`
tenía cinco campos nuevos ese mismo día. Cada uno tecleado a mano en `.js` habría sido
una cadena que nadie coteja.

## 2. Lo entregado

| Pieza | Qué hace |
|---|---|
| `vista/tsconfig.json` | Proyecto propio, `module: ESNext`, `types: []`, referencia a `eje-vision-base` |
| `vista/src/global.d.ts` | `window.eje` declarado como `PuenteEje`, no como `any` |
| `vista/src/vis04.ts` | Sustituye a `vista/vis04.js`, que se borra |
| `vista/dist/` | Destino de emisión, **no versionado** |
| `vis04.html` | `script src` pasa a `./dist/vis04.js`; tres ids nuevos |

## 3. La decisión que no era mecánica: dónde cae lo emitido

La alternativa era emitir junto al fuente y versionar el `.js`. Se rechazó por la regla
del manual: **jamás se versiona código derivado.** Un `.js` emitido y commiteado se
desincroniza de su `.ts` el primer día que alguien edita uno y no el otro, y en un árbol
que se audita eso es una fuente que miente sin fallar.

Sale a `vista/dist/`, que entra en `.gitignore`. El coste es que la vista no se abre sin
compilar antes; se aceptó porque es exactamente el coste que hace que compilar no se
olvide.

## 4. Lo que el tipo hace y el comentario no hacía

`window.eje` tipado como `PuenteEje` cierra una cadena que ya existía y no llegaba hasta
aquí:

```
contrato-ipc.toml  ↔  CAMPOS_*  ↔  struct de Rust  ↔  PuenteEje  ↔  vis04.ts
```

**Un campo que el agente no manda ya no compila la vista.** Antes se leía en `.js`, salía
`undefined`, y se pintaba como celda vacía — indistinguible de un dato que vale cero.

La tabla de condiciones lleva ahora:

```ts
] as const satisfies readonly (readonly [keyof Condiciones, string])[]
```

Media barrera textual la hace el compilador. La otra media se queda: la prueba que lee el
fuente sigue existiendo porque comprueba que la tabla **esté completa**, y eso `satisfies`
no lo dice.

### 4.1 Y una rama que se conservó a propósito

`pintarCondiciones` mantiene el caso `boolean | undefined` → «AUSENTE EN LA RESPUESTA».
Con el tipo puesto parece muerta, y no lo está: el cable trae JSON de otro proceso, y lo
que el tipo promete no es lo que el socket entrega. Colapsarla con `false` diría «esta
condición no se cumple» cuando lo cierto es «esta condición no vino».

## 5. Tres canales que se pintan

`obtener-estado-agente`, el respaldo del inventario y el inventario mismo. `refrescar()`
los pide en **cuatro bloques `try` independientes**: un canal que falla no borra los otros
tres de la pantalla. `detalleDe(fallo: unknown)` porque lo que se captura en TypeScript no
es un `Error` por contrato, es cualquier cosa.

## 6. Lo que sigue ciego, y por qué duele más que antes

`diagnostico.js` no entró. Y lleva escrito dentro que es JavaScript **a propósito**,
con esta premisa:

> no forma parte del producto

Es falsa: se despliega. Un comentario que justifica una excepción con un hecho que no se
cumple es peor que no tener comentario, porque hace que nadie vuelva a preguntar. Queda
como la parte abierta de PA-142, y la decisión es explícita —entra o se retira del
empaquetado—, no «cuando haya tiempo».

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| PA-142 | **Parcial.** `vis04` cerrado; `diagnostico.js` abierto con premisa falsa |
| PA-78 | Mitad B: ya hay qué enseñar. Falta la máquina que lo enseñe |
| PA-143 | El acta de Electron sigue sin escribirse |

---

*Reporte Nº 91 — La vista que dejó de ser ciega · PremosCorp · 28 de agosto de 2026*
