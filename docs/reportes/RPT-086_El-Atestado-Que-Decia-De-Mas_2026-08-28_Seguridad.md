# RPT-086 — El atestado que decía de más

**Tema:** PA-121. `cargo xtask conformidad` construida, y el defecto que emitió en su primera corrida
**Nº de reporte:** 086
**Fecha:** 28 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** **Cerrado.** 104/104 en `xtask`, `CONFORMIDAD.lock` con 14 entradas

- **Depende de:** RPT-005 §9.3 y §9.4 (el diseño), RPT-066 (que lo encontró sin construir), RPT-006 §4 (los tres estados)
- **Aborda:** PA-121 (cerrado). Sigue delegando en PA-14

---

## 1. Lo que hace, en una frase

Ejecuta las tres suites poscuánticas de `motor-pqc` —ACVP, Wycheproof y la
diferencial contra libcrux— y **sólo si las tres pasan** emite `CONFORMIDAD.lock`:
versiones exactas de las dependencias resueltas, resumen SHA-256 de `FUENTES.lock`,
canal del toolchain, y una huella sobre todo ello.

La propiedad que se gana es la de RPT-005 §9.3: **la conformidad es propiedad de las
entradas, no del evento de compilación.** Un atestado del tipo «la CI pasó» caduca en
silencio el día que alguien sube `ml-dsa`. Éste no puede: la huella deja de cuadrar y
`cargo test -p xtask` se pone rojo solo.

## 2. Tres desviaciones del boceto, dichas antes de escribirlas

| Boceto (RPT-005 §9.3) | Lo construido | Por qué |
|---|---|---|
| «ml-kem, ml-dsa y libcrux-\*» | El conjunto se **deriva** | Una lista literal es un índice escrito a mano: no falla, se queda corta y sigue pareciendo el total |
| Versión del toolchain | De `rust-toolchain.toml`, no de `rustc --version` | El recálculo tiene que dar lo mismo en cualquier máquina; si no, la barrera se pone roja por lo que no vigila |
| `motor-pqc/tests/atestacion.rs` | El recálculo vive en `xtask` | Reimplementarlo al lado produce «paridad declarada, no igualdad». La misma función emite y comprueba |

## 3. Y la primera corrida emitió un atestado falso

**17 entradas para 14 dependencias.**

`versiones_de` buscaba cada nombre en `Cargo.lock` y se llevaba **todas** las
coincidencias. Pero `Cargo.lock` tiene varias versiones mayores del mismo crate
conviviendo: `rand` estaba dos veces y `rand_core` tres —0.10.1, 0.9.5 y 0.6.4—.

El fichero afirmaba que `motor-pqc` se probó contra `rand_core 0.6.4`. **No es cierto:**
esa la arrastra otro crate del árbol.

Un atestado que dice de más es peor que no tenerlo, porque parece preciso — y éste está
escrito para que alguien lo lea dentro de años.

### 3.1 La causa: dos estados donde hacían falta tres

`ErrorConformidad` distinguía «resuelve» de «no está». La **ambigüedad** caía dentro del
primero y se resolvía sola, llevándose todo al fichero.

Es RPT-006 §4 en un sitio nuevo: el lector de dependencias. Ahora existe `Ambiguo`, y no
elige — para, y dice qué versiones había.

### 3.2 La fuente correcta ya estaba en el mismo fichero

`Cargo.lock` publica el grafo **ya resuelto** para cada paquete, con las de desarrollo
dentro y desambiguando con la versión exacta sólo donde hace falta:

```
name = "motor-pqc"
dependencies = [
 "aes-gcm",
 "rand 0.9.5",
 "rand_core 0.10.1",
 ...
]
```

Se lee de ahí. El `Cargo.toml` de `motor-pqc` ya no se abre: no hacía falta, y era la
fuente que introducía la ambigüedad.

### 3.3 Cómo apareció, que es la parte que importa

Por una **predicción de constante fallada**: dije 13 paquetes y salieron 17. Y llevaba
dentro un error de aritmética mío — eran 14, no 13.

Se ofreció una explicación cómoda y plausible: que el mecanismo dinámico había capturado
dependencias transitivas y macros que una lista a mano habría omitido, o sea que el
número mayor probaba que el diseño era bueno. **Era falsa.**
`dependencias_declaradas` sólo leía dos tablas de un `Cargo.toml`: no tiene forma de
alcanzar nada transitivo.

Van cinco fallos de constante seguidos (RPT-083 §6.2, RPT-084 §6, éste). Es el primero
que, al **ir a explicar la diferencia en vez de aceptarla**, ha destapado un defecto real.
La regla que queda: una predicción fallada no se cierra con una explicación que encaje,
se cierra mirando el artefacto.

## 4. Lo que este mecanismo NO prueba

Ata **qué** se probó, no **que** se probó. Componer la huella correcta sin ejecutar una
sola suite es posible: basta escribir el fichero.

Cerrarlo exige que la CI sea el único productor de confianza, con una clave que sólo ella
posea. Es el alcance de PA-14, igual que lo dejó RPT-005 §9.4. Queda escrito **en el
propio módulo y en el propio fichero**, y no sólo aquí, para que nadie le atribuya una
garantía que no da.

## 5. Un `E0220` que no era lo que decía

Al condicionar la variante `Divergencia` con `#[cfg(test)]` —sus únicos consumidores son
la barrera, y sin eso `-D warnings` pone roja la CI— quedó el brazo del `match` sin
condicionar. En compilación sin pruebas la variante no existe, así que `Self::Divergencia`
deja de nombrar una variante y el compilador la busca como tipo asociado:

```
error[E0220]: associated type `Divergencia` not found for `Self`
```

Se propuso escribir la ruta absoluta, que habría fallado igual. El arreglo es poner al
brazo la misma condición que a la variante. Se anota porque el mensaje del compilador
apunta lejos de la causa, y volverá a aparecer.

## 6. Estado

| Qué | Cifra |
|---|---|
| Órdenes de `xtask` | 11, todas documentadas |
| Pruebas en `xtask` | 93 → **104** |
| Entradas del atestado | 17 (falso) → **14** |
| Tablero | 140 identificadores, 91 cerrados, **49 pendientes** |

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| PA-121 | **Cerrado.** §1 |
| PA-14 | Sigue siendo lo que convierte la huella en atestado irrefutable. §4 |
| PA-14c | La atestación como auditoría externa fuera de línea; no la sustituye esto |

---

*Reporte Nº 86 — El atestado que decía de más · PremosCorp · 28 de agosto de 2026*
