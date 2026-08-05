# Reporte de Construcción Nº 6 — Contrato IPC y Verificadores Triestáticos

| Campo | Valor |
|---|---|
| **Tema documentado** | Contrato IPC entre Eje-Visión y Eje-Agente, y principio de verificación triestática |
| **Número de reporte** | 006 |
| **Fecha** | 5 de agosto de 2026 |
| **Área designada** | Interfaz |
| **Entidad / Firma** | PremosCorp |
| **Estado** | Canónico — verificado contra ejecución real |

## Trazabilidad

- **Depende de:** RPT-004 §6 (modelo de seguridad de Electron), RPT-002 §9.3 (sin puerto TCP local)
- **Cierra:** PA-20
- **Abre:** PA-21
- **Complementa:** RPT-003 §9 (política de calidad), al que aporta el principio de §4

---

## 1. El Hueco que Cierra

`eje-vision` declaraba sus canales en TypeScript. `eje-agente` no tenía definición equivalente. **Las dos mitades del puente estaban inventadas por separado y no se habrían encontrado.**

No era un descuido puntual: es la consecuencia inevitable de que Rust y TypeScript no puedan compartir tipos. Sin un mecanismo que lo impida, dos declaraciones paralelas divergen.

---

## 2. Arquitectura del Contrato

```
                    ┌───────────────────────────┐
                    │    contrato-ipc.toml      │
                    │  (fuente única de verdad) │
                    └─────────────┬─────────────┘
                                  │
              ┌───────────────────┴───────────────────┐
              ▼                                       ▼
   ┌─────────────────────┐               ┌─────────────────────────┐
   │  crates/eje-ipc     │               │  apps/eje-vision        │
   │  src/pruebas.rs     │               │  contrato.prueba.ts     │
   └──────────┬──────────┘               └────────────┬────────────┘
              │                                       │
              ▼                                       ▼
      cargo test (14 pruebas)              node --test (5 pruebas)
```

### 2.1 Fuente única de verdad

Todo canal, cota y motivo de prohibición vive en `contrato-ipc.toml`. Ninguna de las dos implementaciones declara nada por su cuenta sin que el manifiesto lo registre.

### 2.2 Paridad dual comprobada

**La paridad no se verifica en tiempo de compilación.** Ambos lados la comprueban con **pruebas** que leen el manifiesto y fallan si su definición local diverge:

| Extremo | Fichero | Qué comprueba |
|---|---|---|
| Rust | `crates/eje-ipc/src/pruebas.rs` | Que `enum Canal` coincide con `[[canal]]`, que ningún `[[prohibido]]` es alcanzable, y que `LONGITUD_MAXIMA_MARCO` coincide con el manifiesto |
| TypeScript | `proceso-principal/src/pruebas/contrato.prueba.ts` | Lo mismo, más el **orden** de los canales y que cada prohibición declare su motivo |

El orden importa: si un lado reordena, un diff futuro parecería inocuo cuando en realidad cambió la correspondencia posicional.

### 2.3 Fricción de tres puntos, deliberada

Añadir un canal exige tocar el manifiesto, `crates/eje-ipc/src/lib.rs` y `puente.ts`. **Un canal es una ampliación de la superficie de ataque del proceso privilegiado**, que es el mismo que habla con `guardian-cc` y puede solicitar contención sobre la red del cliente. No debe poder añadirse sin que tres revisiones lo vean.

### 2.4 Canales prohibidos

No son una lista de bloqueo —la autoridad es la lista de permitidos— sino **pruebas de regresión con su motivo escrito**. Una prohibición sin razón se erosiona: alguien la revisa dentro de un año, no encuentra por qué está ahí, y la borra. Por eso una prueba exige que cada `[[prohibido]]` declare su `motivo`.

---

## 3. Transporte con Prefijo de Longitud

**Cota:** `1 048 576` bytes (1 MiB), declarada en `contrato-ipc.toml` y en `LONGITUD_MAXIMA_MARCO`.

**Prefijo:** 4 bytes big-endian antes de cada carga útil. Un flujo sin delimitar obliga al receptor a adivinar dónde termina un mensaje — la misma clase de ambigüedad ya corregida en `eje-almacen` y en el combinador poscuántico.

**Validación antes de reservar.** La longitud declarada se comprueba contra la cota **antes** de tocar memoria. Un prefijo malicioso que declare cuatro gigabytes sería una denegación de servicio de un solo paquete si se reservara primero.

**La cola se descarta.** El transporte puede entregar varios marcos en una lectura; devolver el sobrante mezclaría dos mensajes.

---

## 4. Verificación Triestática

### 4.1 El principio

Todo verificador debe modelar **tres** estados y no permitir que colapsen entre sí:

| Estado | Significado |
|---|---|
| `Conforme` | Se ejecutó el análisis y no hay violaciones |
| `ViolacionDetectada` | Se ejecutó el análisis y hay violaciones |
| `ComprobacionImposible` | **No se comprobó nada** |

El tercero es el que se olvida y el más peligroso. Colapsado en el primero produce **falsos verdes**; colapsado en el segundo manda a buscar problemas inexistentes.

### 4.2 Crónica de cinco degradaciones reales

El principio no es doctrina: es la respuesta a cinco fallos sufridos construyendo un solo guardián.

| # | Causa | Efecto | Clase |
|---|---|---|---|
| 1 | `options.exclude` retiraba `dist` del grafo, y los paquetes hermanos se resuelven a través de su `dist` | La regla crítica quedó **inerte**: verde con la violación presente | **Falso verde** |
| 2 | `npx.cmd` no arranca con `execFile` sin `shell: true` desde CVE-2024-27980 | La excepción se interpretó como violación de frontera | Falso rojo |
| 3 | Expresión regular sobre el informe legible no reconocía su propia salida | Se declaró "el guardián no protege" cuando protegía | Falso rojo |
| 4 | Con `--output-type json`, la herramienta sale con código 0 aunque detecte violaciones | El veredicto seguía tomándose sobre el código de salida | Falso rojo |
| 5 | Una salida ininterpretable devolvía `reglas: []` | Se leía como "sin violaciones" | **Falso verde** |

Cuatro de los cinco los introdujo el arreglo del anterior. La raíz común: **el guardián no podía decir con precisión qué había visto**, y el estado se infería de señales incidentales —código de salida, texto para humanos— en lugar de ser un dato explícito.

### 4.3 Consecuencias adoptadas

**Consumir salida estructurada.** Un informe legible no es un contrato: su formato cambia entre versiones, plataformas y ajustes de color. Se lee `summary.violations[].rule.name` del JSON.

**El estado es un dato, no una inferencia.** `cruzar()` devuelve `estado` explícito. Ningún llamante deduce nada de un código de salida.

**La prueba negativa cubre los tres estados.** Sin ella, la rama de imposibilidad sería fe y no garantía: existiría en el código sin que nada comprobara que sigue existiendo.

| Caso | Estado esperado |
|---|---|
| Árbol limpio | `Conforme` |
| Base importa de empresarial | `ViolacionDetectada` |
| Vista del renderer importa `node:fs` | `ViolacionDetectada` |
| Base importa del proceso principal | `ViolacionDetectada` |
| La herramienta no está instalada | `ComprobacionImposible` |
| La herramienta responde algo que no es JSON | `ComprobacionImposible` |

El último es el traicionero: el proceso termina con éxito aparente, y un guardián descuidado lo leería como "sin violaciones". Es exactamente el incidente 5.

---

## 5. Registro de Verificación

Ejecución real del 5 de agosto de 2026, `npm run verificar`:

```
verificar:tipos       tsc --build .................................. sin errores
verificar:frontera    depcruise .................. 0 violaciones, 23 modulos
verificar:frontera:negativa ......................... 6/6 casos, 3 estados
probar                node --test ......................... 29/29 pruebas
```

Y en el lado Rust, `cargo test --package eje-ipc`: **14/14**.

---

## 6. Puntos Abiertos

| ID | Punto | Bloquea |
|---|---|---|
| ~~**PA-21**~~ | ~~**Tipado de carga útil por canal.** El manifiesto blinda qué canales existen, su orden, la cota y las prohibiciones — pero **no la forma de los mensajes**. Es la siguiente capa donde los dos extremos pueden divergir en silencio~~ | ✅ Cerrado 5-ago-2026 — **RPT-007** |

El patrón a aplicar es el mismo: declarar los esquemas en el manifiesto y comprobar la paridad desde ambos lados. Lo que no debe hacerse es declararlos dos veces y confiar.

> **Nota de cierre (5-ago-2026).** RPT-007 aplicó ese patrón y documentó dos hallazgos que este reporte no anticipaba: la **asimetría de fallo** entre `serde` y TypeScript ante el mismo defecto, y que `satisfies` cubre solo la mitad del problema. Las cifras de §5 son las vigentes en el momento de redactar este reporte y se conservan sin retocar.

---

*Reporte Nº 6 — Contrato IPC y Verificadores Triestáticos · PremosCorp · 5 de agosto de 2026 · Estado: Canónico*
