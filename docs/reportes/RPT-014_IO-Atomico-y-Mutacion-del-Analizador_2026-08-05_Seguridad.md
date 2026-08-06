# RPT-014 — I/O Atómico y Resistencia del Analizador

**Tema:** Persistencia en disco y prueba por mutación del código no autenticado
**Nº de reporte:** 014
**Fecha:** 5 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** Canónico con reservas explícitas — véase §6

- **Depende de:** RPT-013 (formato en disco), RPT-012 (frescura)
- **Cierra:** PA-29
- **Abre:** PA-30
- **Modifica:** `formato::analizar`, que ahora exige orden canónico al leer

---

## 1. Escritura atómica y la guarda RAII

Un corte de energía a mitad de escritura dejaría un inventario truncado. El analizador lo rechazaría, así que el agente se quedaría **sin inventario** — y sin marcados, los equipos críticos dejan de estar protegidos. El fallo va en la dirección peligrosa.

De ahí: temporal en el **mismo directorio** —un renombrado entre sistemas de ficheros no es atómico—, `sync_all` antes de renombrar, y renombrado sobre el destino.

### 1.1 De comprobación explícita a guarda `Drop`

La primera versión limpiaba el `.parcial` con `if resultado.is_err()`. Cubría sólo los caminos de salida que quien la escribió recordó. Sustituida por `LimpiadorTemporal`, que se desarma por consumo tras el renombrado.

### 1.2 La salvedad que la propuesta original no contemplaba

Se afirmó que la guarda cubriría también el pánico, «durante el desenrollado de la pila». **En el binario enviado, no.** El perfil de release de este workspace declara `panic = "abort"` (`Cargo.toml`, línea 52): un pánico aborta el proceso sin desenrollar y el destructor no corre.

| | dev / pruebas | release |
|---|---|---|
| Retorno con `Err`, `?` | cubierto | cubierto |
| Pánico | cubierto | **no cubierto** |

La consecuencia es benigna y conviene dejarla escrita en lugar de callarla: un `.parcial` huérfano **no es corrupción**, porque el destino nunca se tocó. Es basura, y ni siquiera se acumula — `ruta_temporal` es determinista y la siguiente escritura lo trunca. El cargador lee una ruta fija y no puede confundirlos.

La prueba `la_guarda_limpia_ante_panico_cuando_hay_desenrollado` lleva ese nombre a propósito: documenta el mecanismo, no promete la garantía en producción.

### 1.3 Los dos caminos de limpieza

`un_fallo_de_renombrado_limpia_el_parcial` usa un directorio como destino, que es la única forma portable de hacer fallar `rename` sin depender de permisos —que se comportan distinto en Windows y en Unix—. El camino de fallo de E/S a mitad de escritura no es provocable de forma portable; se prueba el mecanismo de la guarda directamente, con salida temprana y con `catch_unwind`.

## 2. Lectura acotada en el lector, no en los metadatos

Consultar `metadata()` y después leer es una condición de carrera: entre ambas llamadas el fichero puede crecer. Y hay rutas que mienten sobre su tamaño. Se lee con `take(LONGITUD_MAXIMA + 1)` y se rechaza si llega el byte de más.

La frontera se comprueba por los dos lados. Un límite que rechace lo legítimo es tan defecto como uno que admita lo excesivo.

## 3. Hallazgo: el analizador normalizaba en silencio

Salió de preguntarse **qué invariante debe comprobar el arnés**, no de una revisión del analizador.

`analizar` leía las entradas en el orden del fichero y dejaba que `Inventario::construir` las ordenase. Consecuencia: **dos ficheros distintos producían el mismo inventario**, y ambos verificaban, porque la firma cubre la raíz y la raíz se calcula sobre las entradas ya ordenadas.

Es malleabilidad benigna —el contenido tras normalizar es idéntico y ninguna decisión cambia— pero rompe una propiedad que el resto del formato sí tiene: **una sola codificación válida por inventario**. Es la misma ambigüedad que cierran el rechazo de bytes sobrantes y `deny_unknown_fields` en el contrato IPC.

Corregido: `analizar` exige orden estrictamente ascendente de dirección y devuelve `ErrorFormato::EntradasDesordenadas`. Por ser estricto, la dirección repetida cae por el mismo camino.

Y, sobre todo, habilita la invariante del §4.

## 4. Arnés determinista — y lo que **no** es

```
Capa 1 — arnés en CI (estable 1.85)   Capa 2 — cargo-fuzz (nightly)
semilla fija, espacio pequeño          mutación guiada por cobertura
NO CRECE entre ejecuciones             crece
red de regresión y guardia             la afirmación de resistencia
contra pánicos evidentes               descansa aquí
```

**El arnés no es fuzzing.** Un mutador ciego con semilla fija repite las mismas rutas en cada ejecución. Vale como red de regresión; no sostiene la afirmación de que el analizador resiste entrada hostil. Esa afirmación descansa exclusivamente en la capa nightly. El reporte lo dice con estas palabras para que nadie lea «arnés en verde» y concluya de más.

### 4.1 Dos invariantes, no una

1. **`analizar` no entra en pánico.** Se cumple por el mero hecho de volver.
2. **Si acepta, la codificación es canónica**: reserializar devuelve los mismos bytes.

La segunda es la que aporta. Sin ella, un analizador que normalizase en silencio pasaría por bueno — que es exactamente el defecto del §3.

La comparación **excluye la firma**. Nada garantiza que `encode(decode(x))` devuelva los mismos bytes para una firma mutada que aún decodifique, y esa normalización sería un falso positivo ajeno al analizador. La canonicidad que importa es la del inventario: orden, longitudes y campos.

### 4.2 Operadores

Volteo de bit, byte a `0x00`, byte a `0xFF`, truncado —el defecto más común en un corte de energía—, cola sobrante, y sobrescritura del campo de número de entradas, que es el que gobierna la reserva de memoria. Entre una y cuatro mutaciones por caso, 20 000 casos sobre fichero válido más 10 000 de bytes arbitrarios, la mitad con cabecera válida para llegar más adentro.

## 5. El objetivo de `cargo-fuzz`

`fuzz/` tiene **workspace propio**. `cargo-fuzz` exige nightly y el proyecto está fijado a estable 1.85; incluirlo en el workspace principal obligaría a todo el proyecto a compilar con nightly. `cargo test --workspace` no lo toca.

```text
cargo install cargo-fuzz
cargo +nightly fuzz run analizar
```

El corpus **se versiona**: es el conjunto de entradas que ya demostraron alcanzar rutas nuevas, y perderlo obliga a redescubrirlas en cada máquina. Los casos que provoquen fallo también, y además deben migrarse al arnés determinista para que CI los vigile.

## 6. Verificación

`crates/guardian-cc` pasa de 62 a **77 pruebas**; el workspace, de 173 a **188**. Clippy con `-D warnings` limpio en los dos workspaces —el principal y `fuzz/`—.

Reparto de las quince nuevas: once de `disco`, dos del arnés y dos de orden canónico.

### 6.1 Lo que costó el arnés

El tiempo de `guardian-cc` pasa de 0,10 s a **3,17 s**. Los tres segundos son íntegramente del arnés: 20 000 casos sobre fichero válido más 10 000 de bytes arbitrarios, en modo depuración.

Se deja así. Tres segundos en la única suite que ejercita el analizador es un precio razonable, y bajarlo tendría que justificarse con una medida, no con una impresión. Si algún día molesta, la palanca es el número de casos —no compilar en release, que desalinearía CI del entorno local y dejaría dos configuraciones donde nadie sabría cuál manda.

### 6.2 El objetivo de fuzzing compila, pero no se ha construido como tal

`cargo clippy --manifest-path fuzz/Cargo.toml` termina limpio: la macro `fuzz_target!` expande, los tipos cuadran y la invariante compila. `cargo +nightly fuzz build` falla con `no such command: fuzz` porque `cargo-fuzz` no está instalado en la máquina.

Es la distinción entre «el código es válido» y «el objetivo se ha construido con libFuzzer». Lo primero está comprobado; lo segundo no, y forma parte de PA-30 junto con la ejecución.

## 7. Reservas explícitas

1. **El objetivo de fuzzing no se ha ejecutado.** Existe y está documentado; nadie ha corrido una sesión. Hasta que alguien lo haga, la afirmación del §4 sobre dónde descansa la resistencia describe una intención, no un resultado.
2. **No hay corpus semilla versionado.** Sin al menos un fichero válido, el fuzzer gasta mucho tiempo antes de acertar con los ocho bytes del mágico. Generarlo requiere firmar, y la clave de prueba vive en el módulo de pruebas de `guardian-cc`.
3. **`escribir_atomico` no rechaza contenido vacío.** Escribir cero bytes sobre un inventario bueno lo destruye, y `analizar` rechaza cualquier fichero por debajo de la cabecera, así que un inventario vacío nunca es válido. Es un guardia barato contra un defecto de quien llama, y no está.
4. **El `sync` del directorio no se hace.** Tras el renombrado, la entrada de directorio puede no estar en disco. No es portable a Windows con `std`, y la consecuencia —volver a la versión anterior del inventario, íntegra— es benigna comparada con el truncado que la escritura atómica sí evita.
5. **Nadie llama a `disco.rs`.** `InventarioLocal::cargar` sigue recibiendo `&[u8]`. Falta el cableado que decida rutas, permisos y momento de carga.

La reserva 1 es la que separa este reporte de poder afirmar lo que su §4 describe.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-30** | **Instalación, construcción y ejecución del fuzzing.** `cargo install cargo-fuzz`, construcción con libFuzzer, sesión inicial, corpus semilla versionado y política sobre los hallazgos | La afirmación de resistencia del analizador |

---

*Reporte Nº 14 — I/O Atómico y Resistencia del Analizador · PremosCorp · 5 de agosto de 2026*
