# RPT-008 — Banco de Pruebas de Conmutadores y Contrato de Contención

**Tema:** Selección de fabricantes Fase 1, banco de pruebas y contrato de contención
**Nº de reporte:** 008
**Fecha:** 5 de agosto de 2026
**Área designada:** Red
**Entidad:** PremosCorp
**Estado:** Canónico con reservas explícitas — véase §7

- **Depende de:** RPT-002 §5 (AGT/RED), RPT-003 §9 (política de calidad), RPT-006 §4 (principio triestático)
- **Cierra:** PA-10; **cierra parcialmente** PA-09 (véase §6)
- **Abre:** PA-22, PA-23
- **Introduce:** `contrato-contencion.toml`

---

## 1. Por qué van unidos

PA-10 seleccionaba fabricantes y PA-09 montaba el banco. Tratados por separado, PA-10 elegiría sobre el papel y PA-09 descubriría después que las imágenes de esos fabricantes no son obtenibles. La disponibilidad del oráculo es un **criterio de selección**, no una consecuencia de ella.

## 2. El criterio real, escrito por fin

La política decía «sin mocks en contención». Es el enunciado, no el criterio. El criterio que este proyecto viene aplicando sin haberlo formulado es:

> **¿Quién escribió el comportamiento contra el que se prueba?**
> Si lo escribimos nosotros, la prueba es un espejo y devolverá nuestros propios supuestos. Si lo escribió otro, es un oráculo.

Es la misma razón por la que `libcrux` sirve de oráculo diferencial a RustCrypto, y por la que los vectores son de NIST y de Google en lugar de nuestros. Aplicado a conmutadores:

| Admisible | Inadmisible |
|---|---|
| Equipo físico | Un respondedor SNMP/SSH escrito por nosotros |
| Imagen virtual del fabricante ejecutando su propio NOS | Un doble que devuelve `Ok` |
| Captura de sesión real reproducida | Una tabla «así responde un switch» |

Una imagen virtual del fabricante **no es un mock**: ejecuta su software real. Lo que no reproduce es el plano de datos en silicio — irrelevante para contención por plano de gestión, que es todo lo que Guardian-CC hace.

Esta reformulación abarata PA-09 en un orden de magnitud sin rebajar el rigor, y sustituye «hace falta hardware» por una pregunta contestable caso a caso.

## 3. Selección de fabricantes — Fase 1

### 3.1 El sesgo que el criterio introduce

Aplicar «disponibilidad de NOS virtual» como criterio **primario** produce una respuesta cómoda y equivocada: selecciona fabricantes de centro de datos. Nokia SR Linux y FRR son de descarga libre; Arista cEOS-lab requiere solo registro. Ninguno de los tres está en el armario de una fábrica ni de un hospital latinoamericano.

El parque OT real lo dominan Siemens SCALANCE, Moxa EDS y Hirschmann/Belden. **Ninguno publica imagen virtual de su NOS.** Las búsquedas no localizaron simulador de SCALANCE — lo más próximo es el conmutador virtual de S7-PLCSIM Advanced, que pertenece a la simulación de PLC y no al NOS del conmutador — ni de Moxa EDS, cuyo «emulador» publicado es un emulador de terminal para conectarse al equipo físico, no un equipo simulado.

Si el criterio se aplica sin corregir este sesgo, la Fase 1 certifica contención sobre equipo que el cliente objetivo no tiene.

### 3.2 La salida: Cisco IOS XE cubre ambos mundos

Los Catalyst IE3300 e IE3400 —la línea industrial rugerizada de Cisco— ejecutan **IOS XE**, la misma familia que los Catalyst de campus. Un solo adaptador sirve al conmutador del pasillo del hospital y al del armario de planta.

Eso convierte a Cisco en el único candidato que satisface las tres condiciones a la vez:

| Condición | Cisco IOS XE | Arista EOS | Nokia SR Linux | Siemens / Moxa / Hirschmann |
|---|---|---|---|---|
| Presencia en el parque objetivo | alta | baja fuera de CPD | muy baja | **alta en OT** |
| Línea industrial con el mismo NOS | **sí** (IE3x00) | no | no | n/a |
| Oráculo escrito por el fabricante | sí — CML‑Free incluye IOL/IOL‑L2; CAT 9000v es imagen IOS‑XE de conmutación | sí — cEOS‑lab, registro gratuito | sí — descarga libre | **no existe** |
| Coste de entrada | registro; CML‑Free sin compra, tope de 5 nodos | registro | ninguno | compra de equipo físico |

**Fase 1 = Cisco IOS XE como fabricante primario; Arista EOS como segundo adaptador.** Arista no aporta cobertura de parque: aporta **una segunda gramática** que obliga a que el contrato de §4 sea de verdad independiente del transporte. Un contrato validado contra un solo fabricante es una abstracción no probada.

### 3.3 Los fabricantes OT quedan pendientes, y hay que decirlo

Siemens, Moxa y Hirschmann **no entran en Fase 1** porque no existe oráculo que no sea el equipo físico. Esto no es un detalle de calendario: significa que la contención sobre el parque OT más común de la región **no estará certificada al cerrar la Fase 1**. Se abre PA-22 para ello, y su resolución pasa por comprar equipo de segunda mano, que es dinero y plazo, no diseño.

Callar esto produciría exactamente el efecto que este proyecto lleva seis reportes evitando: una capacidad que parece verificada y no lo está.

## 4. `contrato-contencion.toml`

La acción de contención descrita como dato. Cuatro decisiones merecen justificación.

### 4.1 Los tres estados, y cuál es el peligroso

```toml
[[estado]] nombre = "Contenido"            # aplicado Y confirmado por relectura
[[estado]] nombre = "ContencionRechazada"  # el equipo dijo que no, con motivo
[[estado]] nombre = "EstadoDesconocido"    # se emitió y no se sabe si surtió efecto
```

`EstadoDesconocido` es `ComprobacionImposible` de RPT-006 §4, y aquí es el estado **dominante**, no el excepcional: plazo agotado, sesión caída entre el envío y la confirmación, escritura aceptada pero relectura muda, aplicación parcial en un apilamiento.

Un puerto que se cree aislado y no lo está es peor que uno que se sabe no aislado, porque el segundo escala a un humano y el primero no. Por eso el manifiesto marca `colapsable = false`.

### 4.2 La relectura es obligatoria para declarar `Contenido`

Sin relectura independiente, `Contenido` sería una declaración de intención con aspecto de hecho observado. Es el mismo defecto que la suite de vectores que solo comprobaba la presencia de los ficheros.

### 4.3 Precondiciones que existen por un daño concreto

La que se olvida es `existe-un-camino-de-gestion-que-no-atraviesa-el-puerto`. Aislar el puerto por el que se administra el conmutador deja el equipo inalcanzable y la acción **irreversible en remoto**. Un incidente de red se convierte en un desplazamiento a sitio.

### 4.4 Contradicción detectada y corregida durante la redacción

El primer borrador del manifiesto declaraba `transporte = "ssh-cli"` para Cisco y `"eapi"` para Arista. `crates/guardian-cc/src/lib.rs` ya define `MecanismoContencion { SnmpV3, Netconf, RadiusCoa, FirewallLocal }` — vocabulario ratificado que además prohíbe explícitamente la suplantación ARP y excluye SNMPv1/v2c por comunidad en claro (RPT-003 §6.4).

Introducir un segundo vocabulario habría duplicado la fuente de verdad justo después de dos reportes dedicados a impedirlo. Y el vocabulario nuevo era **peor**: raspar CLI por SSH es frágil ante cambios de formato y difícil de auditar frente a NETCONF, que devuelve estructura.

Corregido: ambos fabricantes usan `mecanismo = "Netconf"`, y el campo queda atado al enum existente.

Nota aparte sobre `RadiusCoa`: no se ata a fabricante a propósito. Opera sobre la sesión 802.1X y no sobre la configuración del equipo, así que es el único mecanismo que **no deja residuo en el switch si la reversión falla**. Es el candidato natural para OT y se evalúa en PA-22.

### 4.5 Exclusión permanente, anterior a toda política

Tres clases que Guardian-CC no puede contener por ninguna vía, ni con aprobación humana:

- **soporte-vital** — aislar un dispositivo de soporte vital es un evento clínico, no de red
- **seguridad-funcional** — paro de emergencia, cortinas ópticas, enclavamientos: aislarlos puede **provocar** la condición insegura que se pretendía evitar
- **camino-de-gestion** — por §4.3

No es una preferencia configurable. Es un límite del producto, y se evalúa antes que el perfil.

## 5. Perfil OT: sin respuesta automática

```toml
[[perfil]] nombre = "ot"
respuesta_automatica = false
requiere_aprobacion_humana = true
modo_ensayo_por_defecto = true
```

IEC 62443 ordena las prioridades de un sistema de automatización industrial al revés que TI: **disponibilidad y seguridad física por encima de confidencialidad**. Una contención automática que detiene una línea es, en ese marco, el incidente — no la respuesta al incidente.

Queda alineado con lo que la arquitectura ya hace: `eje-red` bloquea el descubrimiento activo en perfil OT y `eje-agente` arranca ese perfil con la capa B deshabilitada. Esta decisión no añade una restricción nueva; **completa una que estaba a medias**.

El modo ensayo recorre todas las precondiciones y la resolución del objetivo, y se detiene justo antes de la escritura. Un ensayo que no ejecuta las precondiciones no prueba nada.

## 6. Qué queda de PA-09

PA-09 se cierra **parcialmente**, y conviene ser exacto sobre el reparto:

| Cerrado | Pendiente |
|---|---|
| Criterio de admisibilidad del oráculo (§2) | Levantar el banco y ejecutarlo |
| Selección de fabricantes Fase 1 (§3) | Descargar las imágenes y confirmar licencia y funcionamiento |
| Contrato de contención (§4) | Adaptadores IOS XE y EOS contra el contrato |
| Política por perfil (§5) | Pruebas de contención contra los NOS virtuales |

La lógica de decisión de Guardian-CC —qué contener, bajo qué perfil, con qué exclusiones— **ya no está bloqueada**: es lógica pura sobre el contrato y se prueba sin ningún conmutador. Lo que sigue bloqueado es la emisión hacia un equipo, que es una porción menor de lo que el bloqueo aparentaba cubrir.

### 6.1 Implementado en este reporte

`crates/guardian-cc` pasa de 3 a **11 pruebas**, todas ejecutadas sin hardware:

- `EstadoContencion` con los tres estados y `escala_a_humano()` — `Desconocido` escala igual que `Rechazada`
- `ClaseExcluida` con las tres clases de exclusión permanente
- `evaluar(clase, perfil) -> Veredicto` con el orden correcto: **exclusión antes que perfil**
- `PerfilSegmento::permite_respuesta_automatica()` — falso en OT
- Cuatro pruebas de paridad contra `contrato-contencion.toml`

Probadas por negativa, mutando el manifiesto:

| Mutación | Prueba que falla |
|---|---|
| `mecanismo = "ssh-cli"` | `todo_mecanismo_declarado_existe_en_el_enum` |
| `seguridad-funcional` → `seguridad-opcional` | `las_clases_excluidas_coinciden_con_el_manifiesto` |
| OT con `respuesta_automatica = true` | `los_perfiles_del_manifiesto_coinciden_con_la_politica` |

La primera es la que habría atrapado mi propio error de §4.4 si hubiera existido antes de escribirlo.

`la_exclusion_permanente_vence_al_perfil_corporativo` merece mención aparte: justifica el orden de evaluación. Si el perfil se comprobara primero, un segmento corporativo ejecutaría contención sobre un dispositivo de soporte vital.

## 7. Reservas explícitas sobre este reporte

Aplicando RPT-003 §9.5, lo que **no** está verificado:

1. **Ninguna imagen ha sido descargada ni ejecutada.** Las condiciones de licencia, el registro exigido, las versiones disponibles y el funcionamiento real proceden de documentación de terceros y de los propios fabricantes leída por búsqueda, no de comprobación. `contrato-contencion.toml` lo marca con `verificado = false` en ambos fabricantes.
2. **No se localizó cuota de mercado de conmutación por fabricante específica de América Latina.** Las cifras públicas encontradas son globales. La afirmación «Cisco domina el parque objetivo» es un supuesto razonable de la industria, **no un dato medido**, y así debe leerse hasta que alguien lo mida.
3. **La ausencia de NOS virtual en Siemens, Moxa y Hirschmann es una ausencia de evidencia**, no evidencia de ausencia. Ninguno publica uno de forma localizable; podría existir bajo acuerdo comercial. Antes de comprar equipo físico conviene preguntar directamente al fabricante.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-22** | **Cobertura de fabricantes OT.** Siemens SCALANCE, Moxa EDS y Hirschmann carecen de oráculo que no sea equipo físico. Requiere presupuesto de adquisición y plazo, no diseño | Certificación de contención sobre el parque OT real |
| **PA-23** | **Clasificación de dispositivo para la exclusión permanente.** El contrato excluye `soporte-vital` y `seguridad-funcional`, pero no define cómo se determina que un dispositivo pertenece a esas clases. Una clasificación errónea convierte la protección en teatro | Habilitación de la contención en hospitales y planta |

PA-23 es el más grave de los dos y no debería quedar detrás de PA-22. Una lista de exclusión que depende de una clasificación no fiable protege sobre el papel.

---

*Reporte Nº 8 — Banco de Pruebas de Conmutadores y Contrato de Contención · PremosCorp · 5 de agosto de 2026*
