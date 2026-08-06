# RPT-016 — Persistencia Autenticada del Registro de Revocaciones

**Tema:** El registro de revocaciones deja de vivir sólo en memoria
**Nº de reporte:** 016
**Fecha:** 5 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** Canónico

- **Depende de:** RPT-015 (revocación), RPT-014 (E/S atómica y arneses), RPT-013 (disciplina del analizador)
- **Cierra:** PA-34
- **Abre:** PA-35

---

## 1. La brecha

RPT-015 §5 hablaba de «un fichero de revocaciones en el almacén local». Lo que la implementación entregó fue `RegistroRevocaciones` **en memoria**: cada arranque olvidaba las revocaciones.

No era grave —el §5 ya establece que perder el registro devuelve al estado previo, no por debajo— pero significaba que la revocación duraba lo que durase el proceso, y el reporte decía otra cosa.

## 2. La decisión: se guardan los certificados, no el par derivado

La opción cómoda era persistir el registro tal cual: pares `(identificador, corte)`. Se descartó.

Ese fichero vive en un almacén que el modelo de amenazas asume manipulable. Sin firma, **editar un corte al alza afloja una revocación sin dejar rastro**. Y a diferencia de borrar el fichero —que es tosco y devuelve a un estado conocido— modificar una entrada entre varias es silencioso.

Llevábamos cinco reportes negándonos a confiar en un fichero declarado manipulable. Así que se guardan los certificados **con su firma**, y al cargar se reverifica cada uno.

Coste: unos 3,4 KB por revocación, para un evento que ocurre casi nunca.

Lo que queda como manipulación posible es **borrar el fichero**, y eso devuelve al estado previo a la revocación, recuperable volviendo a presentar el certificado (RPT-015 §5). El diseño no cierra esa puerta; la deja donde ya estaba.

## 3. Formato

Misma disciplina que RPT-013:

```text
magico     8 bytes  "EJE-REV1"
version    u16 BE
anotaciones u32 BE
  ── por anotación, ancho fijo ──
  revocada 32 | hasta_secuencia u64 BE | sucesora 32 | emitido_en u64 BE | firma
```

Ancho fijo para validar el número declarado contra los bytes restantes **antes de reservar**; orden canónico ascendente por clave revocada; rechazo de bytes sobrantes.

### 3.1 Una diferencia deliberada con el inventario

**Un fichero de revocaciones vacío es válido.** Un inventario vacío nunca lo es —no tiene raíz y no significa nada—, pero cero revocaciones es el estado nominal de la inmensa mayoría de los despliegues. Tratarlos igual habría hecho que un agente sano no pudiera arrancar.

## 4. El segundo analizador, y por qué llegó tarde

`ArchivoRevocaciones::analizar` es la **segunda entrada no autenticada** del producto. Llegó tres reportes después de la primera, y el arnés de RPT-014 no la cubría: protegía un analizador y dejaba el otro a la intemperie.

Corregido con `el_analizador_de_revocaciones_resiste_mutaciones`.

### 4.1 Una invariante más fuerte que la del inventario

En el arnés del inventario hubo que **excluir la firma** de la comparación de canonicidad: `formato::analizar` no verifica nada, así que una firma mutada que aún decodifique podría reencodearse distinta y producir un falso positivo ajeno al analizador.

Aquí la comparación **sí incluye la firma**. Un `Ok` significa que verificó, y una firma que verifica está bien formada, así que reencodearla no puede normalizarla. La invariante es más fuerte precisamente porque el analizador hace más trabajo.

## 5. Verificación

`crates/guardian-cc` pasa de 88 a **97 pruebas**; el workspace, de 199 a **208**. Clippy limpio.

Las dos que sostienen el reporte:

- `alterar_un_corte_en_el_fichero_se_detecta` — el motivo entero de guardar la firma.
- `el_registro_sobrevive_a_un_ciclo_por_disco` — PA-34 de extremo a extremo, pasando por `escribir_atomico` y `leer`.

### 5.1 El arnés costaba diez veces más de lo aceptable

Con 10 000 casos, `guardian-cc` pasó de 3,23 s a **32,28 s**. La causa: cada caso estructuralmente válido dispara una verificación de firma híbrida completa, que en modo depuración cuesta milisegundos y no microsegundos — al contrario que el arnés del inventario, donde `analizar` sólo decodifica.

Se bajó a 2 000. Los 8 000 retirados compraban muy poco: el mutador es ciego y con semilla fija, así que **no crece ni explora rutas nuevas por repetir**. Veintinueve segundos en cada ejecución de cada desarrollador es el precio al que la gente empieza a saltarse la suite, y una suite que se salta protege menos que una más corta que se ejecuta.

Es la aplicación de la palanca que RPT-014 §6.1 dejó identificada, ahora con un número detrás.

## 6. Reservas explícitas

1. **No hay objetivo de `cargo-fuzz` para este analizador.** `fuzz/fuzz_targets/` sigue teniendo sólo el del inventario. El de revocaciones necesita construir una `ClaveInventario` dentro del objetivo, lo que obliga a llevar un generador determinista al crate de fuzzing. Pasa a PA-30.
2. **El operador de mutación del campo de cuenta apunta al desplazamiento del inventario** (bytes 18..22), no al de revocaciones (10..14). En este fichero esa escritura cae dentro del primer identificador: sigue siendo una mutación válida, pero no ataca el campo que gobierna la reserva. Los volteos de bit lo alcanzan por azar; un operador específico lo haría a propósito. No se tocó `mutar` para no desestabilizar el arnés del inventario, que ya estaba verde.
3. **Nadie llama a esto todavía.** `ArchivoRevocaciones` existe y funciona; qué ruta usa el agente, cuándo carga y cuándo guarda no está decidido. Es PA-35, y es el mismo hueco que `disco.rs` tenía en RPT-014 §7.5.

La reserva 3 se repite por tercera vez en el proyecto: mecanismo listo, cableado ausente. Conviene atacarla de una vez en lugar de por partes.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-35** | **Cableado de la persistencia.** Rutas, momento de carga y de guardado, y qué hace el agente al arrancar sin fichero de revocaciones o sin inventario | Despliegue real |

---

*Reporte Nº 16 — Persistencia Autenticada del Registro de Revocaciones · PremosCorp · 5 de agosto de 2026*
