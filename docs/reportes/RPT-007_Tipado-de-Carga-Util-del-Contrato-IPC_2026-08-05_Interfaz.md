# RPT-007 — Tipado de Carga Útil del Contrato IPC

**Tema:** Forma de los mensajes del puente Eje-Visión ↔ Eje-Agente
**Nº de reporte:** 007
**Fecha:** 5 de agosto de 2026
**Área designada:** Interfaz
**Entidad:** PremosCorp
**Estado:** Canónico

- **Depende de:** RPT-006 (contrato IPC y principio triestático), RPT-004 §6.2 (lista de permitidos)
- **Cierra:** PA-21
- **Abre:** ninguno
- **Complementa:** RPT-003 §9 (política de calidad)

---

## 1. El hueco

RPT-006 dejó blindado **qué canales existen**, en qué orden, con qué cota de longitud y cuáles están prohibidos. No blindó **qué viaja por ellos**.

La distinción no es académica. Un canal correcto que transporta una estructura distinta en cada extremo es exactamente el mismo fallo que un canal inexistente, salvo que se manifiesta más tarde y en otro sitio.

## 2. La asimetría del fallo

El motivo por el que este punto era urgente es que **los dos extremos fallan de forma distinta ante el mismo defecto**.

| | Rust (`eje-agente`) | TypeScript (`eje-vision`) |
|---|---|---|
| Campo sobrante | `serde` con `deny_unknown_fields` **rechaza ruidosamente** | se ignora en silencio |
| Campo ausente | `serde` **rechaza ruidosamente** | `respuesta.version` devuelve `undefined` |
| Dónde aflora | en el borde, con el mensaje entero a la vista | tres capas más arriba, sin el mensaje a la vista |

El lado ruidoso da la impresión de que el contrato está protegido. No lo está: solo lo está la dirección renderer → agente. La dirección agente → renderer, que es la que alimenta las vistas de VIS-04, era la desprotegida — y es la que se degrada en silencio.

## 3. Mecanismo adoptado

El mismo patrón que ya sostiene los canales y los vectores de prueba: **un manifiesto declarativo único y una comprobación de paridad ejecutable en cada extremo**.

`contrato-ipc.toml` gana tres tablas:

```toml
[[registro]]        # los cinco registros del puente
[[campo]]           # registro + nombre (camelCase, tal como viaja) + tipo
[[mensaje]]         # canal + direccion + forma
```

Vocabulario de tipos deliberadamente pequeño — `texto`, `entero`, `booleano`, `enumerado`, `lista<T>` —. No es un sistema de tipos: es el mínimo necesario para que una divergencia sea detectable. Ampliarlo sin necesidad convertiría el manifiesto en un segundo lenguaje que mantener.

### 3.1 Lado Rust

`crates/eje-ipc/src/mensajes.rs` define los `struct` con `serde(rename_all = "camelCase", deny_unknown_fields)` y, junto a cada uno, una constante `CAMPOS_*`. La constante se ata al `struct` mediante **desestructuración exhaustiva sin `..`**: añadir un campo rompe la compilación de la prueba, no la deja pasar en silencio.

### 3.2 Lado TypeScript

TypeScript borra sus tipos al compilar, así que la constante `CAMPOS_*` no puede derivarse de la interfaz. Hacen falta **dos** mecanismos, y aquí está la parte que casi se me escapa:

`satisfies readonly (readonly [keyof T, string])[]` cubre **solo el campo sobrante**. Rechaza un nombre que no exista en la interfaz. No dice nada sobre claves que la interfaz tenga y la tabla no declare. Una redacción anterior de este mismo documento afirmaba que `satisfies` obligaba a declarar todas las claves. Era falso, y de haberse quedado así la constante habría sido una lista optativa con aspecto de contrato.

El campo ausente se cubre con una comprobación de exhaustividad explícita:

```ts
type Faltantes<I, T extends readonly (readonly [string, string])[]> =
  Exclude<keyof I, T[number][0]>;

function exigirCompleto<_Faltantes extends never>(): void {}

exigirCompleto<Faltantes<EstadoAgente, typeof CAMPOS_ESTADO_AGENTE>>();
```

Si sobra una clave sin declarar, `tsc` falla **nombrándola**:

```
error TS2344: Type '"campoNuevoSinDeclarar"' does not satisfy the constraint 'never'.
```

Es el equivalente funcional de la desestructuración exhaustiva de Rust.

## 4. Verificación

Las cuatro comprobaciones se ejecutaron sobre árbol modificado, no sobre inspección.

| Caso | Mutación introducida | Resultado |
|---|---|---|
| Árbol limpio | ninguna | `tsc` 0 · Rust 22/22 · Node 31/31 |
| Campo renombrado en el manifiesto | `respuestaAutomatica` → `respuestaAuto` | falla `las cargas útiles coinciden con el manifiesto`, con ambas listas impresas |
| Campo eliminado del manifiesto | `eventosPendientes` → `BORRADO` | falla la misma prueba |
| Campo nuevo en la interfaz TS | `campoNuevoSinDeclarar` | `tsc` falla nombrando la clave |
| Campo inventado en la tabla TS | `["inventado", "texto"]` | `tsc` falla: `not assignable to 'keyof EstadoAgente'` |
| Restaurado | — | verde |

Aplicación directa de RPT-006 §4: un verificador no probado por negativa no es un verificador, es una decoración. Cinco incidentes previos con el guardián de fronteras justifican el gasto.

## 5. Lo que este mecanismo **no** cubre

Registrarlo importa tanto como registrar lo que sí cubre.

1. **Los valores de los enumerados no están en el manifiesto.** El tipo declara `enumerado`; qué variantes admite vive solo en el código. Rust las valida en ejecución (`un_valor_fuera_del_enumerado_se_rechaza`); TypeScript no valida nada en ejecución. Si `PerfilSegmento` gana una variante en Rust y no en TypeScript, nada protesta hasta que llegue un valor real.
2. **TypeScript sigue sin validación en tiempo de ejecución.** Todo lo anterior es estático. Un agente comprometido, o simplemente desactualizado, puede emitir una carga malformada y el renderer la aceptará. Se descartó Zod deliberadamente — una dependencia de validación en la capa privilegiada amplía la superficie —, pero la consecuencia queda anotada.
3. **La paridad es de nombres y tipos, no de semántica.** Que ambos extremos llamen `limiteBytes` a un número no garantiza que ambos lo interpreten como bytes.

El punto (1) es el candidato natural a la siguiente vuelta si las vistas empiezan a depender de las variantes. No se abre como punto formal: cerrar puntos que nadie ha necesitado todavía es cómo se acumula proceso muerto.

## 6. Estado tras este reporte

```
tsc --build .................................. 0 errores
verificar:limites     eslint ................. 0 problemas
verificar:frontera    depcruise .............. 0 violaciones
verificar:frontera:negativa .................. 6/6 casos, 3 estados
probar                node --test ............ 31/31 pruebas
cargo test -p eje-ipc ........................ 22/22 pruebas
```

PA-21 queda cerrado. El puente está descrito por un único fichero versionado y ninguno de los dos extremos puede apartarse de él sin que algo falle antes de llegar a ejecución.

---

*Reporte Nº 7 — Tipado de Carga Útil del Contrato IPC · PremosCorp · 5 de agosto de 2026 · Estado: Canónico*
