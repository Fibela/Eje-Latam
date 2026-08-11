# RPT-039 — Rotación del registro: el techo no está donde parece

**Tema:** Qué ocurre hoy al llegar a `ASIENTOS_MAXIMOS`, y por qué podar no es recortar
**Nº de reporte:** 039
**Fecha:** 8 de agosto de 2026
**Área designada:** Evidencia
**Entidad:** PremosCorp
**Estado:** **Ratificado.** PA-72 implementado; PA-59 (vía C) aprobado y pendiente

- **Depende de:** RPT-029 (persistencia), RPT-033 (ancla), RPT-038 (testigo)
- **Aborda:** PA-59
- **Abriría:** PA-72

---

## 1. El techo no se comprueba al escribir

`ASIENTOS_MAXIMOS = 500_000` sólo aparece en `analizar`, es decir **al leer**.
`RegistroEvidencia::anexar` no lo consulta:

```rust
pub fn anexar(&mut self, ...) -> &Asiento {
    let numero = self.asientos.len() as u64 + 1;
    ...
    self.asientos.push(asiento);
```

Consecuencia, hoy, sin tocar nada: un agente que supera los 500 000 asientos sigue
anexando en memoria, `serializar` escribe el fichero entero, y **el arranque
siguiente no puede leerlo**. `analizar` lo rechaza por exceder el máximo
declarado.

Y ahí está lo grave. Un registro que no se puede leer es
`CargaRegistro::ViolacionDetectada`, y el agente lo **aparta como evidencia de
manipulación** y avisa de que alguien tocó el almacén.

Nadie tocó nada. El agente hizo su trabajo demasiado tiempo.

Esto no es una optimización pendiente: es un defecto que convierte el
funcionamiento normal en una acusación falsa, y que además destruye la
continuidad de la serie justo cuando más eventos ha habido. Lo registro como
**PA-72** y propongo tratarlo aparte de la política de retención, porque la
comprobación al anexar hace falta exista o no la rotación.

## 2. Por qué podar no es recortar

`verificar_cadena` exige numeración **posicional**:

```rust
let numero_esperado = indice as u64 + 1;
if asiento.numero != numero_esperado {
    return Err(ErrorAlmacen::CadenaRota { asiento: numero_esperado });
}
```

Y `anexar` deriva el número de la longitud: `self.asientos.len() + 1`.

De ahí salen dos cosas incompatibles con una poda ingenua:

1. Quitar los asientos más antiguos deja `numero != indice + 1` y la cadena deja
   de verificar **entera**.
2. Si en lugar de eso se renumera, el siguiente `anexar` reinicia la cuenta.

Y renumerar no es una molestia: **es exactamente lo que RPT-029 §4 impidió a
propósito**. El número de asiento se guarda en disco precisamente para que
suprimir uno intermedio no pase desapercibido; sin él, la reconstrucción
renumeraría los supervivientes y la cadena cuadraría. Borrar evidencia sería
gratis.

La propiedad que detecta la supresión es la misma que prohíbe la rotación. No hay
forma de tener las dos con el diseño actual, y cualquier implementación de PA-59
que no diga cuál sacrifica está sacrificando una sin darse cuenta.

## 3. Una rectificación al hilo anterior

Sostuve que la rotación no haría saltar hacia atrás el número de asiento, porque
poda los más antiguos y los números siguen subiendo. Eso es cierto de una
rotación **correcta**, y falso de la que hoy se puede escribir: como `anexar`
toma el número de la longitud, cualquier poda en sitio hace que el siguiente
asiento reutilice números ya usados, y el sello de PA-64 saltaría hacia atrás en
el colector.

Es decir: la preocupación del equipo sobre los falsos positivos en el SIEM era
correcta, por un motivo distinto del que se dio y que yo descarté demasiado
rápido. Congelar la regla hasta cerrar PA-59 es la decisión acertada.

## 4. Tres formas de rotar, y lo que cuesta cada una

**A — Renumerar en sitio.** Simple y **hay que descartarla**: destruye la
detección de supresión intermedia de RPT-029 §4 y rompe el sello. No la propongo;
la escribo para que quede constancia de por qué no.

**B — Base explícita en la cabecera.** El fichero declara `base`, el número del
primer asiento que contiene, y `verificar_cadena` comprueba
`numero == base + indice`. Se conserva el enlace escribiendo también el
`resumen_anterior` del primer asiento superviviente, que hace de génesis del
tramo. La numeración global sigue siendo monótona y el sello no salta.

Lo que se pierde: los asientos podados **desaparecen**. Localmente pasan a ser
`ComprobacionImposible`, no `Conforme` — el tercer estado de RPT-006 §4. El
registro puede demostrar que su tramo está intacto y **no puede demostrar nada
sobre lo anterior**, salvo que el extremo salió hacia el testigo cuando ocurrió.

**C — Segmentar en ficheros.** Al llegar al umbral, el activo se cierra como
`evidencia-000001.alm` y se abre uno nuevo cuyo génesis es el extremo del
anterior. Nada se borra; el directorio crece. Conserva la verificación completa
encadenando segmentos, a costa de que la retención pase a ser un problema de
espacio en disco, que es lo que PA-59 quería resolver.

**B y C no son excluyentes**: cerrar un segmento y **además** poder podar
segmentos antiguos según política es la combinación completa. Lo que sí conviene
es no implementar las dos a la vez.

## 5. Lo que propongo

1. **PA-72 primero, y separado**: comprobar el techo al anexar. Sin rotación
   todavía, el comportamiento correcto al llegar al máximo es **negarse a anexar y
   declararlo como condición**, no seguir y volverse ilegible. Una alerta que no
   se puede anexar es grave; un registro que se lee como manipulado no lo es
   menos y encima miente.
2. **PA-59 por la vía C, luego B.** Segmentar conserva todo y no obliga a decidir
   la política de retención en el mismo cambio. La poda de segmentos antiguos —que
   es donde de verdad se decide qué se tira— llega después, con la política
   escrita y ratificada, no deducida del código.
3. **La regla del SIEM, congelada** hasta que el sello atraviese una rotación
   real en pruebas. Conforme a lo decidido por el equipo, y ahora con el motivo
   técnico correcto anotado en §3.

## 6. Lo que hace falta decidir y no puedo decidir yo

**Cuánta evidencia se conserva** no es una pregunta de ingeniería. Un hospital
con obligación de retención a cinco años y una fábrica que sólo quiere no llenar
el disco no tienen la misma respuesta, y `boveda` ya declara 30 días / 5 GB por
defecto sin que conste de dónde salió ese número.

Antes de implementar la poda del punto B hace falta saber si esa cifra es una
política del producto, un valor de ejemplo o una suposición heredada. Mientras no
se sepa, el punto C —que no tira nada— es la opción que no compromete la
respuesta.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| ~~PA-72~~ | — | ✅ **Cerrado**: `anexar` devuelve `Result`, condición `registroSaturado` (séptima) y contador de amenazas no anotadas |
| PA-59 | Rotación del registro por segmentos (vía C), y política de retención después (vía B) | Operación prolongada |
| ~~PA-73~~ | — | ✅ **Cerrado**: `cargo xtask cobertura`. Ver §8 |

## 8. PA-73 — la cobertura que nadie cuenta

Durante la implementación de PA-72, dos pruebas quedaron anidadas dentro de otra
función. `cargo test` emitió `warning: cannot test inner items` y **siguió**,
reportando 25 en verde. Dos pruebas escritas, cero ejecutadas, suite conforme.
Sólo `-D warnings` en clippy lo convirtió en error.

Esa defensa es real pero condicional: existe porque alguien escribió la lente
`unnameable_test_items`. Nada garantiza que haya una lente para el siguiente
error de la misma familia — un módulo de pruebas que se deja de declarar, un
fichero que sale del árbol, un `#[cfg(feature)]` que nadie activa.

La defensa que no depende de que el compilador se anticipe es **contar**. Y no
contra una cifra escrita a mano —que hay que mantener y que ya me he equivocado
sumando tres veces esta semana— sino comparando dos fuentes que deberían
coincidir:

- los `#[test]` que hay **en el árbol de fuentes**, contados con el mismo
  analizador que ya usa `xtask` para las lindes, que sabe distinguir un atributo
  de una cadena, de un comentario y de un literal crudo;
- los que `cargo test -- --list` declara **registrados**.

Si el primero excede al segundo, hay pruebas fantasma. El mecanismo detecta la
clase entera, no el caso concreto, y no obliga a mantener ningún número.

Queda fuera de esta primera versión el lado TypeScript, donde `node --test`
declara su total pero el recuento estático de `it(` es menos fiable. Se anota
para no fingir que la barrera cubre los dos.

### 8.1. Lo que la propia herramienta encontró en su primera ejecución

Acusó una prueba fantasma que no existía. `eje-captura` declara dos pruebas
mutuamente excluyentes por plataforma, de modo que el árbol tiene dieciséis y
cualquier compilación registra quince. La acusación se habría repetido para
siempre, en Linux y en Windows, sin que nadie hubiera hecho nada mal — y a una
herramienta que acusa siempre se le deja de hacer caso, que es peor que no
tenerla.

Contar mejor no lo arregla: resolver un `#[cfg]` en general es rehacer un trozo
de `rustc`. Lo que sí se puede es decir la verdad. Una prueba condicionada es
`ComprobacionImposible` (RPT-006 §4) y va en una tercera cifra; la desigualdad
compara sólo las exigibles.

**El coste, dicho aquí para que nadie lo descubra tarde:** una prueba
condicionada que se vuelva fantasma no se detecta. Son dos hoy, y se imprimen en
cada ejecución en lugar de callarse.

### 8.2. Por qué no se comprueba la cota superior

La relación completa sería `exigibles ≤ registradas ≤ exigibles + condicionadas`.
Sólo se comprueba la mitad izquierda, y no por descuido: **las pruebas de
documentación se registran y no llevan `#[test]` en ninguna parte**, así que
rompen la cota superior por diseño. Comprobarla convertiría en error el primer
ejemplo ejecutable que alguien escriba.

---

*Reporte Nº 39 — Rotación del registro · PremosCorp · 8 de agosto de 2026*
