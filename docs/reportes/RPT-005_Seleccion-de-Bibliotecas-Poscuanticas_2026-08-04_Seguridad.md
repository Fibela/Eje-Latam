# Reporte de Construcción Nº 5 — Selección de Bibliotecas Poscuánticas

| Campo | Valor |
|---|---|
| **Tema documentado** | Evaluación del ecosistema PQC en Rust y selección para `motor-pqc` |
| **Número de reporte** | 005 |
| **Fecha** | 4 de agosto de 2026 |
| **Área designada** | Seguridad |
| **Entidad / Firma** | PremosCorp |
| **Estado** | Canónico |

## Trazabilidad

- **Depende de:** RPT-002 §5 (AGT-02), RPT-003 §2.4 (motor criptográfico abierto), RPT-003 §9.2 (vectores ACVP)
- **Enmienda:** RPT-003 §9.2 — los vectores ACVP resultan **necesarios pero insuficientes** (§4)
- **Origen del insumo:** objetivos de investigación remitidos por equipo externo, agosto 2026
- **Abre:** PA-16 a PA-19

---

## 1. Dictamen sobre los Objetivos Propuestos

Los tres objetivos del equipo son correctos y se adoptan. Se añaden cinco que faltaban, uno de ellos por conflicto directo con código ya escrito.

| # | Objetivo | Origen |
|---|---|---|
| 1 | Evaluación de crates para FIPS 203 y FIPS 204 | Equipo |
| 2 | Patrones de construcción híbrida | Equipo |
| 3 | Compatibilidad `no_std` y targets del workspace | Equipo |
| **4** | **Conflicto con `#![forbid(unsafe_code)]`** ya presente en `motor-pqc` | Añadido — §5 |
| **5** | **Compatibilidad con la lista de licencias de `deny.toml`** | Añadido — §6 |
| **6** | **Vectores adversarios además de los ACVP** | Añadido — §4, **es el hallazgo central** |
| **7** | **Borrado seguro de material de clave y aleatoriedad** | Añadido — §7.4 |
| **8** | **Coste operativo de una cadena de herramientas C** en CI, instalador firmado y SBOM | Añadido — §5.2 |

---

## 2. Estado del Ecosistema

Datos consultados en crates.io el 4 de agosto de 2026.

| Candidato | ML-KEM | ML-DSA | Lenguaje | Licencia | Vectores ACVP | Auditoría independiente |
|---|---|---|---|---|---|---|
| **RustCrypto** | `ml-kem` 0.3.2 (10-may-2026) | `ml-dsa` 0.1.1 (5-jun-2026) | Rust puro | Apache-2.0 OR MIT | Sí | **No** — advertencia expresa de "úselo bajo su propio riesgo" |
| **libcrux** (Cryspen) | `libcrux-ml-kem` 0.0.10 (15-jul-2026) | `libcrux-ml-dsa` 0.0.10 (15-jul-2026) | Rust, verificado con hax/F* | Apache-2.0 | Sí | **No**, pero verificación formal + auditoría externa en feb-2026 (§3) |
| **aws-lc-rs** | Sí | Tras bandera `unstable` | Enlaces C (AWS-LC) | Apache-2.0 / ISC | Parcial | Validación FIPS 140-3 en curso |
| **liboqs-rust** | Sí | Sí | Enlaces C | MIT | Sí | No |
| **pqcrypto** (PQClean) | Sí | Sí | Enlaces C | Mixta | **No** | No |

### 2.1 Exclusiones inmediatas

**`liboqs-rust` — descartado.** El README de liboqs declara: *"WE DO NOT CURRENTLY RECOMMEND RELYING ON THIS LIBRARY IN A PRODUCTION ENVIRONMENT OR TO PROTECT ANY SENSITIVE DATA."* No hay lectura favorable de esa frase para un producto que protege hospitales y plantas industriales.

**`pqcrypto` / PQClean — descartado.** PQClean se declara destinado a *"propósitos académicos y experimentales"* y no prueba contra vectores NIST. Sin vectores, RPT-003 §9.2 lo excluye por definición.

### 2.2 Observación sobre madurez

**Ninguna implementación PQC en Rust cuenta hoy con auditoría independiente de una firma profesional.** No es una carencia de un candidato: es el estado del ecosistema completo. Cualquier decisión se toma sobre esa base, y conviene que quede escrito antes que descubrirlo en una revisión de cliente.

Ambos candidatos serios son **anteriores a 1.0**: `libcrux-ml-kem` está en 0.0.10 y `ml-dsa` en 0.1.1. La estabilidad de API no está garantizada por ninguno.

---

## 3. La Verificación Formal Tiene Frontera

`libcrux` se presenta como biblioteca criptográfica formalmente verificada con hax y F*, y esa es su principal ventaja declarada. En febrero de 2026 se publicó una auditoría externa —*"Verification Theatre: False Assurance in Formally Verified Cryptographic Libraries"*, ePrint 2026/192— con trece vulnerabilidades en libcrux y hpke-rs.

Lo relevante no es el número, sino su ubicación:

| Ubicación | Hallazgos |
|---|---|
| Código **no verificado** | Nueve, entre ellos un fallo de endianness entre backends que provocó fallos reales de descifrado en el *ratchet* poscuántico de Signal, una validación X25519 obligatoria ausente, reutilización de nonce por desbordamiento de entero, y **dos violaciones de FIPS 204 en el verificador de ML-DSA** |
| Código **formalmente verificado** | Cuatro: en ML-KEM, una constante de descompresión incorrecta, una NTT inversa ausente y una prueba de serialización falsa; en ML-DSA, una especificación de multiplicación incorrecta que **vuelve no sólidas las pruebas axiomatizadas de AVX2** |

La conclusión del trabajo es que todo sistema formalmente verificado incorpora una **frontera de verificación**: el límite entre el código con pruebas comprobadas por máquina y el código que se confía sin ellas. Cuando esa frontera no se comunica con claridad, la etiqueta "formalmente verificado" produce una confianza que el artefacto no sostiene.

**Esto no descalifica a libcrux.** Una biblioteca auditada y corregida está en mejor posición que una nunca examinada, y tras febrero de 2026 libcrux es probablemente la implementación PQC en Rust más escrutada que existe. Lo que descalifica es tratar "formalmente verificado" como sustituto de las pruebas propias.

---

## 4. Hallazgo Central — Los Vectores ACVP No Bastan

RPT-003 §9.2 declara innegociables los vectores oficiales ACVP del NIST para ML-KEM y ML-DSA. Esa exigencia es correcta y se mantiene. **Pero no habría detectado la vulnerabilidad más grave publicada este año en el ecosistema.**

### 4.1 CVE-2026-24850

En enero de 2026 se publicó [GHSA-5x2r-hc65-25f9](https://github.com/RustCrypto/signatures/security/advisories/GHSA-5x2r-hc65-25f9) contra el crate `ml-dsa` de RustCrypto. El verificador aceptaba firmas con **índices de pista repetidos**, cuando FIPS 204 y RFC 9881 exigen que sean estrictamente crecientes.

La causa: una regresión de un solo carácter. Un commit que pretendía *ajustar la decodificación a la especificación* cambió una comparación estricta `<` por `<=` en la función `monotonic` de `ml-dsa/src/hint.rs`.

El efecto es **maleabilidad de firma**: la misma firma lógica admite múltiples codificaciones válidas a nivel de bytes. Un atacante puede tomar una firma legítima y derivar otras "válidas" duplicando índices. Compromete a todo sistema que dependa de la unicidad de la firma para deduplicación, protección contra repetición o identificadores derivados de firma.

Afecta a los tres conjuntos de parámetros. Corregido en `ml-dsa` v0.1.0-rc.4.

### 4.2 Por qué importa para nuestra política

**El fallo se encontró con vectores de Wycheproof, no con vectores ACVP.**

La distinción es de propósito:

| Conjunto | Qué prueba |
|---|---|
| **ACVP** (NIST) | Que la implementación calcula **correctamente** lo que debe calcular |
| **Wycheproof** (C2SP) | Que la implementación **rechaza** lo que debe rechazar: codificaciones no canónicas, valores fuera de rango, entradas maliciosas |

Una implementación puede pasar ACVP al completo y aceptar firmas malformadas. Es exactamente lo que ocurrió: el caso de prueba 18 de `mldsa_44_verify_test.json` —*"firma con una pista repetida"*— esperaba `invalid` y obtenía `valid`.

### 4.3 Enmienda a RPT-003 §9.2

> Los vectores **ACVP son obligatorios y no suficientes**. `motor-pqc` debe validarse además contra el conjunto **Wycheproof** correspondiente a ML-KEM y ML-DSA, y ambos conjuntos deben ejecutarse en CI en cada *pull request*.

Sin esta enmienda, nuestra propia política de calidad habría dado por verificable una implementación con una CVE de maleabilidad de firma activa.

---

## 5. Conflicto con `#![forbid(unsafe_code)]`

### 5.1 El conflicto

`crates/motor-pqc/src/lib.rs` declara `#![forbid(unsafe_code)]` desde el primer commit, coherente con que la seguridad de memoria de Rust es un diferenciador declarado del producto y con el fundamento que desestimó las fronteras FFI/C en RPT-003 §2.7.

Los candidatos con enlaces C —`aws-lc-rs`, `liboqs`, `pqcrypto`— **no rompen esa directiva dentro de nuestro crate** (el atributo solo alcanza al código propio), pero sí destruyen la afirmación de seguridad de memoria de extremo a extremo: el cálculo criptográfico ocurriría en C, sin las garantías del lenguaje.

**RustCrypto y libcrux son Rust.** Con cualquiera de los dos, `motor-pqc` conserva `#![forbid(unsafe_code)]` y la afirmación se sostiene. La medición del `unsafe` en dependencias con `cargo geiger` ya es obligatoria (RPT-003 §9.4) y debe documentarse en el reporte de release.

### 5.2 Coste operativo de una cadena de herramientas C

No contemplado en la propuesta y relevante para el cronograma:

- Requiere compilador C en CI y en cada máquina de desarrollo, incluidas las de Windows
- Complica el instalador firmado con certificado EV (RPT-003 §5)
- Amplía el SBOM con componentes nativos cuya procedencia hay que declarar
- Multiplica los targets a validar en compilación cruzada

Con Rust puro, `cargo build` basta.

---

## 6. Compatibilidad con `deny.toml`

Verificado el 4 de agosto de 2026:

| Crate | Licencia | ¿En la lista permitida? |
|---|---|---|
| `ml-kem` 0.3.2 | Apache-2.0 OR MIT | Sí |
| `ml-dsa` 0.1.1 | Apache-2.0 OR MIT | Sí |
| `libcrux-ml-kem` 0.0.10 | Apache-2.0 | Sí |
| `libcrux-ml-dsa` 0.0.10 | Apache-2.0 | Sí |

**Ninguno exige modificar `deny.toml` ni documentar excepción.** Este gate queda despejado para los cuatro.

---

## 7. Recomendación

### 7.1 No elegir un ganador — sostener la abstracción y contrastar

La evidencia no favorece decisivamente a ninguno de los dos candidatos serios. Ambos son Rust puro, ambos prueban contra ACVP, ambos son anteriores a 1.0, ninguno tiene auditoría independiente, y **a ambos se les encontraron fallos en el verificador durante el primer trimestre de 2026**. Ese patrón no es casual: el verificador de ML-DSA es donde se concentran los errores del estado del arte.

Los traits `EncapsuladoClave`, `FirmaDigital` y `CifradoEnReposo` ya existen en `motor-pqc`. Esa decisión de diseño era correcta y ahora rinde: permite decidir sin quedar atrapados.

### 7.2 Selección

| Papel | Elección | Fundamento |
|---|---|---|
| **Implementación por defecto** | **RustCrypto** (`ml-kem`, `ml-dsa`) | Madurez semántica superior (0.3.2 y 0.1.1 frente a 0.0.10), MSRV 1.85 idéntico al del workspace, integración con los traits de facto del ecosistema Rust, proceso público de divulgación demostrado con CVE-2026-24850 |
| **Oráculo diferencial** | **libcrux** (`libcrux-ml-kem`, `libcrux-ml-dsa`) | Implementación independiente, formalmente verificada y —tras la auditoría de feb-2026— la más escrutada del ecosistema |
| **Backend FIPS (Fase 2)** | `aws-lc-rs` tras bandera de característica | Para despliegues de gobierno o finanzas reguladas que exijan módulo con validación FIPS 140-3. Introduce FFI y debe declararse |

### 7.3 Prueba diferencial — el sustituto asequible de una auditoría

Ambas implementaciones se declaran como dependencias y se contrastan en CI:

1. **ACVP** sobre la implementación por defecto — corrección
2. **Wycheproof** sobre la implementación por defecto — rechazo de entradas maliciosas (§4.3)
3. **Contraste cruzado**: la misma entrada a RustCrypto y a libcrux. Toda discrepancia es un fallo en una de las dos, y se eleva antes de continuar
4. **Fuzzing diferencial** sobre los verificadores, que es donde se concentran los fallos conocidos

Dos implementaciones independientes que discrepan revelan un fallo que ninguna detecta en solitario. No sustituye a una auditoría profesional, pero es lo mejor disponible mientras el ecosistema no tenga ninguna.

### 7.4 Conjuntos de parámetros y construcción híbrida

| Uso | Construcción | Fundamento |
|---|---|---|
| Intercambio de claves | **X25519 + ML-KEM-768** | Categoría 3 del NIST. La componente clásica garantiza que el canal no queda peor que hoy si aparece un ataque contra retículos |
| Firma | **Ed25519 + ML-DSA-65** | Categoría 3. **Ed25519 ya está en uso** para tokens de licencia (RPT-003 §3) y para la firma del paquete empresarial (RPT-004 §5), de modo que la construcción híbrida extiende lo existente en lugar de sustituirlo |
| Datos en reposo | **AES-256-GCM** con clave envuelta por ML-KEM | ML-KEM y ML-DSA no cifran en reposo (RPT-002 §5) |

No se adopta ML-KEM-512 ni ML-DSA-44 (categoría 2, margen insuficiente para infraestructura crítica con horizonte de décadas), ni 1024/87 (categoría 5, coste desproporcionado en nodos IoT/OT limitados).

**Material de clave.** Toda clave privada y todo secreto compartido se envuelve en un tipo con borrado seguro (`zeroize`). Un volcado de memoria de un nodo hospitalario no debe entregar claves privadas ML-KEM.

**Aleatoriedad.** La documentación de libcrux advierte expresamente contra usar `OsRng` directamente y recomienda un DRBG conforme al NIST. La fuente de aleatoriedad se decide en **PA-18**, no por omisión.

---

## 8. Puntos Abiertos

| ID | Punto | Estado |
|---|---|---|
| ~~PA-16~~ | ~~Ratificación de la selección y del enfoque diferencial~~ | ✅ Cerrado |
| ~~PA-17~~ | ~~Vectores ACVP y Wycheproof en el repositorio, anclaje y ejecución~~ | ✅ **Cerrado 4-ago-2026** — ver §9 |
| **PA-18** | Fuente de aleatoriedad: DRBG conforme al NIST frente a `OsRng` | 🟡 Abierto — generación de claves |
| **PA-19** | Momento de reevaluación: ¿se revisa esta decisión cuando alguna implementación reciba auditoría independiente, o en fecha fija? | 🟡 Abierto — gobernanza |
| **PA-14** | Cadena de firma de releases. **Absorbe la atestación de conformidad** (§9.4) | 🟡 Abierto — RPT-004 §10 |

---

## 9. Cierre de PA-17 — Evidencia de Conformidad

### 9.1 Lo que quedó integrado

Tres capas de evidencia independientes, todas en verde en `principal`:

| Capa | Cobertura | Qué establece |
|---|---|---|
| **ACVP** (NIST) | 4 suites: ML-KEM keygen y encapsulado, ML-DSA keygen y sigVer | Que se **calcula** lo correcto |
| **Wycheproof** (C2SP) | 399 casos: 206 ML-DSA (127 inválidos) y 193 ML-KEM | Que se **rechaza** lo incorrecto |
| **Contraste diferencial** | 7 pruebas contra libcrux | Que dos implementaciones independientes **coinciden** e interoperan |

Los ficheros de vectores están versionados y anclados por SHA-256 en `FUENTES.lock`, lo que hace segura la exoneración de `.gitleaks.toml` sobre ese directorio.

**Variantes declaradas fuera de alcance**, con recuento explícito en las propias suites: la interfaz interna de FIPS 204, HashML-DSA (`preHash`), el `externalMu`, y el desencapsulado de ML-KEM en ACVP —este último cubierto por Wycheproof y por el contraste diferencial en ambos sentidos.

### 9.2 Lo que NO establece este cierre

**`Conformidad::apto_para_produccion()` sigue devolviendo `false`, y es correcto que así sea.**

Ningún código conecta las suites que pasan con el tipo que expone ese estado. Cerrar PA-17 significa que la **evidencia** está construida y verificada, no que exista una **atestación** que la comunique al binario en ejecución.

Se consigna aquí para que nadie interprete el cierre de PA-17 como aptitud declarada.

### 9.3 Diseño aprobado — `CONFORMIDAD.lock`

Se evaluaron y descartaron dos mecanismos:

| Mecanismo | Por qué se descarta |
|---|---|
| Constante `true` en el código | No prueba nada; solo traslada la afirmación de sitio |
| Variable de entorno o bandera de compilación (`PQC_CONFORMITY_TOKEN`) | **Falsificable**: la fija cualquiera antes de compilar. Si va firmada, la clave o está en el repositorio —y no es secreto— o solo en la CI, lo que la convierte en dependencia de PA-14 |

**El defecto de fondo de ambos es el mismo: tratan la conformidad como una propiedad del *evento de compilación*, cuando es una propiedad de las *entradas*.**

Si mañana se actualiza `ml-dsa` a 0.1.2 en `Cargo.lock`, un binario compilado hoy seguiría portando su bandera de conforme aunque las suites nunca se ejecutaran contra la versión nueva. Un atestado del tipo "la CI pasó" caduca en silencio, que es el modo de fallo que este proyecto lleva persiguiendo desde RPT-003 §9.5.

**Diseño adoptado.** Atar el atestado a la huella de lo que efectivamente se probó:

```
CONFORMIDAD.lock
  ├── versiones exactas de ml-kem, ml-dsa y libcrux-*   (de Cargo.lock)
  ├── resumen SHA-256 de FUENTES.lock                    (los vectores)
  ├── versión del toolchain
  └── huella = SHA-256 sobre todo lo anterior
```

- `cargo xtask conformidad` — **NO EXISTE TODAVIA**, es diseño; se sigue en PA-121 — ejecutaría las tres suites y, **solo si pasan**, emitiría el fichero.
- `tests/atestacion.rs` **recalcula la huella** desde `Cargo.lock` y `FUENTES.lock` y falla si no coincide con la registrada.

La propiedad que se gana es la que importa: **si alguien sube una dependencia o cambia un vector sin volver a ejecutar la conformidad, las huellas divergen y la CI se pone roja sola.** El atestado se autoinvalida. Es el mismo mecanismo del anclaje Merkle de los vectores, aplicado al árbol de dependencias, y no necesita `build.rs`, ni variable de entorno, ni clave.

### 9.4 Hueco residual — por qué se delega en PA-14

Este diseño ata **qué** se probó, no **que** se probó. Alguien podría calcular la huella correcta sin haber ejecutado las suites.

Cerrarlo del todo exige que la CI sea el único productor de confianza, con una clave que solo ella posea. Eso es exactamente el alcance de **PA-14** (cadena de firma de releases), y por eso la implementación se delega allí en lugar de construir ahora una atestación que quedaría a medias.

**Consecuencia práctica:** hasta que PA-14 se cierre, la conformidad de `motor-pqc` se establece por la CI en verde y por este reporte, no por un valor consultable en tiempo de ejecución.

---

*Reporte Nº 5 — Selección de Bibliotecas Poscuánticas · PremosCorp · 4 de agosto de 2026 · Estado: Canónico*
