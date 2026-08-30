# RPT-066 — La barrera del manual

**Tema:** PA-119. Paridad derivada entre `docs/Comandos.md` y las órdenes de `xtask`
**Nº de reporte:** 066
**Fecha:** 14 de agosto de 2026
**Área designada:** Método
**Entidad:** PremosCorp
**Estado:** Construido. Cierra PA-119. Acuña PA-120 y PA-121

- **Depende de:** RPT-065 (el manual), RPT-060 (el tablero abandonado), RPT-039 §8 (la cobertura), RPT-006 §4 (tres estados)
- **Aborda:** PA-119
- **Acuña:** PA-120, PA-121

---

## 1. Una tabla, tres consumidores

Antes había un `match` que despachaba y una tira de `println!` que anunciaba, y
nada ataba lo uno con lo otro. Ahora hay una sola `ORDENES`:

```rust
struct Orden {
    nombre: &'static str,
    argumentos: &'static str,
    resumen: &'static str,
    ejecutar: fn(&[String]) -> ExitCode,
}
```

De ella salen **el despacho**, **`cargo xtask ayuda`** y el cotejo contra el
manual. Dos de las tres direcciones dejan de necesitar prueba: no se puede
despachar una orden que no esté en la tabla ni anunciar una que no se despache,
porque son el mismo dato.

La tercera —el documento— sí necesita comprobación, y es `cargo xtask manual`.

**Lo que no se hizo:** leer `main.rs` con una expresión regular para extraer las
ramas del `match`. Ya se aprendió que toda prueba que lea código fuente tiene que
quitar los comentarios primero, y se aprendió dos veces —en Rust y en
TypeScript—. Convertir la tabla en la fuente elimina la necesidad de leer fuente.

## 2. Las dos direcciones no son igual de graves

| Dirección | Qué deja | Gravedad |
|---|---|---|
| Orden sin documentar | Una herramienta que sólo usa quien la escribió | Se descubre sola |
| **Comando documentado que no existe** | Alguien teclea algo que falla | Ocurre en la sesión con menos tiempo para averiguar por qué |

La segunda es la que justifica el módulo, y por eso se barre `docs/` entero y no
sólo el manual: un reporte que cita una orden retirada miente exactamente igual.

## 3. Lo que encontró antes de compilarse

`docs/reportes/RPT-005` §9.3 manda teclear `cargo xtask conformidad` — **NO EXISTE TODAVIA**, y de ahí el hallazgo. Esa
orden se diseñó el 4 de agosto con todo detalle —`CONFORMIDAD.lock`, huella
SHA-256 sobre `Cargo.lock` más `FUENTES.lock` más el toolchain, autoinvalidación
si alguien sube una dependencia— y **nunca se construyó**.

Diez días documentada como instrucción. Queda como **PA-121**.

Es el mismo patrón que PA-108 cazando a PA-14b en su primera ejecución, y el
mismo que el aviso del colector en RPT-063 §2: la comprobación no ratifica lo
que hay, **descubre lo que falta**. Van tres veces en dos días.

## 4. La escapatoria mejora el documento, no debilita la barrera

RPT-005 no está mal: es un diseño, y los diseños se escriben antes de existir. Lo
que estaba mal es que **se leía como una instrucción**.

Una lista de excepciones dentro de `manual.rs` lo habría callado sin arreglar
nada — el lector del reporte seguiría copiando el comando. Lo que se exige es el
aviso **en la línea de la cita**:

```markdown
- `cargo xtask atestar-release` — **NO EXISTE TODAVIA**, es diseño; se sigue en PA-14
```

Y vale para su línea, no para el fichero. Hay una prueba dedicada a eso
(`el_aviso_no_se_derrama_a_las_lineas_siguientes`), porque así es como mueren las
comprobaciones: no se apagan, **se les amplía el alcance** hasta que no ven nada.

## 5. La prueba estuvo a punto de ser el cuarto índice

La prueba de fuego sobre el árbol real se escribió primero con los diez nombres
copiados a mano dentro del `#[test]`.

Eso habría creado un cuarto índice escrito a mano, **dentro de la barrera que
existe para cazar esos**, y habría seguido en verde con una orden nueva sin
documentar — que es literalmente el caso que comprueba. La lista sale ahora de
`crate::ORDENES`.

Queda escrito porque el error no lo cazó ninguna herramienta: se vio al releer, y
la próxima vez puede no verse.

## 6. Lo que la barrera **no** cubre, dicho en el propio manual

§13 de `docs/Comandos.md` se reescribió para decir qué verifica y qué no. Cubierto:
la lista de órdenes de §4.1. Sin cubrir: las banderas del agente (§6.2), los
`npm run` (§5) y las opciones del vigía (§6.3). Hoy los comprobé a mano y
coincidían; mañana eso es costumbre, no prueba.

Un documento que sólo declara lo que verifica, callando lo que no, se lee como si
lo verificara todo.

## 7. Cableado, no sólo construido

`cargo xtask manual` entra en la secuencia de §12 del manual y en un job nuevo de
CI, `indices`, junto a `tablero` y `cobertura`. Los tres son lo mismo —índices de
cosas que viven en el código— y ahora corren juntos.

Se dice explícitamente porque la clase de defecto dominante de este proyecto es
**el mecanismo correcto que nadie llama**, y una barrera que sólo se ejecuta
cuando alguien se acuerda es un caso de eso.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-119~~ | ✅ **Cerrado** (§1) |
| PA-120 | El socket y la evidencia comparten directorio. Separar antes de medir PA-117 |
| PA-121 | `cargo xtask conformidad` — **NO EXISTE TODAVIA** — diseñada y no construida (§3) |
| — | Ampliar la barrera a las banderas del agente, los `npm run` y el vigía (§6). Sin número todavía |

---

*Reporte Nº 66 — La barrera del manual · PremosCorp · 14 de agosto de 2026*
