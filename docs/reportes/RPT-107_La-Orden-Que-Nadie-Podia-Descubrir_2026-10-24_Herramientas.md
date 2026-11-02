# RPT-107 — La orden que nadie podía descubrir

| | |
|---|---|
| **Tema** | La orden que nadie podía descubrir |
| **Número** | 107 |
| **Fecha** | 2026-10-24 |
| **Área designada** | Herramientas |
| **Abre** | PA-167 |
| **Avanza** | PA-162 (cableado escrito y probado; sin ejecutar) |
| **Precedentes** | RPT-104 (PA-162), RPT-105 (la propuesta), RPT-106 (PA-165, PA-166) |

---

## 1. PA-167 — la barrera cuyo nombre es más ancho que su alcance

Al cablear `aceptar-inventario` se añadió al despacho y a `docs/Comandos.md`, y
`cargo xtask manual` siguió diciendo **«12 ordenes, todas documentadas»** en
verde. La orden número trece no entró en su cuenta.

El motivo está en la primera línea del fichero de la barrera: *«Paridad entre
`docs/Comandos.md` y las ordenes que **xtask** acepta»*. La lista sale de
`crate::ORDENES`, que es la de xtask. **`eje-manifiesto` y `eje-agente` —las dos
herramientas que alguien teclea— no tienen ninguna barrera derivada** que ate sus
órdenes al manual ni a su propia línea de uso.

La consecuencia inmediata: la orden existía en el despacho, estaba documentada, y
**no aparecía en la línea de uso de la herramienta**. Quien ejecutara
`eje-manifiesto` sin argumentos no podía descubrirla. Es la forma exacta de
PA-122, cuando `--directorio-socket` se añadió al analizador y no al uso y estuvo
un día siendo una opción invisible.

### 1.1 Lo cazó una predicción fallida

Se predijo que el manual pasaría de 12 a 13 órdenes. Salió 12 **y en verde**, y la
pregunta «por qué no subió» llevó al hueco.

Si se hubiera predicho 12, el verde habría confirmado un modelo falso de lo que
esa barrera comprueba, y la orden se habría entregado indescubrible. **El valor no
estuvo en acertar: estuvo en haber escrito un número antes de mirar.** Una
predicción que coincide con un verde por el motivo equivocado es indistinguible de
una que acierta, salvo que se haya escrito antes.

### 1.2 Lo aplicado, y por qué no arregla el fondo

La orden en la línea de uso, y una prueba que lo ata:
`la_linea_de_uso_nombra_aceptar_inventario`. Es **una prueba más por orden,
escrita a mano**, como las que ya existen para `migrar-centinela`.

Decimocuarta lista a mano de la serie, y **la orden decimoquinta volverá a poder
entrar sin ella**. El arreglo de fondo tiene forma conocida: una tabla `ORDENES`
en `eje-manifiesto` con el despacho **derivado** de ella, como ya la tiene xtask,
y que `xtask manual` cotejé las tres herramientas en lugar de una. Queda abierto.

---

## 2. PA-162 — el cableado, y un defecto que apareció al probarlo

### 2.1 La orden

`eje-manifiesto aceptar-inventario --almacen <dir>`, sexto paso de la ceremonia.
El razonamiento de por qué lo hace la ceremonia y no el agente está en RPT-105 §3
y no se repite; lo que importa aquí es que **la autoridad para decir «éste es el
primero» no se añade al sistema: se usa la que ya existe**, con el administrador
delante, en el único momento en que existe.

El sondeo que rompe el círculo —para escribir la marca hace falta la secuencia
verificada, y obtenerla exige una marca— usa la secuencia que el fichero declara,
con lo que la comprobación de reversión compara `declarada < declarada` y es
falsa. No es un atajo: esa secuencia viaja **dentro de `anclada`, que la firma
cubre**, así que mentirla rompe la firma y `cargar` falla. Lo que se escribe en
disco sale del objeto **verificado**.

### 2.2 El defecto, encontrado al escribir la prueba y no al correrla

La guarda del centinela estaba escrita así:

```rust
let centinelas = cargar_centinela(&rutas).map_err(…)?;
```

Y `cargar_centinela` **devuelve `Ok(Centinelas::SIN_ESTABLECER)` cuando el fichero
no existe** (`arranque.rs:949`). Luego la guarda no guardaba nada: un almacén sin
anclar pasaba, el inventario verificaba, y `aceptar_inventario` escribía un
centinela con la marca puesta y **sin anclas** — el estado permanente de
`operativaNoAprovisionada` + `recuperacionNoAprovisionada` que PA-161 cerró,
recreado por la herramienta que viene a completar la ceremonia.

Apareció al redactar `sin_centinela_no_acepta`, antes de ejecutar nada: al
preguntarse qué afirmar sobre el disco tras la negativa, resultó que no habría
negativa.

**La lección, y es la del proyecto otra vez:** `cargar_centinela` colapsa «no hay
fichero» con «hay fichero y no dice nada» — razonablemente, porque a quien arranca
le da igual. Aquí **no** da igual, y heredar la comodidad de otro llamador fue el
error. Arreglado con una comprobación explícita de existencia, como la que
`anclar` ya hacía por la razón simétrica.

### 2.3 Las siete pruebas

| # | Qué afirma |
|---|---|
| 1 | Sin `--almacen`, error de uso |
| 2 | Sin centinela se niega, **y no crea el centinela** |
| 3 | Con la marca ya establecida se niega, **y no la baja** — la guarda de reversión |
| 4 | Sin clave operativa no acepta, y la marca sigue sin establecer |
| 5 | Un inventario firmado por otra clave **no establece la marca** |
| 6 | El camino feliz: acepta, y la marca queda en la secuencia del fichero |
| 7 | Tras aceptar, una secuencia anterior da `ReversionDetectada { aceptada, presentada }` |

Las siete comprueban **el efecto en disco**, no el texto del error. RPT-104 §2.1
cuenta cómo una prueba de guarda pasó en verde estando mal justamente por mirar el
mensaje, y esa lección es la que da forma a estas.

La 6 es la que PA-162 hacía imposible: antes del cableado no existía ningún camino
por el que esa marca llegara a establecerse. La **7** es la que mide el objetivo
de RPT-105 §1 — la reversión, hoy inalcanzable porque `verificar` sale antes por
`FrescuraNoEstablecida`, pasa a detectarse.

### 2.4 Qué falta, y por qué PA-162 sigue abierto

**Las pruebas están escritas y no ejecutadas.** Y faltan las cinco perturbaciones
de RPT-105 §8, incluida la que está puesta para **descubrir un hueco y no para
confirmar un acierto**: invertir el orden de escritura —inventario antes que
centinela— y ver si alguna prueba lo nota. La predicción es que **ninguna lo
nota**; si se acierta, falta una prueba que hay que escribir antes de cerrar.

Hasta entonces no sale ningún `centinela.dat` hacia el laboratorio. Entregar un
artefacto sellado por código sin ejecutar sería pedirle a RUA que mida con una
regla torcida, y ya pagaron una vez por un diagnóstico nuestro basado en una
suposición.

---

## 3. PA-168 — la prueba que llevaba el nombre de lo que no medía

RPT-105 §8 pedía, como quinta perturbación, invertir el orden de escritura de
`aceptar_inventario` —inventario antes que centinela— y ver si alguna de las
siete pruebas nuevas lo notaba. Se predijo que **ninguna**, y que en ese caso
faltaría una octava.

Al buscar quién sí lo notaría apareció, en `guardian-cc`, una fila con el nombre
exacto: `el_centinela_se_escribe_antes_que_el_inventario`. Y al leer su cuerpo:

```rust
disco::escribir_atomico(&almacen.rutas.centinela(), …)?;       // a mano
assert_eq!(cargar_centinela(…).inventario, Establecido(9));    // se relee
assert!(!almacen.rutas.inventario().exists());                 // nadie lo creo
```

**No llamaba a `aceptar_inventario`.** Afirmaba que `escribir_atomico` funciona y
que un fichero que no se creó no está. Habría pasado en verde con esa función
escrita del revés, reescrita de cualquier forma, o borrada.

La predicción era correcta y más amplia de lo que se creyó: **no lo cazaba
nada**, ni las siete nuevas ni la que llevaba su nombre.

### 3.1 Por qué importa más que un nombre mal puesto

El orden de escritura es lo que impide actuar sobre un inventario cuya secuencia
no quedó registrada — es decir, **la ventana de reversión**, la misma que todo
este punto existe para cerrar. El comentario de `aceptar_inventario` lo dice
desde que se escribió: *«Al reves —inventario primero— se actuaria sobre un
inventario cuya secuencia no quedo registrada, reabriendo la ventana de
reversion»*. La propiedad estaba documentada con cuidado y custodiada por nada.

Es PA-154 en su forma más pura —verificar *que* algo se dice y no *que se diga lo
correcto*— en la función que PA-162 acababa de cablear a producción.

### 3.2 El arreglo, y cómo se vuelve observable un orden

Reescrita para que **llame** a `aceptar_inventario` y haga imposible la
**segunda** escritura: se ocupa la ruta del inventario con un directorio.
`escribir_atomico` renombra sobre el destino, y renombrar sobre un directorio
falla en Linux y en Windows.

- Orden correcto → el centinela ya está en N cuando la segunda falla.
- Orden invertido → la primera falla, **el centinela no se toca**, y la prueba cae.

La perturbación de invertir el orden pasa a poner algo rojo, que es lo que debía
hacer desde el principio.

### 3.3 La lección, y es la cuarta de la semana

**El nombre de una prueba es una afirmación sobre su cuerpo, y nadie la
comprueba.** Las otras tres de la serie: una guarda que medía el texto del error
en vez del efecto (RPT-104 §2.1); una barrera cuyo nombre era más ancho que su
alcance (PA-167, §1); y `describir`, que decía describir el estado y descartaba
su carga (PA-163).

Las cuatro se encontraron igual: **escribiendo una predicción antes de mirar.**
Ninguna la habría encontrado leyendo con atención, porque las cuatro leen bien.

---

## 4. Lo que queda

- Ejecutar las siete y las cinco perturbaciones → cierra PA-162.
- `cargo xtask tablero`, `manual`, `evidencia`, `verificar`, y CI en verde →
  cierra PA-165, que sigue abierto porque su criterio escrito exige ver una
  corrida **en verde** y la primera salió roja (RPT-106 §8.3).
- La tabla `ORDENES` derivada → cierra PA-167.
- La predicción del `sello=` para el 7b de RUA, leyendo `ciclo.rs` antes de
  escribirla.
- PA-164, PA-160, PA-152 y la tarea #30 siguen donde estaban.
