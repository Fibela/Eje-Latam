# RPT-110 — El ruido que tapaba la fuga

| | |
|---|---|
| **Tema** | El ruido que tapaba la fuga |
| **Número** | 110 |
| **Fecha** | 2026-11-03 |
| **Área designada** | Seguridad |
| **Abre** | — |
| **Cierra** | **PA-165** — primera corrida entera en verde, leída en CI #8 (§12). Y confirma **PA-166** en CI |
| **Precedentes** | RPT-109 (la barrera que prohibía lo correcto), RPT-106, RPT-086 §3, RPT-003 §2.7 y §9.4, RPT-005 §7.4 |

---

## 1. El hallazgo, en una frase

**`cargo deny` llevaba días reportando una vulnerabilidad de canal lateral temporal
bajo el motor poscuántico, y nadie podía leerla porque dos reglas mal
especificadas del mismo trabajo la ahogaban en ruido.**

```
advisories FAILED, bans FAILED, licenses ok, sources ok
```

## 2. La fuga: RUSTSEC-2026-0385

`ctutils 0.4.2` — *«Non-constant-time mask generation and `u128` comparisons»*.

`ctutils::Choice` genera máscaras con funciones `const fn`, que por serlo **no
pueden reusar la generación de máscara respaldada por `asm!` del crate `cmov`**,
que es la que garantiza el tiempo constante. En versiones anteriores a 0.4.4 el
código generado **podía incluir un salto en plataformas de uso común**. Además las
comparaciones de `u128` vía `ctutils::Choice`, pensadas para tiempo constante,
generaban saltos en objetivos de 32 bits.

Impacto, del propio aviso: *«Branch instructions on secret values can potentially
leak such values via covert channels.»*

### Dónde está

```
ctutils 0.4.2
└── hybrid-array 0.4.14
    ├── block-buffer 0.12.1 → digest 0.11.3 → sha3 0.11.0 → ml-kem 0.3.2 ─┐
    │                                         shake 0.1.0 → ml-dsa 0.1.1 ─┤
    │                                         signature 3.0.0 → ml-dsa ───┤
    ├── crypto-common 0.2.2 → kem 0.3.0 → ml-kem ────────────────────────┼─> motor-pqc
    └── module-lattice 0.2.3 → ml-kem, ml-dsa ───────────────────────────┘
                                                    └─> eje-agente
                                                    └─> guardian-cc
                                                    └─> eje-red
                                                    └─> eje-manifiesto
```

**Las primitivas de tiempo constante que sostienen ML-KEM y ML-DSA.** Para un
producto cuya propuesta es resiliencia post-cuántica práctica, esto no es una
advertencia de la compuerta: es la clase de defecto para la que la compuerta
existe.

**Resolución:** `cargo update -p ctutils` a >=0.4.4, y la suite entera detrás.
Verificado upstream con BINSEC, que ahora corre en su CI con `cargo checkct`.

> **Pendiente.** En el momento de escribir esto el lock sigue en `ctutils 0.4.2`.
> El reporte no lo declara resuelto.

## 3. El ruido, parte uno: `wildcards = "deny"`

`error[wildcard]`, diecisiete veces:

```
eje-agente      7   guardian-cc, motor-pqc, eje-almacen, eje-ipc, boveda, eje-red, eje-captura
xtask           4
eje-manifiesto  2   guardian-cc, motor-pqc
eje-red         2
guardian-cc     2   eje-almacen, motor-pqc
```

**Ninguna es una versión flotante.** Son los crates internos del propio workspace.
Una dependencia `path` no declara requisito de versión, y `cargo-deny` lo cuenta
como comodín. La regla existe para impedir que `foo = "*"` entre en el árbol; lo
que atrapaba era la arquitectura del proyecto.

## 4. El ruido, parte dos: la prohibición de `rand_core`

Documentada en RPT-109: prohibía el crate de rasgos en lugar de los generadores
peligrosos, y bloqueaba trece dependencias criptográficas correctas —dos propias—
sin atrapar ni un peligro real.

## 5. La conclusión que une las dos

> **Las dos comprobaciones que fallaban lo hacían por el mismo motivo: reglas
> escritas contra un riesgo real que, al no haberse ejecutado nunca, nunca se
> calibraron contra el árbol.**

Trece crates legítimos en un caso, diecisiete dependencias internas en el otro,
cero peligros atrapados entre ambas. El `deny.toml` era aspiracional: describía la
política que se quería, no la que el árbol podía satisfacer.

Y el precio no fue el falso positivo. Fue que **`advisories`, la única de las
cuatro que estaba haciendo su trabajo, quedó enterrada**. Las cuatro
comprobaciones comparten un trabajo y un estado de salida: dos reglas descalibradas
compran el silencio de las otras dos.

**Esto es lo que RPT-106 describía con otra forma.** Allí una barrera no corría.
Aquí corría, fallaba, y su fallo no era legible. Una barrera cuyo rojo nadie puede
atribuir no protege: sólo consume atención hasta que alguien aprende a ignorarla.

## 6. Lo decidido

### 6.1 `publish = false` en los diez crates de biblioteca

Los crates llevan dependencias `path` entre sí, y **crates.io rechaza paquetes con
dependencias `path`** salvo en `dev-dependencies`. De modo que hoy son
impublicables tal como están: declararlo **enuncia un hecho, no impone una
política.**

Y es la condición que `allow-wildcard-paths` necesita, porque sólo exime los
caminos `path` de crates privados.

`publish = false` va en `[workspace.package]`, y los diez crates lo heredan con
`publish.workspace = true`, igual que ya hacían con `version`, `license` y el
resto. `xtask` y `fuzz` ya lo declaraban.

**No toca la frontera open-core de RPT-003 §2.7.** Abierto bajo Apache-2.0 y
publicado en crates.io son cosas distintas: el cliente institucional audita el
repositorio, no el registro.

### 6.2 `allow-wildcard-paths = true` en `[bans]`

Exime los caminos `path` de los crates privados. **Las versiones flotantes de
crates.io siguen prohibidas**, que es lo que la regla quería decir.

Se descartó poner `version = "0.1.0"` junto a cada `path`: elimina el comodín de
raíz y deja la puerta abierta a publicar, pero obliga a mantener cada subida de
versión sincronizada en dos sitios — y «dos sitios donde decidir lo mismo» es
precisamente lo que la plantilla de configuración de este proyecto llama un sitio
donde ganarle a la firma.

## 7. Lo que los duplicados sí dicen

`multiple-versions = "warn"` reporta quince crates duplicados. **No son quince
problemas: son uno.**

```
block-buffer 0.10.4 → digest 0.10.7 → blake2, curve25519-dalek, hmac, sha2
block-buffer 0.12.1 → digest 0.11.3 → sha3, shake, signature 3.0 → ml-kem, ml-dsa
```

Es la **transición de generación de RustCrypto, 0.10 → 0.11, a caballo**.
`motor-pqc` vive en la nueva porque ML-KEM y ML-DSA sólo existen allí;
`ed25519-dalek`, `hkdf` y `sha2` siguen en la vieja. De ahí salen `block-buffer`,
`crypto-common`, `digest`, `const-oid`, `der`, `pkcs8`, `spki` y `signature`:
**ocho de los quince son un único fenómeno.**

Y los tres `rand_core` que RPT-086 §3 documentó como «ya nos costaron una sesión»
son el mismo fenómeno desde otro ángulo: `0.6.4` cuelga de la generación vieja y
`0.10.1` de la nueva.

Eso cambia la lectura. No hay quince deudas: hay **una transición de ecosistema a
medias**, que se cierra sola cuando `ed25519-dalek`, `hkdf` y `sha2` publiquen
sobre `digest 0.11`. Hasta entonces `warn` es la postura correcta, y endurecerla
sería pelear contra el calendario de RustCrypto.

Esto **no cierra PA-18** (fuente de aleatoriedad, RPT-005 §7.4), ni el endurecimiento
que RPT-109 §5 dejó anotado como pendiente.

## 8. Dos cosas medidas de paso

1. **`rust-version` está en dos sitios.** `[workspace.package]` dice
   `rust-version = "1.85"` y `rust-toolchain.toml` dice `channel = "1.85"`. Hoy
   coinciden. El día que no coincidan, uno de los dos mandará y el otro mentirá —
   y RPT-109 §11.2 documenta lo que pasa cuando un fichero de configuración afirma
   una cobertura que no tiene.
2. **El toolchain fijado impide compilar la propia compuerta.** `cargo-deny 0.20.2`
   exige rustc >=1.88; el rustc activo en el repositorio es 1.85.1. Hubo que
   instalarla con `cargo +stable install`. La disciplina de pre-commit que
   RPT-003 §9.4 pide es, con el pin actual, imposible de cumplir con la misma
   versión que corre CI.

## 9. Lo que falta para PA-165

| | Estado |
|---|---|
| `bans` — comodines | Arreglado en este commit; sin verificar contra la herramienta |
| `bans` — `rand_core` | Arreglado en RPT-109 |
| `advisories` — `ctutils` | **Pendiente**: `cargo update -p ctutils` + suite |
| `licenses`, `sources` | Ya en verde |

**PA-165 no se cierra con este reporte.** Su criterio sigue siendo una corrida
entera en verde, y falta la vulnerabilidad de §2.

## 10. Apéndice de método — tres errores propios

1. **Busqué el comodín equivocado.** Comprobé `version = "*"` en los manifiestos,
   no encontré ninguno, y descarté `wildcards` como causa. Era cierto e
   irrelevante: el comodín que cuenta `cargo-deny` es la *ausencia* de requisito de
   versión, que es lo que toda dependencia `path` tiene.
2. **Crucé la base RustSec sólo en su ventana reciente.** Mi marco era «algo cambió
   fuera en la última hora», así que miré los doce avisos más nuevos y concluí que
   ninguno afectaba al árbol. El de `ctutils` es anterior a esa ventana. La premisa
   inventada de RPT-109 §10 no sólo me hizo buscar la causa equivocada: me hizo
   buscar en el sitio equivocado.
3. **Reporté con una herramienta que acababa de ver fallar.** Escribí un analizador
   de manifiestos para comprobar dos valores por defecto; dijo «ningún duplicado»
   cuando `sha2 = "0.10"` estaba a la vista en tres ficheros. No capturaba la forma
   `nombre.workspace = true`. Se rehízo con grep plano.

Los tres comparten forma: **una medición que no mide lo que uno cree, aceptada
porque su resultado encajaba con la hipótesis de turno.** Es el mismo defecto que
§5 describe en el `deny.toml`, cometido sobre el diagnóstico en lugar de sobre la
configuración.

---

## 11. Addendum — el pin del toolchain impedía remediar la vulnerabilidad

`cargo update -p ctutils` **no hizo nada**, y lo dijo:

```
Locking 0 packages to latest Rust 1.85 compatible versions
```

El resolutor de cargo respeta `rust-version = "1.85"` de `[workspace.package]`.
Medido contra el registro:

| Versiones de `ctutils` | `rust-version` exigida |
|---|---|
| 0.4.8 … 0.4.3 | **1.87** |
| 0.4.2 y anteriores | 1.85 |

El corte cae exactamente entre 0.4.3 y 0.4.2. Y el aviso se corrige en **0.4.4**.

> **No existe una versión corregida compatible con 1.85. El pin del toolchain
> impedía remediar una fuga de canal lateral temporal en el motor poscuántico.**

Lo que RPT-109 §4 describió como «la compuerta no mide lo que declara», y §11.2
como «no se puede compilar la propia compuerta», era el mismo defecto visto desde
dos ángulos benignos. Éste es el tercero y no es benigno: **el pin estaba
bloqueando un arreglo de seguridad, en silencio, con un mensaje que nadie leyó
como un bloqueo.**

### 11.1 Por qué 1.88 y no 1.87

| Exigencia | Versión mínima |
|---|---|
| `ctutils` >= 0.4.4 (remediar RUSTSEC-2026-0385) | 1.87 |
| `cargo-deny` 0.20.2 — la que CI corre vía `@v2` | 1.88 |

**1.88 es el número más bajo que permite a la vez remediar la vulnerabilidad y
correr la compuerta en local con la misma versión que CI**, que es la disciplina de
pre-commit que RPT-003 §9.4 exige y que con 1.85 era imposible de cumplir.

Son **tres releases**, no las doce del `1.97.0` que circulaba sin respaldo
documental. El número sale de dos mediciones, no de una preferencia.

Nota de crédito: la propuesta de mover el toolchain venía de RUA, y este reporte
criticó el `1.97.0` por no aparecer en ningún fichero del repositorio. Esa crítica
era correcta sobre el número y equivocada sobre el fondo: **el pin sí tenía que
moverse, y por una razón más urgente que el housekeeping.**

### 11.2 El riesgo del bump, y cómo se acota

`RUSTFLAGS: "-D warnings"` y `cargo clippy -- -D warnings` significan que cada
release de Rust puede añadir lints que tiñan `Analisis estatico`. Tres releases es
una superficie pequeña, pero no nula.

Se acota corriendo la compuerta completa en local **antes** de empujar: `fmt`,
`clippy -D warnings`, la suite entera y `cargo deny check`. Si clippy encuentra algo,
es trabajo real y va en su propio commit, separado del bump.

### 11.3 Dos sitios, a propósito

`rust-version` en `[workspace.package]` y `channel` en `rust-toolchain.toml` dicen
ahora lo mismo, y §8.1 señalaba que dos sitios para una decisión es un problema. Se
mantienen los dos porque **sirven a herramientas distintas**: el primero gobierna el
resolutor de cargo —que es quien bloqueó el `update`— y el segundo gobierna rustup.
No se pueden unificar: `dtolnay/rust-toolchain` **no lee** el segundo, porque su
campo `toolchain` es obligatorio y aborta si llega vacío (RPT-109 §11.2).

Quedan documentados en ambos ficheros con la obligación de coincidir. Es la clase de
invariante que merece una prueba en `cargo xtask verificar`, y se deja anotado como
tal sin abrir punto todavía.

---

## 12. PA-165 cerrado — la primera corrida entera en verde

**CI #8, commit `a5d626a`, estado `Success` en 4m 5s.** Los nueve trabajos, leídos
uno a uno:

| Trabajo | |
|---|---|
| Formato | ✅ |
| Analisis estatico | ✅ |
| Implementaciones inconclusas | ✅ |
| Indices derivados, no escritos a mano | ✅ |
| Secretos expuestos | ✅ |
| Licencias y vulnerabilidades | ✅ |
| SBOM | ✅ |
| Pruebas (ubuntu-latest) | ✅ |
| Pruebas (windows-latest) | ✅ |

El criterio de PA-165 estaba escrito en su propia fila del tablero —*«el punto se
deja abierto hasta que la primera ejecución real se observe en verde»*— y **se
cumplió sin reinterpretarlo**, que fue la tentación recurrente de toda la
investigación. Se intentó cerrarlo por aproximación tres veces: con ocho de nueve
en CI #3, con ocho de nueve en CI #4, y con siete de nueve leídos en CI #7 —esa
última a punto de cantarse como verde completo hasta que se fue a buscar los dos
trabajos de la matriz que faltaban, y resultaron estar en rojo—.

### 12.1 Y PA-166 confirmado en CI, con dos días de retraso deliberado

El trabajo `Pruebas (windows-latest)` viene pasando **desde CI #3**, luego la
confirmación a máquina e independiente del arreglo de PA-166 existía desde el
2-nov. No se anotó hasta ahora por decisión explícita de RUA: **tener un trabajo
verde dentro de una corrida roja no es haber leído la corrida.** Esa espera de dos
días es la razón por la que la línea del tablero se puede escribir hoy sin ningún
matiz.

### 12.2 Lo que el camino hasta aquí dejó medido

| | |
|---|---|
| Corridas rojas antes del verde | **cinco** (#3 a #7) |
| Reglas de `deny.toml` mal especificadas | 2 — `rand_core` (RPT-109) y `wildcards` (§3) |
| Dependencias legítimas bloqueadas por ellas | 13 + 17 |
| Peligros reales que atrapaban | **cero** |
| Vulnerabilidades reales que la compuerta sí encontró | 1 — RUSTSEC-2026-0385 (§2) |
| Días que ese aviso estuvo reportándose sin poder leerse | ≥ 4 |
| Errores de método propios, registrados | 3 en §10, 1 en §11, 3 más abajo |

### 12.3 Tres errores más, del tramo final

Van aquí y no en §10 porque ocurrieron después de publicarlo, y porque los tres
tienen la misma forma que el defecto que este reporte describe en el `deny.toml`:
**un instrumento que no mide lo que se cree que mide.**

1. **Se empujó `34eeb1e` sin correr la compuerta.** El bump a 1.88 salió porque la
   verificación y el commit se entregaron en el mismo paso, y cuando `rustup` abortó
   la cadena con un fallo de su propio self-update, el commit seguía disponible para
   ejecutarse. **Lo cazó `el_atestado_en_disco_describe_este_arbol`**, que puso los
   dos `Pruebas` en rojo y nombró el delta exacto: «canal del toolchain -> 1.88». La
   instrumentación del proyecto funcionó mejor que el proceso que la rodeaba.
2. **Un filtro diseñado para no dar evidencia del éxito.** `Select-String` sobre
   `FAILED` imprime nada cuando todo pasa, con lo que «pasó» y «no se ejecutó» se
   vuelven indistinguibles — y la vez anterior el caso real había sido el segundo.
3. **`Select-String` es insensible a mayúsculas por defecto**, así que el patrón
   `FAILED` casaba con el `0 failed` de cada línea `test result: ok`: se contaron 29
   éxitos como 29 fracasos. Resuelto con `-CaseSensitive`.

La lección es la de §10 con una vuelta más: no basta con exigir una medición antes
de afirmar. **El instrumento de medida también es código sin probar**, y en una
sesión larga falla tanto como lo que mide.

### 12.4 Lo que sigue abierto

- **PA-14c** — *«Atestación de conformidad PQC (`CONFORMIDAD.lock`)»*, 🟡 abierto.
  El atestado ata *qué* se probó, no *que* se probara: componer la huella sin
  ejecutar nada es posible, y lo declara la propia salida de
  `cargo xtask conformidad`. Misma forma que los tres errores de §12.3.

  **Dos referencias obsoletas, de paso.** El mensaje de `xtask` dice «Cerrarlo es
  PA-14» y la fila de PA-121 dice «eso sigue siendo PA-14», pero **`PA-14` está
  «✅ Partido»** desde el 5-ago-2026 (RPT-021 §1) en `PA-14a`, `PA-14b` —cerrado— y
  `PA-14c`. El punto vivo es el tercero. Es la familia de PA-160: un identificador
  citado desde el código apuntando a un padre que ya no existe como punto, y
  ninguna barrera lo nota porque la de PA-108 sólo comprueba que el identificador
  **tenga fila**, no que siga siendo el vigente. No se toca aquí.
- **PA-18** — fuente de aleatoriedad, DRBG conforme al NIST (RPT-005 §7.4). La
  proliferación de versiones de `rand_core` de §7 le pertenece.
- **PA-169** — bloqueante, con la rev-2 de RUA en discusión y el Paso 0 pendiente.
- **Los tres nombres de rama** — `principal`, `Principal` (raíz huérfana) y `Origen`
  (por defecto en GitHub, retirada por RPT-002 §2.1). Sin consolidar.
- **El `@v2` de `cargo-deny-action` y el `@master` que el arreglo del toolchain
  necesitaría** — referencias flotantes en la capa que vigila a todas las demás.
  Conviene fijarlas a un SHA.
