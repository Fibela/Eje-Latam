# RPT-076 — El guardián mira el código

**Tema:** PA-129. La comprobación leía la línea cruda y acusaba a la prosa que la explica
**Nº de reporte:** 076
**Fecha:** 17 de agosto de 2026
**Área designada:** Método
**Entidad:** PremosCorp
**Estado:** Construido y probado. Cierra PA-129

- **Depende de:** RPT-003 §9.5 (el guardián), RPT-075 §5 (el primer falso positivo), RPT-067 §7 (la misma familia en una prueba)
- **Aborda:** PA-129

---

## 1. Dos acusaciones en dos días, las dos a comentarios

El guardián bloqueó el *build* dos veces por texto que **explicaba** un patrón
prohibido en lugar de cometerlo:

- un comentario que decía por qué **no** hay dirección de escucha por omisión
  (RPT-075 §5);
- otro que explicaba por qué un estado degradado **no es un mock** — escrito
  precisamente porque el equipo de pruebas había hecho esa pregunta.

La primera vez se reescribió el comentario. La segunda no: el texto que el
escáner pedía cambiar era **la respuesta a una pregunta que alguien había
hecho**, y adaptarla a un regex habría borrado justo eso.

## 2. El remedio estaba a dos ficheros de distancia

`xtask/src/exclusion.rs` lleva desde el principio un analizador léxico completo
—cadenas, cadenas crudas, literales de carácter, comentarios de línea y de bloque
anidados— construido porque un `grep` sobre líneas produce falsos positivos y
falsos negativos.

La comprobación de patrones no lo usaba. Miraba `fuente.lines()`.

## 3. Un solo recorrido, no dos

`recorrer` devuelve ahora las **dos** respuestas del mismo paso: qué líneas
pertenecen a un bloque `#[cfg(test)]` y qué caracteres están dentro de un
comentario.

Escribir un segundo analizador habría duplicado la máquina de estados, y bastaría
con que una copia se quedara atrás para que el guardián empezara a mentir por un
lado. Es el patrón de toda la semana —índices escritos a mano— aplicado a un
lexer.

El contenido de los comentarios **se sustituye por espacios, no se borra**: el
número de línea y la columna siguen significando lo mismo.

## 4. Lo que casi se rompe arreglándolo

Quitar los comentarios para **todas** las comprobaciones habría dejado ciega la
de marcadores pendientes, cuyo patrón es literalmente:

```
//\s*(TODO|FIXME|XXX|HACK|PENDIENTE)
```

Habría dejado de encontrar nada, **en silencio y para siempre**, mientras las
otras cinco seguían en verde. Exactamente el fallo que este guardián existe para
no tener, cometido al repararlo.

**Lo cazó una prueba que ya estaba ahí**: `detecta_cada_categoria_en_codigo_de_produccion`,
con un caso por comprobación. No se buscó ni se escribió para esto — estaba puesta
por si alguien tocaba el guardián, y alguien lo tocó.

## 5. Cada comprobación declara qué mira

```rust
pub enum Ambito {
    /// El código, con los comentarios sustituidos por espacios.
    Codigo,
    /// La línea entera. Para lo que **es** un comentario por naturaleza.
    LineaEntera,
}
```

Cinco miran código: un `todo!()`, un `mock`, una IP fija son **instrucciones**, y
un comentario que las mencione las explica. Una mira la línea entera: un `// TODO`
no aparece en ningún otro sitio.

La corrección no es «el guardián mira el código». Es que **el ámbito se declara**,
y quien se equivoque falla de forma visible en lugar de dejar de mirar.

## 6. Tres pruebas nuevas, y una de ellas es la que importa

| Prueba | Qué sujeta |
|---|---|
| `un_patron_citado_en_un_comentario_no_es_un_hallazgo` | El falso positivo que originó todo |
| `un_marcador_pendiente_se_caza_aunque_este_en_un_comentario` | Que el arreglo no dejó ciega a la que vive en comentarios |
| `un_punto_final_dentro_de_una_cadena_sigue_siendo_un_hallazgo` | Que una cadena **es** código: `bind("…")` se sigue cazando |
| `el_codigo_anterior_a_un_comentario_sigue_mirandose` | Que no basta con poner un comentario detrás para esconder algo |

Las dos últimas son las que impiden que esto se convierta en un guardián
decorativo — que sería peor que el falso positivo que venía a resolver.

## 7. Lo que se rechazó

Ofuscar los literales para que el escáner no los viera. Se propuso, y habría sido
**peor que un `#[allow]`**: un `allow` al menos declara que se está silenciando
algo; adaptar la prosa esconde el silencio dentro de un texto que parece normal.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-129~~ | ✅ **Cerrado** (§5) |
| PA-79 | En construcción. Este arreglo salió de un comentario suyo |

---

*Reporte Nº 76 — El guardián mira el código · PremosCorp · 17 de agosto de 2026*
