# Reporte de Construcción Nº 3 — Gobernanza y Cierre de Puntos Abiertos

| Campo | Valor |
|---|---|
| **Tema documentado** | Gobernanza, Licenciamiento y Política de Calidad |
| **Número de reporte** | 003 |
| **Fecha** | 4 de agosto de 2026 |
| **Área designada** | Gobernanza |
| **Entidad / Firma** | PremosCorp |
| **Estado** | Canónico — frontera open-core **ratificada en firme** (§2.7, 4-ago-2026) |

## Trazabilidad

- **Depende de:** RPT-002 — Arquitectura Consolidada v2.0
- **Cierra:** PA-01 a PA-06 de RPT-002 §12
- **Modifica:** RPT-002 §5 (AGT-01, corrección de requisitos de firma en Windows), §11 (áreas designadas), §12 (estado de puntos abiertos)
- **Origen del insumo:** propuestas de resolución remitidas por equipos externos, agosto 2026

---

## 1. Resumen de Dictámenes

| Punto | Propuesta recibida | Dictamen |
|---|---|---|
| PA-01 | Cerrar `guardian-cc` y `motor-pqc` como binarios propietarios | 🔴 **Rechazada y desestimada en firme** — §2, constancia §2.7 |
| PA-02 | Licencia Ed25519 firmada, sin degradar seguridad | 🟢 **Aceptada con ajustes** — §3 |
| PA-03 | Retirar "Zero-Knowledge" | 🟢 **Aceptada, reemplazo corregido** — §4 |
| PA-04 | Iniciar Npcap OEM + WHQL en paralelo | 🟡 **Aceptada, corrección técnica mayor** — §5 |
| PA-05 | Credenciales en ALM-01/Bóveda con DPAPI/Keyring | 🟡 **Aceptada, ubicación corregida** — §6 |
| PA-06 | STUN/DERP oficial con opción autoalojada | 🟢 **Aceptada con matices** — §7 |
| Extra | Monetizar Inteligencia Regional como suscripción | 🟢 **Aceptada — es la clave que resuelve PA-01** |
| Extra | Probar exhaustivamente el no-bypass de `SIMULATION_ONLY` | 🟡 **Aceptada y elevada a requisito de arquitectura** — §8.1 |
| Extra | Priorizar autoprotección del agente | 🟡 **Aceptada, alcance acotado** — §8.2 |

---

## 2. PA-01 · Frontera Open-Core — **RECHAZADA**

### 2.1 Propuesta recibida

Abrir `eje-agente`, `eje-almacen` y `eje-red` bajo Apache 2.0; cerrar `guardian-cc`, `motor-pqc`, `VIS-02` y `SIM-01` como "crates/librerías binarias privadas que se enlazan dinámicamente o durante el build comercial".

### 2.2 Objeción técnica: el mecanismo propuesto no existe en Rust

**Rust no tiene ABI estable.** No es posible distribuir un crate compilado y enlazarlo dinámicamente contra otro crate de forma soportada; el layout de tipos, el name mangling y las convenciones de llamada cambian entre versiones del compilador y hasta entre banderas de compilación.

Las únicas vías reales son:

| Vía | Consecuencia |
|---|---|
| Frontera FFI con ABI de C (`extern "C"`, `#[repr(C)]`) | Introduce `unsafe` en el límite del motor de inspección. **En un producto de seguridad, añadir FFI insegura justo en la superficie que procesa tráfico hostil es una pérdida neta de seguridad.** |
| Repositorio privado + enlace estático en el build comercial | Técnicamente viable, pero **desestimada en firme (§2.7)**: produce un producto real que la comunidad no puede compilar ni auditar. |
| Distribuir fuente con licencia propietaria | No protege nada frente a un fork determinado; solo añade fricción legal. |

La propuesta de "enlace dinámico" debe descartarse por inviable.

### 2.3 Objeción estratégica: la frontera está invertida

Más grave que el problema técnico: **si `eje-agente` es abierto pero no puede inspeccionar nada sin `guardian-cc` cerrado, el código abierto es una cáscara vacía.** Eso no genera adopción; genera la percepción de *open-washing*, que es peor que no abrir nada.

Y hay una razón adicional, específica de este producto: **la auditabilidad es el argumento de venta.** Un director de seguridad hospitalario debe autorizar un binario con capacidad de aislar puertos de switch en su red. Lo primero que preguntará es si puede auditarlo. Cerrar `guardian-cc` destruye exactamente la ventaja que justificaba abrir el resto.

### 2.4 `motor-pqc` debe ser abierto — no es negociable

Esto no es una preferencia estratégica sino un requisito de la disciplina.

- **Principio de Kerckhoffs.** Un motor criptográfico cuya seguridad dependa del secreto de su implementación es, por definición, un motor no evaluable.
- **ML-KEM (FIPS 203) y ML-DSA (FIPS 204) son estándares públicos del NIST.** No hay propiedad intelectual que proteger: la especificación completa está publicada.
- **Criptografía propietaria es una señal de alarma reconocida** en cualquier evaluación de seguridad seria. Un auditor que encuentre un motor criptográfico de caja negra en un producto que vende resiliencia poscuántica lo marcará como hallazgo, no como diferenciador.

Vender resiliencia poscuántica con cripto cerrada es una contradicción que no sobrevive a la primera auditoría de cliente.

### 2.5 Contrapropuesta: invertir la frontera

El criterio correcto no es *núcleo vs. módulos*, sino **mecanismo vs. contenido y operación**.

**Abierto — Apache 2.0 (el mecanismo):**

- `eje-agente` — núcleo, planificador, IPC, `AGT-05` correlacionador
- `guardian-cc` — motor de inspección L2/L3 y analizadores de protocolo
- `motor-pqc` — íntegro, obligatorio (§2.4)
- `eje-almacen` y `boveda` — **incluido el encadenamiento Merkle**: el valor probatorio de una cadena de custodia depende de que el algoritmo sea auditable; una cadena forense cerrada no sirve en un proceso judicial
- `eje-red` — Capas A y B
- SDK, esquemas de datos y formatos de intercambio

**Propietario — PremosCorp Enterprise (el contenido y la operación):**

- **Suscripción de Inteligencia Regional** — firmas, IoC, reglas de correlación y clasificadores de phishing latinoamericano. *Fuente principal de ingreso.*
- `VIS-02` Tablero Directivo y `SIM-01` — escenarios de crisis, plantillas sectoriales, modelos de impacto
- `NUC-*` completo (Fase 2) — correlación multiinquilino, gestión de licencias
- Conectores de contención certificados por fabricante de switch
- Soporte, SLA y paquetes de cumplimiento normativo por jurisdicción

### 2.6 Fundamento

**El código de inspección es una mercancía a tres años; las firmas regionales actualizadas cada semana no lo son.**

Un competidor puede forkear `guardian-cc` en una tarde. Lo que no puede forkear es un equipo que investiga campañas de Grandoreiro en Bogotá y publica firmas el mismo día. La ventaja competitiva de Eje-Latam nunca estuvo en el código — estuvo en la operación regional. Cerrar el código protege el activo equivocado y sacrifica el argumento de confianza.

Es además el modelo que sostiene a los proyectos comparables del sector: los motores de inspección de red de referencia son abiertos y auditados; el negocio vive de las reglas, la operación y el soporte.

---

## 2.7 Constancia de Ratificación — Decisión Firme

**Fecha:** 4 de agosto de 2026 · **Instancia:** Dirección, PremosCorp · **Estado:** Firme

> Queda **desestimada** la propuesta inicial de ocultar binarios mediante crates cerradas o fronteras FFI/C, por considerarse técnicamente inviable, perjudicial para la seguridad de memoria (*memory safety*) y destructiva para la confianza del cliente institucional.

En consecuencia:

1. Se **ratifica la frontera invertida** de §2.5. `guardian-cc`, `motor-pqc`, `eje-almacen` —incluida la cadena Merkle— y `eje-red` quedan bajo **Apache 2.0** de forma definitiva.
2. La **Suscripción de Inteligencia Regional** queda establecida como fuente principal de ingreso.
3. **PA-07 se cierra.** El primer *commit* de código queda desbloqueado.

### Fundamentos registrados de la desestimación

| Fundamento | Detalle |
|---|---|
| **Inviabilidad técnica** | Rust carece de ABI estable; el enlace dinámico de crates no es una operación soportada (§2.2) |
| **Perjuicio a la seguridad de memoria** | Una frontera FFI/C introduce `unsafe` en el límite que procesa tráfico hostil, anulando las garantías que motivaron elegir Rust |
| **Destrucción de confianza institucional** | Un binario cerrado con autoridad de contención sobre infraestructura crítica es inauditable para el cliente que debe autorizarlo |
| **Descalificación en auditoría** | Un motor criptográfico de caja negra sobre estándares NIST públicos constituye hallazgo de auditoría, no diferenciador (§2.4) |

### Efecto de cierre

Esta constancia **cierra la discusión sobre el ocultamiento de binarios del mecanismo**. Toda propuesta futura de cerrar `guardian-cc`, `motor-pqc`, `eje-almacen` o `eje-red` debe presentarse como **enmienda formal a esta constancia**, aportando fundamento nuevo que refute los cuatro puntos anteriores. No se reabre por reiteración.

> La vía de "repositorio privado con enlace estático en el build comercial", que §2.2 identificaba como única alternativa técnicamente sostenible, **queda igualmente desestimada** por el segundo y tercer fundamento: sigue produciendo un producto real que la comunidad no puede compilar ni auditar.

---

## 3. PA-02 · Licencias Fuera de Línea — **ACEPTADA CON AJUSTES**

### 3.1 Aceptado

- Token firmado asimétricamente con **Ed25519**, caducidad de 1 año.
- **Principio de contingencia:** al expirar en un nodo aislado, el agente **nunca desactiva detección ni contención**. Se eleva de regla de licenciamiento a **principio de producto**: *ninguna condición comercial degrada jamás una función de seguridad.*

### 3.2 Ajuste 1 — Formato

Preferir **token compacto firmado con Ed25519 sobre estructura canónica** en lugar de JWT. JWT arrastra negociación de algoritmo en la cabecera, y la familia de vulnerabilidades `alg: none` y de confusión de algoritmo es un clásico evitable. Si se usa JWT, **fijar el algoritmo en el verificador e ignorar la cabecera**.

### 3.3 Ajuste 2 — Reloj del sistema

La caducidad se valida contra el reloj local, que el operador controla; retrasar el reloj burla la expiración trivialmente. **Se acepta este bypass de forma consciente y documentada**, porque bajo el principio de §3.1 el modo expirado no desactiva seguridad: el incentivo para burlarlo es marginal y la alternativa (contador monotónico persistido) añade complejidad y riesgo de bloqueo indebido en un hospital. Registrar la decisión evita que un auditor futuro la reporte como descuido.

### 3.4 Ajuste 3 — Qué se degrada exactamente

La propuesta deshabilita "la generación de reportes ejecutivos en `VIS-02`". Corrección necesaria: **durante un incidente activo, `VIS-02` debe seguir mostrando el estado operativo en vivo aunque la licencia esté vencida.** Dejar a un comité de crisis hospitalario sin tablero de decisiones por una fecha de facturación es un fallo de producto con consecuencias reales.

| Estado de licencia | Comportamiento |
|---|---|
| Vigente | Completo |
| Vencida — sin incidente | Visualización en vivo activa. Se deshabilita **exportación** de reportes, comparativas históricas y `SIM-01`. Aviso discreto persistente. |
| Vencida — **incidente activo** | **`VIS-02` completo, sin restricción.** El aviso pasa a segundo plano. Se registra el uso en modo gracia en `ALM-01` para conciliación comercial posterior. |
| Cualquiera | `AGT-01` a `AGT-07` operan siempre al 100 % |

---

## 4. PA-03 · Terminología "Zero-Knowledge" — **ACEPTADA, REEMPLAZO CORREGIDO**

Aceptado el retiro del término. Sin una prueba criptográfica real (ZK-SNARK / ZK-STARK), usarlo atrae escrutinio que no aporta.

**Corrección al reemplazo propuesto:** la propuesta sugiere sustituirlo por "Arquitectura de Cero Confianza (Zero-Trust)". **No son intercambiables.** *Zero-Knowledge* describía una propiedad de la gestión de claves; *Zero-Trust* es un modelo de control de acceso a la red. Sustituir uno por otro traslada la imprecisión en lugar de eliminarla.

| Contexto | Término correcto |
|---|---|
| `NUC-04`, gestión de claves | **"Claves administradas por el cliente"** o **"PremosCorp no posee acceso a las claves del cliente"** |
| `AGT-01`, modelo de acceso a red | **"Confianza Cero" / Zero-Trust** — ya en uso, correcto |

La segunda formulación tiene además la ventaja de ser una afirmación verificable en auditoría, no una etiqueta.

---

## 5. PA-04 · Captura en Windows — **CORRECCIÓN TÉCNICA MAYOR**

### 5.1 Corrección de un error propio en RPT-002

RPT-002 §5 (AGT-01) indicaba, para Windows: *"Npcap OEM · Firma digital + atestación WHQL de Microsoft"*. **Esto es incorrecto y lo corrijo aquí.** La propuesta recibida arrastra el mismo error al agrupar tres trámites distintos.

### 5.2 Los tres trámites son independientes

| Trámite | ¿Cuándo se necesita? | Plazo aproximado | ¿En ruta crítica? |
|---|---|---|---|
| **Licencia Npcap OEM** (Nmap Project) | Solo para **redistribuir** Npcap dentro del instalador | Semanas | **Sí** |
| **Certificado EV Code Signing** | Para firmar el instalador y binarios propios. Necesario **en cualquier caso** | 1–3 semanas tras validación de identidad | **Sí** |
| **Atestación WHQL / Partner Center** | **Solo si se escribe un driver de kernel propio** | Meses | **NO, si se usa Npcap** |

**Consecuencia práctica: al usar Npcap, la atestación WHQL sale de la ruta crítica.** Npcap se distribuye ya firmado por su fabricante; PremosCorp no firma ningún driver de kernel. Esto elimina el trámite más largo y caro que se había asumido, y libera meses de cronograma.

Se conserva la recomendación de arrancar por Linux — pero por la razón correcta: es donde se puede iterar sin depender de terceros, no porque Windows esté bloqueado por WHQL.

### 5.3 Adición: kernels industriales antiguos

`eBPF` no es una opción universal. Los entornos OT operan con frecuencia sobre distribuciones de ciclo largo con kernels 3.10–4.x, donde eBPF/XDP no está disponible o está severamente limitado.

**`guardian-cc` debe implementar `AF_PACKET` + `PACKET_MMAP` como ruta de captura de referencia**, y usar eBPF/XDP como optimización cuando el kernel lo permita. Diseñar solo para eBPF deja fuera buena parte del parque industrial instalado, que es precisamente el mercado objetivo.

---

## 6. PA-05 · Credenciales de Switch — **ACEPTADA, UBICACIÓN CORREGIDA**

### 6.1 Objeción: no deben residir en ALM-01 ni en la Bóveda

La propuesta indica guardarlas "en ALM-01/Bóveda Aislada protegidas por DPAPI o Keyring". **Ambos destinos son incorrectos por diseño:**

- **`ALM-01` es de solo anexado con encadenamiento Merkle.** Las credenciales rotan; un registro append-only no permite eliminarlas. Se acumularían secretos inmutables y perpetuos dentro del registro forense. Peor: **al exportar la evidencia para un proceso judicial se exportarían las credenciales de la infraestructura del cliente.**
- **La Bóveda Aislada es una cola de eventos pendientes de reconciliación**, no un almacén de secretos.

### 6.2 Resolución

Almacén de secretos **separado y dedicado**, delegado al sistema operativo:

| Plataforma | Mecanismo |
|---|---|
| Windows | DPAPI en ámbito de máquina, con ACL restringida a la cuenta de servicio |
| Linux | Secret Service API, o fichero cifrado con clave derivada + permisos `0600` en sistemas sin escritorio |
| macOS | Llavero del sistema |

Nunca en texto plano en TOML/JSON — aceptado íntegramente.

### 6.3 La mitigación real es el alcance de la credencial, no su cifrado

DPAPI de máquina protege frente a lectura de disco en reposo, **no frente a un atacante que ya obtuvo SYSTEM** — cualquier proceso SYSTEM puede descifrarlo. Y un atacante con SYSTEM en el nodo del agente es exactamente el escenario que importa.

La defensa efectiva es **limitar lo que la credencial puede hacer**:

- Cuenta de switch dedicada, **sin privilegios de administración**
- Permisos acotados a apagado y reasignación de VLAN **en puertos de acceso**, nunca configuración global, ni troncales, ni enrutamiento
- Credencial distinta por sitio, para acotar el radio de daño
- Toda acción de contención registrada en `ALM-01` con actor, puerto y justificación

### 6.4 Corrección de terminología

La propuesta menciona *"SNMPv3 comunitario"*. Las **cadenas de comunidad son de SNMPv1/v2c** y viajan sin cifrar. **SNMPv3 no usa comunidades**: usa usuario con autenticación y privacidad (`authPriv`). Solo se autoriza **SNMPv3 en modo `authPriv`**; SNMPv1 y v2c quedan prohibidos para contención.

---

## 7. PA-06 · Servidor STUN/DERP — **ACEPTADA CON MATICES**

Aceptado: instancia oficial por defecto, con campo en `RED-02` y `VIS-03` para servidor propio del cliente.

**Matiz 1 — STUN y DERP no son lo mismo.** STUN descubre la dirección pública; **no resuelve NAT simétrico**. Para eso hace falta **relevo** (TURN/DERP), que transporta el tráfico y tiene **costo real de ancho de banda** a presupuestar. La propuesta los trata como un único servicio.

**Matiz 2 — Puerto 3478/UDP se bloquea con frecuencia** en redes corporativas y casi siempre en OT. Se requiere ruta alternativa sobre **TLS/443**.

**Matiz 3 — Privacidad, y es delicado.** Un STUN operado por PremosCorp observa las direcciones IP públicas de todos los clientes: metadatos sensibles en un producto cuyo argumento central es la soberanía del dato. La opción autoalojada debe ser **prominente en el lanzador, documentada y sin coste adicional** — no una casilla escondida. De lo contrario la promesa de soberanía se contradice en la práctica.

**Matiz 4 — En modo OT, la Capa B queda deshabilitada por defecto.** Cualquier conexión saliente a internet desde un segmento industrial puede vulnerar la segmentación en zonas y conductos que exige IEC 62443. Habilitarla debe ser una acción deliberada y registrada del cliente.

---

## 8. Dictamen sobre Observaciones Complementarias

### 8.1 `SIMULATION_ONLY` — aceptada y **elevada a requisito de arquitectura**

La observación es correcta y merece más que pruebas: **un test demuestra la ausencia de fallos conocidos, no la ausencia de fallos.** Para un control cuyo fallo puede desconectar equipamiento médico, validar no basta — hay que hacer el fallo estructuralmente imposible.

**Requisito de diseño:** `SIM-01` y la ruta de contención de `AGT-01` residen en **dominios de capacidad separados**. El simulador **no posee la capacidad de invocar contención**; no es que la invoque y sea rechazada — es que la operación no está en su superficie alcanzable.

Defensa en profundidad, en tres capas:

1. **Arquitectura** — separación de capacidades (principal)
2. **Verificación** — la marca firmada `SIMULATION_ONLY`, con rechazo por defecto ante marca ausente, ilegible o inválida
3. **Prueba** — banco de pruebas adversario dedicado que intente el bypass en cada release

### 8.2 Autoprotección del agente — aceptada, **alcance acotado con honestidad**

Se acepta la priorización. Pero debe fijarse el límite real: **un agente en espacio de usuario no puede protegerse de un atacante con SYSTEM o root.** Prometer *tamper-proof* es insostenible.

| Alcanzable | No alcanzable sin driver de kernel |
|---|---|
| Detección de manipulación de binario y configuración | Impedir la terminación del proceso por SYSTEM/root |
| Verificación de integridad en arranque y en caliente | Impedir la lectura de memoria del proceso |
| Alerta remota inmediata ante manipulación | Impedir la desinstalación por un administrador local |
| Registro firmado del evento en `ALM-01` | |

Posición comercial correcta: **"a prueba de evidencia" (*tamper-evident*), no "a prueba de manipulación" (*tamper-proof*).** Y dado que RPT-002 §5 descarta escribir driver de kernel propio (§5.2), el límite es estructural, no de esfuerzo.

### 8.3 Inteligencia Regional como suscripción — **aceptada, es la pieza que cierra PA-01**

La observación identifica correctamente el activo monetizable. Adoptada como fuente principal de ingreso en §2.5.

### 8.4 Modo pasivo en OT — aceptada

Se incorpora la restricción a la documentación contractual y al material de demostración, no solo a la especificación técnica. **Acción:** cláusula de limitaciones conocidas en la ficha de producto industrial.

---

## 9. Política de Calidad, Pruebas y Auditoría

Aplica desde el primer *commit* de código. **Ninguna de estas verificaciones es opcional ni posponible a "cuando haya tiempo".**

### 9.1 Regla de documentación

Todo cambio funcional actualiza la documentación **en el mismo commit**. Un *pull request* que modifique comportamiento sin tocar `docs/` se rechaza automáticamente. La documentación desactualizada de un producto de seguridad es un pasivo, no una omisión menor.

### 9.2 Pruebas unitarias y de propiedades

| Ámbito | Requisito |
|---|---|
| Cobertura mínima por crate | 70 % de líneas; **90 % en `motor-pqc` y `guardian-cc`** |
| Analizadores de protocolo | **Pruebas basadas en propiedades** obligatorias (`proptest`). Los parsers de red son la superficie de ataque número uno de todo el producto. |
| `motor-pqc` | **Vectores de prueba oficiales NIST (ACVP)** para ML-KEM y ML-DSA. Innegociable: una implementación poscuántica sin vectores oficiales no es verificable. |
| `ALM-01` | Pruebas de invariante de la cadena Merkle: toda mutación fuera de la ruta de anexado debe ser detectada |
| Ruta de contención | **Prohibido el uso de mocks.** Banco de pruebas con switch físico o emulador de fabricante. Un mock que devuelve éxito valida el mock, no la contención. |

### 9.3 Fuzzing

`cargo-fuzz` de ejecución continua sobre los analizadores L2/L3 de `guardian-cc` y sobre el verificador de tokens de licencia. Corpus versionado en el repositorio. Todo fallo encontrado se convierte en caso de regresión permanente.

### 9.4 Verificaciones automáticas en CI

Ejecución obligatoria en cada *push* y *pull request*:

| Verificación | Herramienta | Busca |
|---|---|---|
| Secretos en código e historia | `gitleaks` | Claves, tokens, credenciales expuestas |
| Vulnerabilidades en dependencias | `cargo audit` | CVE conocidas en el árbol de dependencias |
| Licencias y dependencias prohibidas | `cargo deny` | **Crítico para la frontera open-core: impide que una dependencia copyleft contamine un crate Apache 2.0** |
| Análisis estático | `cargo clippy -D warnings` | Antipatrones, `unwrap` en ruta de producción |
| Código inseguro | `cargo geiger` + MIRI | Todo bloque `unsafe` requiere justificación escrita en comentario |
| **Implementaciones inconclusas** | `cargo xtask verificar` | **`todo!()`, `unimplemented!()`, `panic!("TODO")` y endpoints sin implementar bloquean el build de release** |
| Datos simulados en producción | `cargo xtask verificar` | Detecta *mocks*, datos de ejemplo y credenciales de prueba fuera de `#[cfg(test)]` |
| **El guardián sigue vivo** | Prueba negativa en CI | Presenta una violación deliberada y exige que el guardián falle. Ver §9.5 |
| Formato | `cargo fmt --check` | Consistencia |
| Inventario de componentes | `cyclonedx` | SBOM firmado por cada release |

**Ganchos de pre-commit** con `gitleaks` y `cargo fmt` en local, para que ninguna clave llegue nunca a la historia del repositorio. Un secreto en la historia de Git es permanente aunque se borre del árbol.

### 9.5 Política de resolución de hallazgos del guardián

Incorporada de propuesta de equipo externo (agosto 2026), **aceptada con dos correcciones**.

#### Aceptado

| Hallazgo | Resolución |
|---|---|
| `todo!()` / `unimplemented!()` | Implementar la lógica, o modelar la imposibilidad con el sistema de tipos |
| `// TODO`, `// FIX` | Resolver en el mismo PR, o **eliminar el comentario y abrir un issue** referenciando el componente. La rama `Principal` refleja solo estado funcional probado |
| Puntos finales fijos (`localhost:8080`) | Sustituir por configuración inyectada (`ConfiguracionRed`). Ninguna IP ni puerto fijo en las bibliotecas base |
| Mocks y URL de prueba | Permitidos **solo** dentro de `#[cfg(test)]`, correctamente delimitados |

Principio rector aceptado: **no se relajan las reglas del guardián.** Se sustituye el marcador por código real o por un issue formal.

#### Corrección 1 — `NoImplementado` es detectado por el propio guardián

La propuesta recomienda sustituir `todo!()` por `Result::Err(ErrorPlataforma::NoImplementado)`. **Ese remedio activa el guardián**: el patrón de §9.4 detecta `NoImplementado` precisamente porque señala trabajo pendiente disfrazado de error.

La distinción que hay que preservar:

| Situación | Variante correcta | ¿Detectada? |
|---|---|---|
| Falta implementar; se hará más adelante | *(no existe variante válida — abrir issue)* | Sí, y debe serlo |
| La operación **no existe por diseño** en esta plataforma o configuración | `ErrorPlataforma::NoSoportado` | No |

`NoImplementado` significa "pendiente". `NoSoportado` significa "no aplica, permanentemente". Solo la segunda es un estado final legítimo. Se adopta `NoSoportado` y el patrón del guardián se mantiene sin cambios.

#### Corrección 2 — El script de PowerShell no hace lo que la propuesta afirma

La propuesta declara que "el script de verificación está programado para omitir las comprobaciones dentro de los bloques de prueba". Eso es cierto en la versión bash, que cuenta llaves y **reanuda el análisis tras el bloque**. La versión PowerShell propuesta usa:

```powershell
if ($Linea -match "#\[cfg\(test\)\]") { break }
```

`break` **abandona el fichero completo** al primer `#[cfg(test)]`. Todo lo que siga —incluidas violaciones reales— queda sin revisar, y el script informa conformidad. Un guardián con falsos negativos silenciosos es peor que ninguno: produce confianza injustificada.

Además omite la mitad de los patrones de §9.4: no detecta `mock`, `dummy`, `stub`, `fake`, `panic!("TODO")`, `127.0.0.1`, `example.com` ni `NotImplemented`. Dos guardianes con reglas distintas hacen que lo que pasa en local falle en CI, y el equipo termina desconfiando de la CI.

**No se adopta el `.ps1` en su forma actual.**

#### Resolución de PA-11 — `cargo xtask verificar` *(cerrado 4-ago-2026)*

Mecanismo único: el guardián reside en el crate **`xtask`** del propio workspace.

| Propiedad | Consecuencia |
|---|---|
| Corre en Windows, Linux y CI con la misma invocación | No hay guardianes divergentes que hagan desconfiar de la CI |
| No depende de bash ni de PowerShell | Se elimina la dependencia de Git Bash para desarrollar en Windows |
| **Se prueba con `cargo test`** | Trece pruebas, entre ellas el caso exacto que rompía el `.ps1` |
| Es un crate más del workspace | Sin herramientas ni toolchains adicionales; `publish = false` lo mantiene fuera del producto |

**Exclusión de bloques de prueba consciente del léxico.** El guardián cuenta llaves y **reanuda** el análisis al cerrar el bloque, ignorando las llaves que aparecen dentro de cadenas, cadenas crudas, literales de carácter y comentarios. Un contador ingenuo se equivoca ante `let s = "}";` dentro de un módulo de pruebas, y ese error reabre la exclusión antes de tiempo o la cierra tarde — en ambos casos en silencio.

**Prueba negativa obligatoria en CI.** Antes de aceptar el veredicto del guardián, la CI le presenta un `todo!()` deliberado y exige que falle. Esta política nace de la experiencia directa: durante la construcción de esta plataforma **dos guardianes distintos pasaron en verde con la violación presente** — el `.ps1` por el `break`, y la configuración de dependency-cruiser de `eje-vision` por excluir `dist` del grafo. Ninguno se habría detectado sin provocarlos.

`scripts/verificar-inconclusos.sh` queda retirado; se conserva como redirector para no fallar en silencio ante invocaciones antiguas.

#### Integración en el flujo

Gancho de `pre-commit` que ejecute el guardián y `gitleaks` sobre los ficheros modificados. El gancho es una conveniencia para el desarrollador; **la CI sigue siendo la autoridad**, porque los ganchos locales se pueden omitir con `--no-verify`.

### 9.6 Auditoría externa

Antes de la primera instalación productiva en entorno hospitalario o industrial:

- Auditoría criptográfica independiente de `motor-pqc`
- Prueba de penetración contra `AGT-07` y la ruta de contención
- Verificación por tercero del no-bypass de `SIMULATION_ONLY` (§8.1)

---

## 10. Estado Actualizado de Puntos Abiertos

| ID | Punto | Estado |
|---|---|---|
| PA-01 | Frontera open-core | ✅ **Resuelto y ratificado en firme** — frontera invertida (§2.5), constancia §2.7 |
| PA-02 | Licencias fuera de línea | ✅ Resuelto (§3) |
| PA-03 | Término "Zero-Knowledge" | ✅ Resuelto (§4) |
| PA-04 | Npcap OEM y WHQL | ✅ Resuelto — **WHQL fuera de ruta crítica** (§5) |
| PA-05 | Credenciales de switch | ✅ Resuelto (§6) |
| PA-06 | Alojamiento STUN/DERP | ✅ Resuelto (§7) |

### Puntos abiertos nuevos

| ID | Punto | Bloquea |
|---|---|---|
| ~~PA-07~~ | ~~Ratificación de la frontera open-core invertida~~ | ✅ **Cerrado 4-ago-2026** — constancia §2.7. **Primer commit desbloqueado.** |
| **PA-08** | Presupuesto de ancho de banda del relevo DERP y política de cuotas (§7) | Diseño de `RED-02` |
| **PA-09** | Definir el emulador o banco físico de switch para pruebas de contención (§9.2) | Diseño de pruebas de `AGT-01` |
| **PA-10** | Selección de fabricantes de switch para los conectores certificados de Fase 1 | Alcance comercial |

---

*Reporte Nº 3 — Gobernanza y Cierre de Puntos Abiertos · PremosCorp · 4 de agosto de 2026 · Estado: Canónico*
