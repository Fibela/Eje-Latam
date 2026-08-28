# Reporte de Construcción Nº 2 — Arquitectura Consolidada

| Campo | Valor |
|---|---|
| **Tema documentado** | Arquitectura Consolidada y Corrección del Corpus Técnico |
| **Número de reporte** | 002 |
| **Fecha** | 4 de agosto de 2026 |
| **Área designada** | Arquitectura |
| **Entidad / Firma** | PremosCorp |
| **Versión de arquitectura** | 2.0 (Soberanía Local — Opción 1A) |
| **Estado** | Canónico — con enmiendas de RPT-003 |

> **Enmiendas vigentes.** RPT-003 (Gobernanza, 4-ago-2026) modifica este documento en §5 (AGT-01, requisitos de firma en Windows), §7 (frontera open-core), §11 (áreas designadas) y §12 (puntos abiertos). Las secciones enmendadas lo señalan en su lugar.

## Trazabilidad

Este reporte **reemplaza y deja obsoletos** los siguientes documentos previos:

- Reporte de Construcción Nº 1 — v1.0 (desglose modular M1–M12, arquitectura regional `PREMOS-BRAIN`)
- Reporte de Construcción Nº 1 — v1.2 (Local-First & Conectividad Dinámica)
- Síntesis Oficial de Decisiones Aprobadas (agosto 2026) — **incorporada íntegramente** en este documento

Ante cualquier discrepancia entre este reporte y los anteriores, **prevalece este documento**. Los anteriores se conservan únicamente como registro histórico y no deben citarse como especificación.

---

## 1. Declaración de Propósito

Eje-Latam es una plataforma de ciberseguridad **Local-First** para entornos corporativos, industriales (OT) y sanitarios de Latinoamérica. Su premisa es la **soberanía del dato**: cada instalación es un nodo autónomo que opera con capacidad plena de detección, cifrado poscuántico, persistencia y respuesta **sin depender de conectividad a internet ni de infraestructura de PremosCorp**.

La plataforma se diferencia en tres ejes que las suites globales atienden mal en la región:

1. **Cobertura de dispositivos sin sistema operativo instalable** (PLC industriales, cámaras, bombas de infusión) mediante inspección de red adyacente, no mediante agentes pesados sobre SO comercial.
2. **Transición poscuántica práctica** para empresas medianas, vía envoltorio híbrido local en lugar de reprogramación de sistemas heredados.
3. **Inteligencia de amenazas con foco 100 % latinoamericano** — troyanos bancarios regionales, fraude por mensajería y suplantación de plataformas de pago locales.

---

## 2. Nomenclatura Oficial

**Regla general:** la documentación y la interfaz usan español con acentuación completa; los identificadores de código, rutas, paquetes y binarios usan ASCII sin acentos.

| Identificador en código | Nombre en UI / Doc | Lenguaje / Tecnología | Fase |
|---|---|---|---|
| `eje-agente` | Eje-Agente | Rust (demonio, motor principal) | 1 |
| `eje-vision` | Eje-Visión | TypeScript / React / Electron | 1 |
| `eje-red` | Eje-Red | Rust (conectividad P2P y LAN) | 1 |
| `eje-almacen` | Eje-Almacén | Rust + libSQL (base de datos local) | 1 |
| `boveda` | Bóveda Aislada | Rust (persistencia en búfer fuera de línea) | 1 |
| `guardian-cc` | Guardián de Confianza Cero | Rust (inspección pasiva IoT/OT) | 1 |
| `motor-pqc` | Motor Poscuántico | Rust (ML-KEM / ML-DSA + AES-256-GCM) | 1 |
| `eje-nucleo` | Eje-Núcleo | Go (backend regional en la nube) | **2** |

### 2.1 Nomenclatura retirada

Quedan **prohibidos** en toda documentación, código y material comercial los siguientes términos:

| Término retirado | Motivo | Reemplazo |
|---|---|---|
| `PREMOS-NODE`, `PREMOS-BRAIN`, `PREMOS-VISION` | Duplicaba el branding con `Eje-*` | `eje-agente`, `eje-nucleo`, `eje-vision` |
| `Agente-Eje`, `Núcleo-Eje`, `Red-Eje`, `Vision-Eje` | Convención de sufijo, inconsistente con la marca | Prefijo `Eje-*` |
| **"Infección Pasiva"** | **Error grave: describe al agente como agente infeccioso** | **"Inspección Pasiva"** |
| "post-cuántico" | Forma no preferida por la RAE ante consonante | "poscuántico" |
| "telemetría anonimizada" | Jurídicamente inexacto bajo LGPD / LFPDPPP / Ley 1581 | "telemetría seudonimizada" |
| `Eje-Storage`, `Eje-Core`, `Eje-Agent`, `Offline Vault` | Anglicismos | `eje-almacen`, `eje-nucleo`, `eje-agente`, `boveda` |

---

## 3. Arquitectura Consolidada v2.0

### 3.1 Decisión estructural (Opción 1A)

**Toda la funcionalidad de Fase 1 reside en Rust.** La capa Go se difiere íntegramente a Fase 2 como servicio en la nube.

Justificación: en la arquitectura v1.2 el proceso Go corría localmente junto al demonio Rust, lo que obligaba a pagar el costo de dos cadenas de herramientas, enlazado estático en Windows, recolección de basura cruzada y una frontera cgo/FFI — **sin aportar ninguna capacidad que Rust no cubriera ya**. La capa Go recupera su justificación solo cuando existe un backend multiinquilino real que correlacione entre organizaciones, y eso es Fase 2.

**Consecuencia obligatoria:** el *Correlacionador de Eventos Local* (antes módulo 2.1, en Go) **se traslada a Rust dentro de `eje-agente`** como `AGT-05`. Sin este traslado, la Fase 1 se queda sin motor de correlación. Ver §9.1.

### 3.2 Diagrama de ejecución local

```
========================================================================================
                    ENTORNO DE EJECUCIÓN LOCAL — APLICACIÓN EJE-LATAM
                              Fase 1 · 100 % autónoma
========================================================================================

 ┌──────────────────────────────────────────────────────────────────────────────────┐
 │                        1. CAPA DE INTERFAZ Y CONTROL                             │
 │                     eje-vision · TypeScript / React / Electron                   │
 ├──────────────────────┬──────────────────────┬──────────────────────┬─────────────┤
 │ VIS-01               │ VIS-02               │ VIS-03               │ VIS-04/05   │
 │ Consola Eje-Almacén  │ Tablero Directivo    │ Lanzador GUI         │ Panel CC +  │
 │ · Cliente SQL manual │ · Simulación de      │ · Selección esquema  │ Mapa de     │
 │ · Visor de esquemas  │   guerra directiva   │ · Selección modo red │ Calor       │
 │ · Import / Export    │ · Impacto operativo  │ · Términos y licencia│ Regional    │
 └──────────────────────┴──────────────────────┴──────────┬───────────┴─────────────┘
                                                          │
                     IPC nativo del sistema operativo     │
        (socket Unix con ACL / named pipe de Windows — NO puerto TCP local)
                                                          ▼
 ┌──────────────────────────────────────────────────────────────────────────────────┐
 │                     2. CAPA DE AGENTE, SEGURIDAD Y CORRELACIÓN                   │
 │                              eje-agente · Rust                                   │
 ├──────────────────────┬──────────────────────┬──────────────────────┬─────────────┤
 │ AGT-01 guardian-cc   │ AGT-02 motor-pqc     │ AGT-05 Correlacionador│ AGT-06     │
 │ · Inspección pasiva  │ · Envoltorio híbrido │  Local                │ Autoactua- │
 │   L2/L3 (PLC, cámara)│   ML-KEM / ML-DSA    │ · Heurística local    │ lización   │
 │ · Detección anomalías│ · AES-256-GCM reposo │ · Correlación P2P     │ · Anillos  │
 │ · Orden de contención│ · Canales TLS        │                       │ · Canario  │
 ├──────────────────────┼──────────────────────┼──────────────────────┼─────────────┤
 │ AGT-03 Sonda de      │ AGT-04 Bóveda        │ AGT-07 Autoprotección │ SIM-01     │
 │  Inteligencia Regional│  Aislada (boveda)   │  del Agente           │ Simulador  │
 │ · IoC locales        │ · Cola cifrada       │ · Antimanipulación    │ de Crisis  │
 │ · Filtro de Bloom    │ · Reconciliación     │ · Integridad propia   │ Local      │
 └──────────────────────┴──────────┬───────────┴──────────────────────┴─────────────┘
                                   │
                                   ▼
 ┌──────────────────────────────────────────────────────────────────────────────────┐
 │                  3. CAPA DE ALMACENAMIENTO — eje-almacen · libSQL                │
 ├──────────────────────────────────────────┬───────────────────────────────────────┤
 │ ALM-01 Registro de Evidencia             │ ALM-02 Sandbox del Analista           │
 │ · Solo anexado (append-only)             │ · Base desacoplada                    │
 │ · Encadenamiento de hashes (Merkle)      │ · ALTER / DROP permitidos             │
 │ · Cadena de custodia forense             │ · Copia de solo lectura de evidencia  │
 │ · IMPOSIBLE de alterar desde la GUI      │ · Consultas libres del analista       │
 └──────────────────────────────────────────┴───────────────────────────────────────┘
                                   ▲
                                   │
========================================================================================
              4. CAPA DE CONECTIVIDAD HÍBRIDA — eje-red · Rust
========================================================================================
                                   │
          ┌────────────────────────┴────────────────────────┐
          ▼                                                 ▼
 ┌─────────────────────────────────┐        ┌─────────────────────────────────────┐
 │ RED-01 · CAPA A — RED LOCAL     │        │ RED-02 · CAPA B — REMOTO / P2P      │
 ├─────────────────────────────────┤        ├─────────────────────────────────────┤
 │ · mDNS / difusión UDP           │        │ · Registro dinámico de IP           │
 │ · Modo PASIVO obligatorio en    │        │ · Atravesamiento de NAT             │
 │   segmentos OT (ver §9.2)       │        │ · Requiere señalización STUN/DERP:  │
 │ · Sincronización con nodos      │        │   nube PremosCorp o autoalojado     │
 │   vecinos de la subred          │        │ · P2P sin infraestructura SOLO en   │
 │                                 │        │   subredes con ruteo directo        │
 └─────────────────────────────────┘        └─────────────────────────────────────┘
                                   │
                                   ▼
                        ┌──────────────────────────┐
                        │  FASE 2 — eje-nucleo · Go│
                        │  premoscorp.com          │
                        │  NUC-01 · NUC-02         │
                        │  NUC-03 · NUC-04         │
                        └──────────────────────────┘
```

---

## 4. Renumeración de Módulos

La numeración `M1`–`M12` del Reporte Nº 1 queda **anulada**. Presentaba tres defectos: `M8` y `M11` aparecían en los diagramas sin descripción alguna, `M12` se describía sin figurar en ningún diagrama, y el mismo módulo se citaba como `M10` y `M12` en secciones distintas.

El esquema nuevo usa **prefijo por área**, lo que permite añadir módulos sin renumerar el corpus completo.

| Código nuevo | Módulo | Componente | Código anterior |
|---|---|---|---|
| `AGT-01` | Guardián de Confianza Cero (IoT/OT) | `eje-agente` | M1 |
| `AGT-02` | Motor Poscuántico | `eje-agente` | M2 |
| `AGT-03` | Sonda de Inteligencia Regional | `eje-agente` | M3 |
| `AGT-04` | Bóveda Aislada | `eje-agente` | M4 |
| `AGT-05` | Correlacionador de Eventos Local | `eje-agente` | *2.1 (era Go)* |
| `AGT-06` | Módulo de Autoactualización | `eje-agente` | 3.3 |
| `AGT-07` | Autoprotección del Agente | `eje-agente` | **nuevo** |
| `SIM-01` | Simulador de Crisis Local | `eje-agente` | M6 (parcial) |
| `ALM-01` | Registro de Evidencia (solo anexado) | `eje-almacen` | 4.1 (parcial) |
| `ALM-02` | Sandbox del Analista | `eje-almacen` | 4.1 (parcial) |
| `RED-01` | Capa A — Descubrimiento local | `eje-red` | Capa A |
| `RED-02` | Capa B — P2P / NAT | `eje-red` | Capa B |
| `VIS-01` | Consola Eje-Almacén (SQL) | `eje-vision` | 1.1 |
| `VIS-02` | Tablero Directivo C-Level | `eje-vision` | M9 / 1.2 |
| `VIS-03` | Lanzador GUI | `eje-vision` | 1.3 |
| `VIS-04` | Panel Confianza Cero e Inventario Vivo | `eje-vision` | **M11 (rescatado)** |
| `VIS-05` | Mapa de Calor Regional | `eje-vision` | **M10 / M12 (unificados)** |
| `CON-SIM` | Consola de Simulación | `eje-vision` | **nuevo — RPT-004 §2.2**. Ordena y observa simulacros; el motor `SIM-01` permanece en `eje-agente` |
| `NUC-01` | Correlación Regional Multiinquilino | `eje-nucleo` · F2 | M5 |
| `NUC-02` | Simulador de Crisis Regional | `eje-nucleo` · F2 | M6 |
| `NUC-03` | Hub de Transición Criptográfica | `eje-nucleo` · F2 | M7 |
| `NUC-04` | Gestor de Claves y Licencias | `eje-nucleo` · F2 | **M8 (rescatado)** |

---

## 5. Especificación de Módulos — Fase 1

### AGT-01 · Guardián de Confianza Cero

**Modelo de despliegue — Sensor Adyacente.** El agente **no se instala** en PLC, cámaras ni bombas de infusión. Opera como sensor de red que recibe copia del tráfico mediante puerto SPAN, TAP pasivo, o desde el gateway del segmento. Esta distinción es contractual, no cosmética: define qué se le puede prometer al cliente y cómo se dimensiona el despliegue.

**Captura de tráfico.**

> ⚠️ **ENMENDADO por RPT-003 §5.** La versión original de esta tabla exigía atestación WHQL de Microsoft. **Era incorrecto:** WHQL solo aplica si se escribe un driver de kernel propio. Npcap se distribuye ya firmado por su fabricante, por lo que **WHQL queda fuera de la ruta crítica.**

| Plataforma | Mecanismo de captura | Requisito real |
|---|---|---|
| Windows | Npcap (ruta de referencia) | **Licencia Npcap OEM** para redistribuir + **certificado EV Code Signing** para el instalador propio. **Sin WHQL.** |
| Linux | `AF_PACKET` + `PACKET_MMAP` (referencia)<br>eBPF/XDP (optimización) | `CAP_NET_RAW`; `CAP_BPF`/`CAP_PERFMON` solo para la ruta eBPF |
| macOS | BPF (`/dev/bpf*`) | Perfil de permisos del sistema |

> **Kernels industriales antiguos (RPT-003 §5.3).** Los entornos OT operan con frecuencia sobre kernels 3.10–4.x donde eBPF/XDP no está disponible. `AF_PACKET` + `PACKET_MMAP` es la **ruta de captura de referencia** en Linux; eBPF es optimización cuando el kernel lo permita. Diseñar solo para eBPF excluye buena parte del parque industrial instalado.

> **Riesgo de cronograma:** la licencia Npcap OEM (semanas) y el certificado EV Code Signing (1–3 semanas) tienen plazos externos. Deben iniciarse en paralelo al desarrollo.

**Contención — política de aislamiento.**

Queda **terminantemente prohibido** el uso de suplantación ARP para contención en redes de producción. Es la misma técnica que un ataque de intermediario, activa las alertas del propio cliente, puede envenenar cachés de dispositivos legítimos y en entornos OT es capaz de provocar un incidente de seguridad física.

Mecanismos autorizados, en orden de preferencia:

1. Apagado o cuarentena de puerto de switch vía **SNMP** o **NETCONF**
2. **802.1X CoA** (cambio de autorización) para reasignación a VLAN de cuarentena
3. Reglas de firewall local (`nftables` / `iptables` / WFP en Windows) cuando el nodo ejecuta el agente

Los tres requieren integración con la infraestructura del cliente y credenciales delegadas. Esto es trabajo adicional y debe reflejarse en el alcance.

### AGT-02 · Motor Poscuántico

Envoltorio híbrido de tráfico local con algoritmos aprobados por NIST: **ML-KEM** (FIPS 203) para intercambio de claves y **ML-DSA** (FIPS 204) para firmas.

**Limitación que debe declararse al cliente:** el proxy envuelve el extremo cliente. Si el sistema remoto solo habla TLS clásico, el canal híbrido **termina en el proxy local** y el último salto sigue siendo clásico. La protección poscuántica de extremo a extremo solo existe cuando **ambos extremos ejecutan Eje-Latam** o el destino soporta nativamente TLS 1.3 híbrido. Vender lo contrario es insostenible bajo auditoría.

**Corrección técnica:** ML-KEM y ML-DSA **no cifran datos en reposo**. La protección de archivos sensibles usa **AES-256-GCM**, con la clave simétrica envuelta mediante ML-KEM. El documento anterior atribuía a ML-KEM/ML-DSA una función que no cumplen.

### AGT-03 · Sonda de Inteligencia Regional

Cobertura enfocada en vectores latinoamericanos: troyanos bancarios regionales (familias tipo Grandoreiro, Mekotio), campañas de suplantación gubernamental por mensajería, y phishing dirigido contra plataformas de pago locales.

**Corrección de estructura de datos.** La especificación anterior proponía una "base de datos vectorial ultracompacta en memoria" para cotejar hashes de procesos. Es un desajuste conceptual: el cotejo exacto de hashes es un problema de pertenencia a conjunto, no de similitud semántica. Se adopta:

- **Filtro de Bloom + tabla hash** para IoC exactos (hashes, dominios, IP) — órdenes de magnitud más compacto y rápido
- **Índice vectorial** reservado exclusivamente para clasificación de *texto* de phishing por embeddings, si se implementa esa capacidad

**Corrección de persistencia.** La especificación anterior decía "en memoria" en un módulo y persistencia en `RocksDB/SQLite` en otro. Se unifica: **libSQL** como único motor, sin RocksDB. El conjunto activo de IoC se mantiene en memoria como caché, respaldado por libSQL.

### AGT-04 · Bóveda Aislada (`boveda`)

Cuando `eje-almacen` está bloqueado o el sistema sufre un apagón digital, las transacciones se escriben en cola de ficheros cifrados y se reconcilian al restablecerse el servicio.

**Puntos que la especificación anterior no cubría y quedan definidos aquí:**

- **Política de retención:** ventana máxima configurable por despliegue; por defecto 30 días o 5 GB, lo que se alcance primero.
- **Comportamiento ante disco lleno:** rotación con descarte del evento más antiguo **y alerta obligatoria en `VIS-04`**. Un disco lleno en un nodo hospitalario es una interrupción, no un detalle.
- **Caducidad de reglas:** el agente sigue ejecutando respuesta automática sin nube, pero las reglas tienen marca temporal. Superado el umbral de obsolescencia, `AGT-01` degrada a **modo solo-detección** y lo notifica. Actuar automáticamente con inteligencia vencida es peor que no actuar.

### AGT-05 · Correlacionador de Eventos Local

Módulo trasladado desde la capa Go. Procesa telemetría de nodos vecinos recibida por `eje-red`, aplica heurística de amenazas y genera correlaciones dentro del perímetro del cliente. No sale de la red del cliente en Fase 1.

### AGT-06 · Módulo de Autoactualización

Respuesta explícita al patrón de fallo de julio de 2024, en el que una actualización de contenido mal validada de un proveedor global interrumpió hospitales y aerolíneas a escala mundial. La especificación anterior proponía **exactamente ese patrón** —descarga silenciosa y aplicación automática— como característica destacada.

| Perfil | Comportamiento |
|---|---|
| **Modo OT / Industrial / Clínico** *(por defecto en estos entornos)* | Actualización automática **deshabilitada**. Requiere aprobación manual por el flujo de gestión de cambios del cliente. Paquete offline firmado disponible. |
| **Modo Corporativo** | Despliegue en anillos con nodos canario. Ventana de observación entre anillos. Reversión automática ante fallo del canario. |

Toda actualización se verifica por firma digital antes de aplicarse. Se registra en `ALM-01`.

### AGT-07 · Autoprotección del Agente *(módulo nuevo)*

Ausente por completo del corpus anterior. Un binario con autoridad para aislar nodos en infraestructura crítica es un objetivo de alto valor: si se compromete, el atacante hereda capacidad de contención sobre toda la planta.

Alcance mínimo: resistencia a manipulación del binario y la configuración, verificación de integridad propia en arranque y en caliente, protección del proceso frente a terminación no autorizada, y protección de las credenciales delegadas de switch (§AGT-01) que constituyen el activo más sensible del despliegue.

### SIM-01 · Simulador de Crisis Local

**Requisito de seguridad de vida.** El simulador inyecta eventos en agentes que poseen autoridad de aislamiento. En un hospital, un aislamiento accidental disparado por un simulacro puede desconectar equipamiento médico.

Control obligatorio, no opcional:

- Toda inyección lleva la marca criptográfica **`SIMULATION_ONLY`**, firmada, no falsificable y no removible.
- El motor de respuesta de `AGT-01` **rechaza categóricamente** ejecutar cualquier orden de aislamiento, bloqueo o modificación de firewall derivada de un evento marcado como simulación.
- El rechazo es la ruta por defecto: ante una marca ausente, ilegible o inválida en contexto de simulacro, el motor **no actúa**.

### ALM-01 / ALM-02 · Eje-Almacén

La especificación anterior permitía al analista ejecutar `ALTER` y `DROP` sobre las mismas tablas que custodian la evidencia, lo que destruye el valor probatorio de todo el registro. Se separan dos bases:

| | ALM-01 · Registro de Evidencia | ALM-02 · Sandbox del Analista |
|---|---|---|
| Escritura | Solo anexado, exclusiva del agente | Libre |
| DDL desde la GUI | **Imposible** | `ALTER` / `DROP` permitidos |
| Integridad | Encadenamiento de hashes (árbol de Merkle) | No aplica |
| Propósito | Cadena de custodia forense, auditoría, proceso judicial | Exploración, hipótesis, informes ad hoc |
| Origen de datos | Agente | Copia de solo lectura de ALM-01 |

**Modos de esquema del lanzador** (`VIS-03`): Estándar preconfigurado (marcas de tiempo, IoC regionales, registros de red, eventos de confianza cero), Aislado / IoT Ligero (persistencia reducida para nodos limitados), Personalizado (opera **solo sobre ALM-02**).

### RED-01 / RED-02 · Eje-Red

**Capa A — red local.** Autodescubrimiento por mDNS y difusión UDP dentro de la subred. **En segmentos OT el modo activo queda deshabilitado por defecto** (§9.2).

**Capa B — remoto.** La afirmación previa de "túneles P2P sin depender de servidores centrales" era técnicamente imposible: el atravesamiento fiable de NAT requiere un punto de encuentro (STUN/DERP), y sin él se falla frente a CGNAT y NAT simétrico, que dominan el acceso residencial y de pyme en la región.

Postura corregida y honesta:

- Se integra un servidor **ultraligero de señalización STUN/DERP**, alojado en `premoscorp.com` **o desplegable en la infraestructura propia del cliente** (lo que preserva la soberanía para quien la exija).
- La P2P verdaderamente sin infraestructura externa aplica **únicamente a subredes con ruteo directo**.
- El túnel debe ser **auditable y desactivable**. Un canal P2P con atravesamiento de NAT tiene la misma forma que un canal encubierto de exfiltración; sin política explícita, lista de permitidos y registro, el equipo de seguridad del cliente lo tratará —con razón— como un riesgo.

### VIS-01 … VIS-05 · Eje-Visión

Comunicación con `eje-agente` por **IPC nativo del sistema operativo**: socket de dominio Unix con ACL en Linux/macOS, named pipe con descriptor de seguridad en Windows. **No se expone un puerto TCP local** (§9.3).

- **VIS-01 Consola Eje-Almacén** — cliente SQL, visor de esquemas, importación/exportación. Opera contra ALM-02.
- **VIS-02 Tablero Directivo** — traduce el incidente a impacto operativo, financiero y reputacional, con acciones estratégicas ("Aislar Red Industrial", "Notificar al Regulador", "Activar Redundancia Fuera de Línea"). Diferenciador frente a los SIEM tradicionales, que saturan al directivo con registros técnicos.
- **VIS-03 Lanzador** — términos y licencia, selección de esquema y modo de red.
- **VIS-04 Panel de Confianza Cero e Inventario Vivo** *(rescatado)* — inventario de dispositivos IoT/OT descubiertos, postura por nodo, estado de la Bóveda y alertas de capacidad.
- **VIS-05 Mapa de Calor Regional** *(unificado)* — visualización de vectores activos y comparación de postura contra el promedio sectorial. **Dependencia:** requiere datos regionales agregados, es decir `NUC-01`. En Fase 1 se limita a datos del propio despliegue.

---

## 6. Protección de Datos Personales

Se sustituye "telemetría anonimizada" por **seudonimización estricta**. Las direcciones IP y los nombres de host son datos personales bajo **LGPD** (Brasil), **LFPDPPP** (México) y **Ley 1581 de 2012** (Colombia), y la anonimización irreversible de telemetría de red es en la práctica inalcanzable.

Mecanismo: hash con sal por cliente (*salt hashing*) aplicado **antes de que el dato salga del entorno local**. La sal nunca abandona el nodo.

Siendo un producto regional, el cumplimiento por jurisdicción es argumento comercial, no carga administrativa.

---

## 7. Modelo de Negocio — Open-Core

> ⚠️ **ENMENDADO por RPT-003 §2.** La frontera se **invirtió**: el criterio es *mecanismo vs. contenido*, no *núcleo vs. módulos*. `guardian-cc` y `motor-pqc` quedan **abiertos** — cerrarlos vaciaba el argumento de auditabilidad, y la criptografía de caja negra es descalificatoria en auditoría. El activo protegido pasa a ser la operación regional. Ver RPT-003 §2.5.

| Ámbito | Componentes | Licencia |
|---|---|---|
| **Mecanismo** | `eje-agente`, `guardian-cc`, `motor-pqc`, `eje-almacen` (incl. cadena Merkle), `boveda`, `eje-red`, SDK y esquemas | **Apache 2.0** |
| **Contenido y operación** | Suscripción de Inteligencia Regional (firmas, IoC, reglas), `VIS-02`, `SIM-01`, `NUC-*`, conectores de contención certificados, soporte y cumplimiento por país | **Propietaria PremosCorp** |

Fundamento: el código de inspección es una mercancía a tres años; las firmas regionales actualizadas cada semana no lo son. La **Suscripción de Inteligencia Regional** es la fuente principal de ingreso.

---

## 8. Registro de Correcciones Aplicadas

| # | Observación | Estado |
|---|---|---|
| 1 | "Infección Pasiva" → "Inspección Pasiva" | ✅ Corregido, término prohibido |
| 2 | "post-cuántico" → "poscuántico" | ✅ Corregido |
| 3 | Contradicción Local-First vs correlación regional | ✅ Resuelto: Local-First canónico, regional → Fase 2 |
| 4 | "<500 ms" a escala regional, insostenible | ✅ Eliminado del alcance |
| 5 | NAT traversal "sin infraestructura externa" | ✅ Corregido: STUN/DERP, autoalojable |
| 6 | Agente no puede correr en PLC / cámaras | ✅ Redefinido como Sensor Adyacente |
| 7 | Rust↔Go: gRPC vs FFI, y justificación de Go | ✅ Resuelto: Go difierido a Fase 2 |
| 8 | RocksDB vs SQLite vs libSQL | ✅ Unificado en libSQL |
| 9 | Numeración M8, M10, M11, M12 rota | ✅ Renumerado por área; M8 y M11 rescatados |
| 10 | Suplantación ARP para contención | ✅ Prohibido; SNMP / NETCONF / 802.1X CoA |
| 11 | Autoactualización silenciosa en OT y salud | ✅ Deshabilitada por defecto en OT; anillos y canario |
| 12 | Simulacro capaz de disparar aislamiento real | ✅ Marca `SIMULATION_ONLY` con rechazo obligatorio |
| 13 | Proxy PQC no protege extremo a extremo | ✅ Limitación declarada |
| 14 | ML-KEM/ML-DSA atribuidos a cifrado en reposo | ✅ Corregido: AES-256-GCM + envoltura ML-KEM |
| 15 | Base vectorial para hashes de IoC | ✅ Corregido: filtro de Bloom + tabla hash |
| 16 | SQL manual destruía la cadena de custodia | ✅ Separado ALM-01 / ALM-02 |
| 17 | Ausencia de driver de captura y privilegios | ✅ Especificado: Npcap + EV Code Signing, `AF_PACKET`, eBPF. **WHQL descartado** (RPT-003 §5) |
| 18 | Ausencia de modelo de amenaza del agente | ✅ Añadido AGT-07 |
| 19 | "Telemetría anonimizada" | ✅ Corregido: seudonimización con sal por cliente |
| 20 | Licencia y frontera comercial | 🟡 Modelo definido, frontera pendiente (§9.4) |

---

## 9. Incoherencias Detectadas en la Síntesis Oficial

Conflictos **introducidos o no resueltos** por la Síntesis, que este reporte cierra o eleva a decisión.

### 9.1 · Correlacionador local huérfano — RESUELTO

Al diferir `eje-nucleo` a Fase 2, el *Correlacionador de Eventos Local* (antes 2.1, en Go) y el *Gestor de Tareas y Cron* (2.3) quedaban sin componente asignado. La Fase 1 habría quedado **sin motor de correlación**. Se trasladan a Rust como `AGT-05`, y la programación de tareas se absorbe en el planificador interno de `eje-agente`.

### 9.2 · "Enlaces BGP dedicados" en la Capa A — CORREGIDO

La Síntesis describe la Capa A como "autodescubrimiento P2P puro mediante mDNS y UDP Broadcast dentro de la subred local **o enlaces BGP dedicados**". BGP es un protocolo de enrutamiento entre sistemas autónomos; no es un mecanismo de descubrimiento en LAN y no pertenece a esa capa. Se elimina de la Capa A. Si el cliente posee enlaces dedicados entre sedes, corresponden a la **Capa B** como transporte, no a la Capa A como descubrimiento.

**Adicionalmente:** la difusión mDNS/UDP en redes industriales puede perturbar equipos frágiles; algunos PLC antiguos degradan bajo tráfico de difusión inesperado. `RED-01` opera en **modo pasivo por defecto en segmentos marcados como OT** — escucha, no emite.

### 9.3 · WebSocket local como superficie de ataque — CORREGIDO

La Síntesis admite "IPC nativo / IPC de Electron **o WebSockets locales protegidos**". Un WebSocket local abre un puerto TCP en la máquina, accesible a cualquier proceso local y a cualquier página web que el usuario visite (ataques de *DNS rebinding* y de origen cruzado contra servicios en `localhost` son un vector conocido y explotado). Se **elimina** la opción WebSocket. Único transporte autorizado: socket de dominio Unix con ACL, o named pipe de Windows con descriptor de seguridad.

### 9.4 · Frontera open-core no ejecutable — ✅ RESUELTO EN RPT-003 §2

Resuelto invirtiendo la frontera. Se descartó además el mecanismo propuesto de "enlace dinámico de crates privados": **Rust carece de ABI estable**, y la alternativa vía FFI de C introduciría `unsafe` justo en la superficie que procesa tráfico hostil.

### 9.5 · Validación de licencias fuera de línea — ✅ RESUELTO EN RPT-003 §3

Token Ed25519 con caducidad. Principio elevado a regla de producto: **ninguna condición comercial degrada jamás una función de seguridad.** Durante un incidente activo, `VIS-02` opera completo aunque la licencia esté vencida.

"Zero-Knowledge" retirado (RPT-003 §4), con la advertencia de que *Zero-Trust* **no es su reemplazo** — son conceptos distintos.

### 9.6 · Dependencia no declarada de VIS-05 — SEÑALADO

El Mapa de Calor Regional compara la postura del cliente "contra el promedio del sector en Latam". Ese promedio **solo existe si hay agregación multiinquilino**, es decir `NUC-01`, que es Fase 2. En Fase 1 `VIS-05` no puede entregar comparativa sectorial. Debe comunicarse así en el material comercial para no comprometer una capacidad inexistente.

---

## 10. Matriz de Componentes del Repositorio

| Componente | Lenguaje / Tech | Función principal | Entorno | Fase |
|---|---|---|---|---|
| `eje-agente` | Rust | Inspección L2/L3, PQC, correlación local, bóveda, autoprotección | Demonio de fondo | 1 |
| `eje-almacen` | Rust + libSQL | Evidencia solo-anexado + sandbox del analista | Embebido en agente | 1 |
| `eje-red` | Rust | Capa A (LAN) y Capa B (P2P / NAT) | Embebido en agente | 1 |
| `eje-vision` | TypeScript / React / Electron | GUI, consola SQL, tablero directivo | Aplicación de escritorio | 1 |
| `eje-nucleo` | Go | Correlación regional multiinquilino, licencias, Threat Intel | Nube (premoscorp.com) | **2** |

---

## 11. Convención de Reportes

**Formato:** `[Tema documentado/desarrollado] [Nº de reporte] [Fecha] [Área designada]`

**Nombre de archivo:** `RPT-NNN_Tema-En-Kebab_AAAA-MM-DD_Area.md`

**Ubicación:** `docs/reportes/`

**Áreas designadas:** `Arquitectura` · `Agente` · `Red` · `Almacenamiento` · `Interfaz` · `Seguridad` · `Cumplimiento` · `Producto` · `Gobernanza` · `Calidad`

Todo reporte incluye tabla de metadatos, sección de trazabilidad indicando qué documentos reemplaza o enmienda, y estado (`Borrador` / `En revisión` / `Canónico` / `Obsoleto`).

**Regla de mantenimiento (RPT-003 §9.1):** todo cambio funcional actualiza la documentación **en el mismo commit**. Un *pull request* que modifique comportamiento sin tocar `docs/` se rechaza.

---

## 12. Puntos Abiertos

> ⚠️ **ENMENDADO.** PA-01 a PA-06 quedan cerrados en RPT-003 §10. Se conservan aquí con su estado para trazabilidad.

| ID | Punto | Estado |
|---|---|---|
| PA-01 | Frontera open-core | ✅ Resuelto y **ratificado en firme** — RPT-003 §2.7 |
| PA-02 | Validación de licencias fuera de línea | ✅ Resuelto — RPT-003 §3 |
| PA-03 | Término "Zero-Knowledge" | ✅ Resuelto — RPT-003 §4 |
| PA-04 | Trámites de captura en Windows | ✅ Resuelto — RPT-003 §5. **WHQL fuera de ruta crítica** |
| PA-05 | Credenciales delegadas de switch | ✅ Resuelto — RPT-003 §6. **No residen en ALM-01** |
| PA-06 | Alojamiento del servidor STUN/DERP | ✅ Resuelto — RPT-003 §7 |
| ~~PA-07~~ | ~~Ratificación de la frontera invertida~~ | ✅ Cerrado 4-ago-2026 — RPT-003 §2.7 |
| PA-08 | Presupuesto de ancho de banda del relevo DERP | 🟡 Abierto — `RED-02` |
| PA-09 | Banco de pruebas de switch para contención | 🔵 **Parcial** 5-ago-2026 — RPT-008 §6. Criterio, contrato y política cerrados; falta levantar el banco |
| ~~PA-10~~ | ~~Fabricantes de switch para conectores de Fase 1~~ | ✅ Cerrado 5-ago-2026 — RPT-008 §3. **Cisco IOS XE primario, Arista EOS segundo** |
| ~~PA-11~~ | ~~Mecanismo único del guardián de inconclusos~~ | ✅ Cerrado 4-ago-2026 — `cargo xtask verificar`, RPT-003 §9.5 |
| PA-12 | Empaquetador de `eje-vision` | 🟡 Abierto — RPT-004 §10 |
| PA-15 | `CON-SIM` deshabilitado durante incidente activo | 🟡 Abierto — RPT-004 §10 |
| PA-13 | Biblioteca de componentes de la capa base y su licencia | 🟡 Abierto — RPT-004 §10 |
| ~~PA-14~~ | ~~Cadena de firma de releases y del paquete empresarial~~ | ✅ **Partido** 5-ago-2026 — RPT-021 §1. Eran tres puntos con un número, con verificadores distintos |
| PA-14a | **Firma de release.** Certificado de firma de código, custodia en hardware y sellado de tiempo | 🔴 **Abierto, bloquea despliegue de Eje-Visión** — RPT-021 §8 |
| ~~PA-14b~~ | ~~Firma del paquete empresarial y de los módulos en caliente~~ | ✅ Cerrado 5-ago-2026 — RPT-021 §1. Firma híbrida, ya en su sitio desde RPT-004 §5. **Es el único de los tres hijos de PA-14 donde controlamos las dos puntas** |
| PA-14c | Atestación de conformidad PQC (`CONFORMIDAD.lock`) | 🟡 Abierto — RPT-021 §1. Post-MVP |
| PA-46 | **Repositorio firmado para Linux.** No es opcional: `eje-captura` sólo funciona en Linux, así que el agente **no tiene otra plataforma** | 🔴 **Abierto, bloquea despliegue de `eje-agente`** — RPT-021 §5 |
| PA-47 | Procedimiento de rotación del certificado de firma | 🟡 Abierto — RPT-021 §8 |
| ~~PA-16~~ | ~~Selección de bibliotecas poscuánticas~~ | ✅ Cerrado — RPT-005 §7.2 |
| ~~PA-17~~ | ~~Vectores ACVP y Wycheproof: anclaje y ejecución~~ | ✅ Cerrado 4-ago-2026 — RPT-005 §9 |
| PA-18 | Fuente de aleatoriedad: DRBG conforme al NIST | 🟡 Abierto — RPT-005 §7.4 |
| PA-19 | Ventana de reevaluación de la decisión PQC | 🟡 Abierto — RPT-005 §8 |
| ~~PA-20~~ | ~~Contrato IPC entre Eje-Visión y Eje-Agente~~ | ✅ Cerrado 5-ago-2026 — RPT-006 |
| ~~PA-21~~ | ~~Tipado de carga útil por canal del contrato IPC~~ | ✅ Cerrado 5-ago-2026 — RPT-007 |
| PA-22 | Cobertura de fabricantes OT (Siemens, Moxa, Hirschmann): sin oráculo que no sea equipo físico | 🟡 Abierto — RPT-008 §8 |
| ~~PA-23~~ | ~~Clasificación de dispositivo para la exclusión permanente~~ | ✅ Cerrado 5-ago-2026 — RPT-009. **Sin umbral configurable; la inferencia solo excluye** |
| ~~PA-24~~ | ~~Productores de evidencia: inventario firmado~~ | ✅ Cerrado 5-ago-2026 — RPT-013. **Formato en disco, analizador defensivo y recorrido completo**. Huella y OUI siguen en PA-25 |
| ~~PA-29~~ | ~~Acceso al sistema de ficheros, escritura atómica y fuzzing del analizador~~ | ✅ Cerrado 5-ago-2026 — RPT-014. **Guarda RAII, lectura acotada, orden canónico al leer y arnés determinista** |
| PA-30 | Instalación de `cargo-fuzz`, construcción del objetivo, ejecución y corpus semilla | 🟡 Abierto — RPT-014 §8 |
| ~~PA-31~~ | ~~Entrega del certificado de revocación a un agente sin red~~ | ✅ Cerrado 5-ago-2026 — RPT-015. **Viaja con el paquete de inventario; no se busca en servidor** |
| ~~PA-32~~ | ~~Custodia de la clave de recuperación del cliente~~ | ✅ Cerrado 5-ago-2026 — RPT-015 §8.1. **2-de-3 fuera de línea**, con salvedad sobre la localidad de los fragmentos |
| ~~PA-33~~ | ~~Enmienda de RPT-012: el centinela retrocede ante certificado de revocación válido~~ | ✅ Cerrado 5-ago-2026 — RPT-015 §6.1 y §10 |
| ~~PA-34~~ | ~~Persistencia del registro de revocaciones en disco~~ | ✅ Cerrado 5-ago-2026 — RPT-016. **Se guardan los certificados firmados, no el par derivado** |
| ~~PA-35~~ | ~~Cableado de la persistencia: rutas, momento de carga y arranque sin ficheros~~ | ✅ Cerrado 5-ago-2026 — RPT-017. **Borrar el inventario se detecta como supresión mediante el centinela** |
| PA-36 | Vigilancia del directorio de datos en caliente | 🟡 Abierto — RPT-017 §9. **Pospuesto deliberadamente** hasta tener flujo de red |
| PA-37 | Crate `eje-captura`: AF_PACKET de sólo lectura, frontera de `unsafe` | 🔵 **Parcial** 5-ago-2026 — RPT-018 §9. Crate y guardianes verificados; **`linux.rs` nunca se ha compilado** |
| ~~PA-40~~ | ~~Compilar `linux.rs` y ejecutarlo contra una interfaz~~ | ✅ **Cerrado por observación** 25-ago-2026 — RPT-079 §11.6. Los **tres** criterios de §8 a la vez: captura abierta sin `NO DISPONIBLE`, `Descartes del nucleo: 0 (vista completa)` —que exige que `estadisticas()` hablara con el núcleo— y `Tramas observadas` moviéndose de 0 a **4** con `ping` de fondo. El primero solo no bastaba: un `abrir()` que devuelve `Ok` sobre una interfaz muerta daría cero tramas para siempre |
| PA-41 | Intervalo de consulta de alertas desde VIS-04 | 🟡 Abierto — RPT-019 §8 |
| ~~PA-42~~ | ~~Salida de alertas fuera del equipo (syslog o equivalente)~~ | ✅ Cerrado 6-ago-2026 — RPT-031 (diseño) y RPT-032. **Inútil sin PA-61** |
| PA-61 | **Segunda interfaz en la matriz de despliegue.** Un sensor con una sola tarjeta **no puede** emitir: la que vigila suele ser receive-only | 🔴 **Abierto, bloquea PA-42** — RPT-031 §2 |
| PA-62 | Syslog sobre TLS | 🟡 Abierto — RPT-031 §6 |
| PA-63 | Autenticidad de la alerta emitida | 🟡 Abierto — RPT-031 §6 |
| ~~PA-43~~ | ~~Manejadores de `consultar-alertas` y `obtener-condiciones`~~ | ✅ Cerrado 6-ago-2026 — RPT-028. **Abre PA-56** |
| ~~PA-56~~ | ~~Persistencia del registro de evidencia~~ | ✅ Cerrado 6-ago-2026 — RPT-029. **Abre PA-57, PA-58 y PA-59** |
| ~~PA-57~~ | ~~Anclaje del extremo de la cadena fuera del fichero~~ | ✅ Cerrado 6-ago-2026 — RPT-033. **Abre PA-64** |
| ~~PA-64~~ | ~~Anclar el extremo fuera de la máquina~~ (reformulado desde «firmar el ancla») | ✅ Cerrado 8-ago-2026 — RPT-038. **Abre PA-70 y PA-71**. La firma local se descartó: la clave viviría donde el atacante escribe |
| PA-70 | **Fichero de sólo-anexado en Linux** (`chattr +a`). Defensa en profundidad: detiene a quien escribe sin ser root | 🟡 Abierto — RPT-038 §3. Depende de PA-40 |
| ~~PA-72~~ | ~~El techo de asientos no se comprobaba al anexar~~ | ✅ Cerrado 8-ago-2026 — RPT-039 §1. Superarlo dejaba el registro ilegible y el arranque siguiente lo acusaba de manipulación |
| ~~PA-73~~ | ~~Nadie cuenta las pruebas que se ejecutan~~ | ✅ Cerrado 8-ago-2026 — RPT-039 §8. `cargo xtask cobertura`. Tres cifras, no dos: las condicionadas por `#[cfg]` quedan declaradas fuera de la vigilancia |
| PA-71 | **Sellado del ancla en elemento seguro** (TPM 2.0 / SE). Lo único que aísla la clave del sistema de ficheros | 🟡 Abierto — RPT-038 §3. Condición de BOM, con PA-61 |
| ~~PA-58~~ | ~~Cablear la persistencia en el recorrido del agente~~ | ✅ Cerrado 6-ago-2026 — RPT-030. **Abre PA-60** |
| PA-60 | **Anexado incremental del registro.** Reformulado por RPT-034 §1.1: con persistencia por ciclo deja de ser correctitud y pasa a ser **rendimiento** | 🟡 Abierto — RPT-030 §5, RPT-034 §1.1 |
| ~~PA-65~~ | ~~Unidad de servicio y arranque automático~~ | ✅ **Cerrado por observación** 25-ago-2026 — RPT-079 §11.10. `is-enabled` → `enabled`; tras un **reinicio de verdad**, servicio `active` y el socket de vuelta en `srw-rw---- root:vboxeruser /run/eje-latam/agente.sock`. El tercer criterio es el que decide: `/run` es `tmpfs` y se vacía al arrancar, así que un socket que reaparece prueba que `RuntimeDirectory=` funciona fuera de un `systemctl start`. Y la consola volvió a hablarle sin tocar nada |
| ~~PA-66~~ | ~~Bucle de servicio~~ | ✅ Cerrado 6-ago-2026 — RPT-036. **Abre PA-68** |
| ~~PA-68~~ | ~~Probar el ciclo, no sólo sus piezas~~ | ✅ Cerrado 6-ago-2026 — RPT-037. **Abre PA-69**. Encontró la reemisión del historial completo en cada vuelta |
| ~~PA-69~~ | ~~La evidencia en riesgo no tiene canal~~ | ✅ Cerrado 9-ago-2026 — RPT-044. Octava condición `evidenciaEnRiesgo`, reintento mientras haya sucio y asiento `persistencia-restablecida` **al recuperar** |
| ~~PA-67~~ | ~~Servicio continuo~~ | ✅ Cerrado 6-ago-2026 — RPT-034, RPT-035 y RPT-036. **No cierra PA-41**: ese es la cifra del intervalo, y sigue sin medir |
| ~~PA-59~~ | ~~Rotación del registro~~ | ✅ Cerrado 8-ago-2026 — RPT-040, vía C (segmentación). **Abre PA-74**. La poda por política queda para la vía B, cuando la retención esté decidida |
| ~~PA-74~~ | ~~Una consulta tras rotar parece completa y no lo es~~ | ✅ Cerrado 8-ago-2026 — RPT-041. `{ primerDisponible, sucesos }`, leído del disco en cada consulta. **Abre PA-75** |
| ~~PA-75~~ | ~~La paridad valida esquemas, no usos~~ | ✅ Cerrado 8-ago-2026 — RPT-042, frontera TypeScript. **Abre PA-76**. Encontró además que el manifiesto seguía declarando `lista<SucesoAlerta>` |
| ~~PA-76~~ | ~~La paridad de uso no cubre Rust~~ | ✅ Cerrado 8-ago-2026 — RPT-043. Comprobacion **conductual**: se llama al manejador y se comparan las claves del JSON con lo declarado. No encontro nada |

<!--
  PA-108. Las filas de aqui abajo se recuperaron el 13-ago-2026, cuando se vio
  que el tablero se habia quedado en PA-76 mientras se acunaban treinta y nueve
  identificadores nuevos en los reportes.

  Esto no era un descuido de escritura: `cargo xtask tablero` lee ESTE tablero
  como fuente de verdad, asi que durante dos semanas conto una parte del
  proyecto y la presento como el total. La herramienta no mentia sobre lo que
  leia; el sitio que lee habia dejado de escribirse.
-->
| ~~PA-77~~ | ~~¿Dónde vive la consola? ¿Tiene escritorio el sensor?~~ | ✅ Cerrado 13-ago-2026 — RPT-051. **Opción D ratificada**: sensor headless por omisión, consola local aparte, consola de sala leyendo del colector. **Abre PA-107** |
| PA-78 | **Nadie ha visto a los dos procesos hablarse.** Los vectores prueban el formato, no la conversación | 🔵 **Parcial** 25-ago-2026 — RPT-079. **Mitad A cerrada:** los dos procesos se hablan por un socket real, un marco de **1 038 208 bytes** se reensambla entero, y las tres causas de fallo —sin permiso, sin socket, sin respuesta— llegan separadas hasta arriba. Falta la **mitad B**: que el operador lo vea en pantalla. La VM no tiene escritorio (§4.2) y un punto cerrado a medias es peor que uno abierto. De la preparación salió PA-132 y de la ejecución PA-135 y PA-136 |
| ~~PA-132~~ | ~~La consola por omisión llamaba a una puerta que no existe~~ | ✅ **Cerrado** 21-ago-2026 — RPT-079 §2.1. El agente abría `/run/eje-latam/agente.sock` desde RPT-067 y la consola seguía en `/run/eje/agente.sock`: **con los valores de fábrica, un sensor sano y una consola sana no se encontraban**. Lo tapaba `EJE_SOCKET`, que los guiones de desarrollo pasan siempre, así que sólo aparecía en un despliegue de verdad. El punto de encuentro pasa a `contrato-ipc.toml` con paridad probada **a los dos lados**, y la constante sale de `arranque.ts` a un módulo sin Electron — vivir donde ninguna prueba podía mirarla es **por lo que** se quedó atrás. De regalo: `EJE_SOCKET=` vacío ya no se toma por un destino (PA-118 en otro sitio). Sexto índice escrito a mano de la serie y el primero que **apunta a otro sitio** en vez de quedarse corto |
| PA-135 | **Cuatro de los seis canales estaban declarados y no cableados.** `obtener-estado-agente`, `obtener-inventario`, `obtener-estado-boveda` y `consultar-sandbox` no tenían manejador; sólo respondían `consultar-alertas` y `obtener-condiciones` | 🔵 **Parcial** 26-ago-2026 — RPT-081. El contrato gana un tercer estado, `servido`, y la barrera lee de ahí: **la lista de dos que la comprobación llevaba escrita a mano dentro** —séptimo índice de la serie, y el único alojado en la propia comprobación que lo habría cazado— sale del manifiesto. `obtener-estado-agente` cableado. Quedan `obtener-inventario` (exige decidir qué es la postura de un nodo sin marcado) y los dos sin sustrato, declarados `servido = false` **con su motivo**, que es la doctrina de los tres estados aplicada al contrato |
| PA-138 | **`obtener-inventario` no tiene productor posible, y no por la postura.** `AlmacenObservacion` guarda los dispositivos en dos colecciones privadas —`volatil` y `pegajoso`— y **no expone ninguna forma de listarlos**: sólo cuenta cuántos hay. Un canal que devuelve una lista de dispositivos descubiertos no puede escribirse contra un almacén que no enumera | 🟡 Abierto — RPT-081 §6, al intentar escribir el productor. De los **cuatro** campos de `NodoInventario` el agente tiene fuente honesta para **uno**, `direccionEnlace`. `identificador` no tiene origen; `clase` exigiría traducir `ClaseExcluida` —soporte vital, seguridad funcional— a `ClaseDispositivo` —plc, cámara, médico— y **no son la misma taxonomía**; `postura` no tiene valor para «no se sabe» (PA-139). Además hay **tres** colecciones candidatas a ser «el inventario vivo» —el volátil, el pegajoso, y el recuento por vuelta que `main.rs` arma para la pantalla— y nadie ha tenido que elegir |
| PA-139 | **`Postura` no tiene valor para «no se sabe», y mezcla juicio con medida.** Declara `conforme\|anomalo\|contenido`: los tres afirman algo sobre el mundo. Un equipo visto en el cable sin marcado firmado no es ninguno. Y `contenido` es un **estado operativo**, no una postura: contener a un equipo borra la razón por la que se contuvo | 🟡 Abierto — RPT-081 §5. `Postura::Contenido` es además **inalcanzable**: el agente no contiene nada (RPT-020). El cambio —cuarta postura `indeterminado` y contención a su propio campo— cruza contrato, `eje-ipc`, TypeScript, VIS-04 y la paridad de los dos lados, así que se hará **una vez y con evidencia**, no antes de PA-138 |
| PA-137 | **`boveda::VigenciaReglas::permite_respuesta_automatica` existe, está probada y no la llama nadie.** El contrato describe `respuestaAutomatica` como «según vigencia de reglas», así que el nombre promete una guarda que hoy no se evalúa | 🟡 Abierto — RPT-081 §4. No se incluyó inventando `Vigentes`: no hay distribución de reglas, y suponerla sería un dato fabricado. Hoy el campo es la conjunción de las **dos** guardas que sí deciden — perfil y estado de arranque |
| PA-136 | **El plano de control espera al de datos, y la ventana no tiene techo.** La latencia de la consola **es** la ventana de captura: los otros cuatro tramos de la vuelta suman menos de 1 ms sobre vueltas de 11 s. Y el bucle no está acotado por `PLAZO`: sigue mientras lleguen tramas, así que `duración = --tramas ÷ ritmo de tramas`, sin techo | 🔴 **Medido y demostrado** 26-ago-2026 — RPT-083. Con el valor por omisión (`--tramas 200`) y un **goteo** de tráfico —el de un PLC sondeando, es decir un segmento OT normal— las vueltas duran **10,7 y 12,9 s** y **tres de seis canales vencen** el plazo de 5 s. Sin ninguna avería en ninguna parte. El arreglo resultó pequeño: `atender` cuesta microsegundos, y llamarlo entre trama y trama sirve el estado de la vuelta anterior —ya persistido, RPT-034 §4 intacto— con la **misma edad de dato** que atender al final, sólo que respondiendo. Queda decidir qué se sirve en la primera vuelta, cuando aún no hay condiciones |
| ~~PA-133~~ | ~~Un sensor recién instalado sin configuración firmada no arrancaba~~ | ✅ **Cerrado** 25-ago-2026 — RPT-080. El servicio **arranca y lo declara**; a mano, donde hay una persona esperando, se explica el uso. La frontera la marca `--ciclos`, que ya separaba servicio de recorrido desde RPT-072, y ahora tiene nombre: `es_servicio`. La barrera no es la rama sino la regla — `el_servicio_arranca_diga_lo_que_diga_la_configuracion` recorre los tres estados, y el `match` que los enumera **no compila** si alguien añade un cuarto sin decidir. **Lo incómodo:** el instalador y RPT-077 §6 ya describían este comportamiento desde el primer día; la prosa iba por delante del mecanismo, y ninguna barrera de texto podía cazarlo porque el texto era correcto |
| ~~PA-134~~ | ~~La frase de paso se leía hasta el fin de la entrada~~ | ✅ **Cerrado** 26-ago-2026 — RPT-082. `read_line` en lugar de `read_to_string`: Enter termina la frase, y **lo que se pegue detrás ya no entra en ella** — pegar las dos órdenes del aprovisionamiento juntas habría cifrado la semilla con el texto de un comando, y no se habría sabido hasta que `configurar` fallara con un mensaje correcto apuntando a la causa equivocada. La causa raíz no era la función sino que **leía `stdin()` directamente y ninguna prueba podía mirarla** (misma forma que PA-132). Sale a `leer_frase(&mut impl BufRead)`: el binario, que corría **cero** pruebas, pasa a cinco. Y entrada cerrada sin nada da error — no es una frase vacía, es que nadie escribió |
| ~~PA-128~~ | ~~El vigía traía un punto de escucha fijo en el código~~ | ✅ **Cerrado** 17-ago-2026 — RPT-075. `--escuchar` pasa a ser obligatoria: esa cadena decide en qué interfaz se expone un servicio de red, y un valor por omisión se convierte en `0.0.0.0` el día que alguien quiere tráfico de otro equipo. Lo cazó `cargo xtask verificar crates`, que llevaba **desde el 13 de agosto** sin ejecutarse pese a estar en la lista de obligatorias |
| ~~PA-129~~ | ~~El guardián miraba la línea cruda, no el código~~ | ✅ **Cerrado** 17-ago-2026 — RPT-076. Cada comprobación declara su `Ambito`: cinco miran código, la de `// TODO` mira la línea entera **porque ahí es donde vive**. Un solo recorrido produce las dos respuestas. Quitar comentarios para todas habría dejado ciega la de marcadores: lo cazó una prueba que ya existía |
| ~~PA-79~~ | ~~Configuración firmada~~ | ✅ **Cerrado** 21-ago-2026 — RPT-074, RPT-077 y RPT-078. Hechos los pasos **1 a 4** de §10: formato binario con arnés de mutación, `eje-manifiesto configurar`, las dos condiciones, y **la obediencia** — con configuración firmada mandan sus valores y una bandera dictada aborta el arranque; `EnvironmentFile` fuera de la unidad. Al cablearlo salieron dos cosas que el diseño no vio: el **círculo del almacén** (la clave que verifica la configuración vive dentro del almacén, así que firmarlo es firmar dónde se busca esa clave) y que la versión estricta de «no verifica» dejaba huérfana una condición recién hecha. Y el paso **5**: el centinela lleva **dos** marcas de agua en la misma escritura atómica, una por serie, y `analizar` exige que le pasen la de configuración — no se puede leer una configuración sin decir contra qué se fecha. La marca avanza **antes** de obedecer, y si no se puede anotar, no se obedece. Al meter la segunda marca apareció que `aceptar_inventario` componía el fichero entero y **habría borrado la de configuración en silencio**. **No cierra la puerta a root**: convierte el ajuste silencioso en avería visible |
| PA-131 | **La serie de configuración no tiene camino de vuelta.** El techo de secuencia impide el congelado permanente (PA-33 aplicado en RPT-078 §5), pero falta el equivalente de `reiniciar_por`: un certificado de recuperación con corte en espacio de secuencia **de configuración**. Reutilizar el del inventario volvería a mezclar las dos series | 🟡 Abierto — RPT-078 §5. Sin esto, una emisión con secuencia alta obliga a seguir subiendo desde ahí; no congela, pero estrecha |
| PA-80 | El marco no lleva identificador de correlación; por eso hay una conexión por petición | 🟡 Abierto — RPT-046 §11.2 |
| ~~PA-81~~ | ~~Un fallo de captura mata el proceso y con él la escucha~~ | ✅ Cerrado 11-ago-2026 — RPT-047. El agente se degrada y sigue sirviendo el porqué. **Abre PA-87 y PA-91** |
| ~~PA-82~~ | ~~El socket se crea en `0600`, lo que obliga a consola y agente al mismo usuario~~ | ✅ Cerrado 11-ago-2026 — RPT-046 §11.4, verificado por observación. `--grupo-ipc` y `0660`. **Abre PA-84** |
| ~~PA-83~~ | ~~La latencia de atención está acotada por la vuelta y no se ha medido~~ | ✅ Cerrado 13-ago-2026 — RPT-050. 500 ms de espera, 0,3 ms de transferencia |
| PA-84 | `--grupo-ipc` aceptaría un **nombre** de grupo y no un número | 🟡 Abierto — `Puesta-en-marcha-local.md` §5, RPT-054 §6. El instalador conoce el grupo que crea |
| ~~PA-85~~ | ~~`main` devolvía `Result` y el fallo salía con `Debug` en lugar de `Display`~~ | ✅ Cerrado 11-ago-2026 |
| PA-87 | ¿Debe un sensor que no observa terminar tras N vueltas? | 🟡 Abierto — RPT-047 §6 |
| ~~PA-89~~ | ~~El arranque local exigía una secuencia de órdenes que nadie había escrito~~ | ✅ Cerrado 12-ago-2026 — `scripts/arrancar-agente.sh` y `Puesta-en-marcha-local.md` |
| ~~PA-90~~ | ~~El puente no exportaba `RespuestaAlertas`~~ | ✅ Cerrado 12-ago-2026 |
| ~~PA-91~~ | ~~`Condiciones` creció y `EMISIBLES` se quedó atrás sin que nada protestara~~ | ✅ Cerrado 12-ago-2026 — barrera con desestructuración exhaustiva. Reforzada en RPT-055 (`NO_EMISIBLES`) y RPT-058 (`enumerar`) |
| ~~PA-92~~ | ~~Cadencia de refresco de VIS-04, sin medir~~ | ✅ Cerrado 13-ago-2026 — RPT-050 §6. ≥500 ms; 2 s con cursor; 0 con `hayMas` |
| ~~PA-93~~ | ~~Qué muestra VIS-04 cuando no encuentra al agente, para quien no es técnico~~ | ✅ Cerrado 12-ago-2026 — RPT-048, `sin-agente.ts` |
| ~~PA-95~~ | ~~La respuesta acotada no se declaraba~~ | ✅ Absorbido por PA-96 |
| ~~PA-96~~ | ~~La cota de `consultar-alertas` era por número de sucesos y no por bytes~~ | ✅ Cerrado 12-ago-2026 — RPT-049, verificado por observación. Con detalles largos el canal quedaba **permanentemente muerto** |
| PA-97 | `componerSucesos` (RPT-048 §4) no lee `hayMas` todavía | 🟡 Abierto — RPT-049 §7 |
| ~~PA-98~~ | ~~El cursor `desdeAsiento` existía desde RPT-019 y nadie lo usaba~~ | ✅ Cerrado 13-ago-2026 — RPT-050 §7. Bitácora con cursor y detección de salto |
| ~~PA-99~~ | ~~El régimen permanente no se había medido~~ | ✅ Cerrado 13-ago-2026 — RPT-050 §8. 55 bytes, un trozo, 500,2 ms |
| PA-100 | El coste de una consulta **en el sensor** se infiere desde el cliente, no se mide dentro | 🟡 Abierto — RPT-050 §8.1 |
| ~~PA-101~~ | ~~El texto inicial de VIS-04 no distinguía «el módulo no arrancó» de «no hay datos»~~ | ✅ Cerrado 12-ago-2026 — RPT-056 §3 le puso prueba |
| ~~PA-102~~ | ~~Ninguna prueba ataba `vis04.js` a los identificadores de su HTML ni al contrato~~ | ✅ Cerrado 13-ago-2026 — RPT-056 §3 |
| PA-103 | La rama `noServido` del panel **no se ha ejecutado nunca** | 🟡 Abierto — RPT-053 §9 |
| ~~PA-104~~ | ~~Desde la sala, un sensor en calma y uno desenchufado producen el mismo dato: ninguno~~ | ✅ **Cerrado por observación** 13-ago-2026 — RPT-057 §4. Se apagó un sensor y la sala se enteró |
| ~~PA-105~~ | ~~Nadie vigilaba la ausencia del latido~~ | ✅ Cerrado 13-ago-2026 — RPT-057. `crates/eje-vigia`, detector de referencia |
| ~~PA-106~~ | ~~Sin prueba de paridad entre lo que ve el técnico por IPC y lo que ve la sala por syslog~~ | ✅ Cerrado 13-ago-2026 — RPT-056. **Paridad declarada, no igualdad**: dos condiciones no viajan por syslog a propósito |
| ~~PA-107~~ | ~~Empaquetado dual~~ | ✅ **Cerrado** 15-ago-2026 — RPT-062, RPT-063 y RPT-069. Las cinco comprobaciones de RPT-054 §8, y el artefacto instalado **funciona**: arranca, escucha, vuelve tras `kill -9` y no escribe donde no debe. Se mantuvo parcial un día de más a propósito, mientras PA-124 dejaba al cliente sin consola. El formato de distribución sale aparte como PA-126 |
| ~~PA-116~~ | ~~Caja de arena del instalador~~ | ✅ **Cerrado por observación** 14-ago-2026 — RPT-063. `cargo xtask probar-instalador`. Descubrió que el ejemplo traía un colector de mentira que **hacía callar al aviso** |
| ~~PA-117~~ | ~~Prueba de fuego del ciclo de vida~~ | ✅ **Cerrado por observación** 15-ago-2026 — RPT-069, en VirtualBox con `systemd` como PID 1. `kill -9` al 1645 y vuelve como 2518; `ProtectSystem=strict` da `Read-only file system` en `/etc` **y** deja escribir en `/var/lib` — bisturí, no martillo. La misma sesión destapó PA-122 a PA-125 |
| ~~PA-118~~ | ~~La unidad convertía «no hay colector» en «el colector está caído»~~ | ✅ **Cerrado por observación** 14-ago-2026 — RPT-064. `${VARIABLE}` vacía entrega una cadena vacía, y el agente la tomaba por destino. Anulaba la décima condición |
| ~~PA-119~~ | ~~Paridad entre `docs/Comandos.md` y las órdenes que `xtask` acepta~~ | ✅ **Cerrado por observación** 14-ago-2026 — RPT-066. `cargo xtask manual`. El despacho, la ayuda y el manual salen ahora de una sola tabla. En su primer barrido encontró que RPT-005 §9.3 mandaba teclear una orden **nunca construida** → PA-121 |
| PA-120 | **El socket y la evidencia comparten directorio.** Separar a `/run/eje-latam/agente.sock` vía `RuntimeDirectory=`, dejando `/var/lib/eje-latam` sólo para el registro | ✅ **Cerrado por observación** 15-ago-2026 — RPT-067 y RPT-069. `RuntimeDirectory=` crea `/run/eje-latam`, el socket nace ahí (`srw-rw---- root:vboxeruser`) y el directorio **desaparece al parar el servicio**: el socket huérfano deja de ser posible por construcción |
| ~~PA-124~~ | ~~La unidad dejaba al sensor sin escucha local~~ | ✅ **Cerrado por observación** 15-ago-2026 — RPT-069 §3. `CapabilityBoundingSet=CAP_NET_RAW` quitaba `CAP_CHOWN` y el socket no podía recibir su grupo. Con la unidad corregida: `srw-rw---- root:vboxeruser /run/eje-latam/agente.sock`. Dos mecanismos correctos que juntos se anulaban |
| PA-126 | **El formato de distribución del paquete sigue sin decidirse.** Hoy es un directorio suelto con `instalar.sh`; falta decidir `.deb`, `.rpm` o tar firmado, y cómo se verifica su procedencia | 🔵 **Parcial** 17-ago-2026 — RPT-073. `.tar.gz` **reproducible** con `MANIFIESTO` de resúmenes; el instalador comprueba integridad **antes** de escribir y falla cerrado sin `sha256sum`. **La autenticidad se transfiere a PA-14a**: `DominioClave::PremosCorp` existe en el vocabulario y no tiene sitio donde vivir en `RutasAlmacen`. El instalador declara a gritos que el paquete no está firmado |
| ~~PA-125~~ | ~~Ninguna condición declara que la escucha local esté caída~~ | ✅ **Cerrado por observación** 16-ago-2026 — RPT-070 §7. Undécima condición `escuchaNoDisponible`. **Sí es emisible**, y es la única de las tres del canal que lo es: describe el otro canal. Observado en máquina real: `condicion=escuchaNoDisponible estado=activa` con prioridad `<107>` y `condiciones=accionAdministrativa,escuchaNoDisponible` en el latido |
| ~~PA-123~~ | ~~En modo continuo el agente escribía el informe completo cada vuelta~~ | ✅ **Cerrado por observación** 17-ago-2026 — RPT-072 §8. De 13 788 líneas en cinco minutos a **21**, y una por minuto en régimen: 1 440 al día frente a 4,3 millones. El contador de vueltas confirma 118 por minuto — **se calló sin perder resolución** |
| ~~PA-122~~ | ~~La línea de uso de `eje-agente` y su analizador eran dos listas escritas a mano~~ | ✅ **Cerrado** 17-ago-2026 — RPT-071. Tabla `OPCIONES` como fuente única; la puerta va **antes** del `match`, así que aceptar una bandera no anunciada es imposible. Destapó una sexta lista a mano en `tests/uso.rs` que decía cubrir «las que se añaden tarde» y no cubría la que se añadió tarde |
| PA-121 | **`cargo xtask conformidad` — NO EXISTE TODAVIA — está diseñada y no construida.** RPT-005 §9.3 la describe con detalle: `CONFORMIDAD.lock`, huella sobre `Cargo.lock` + `FUENTES.lock` + toolchain | 🟡 Abierto — hallazgo de la barrera de PA-119. Documentada como instrucción durante diez días sin que existiera |
| ~~PA-108~~ | ~~Índice único de puntos abiertos~~ | ✅ Cerrado 13-ago-2026 — RPT-060. El índice ya existía y **llevaba dos semanas sin alimentarse**: se recuperan PA-77 a PA-115 y `cargo xtask tablero` falla si un `PA-nn` citado en `docs/` no tiene fila. Cazó PA-14b en su primera ejecución |
| ~~PA-109~~ | ~~`SinColector` sólo llegaba a `journald`; VIS-04 no podía verlo~~ | ✅ Cerrado 13-ago-2026 — RPT-055. Décima condición, **segunda no emisible** |
| PA-110 | ¿Un asiento de arranque que declare la ausencia de colector? Exige clase de evento nueva | 🟡 Abierto — RPT-054 §4.4 |
| PA-111 | **Verificación del artefacto desde fuera del proceso.** Un binario no puede verificarse a sí mismo | 🟡 Abierto — RPT-054 §3.1. **No** reutilizar el nombre `verificarPaquete`, que ya existe con otro significado |
| PA-112 | **Firmar el latido.** Protege contra quien reproduce desde la red; **no** contra quien compromete el sensor, que tiene la clave | 🟡 Abierto — RPT-057 §2 con su corrección de alcance. **Bloqueado por el aprovisionamiento** (PA-49, PA-51): no se firma sin clave |
| ~~PA-113~~ | ~~Dos agentes en la misma máquina compartían identidad y el latido de uno tapaba la muerte del otro~~ | ✅ **Cerrado por observación** 13-ago-2026 — RPT-059 §5. Identidad `(máquina, interfaz)` |
| ~~PA-114~~ | ~~El resumen del agente imprimía siete de diez condiciones~~ | ✅ Cerrado 13-ago-2026 — RPT-058 §4. `Condiciones::enumerar` es el único sitio con los diez nombres |
| ~~PA-115~~ | ~~El sello de RPT-038 no lleva interfaz~~ | ✅ **Cerrado por observación** 14-ago-2026 — RPT-061. Se construyó el testigo **con el defecto intacto**, se observó la acusación falsa en ejecución real, y después se corrigió |
| PA-44 | ~~Agente mínimo que recorra captura → veredicto~~ | ✅ Cerrado 5-ago-2026 — RPT-020. **Habilita PA-40** |
| ~~PA-45~~ | ~~Declaración de segmentos por identificador de VLAN~~ | ✅ Cerrado 6-ago-2026 — RPT-022. Fase 1: `FormatoObsoleto`; Fase 2: bloque firmado de VLAN. **Abre PA-48** |
| ~~PA-48~~ | ~~Emisor de manifiestos~~ | ✅ Cerrado 6-ago-2026 — RPT-025 y RPT-026. **Abre PA-53 y PA-54** |
| PA-53 | Lectura de la frase de paso sin eco en pantalla | 🟡 Abierto — RPT-026 §5 |
| ~~PA-54~~ | ~~Generación y custodia de la clave de recuperación~~ | ✅ Cerrado 6-ago-2026 — RPT-027. Reparto 2-de-3 verificable. **Abre PA-55** |
| PA-55 | Elevar el reparto a 3-de-5, o exigir dos custodios externos a la organización | 🟡 Abierto — RPT-027 §6 |
| PA-52 | Techo de secuencia en el camino en memoria, no sólo en el analizador de fichero | 🟡 Abierto — RPT-025 §2 |
| ~~PA-49~~ | ~~Aprovisionamiento de la clave de verificación en el agente~~ | ✅ Cerrado 6-ago-2026 — RPT-024. Dominio en el fichero, no en la ruta. **Abre PA-51** |
| PA-50 | Custodia y rotación de la clave del cliente | 🟡 Abierto — RPT-023 §7 |
| PA-51 | **Procedimiento e instrumentación del aprovisionamiento.** Hoy son dos ficheros copiados a mano sin comprobación | 🟡 Abierto — RPT-024 §7 |
| ~~PA-38~~ | ~~Almacén de observación partido: volátil con expulsión, pegajoso sin ella~~ | ✅ Cerrado 5-ago-2026 — RPT-018 §6. **La saturación bloquea en lugar de olvidar** |
| PA-39 | Privilegios de captura (`CAP_NET_RAW`) | 🟡 Abierto — RPT-018 §9 |
| PA-27 | Reversión del inventario firmado y revocación de la clave del administrador | 🔵 **Parcial** 5-ago-2026 — RPT-012. Secuencia firmada y centinela; falta el ancla y la revocación |
| PA-28 | Ancla de confianza para el centinela de frescura (TPM 2.0 o equivalente) | 🟡 Abierto — RPT-012 §7 |
| PA-25 | Distribución de la base OUI en Local-First | 🟡 Abierto — RPT-010 §9 |
| PA-26 | Limpieza auditada de la ambigüedad pegajosa | 🟡 Abierto — RPT-010 §9. **Requisito, no mejora**, desde RPT-018 §6 |

### 12.0 Reglas de alcance

Dos límites que no son tareas y por eso no llevan número, pero que se reabren **sólo de forma explícita**.

**`eje-agente` no corre en Windows.** `eje-captura` devuelve `PlataformaNoSoportada` fuera de Linux porque Npcap exige licencia OEM (RPT-003 §5.4). Si un cliente llegara a exigir agente nativo en Windows, eso **no es un objetivo de compilación**: reabre la compra de esa licencia, con su coste y su plazo. No se reabre de forma implícita bajo ninguna circunstancia.

**La captura es AF_PACKET simple.** Ni `PACKET_MMAP` con anillos ni eBPF. `RutaCaptura::Ebpf` es una variante del enum sin implementación detrás, y RPT-018 §7 dejó los anillos fuera de la primera entrega a propósito: si el diseño no aguanta con lectura simple, tampoco aguantará con anillos. La diferencia de rendimiento en red cargada es de un orden de magnitud, así que conviene medirla antes de prometer capacidad.

### 12.1 Triaje

El recuento vigente lo da `cargo xtask tablero`, que lo **lee** del tablero de arriba. Aquí no se transcribe: cuatro veces se resumió a mano y las cuatro reintrodujo puntos ya cerrados, que es precisamente por lo que existe el comando.

Lo que sí conviene decir es la forma del saldo: la mayoría de lo pendiente **no impide desplegar nada**, y mezclarlo todo en una tabla larga hace parecer incompleto un producto que no lo está.

> Este apartado llegó a llevar cifras escritas a mano. La primera decía 21 y el comando la corrigió el día que se escribió; la siguiente quedó desfasada al añadir PA-46 y PA-47 sin que nadie lo notara. Se han retirado: una cifra que envejece en silencio es peor que ninguna.

Tres categorías, y la distinción no es técnica sino de decisión.

**🔴 Bloquean el despliegue.** Sin esto el binario es un prototipo observable, no un producto.

> **6-ago-2026: ninguno de los cinco se resuelve escribiendo Rust.** Cuatro esperan a algo externo —una máquina Linux, una compra, un repositorio, un permiso de despliegue— y el quinto, PA-12, es construible pero su resultado no se puede verificar sin las plataformas de destino y depende de PA-14a y PA-46 para significar algo.
>
> Es un cambio de naturaleza, no de tamaño. Durante veinte reportes el cuello de botella fue lo que faltaba por escribir; ahora es lo que falta por comprar y por montar. Conviene decirlo porque la lista de 🟡 sigue siendo larga y puede leerse como si quedara mucha ingeniería: la que queda no bloquea.

| ID | Por qué bloquea |
|---|---|
| PA-40 | El agente nunca ha leído una trama real |
| PA-14a | **Certificado de firma de código.** No se puede entregar a un cliente un agente sin firmar que además le pide confiar en firmas |
| PA-46 | **Repositorio firmado para Linux.** `eje-captura` sólo funciona en Linux, así que el agente **no tiene otra plataforma** |
| PA-12 | Sin empaquetador no hay nada que instalar |
| PA-39 | Sin `CAP_NET_RAW` resuelto, el agente no captura en el sitio del cliente |
| PA-61 | Un sensor con **una sola tarjeta de red** no puede emitir alertas: la interfaz que vigila suele ser receive-only. Es condición de compra del hardware, y descubrirlo en planta es caro |

> Esta tabla decía «PA-14» a secas cuando RPT-021 ya lo había partido, y le faltaban PA-46 y PA-47 —añadidos por ese mismo reporte sin pasar por aquí—. Lo que destapó la deriva fue cuadrar el recuento del comando contra la suma de estas tres listas.
>
> Pero al mirar por qué no cuadraban apareció algo peor, y en el comando: `identificador_de` se detenía en los dígitos, así que **`PA-14`, `PA-14a` y `PA-14c` colapsaban en un solo identificador** y la deduplicación conservaba el primero — la fila cerrada del padre. El comando escondía un bloqueante y un punto abierto, que es exactamente el fallo que existe para impedir. Corregido en 6-ago-2026 con `un_punto_partido_no_se_come_a_sus_hijos` y una comprobación contra el tablero real.

**🟡 Deuda conocida, asumible con los ojos abiertos.** Empeoran el perfil de riesgo sin impedir que el producto funcione. Desplegar con estos abiertos es decisión de negocio, no fallo de ingeniería.

PA-08, PA-13, PA-14c, PA-15, PA-18, PA-19, PA-25, PA-26, PA-27, PA-28, PA-30, PA-36, PA-37, PA-41, PA-47, PA-50, PA-51, PA-52, PA-53, PA-55, PA-59, PA-60, PA-62, PA-63, PA-65, PA-70, PA-71.

> **6-ago-2026.** El servicio continuo se llamó PA-41 durante dos reportes. PA-41 ya existía desde RPT-019 §8 —el intervalo de consulta— y no es lo mismo: el demonio es el mecanismo, PA-41 es la cifra, y RPT-034 §5.4 dice que no está medida. El demonio pasa a PA-67. Los identificadores se asignan escribiendo prosa, y ahí `cargo xtask tablero` no llega.

De estos, **PA-26, PA-28 y PA-64 son los que se degradan con el uso o dejan una frontera abierta**: la mitad pegajosa crece sin límite, y tanto el centinela del inventario como el ancla de la evidencia siguen siendo rebobinables por quien controle el almacén.

De estos, **PA-26 y PA-28 son los que se degradan con el uso** —la mitad pegajosa crece sin límite y el centinela sigue siendo rebobinable—, así que su plazo no es indefinido aunque no bloqueen hoy.

**🔵 Dependen de terceros.** La ingeniería está hecha; la tasa de avance es cero hasta que alguien externo actúe. **No son secundarios**: PA-09 es el oráculo contra el que se prueban los adaptadores de contención, y sin él ningún adaptador pasa de espejo a verificación (RPT-008 §2).

| ID | Espera a |
|---|---|
| PA-09 | Descargar las imágenes virtuales de Cisco y Arista |
| PA-22 | Comprar equipo OT de segunda mano |

---

*Reporte Nº 2 — Arquitectura Consolidada · PremosCorp · 4 de agosto de 2026 · Estado: Canónico*

> **6-ago-2026 (II).** PA-68 no se cerró comprobando que el bucle seguía funcionando: se cerró encontrando un segundo defecto de la misma familia que el reloj congelado. `main.rs` consultaba las alertas a emitir con `desde_asiento: 0` **en cada vuelta**, de modo que en modo continuo el SIEM del cliente habría recibido el historial completo de alertas una vez por ciclo, indefinidamente. Correcto en un recorrido de una pasada; inservible en un demonio. Los dos defectos comparten causa: código escrito para ejecutarse una vez, ejecutándose muchas.

> **8-ago-2026.** PA-64 se cerró cambiándole el objetivo, no cumpliéndolo. Decía «sellar criptográficamente el ancla» para cerrar la manipulación local por quien tiene permisos de escritura, y una firma local no cierra eso: la clave privada tendría que vivir, disponible sin intervención humana, en el mismo disco que el atacante escribe. Además habría dado al agente una capacidad de firma que RPT-015 y RPT-024 le negaron a propósito. Lo que cierra el vector es que el extremo salga de la máquina. Queda dicho aquí porque un punto cerrado con el enunciado original habría dejado escrito que el hueco está tapado.

> **8-ago-2026 (II).** PA-72 salió de mirar PA-59, no de buscarlo. `ASIENTOS_MAXIMOS` sólo se comprobaba en `analizar` —al leer—, así que un agente que superara los 500 000 asientos escribía un fichero que él mismo no podía releer, y el arranque siguiente lo apartaba como evidencia de manipulación. Es la tercera vez en dos días que un defecto aparece **al mover código, no al revisarlo**: el reloj congelado de RPT-036 §3, la reemisión del historial de RPT-037 §3 y éste. Los tres eran correctos ejecutándose una vez.

> **8-ago-2026 (III).** PA-74 estuvo a punto de darse por cerrado con el tipo declarado y el contrato del puente devolviendo todavía un array. Las dos pruebas de paridad pasaban: comprueban que los campos coinciden entre manifiesto y código, no que alguien use el registro declarado. Es un hueco de la barrera de PA-20 y queda como PA-75. Van siete veces esta semana que aparece un mecanismo sin cablear; la única novedad es que esta vez se vio antes de escribir «cerrado».

> **8-ago-2026 (IV).** Al construir la barrera de PA-75 apareció que `contrato-ipc.toml` —la fuente de verdad— seguía declarando `forma = "lista<SucesoAlerta>"` para `consultar-alertas`. Se habían cambiado Rust, TypeScript, los campos de ambos lados y la firma del puente; no la declaración del canal. La prueba que existía comprobaba que el canal apareciera en el manifiesto, no que su forma correspondiera con algo. Van ocho piezas sin cablear esta semana, y ésta estaba en el documento que gobierna a las demás.

> **8-ago-2026 (V).** PA-76 es el primero de nueve en el que la barrera nueva **no encuentra nada**: el manejador ya servia lo que el manifiesto declara. Queda anotado con el mismo enfasis que los ocho hallazgos anteriores, porque un registro que solo destaca cuando caza algo mide el entusiasmo de quien escribe y no el estado del sistema.

> **9-ago-2026.** PA-69 tenía dos partes y el enunciado sólo nombraba una. La visible era que la pérdida no llegaba a VIS-04; la de fondo era que **la guarda de escritura sólo miraba si esa vuelta había anexado**, así que un fallo transitorio dejaba las alertas en memoria hasta la amenaza siguiente. Y queda un caso sin cerrar en local: si el proceso muere durante el fallo, la única prueba está en el colector, donde el sello dejó de avanzar.
