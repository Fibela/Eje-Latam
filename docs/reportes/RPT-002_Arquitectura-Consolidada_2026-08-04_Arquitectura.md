# Reporte de Construcción Nº 2 — Arquitectura Consolidada

| Campo | Valor |
|---|---|
| **Tema documentado** | Arquitectura Consolidada y Corrección del Corpus Técnico |
| **Número de reporte** | 002 |
| **Fecha** | 4 de agosto de 2026 |
| **Área designada** | Arquitectura |
| **Entidad / Firma** | PremosCorp |
| **Versión de arquitectura** | 2.0 (Soberanía Local — Opción 1A) |
| **Estado** | Canónico |

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
| `NUC-01` | Correlación Regional Multiinquilino | `eje-nucleo` · F2 | M5 |
| `NUC-02` | Simulador de Crisis Regional | `eje-nucleo` · F2 | M6 |
| `NUC-03` | Hub de Transición Criptográfica | `eje-nucleo` · F2 | M7 |
| `NUC-04` | Gestor de Claves y Licencias | `eje-nucleo` · F2 | **M8 (rescatado)** |

---

## 5. Especificación de Módulos — Fase 1

### AGT-01 · Guardián de Confianza Cero

**Modelo de despliegue — Sensor Adyacente.** El agente **no se instala** en PLC, cámaras ni bombas de infusión. Opera como sensor de red que recibe copia del tráfico mediante puerto SPAN, TAP pasivo, o desde el gateway del segmento. Esta distinción es contractual, no cosmética: define qué se le puede prometer al cliente y cómo se dimensiona el despliegue.

**Captura de tráfico.**

| Plataforma | Mecanismo | Requisito |
|---|---|---|
| Windows | Npcap OEM | Firma digital + atestación WHQL de Microsoft |
| Linux | `CAP_NET_RAW` + sockets eBPF | Capacidad concedida, sin driver propio |
| macOS | BPF (`/dev/bpf*`) | Perfil de permisos del sistema |

> **Riesgo de cronograma:** la certificación WHQL y la licencia Npcap OEM son procesos con plazos externos no controlables. Deben iniciarse en paralelo al desarrollo, no al final.

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

| Componente | Licencia |
|---|---|
| Núcleo de `eje-agente`, conectores de red básicos, motor de almacenamiento local, SDK de integración | **Apache 2.0** |
| Simulador de crisis avanzado, módulos de guerra directiva, correlación regional, suscripción a firmas de Threat Intel | **Propietaria PremosCorp** |

> **Punto abierto (§9.4):** la frontera aún no es ejecutable. `guardian-cc` y `motor-pqc` viven dentro de `eje-agente`, declarado abierto; si ambos quedan bajo Apache 2.0, el diferenciador técnico completo es forkeable. Requiere decisión antes del primer *commit* de código.

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
| 17 | Ausencia de driver de captura y privilegios | ✅ Especificado: Npcap/WHQL, CAP_NET_RAW, eBPF |
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

### 9.4 · Frontera open-core no ejecutable — DECISIÓN PENDIENTE

Ver §7. `guardian-cc` y `motor-pqc` residen dentro de `eje-agente`, declarado Apache 2.0. Sin una separación explícita de repositorios o módulos, el diferenciador técnico central queda forkeable por cualquier competidor. **Bloquea el primer commit de código.**

### 9.5 · Validación de licencias fuera de línea — DECISIÓN PENDIENTE

`NUC-04` (Gestor de Claves y Licencias) reside en Fase 2, en la nube. Pero la premisa central del producto es operar **indefinidamente sin conectividad**. Ambas cosas no son compatibles sin un diseño explícito: se requieren tokens de licencia firmados fuera de línea, con vigencia y período de gracia definidos, y una política declarada de qué ocurre al expirar en un hospital aislado. La respuesta correcta casi con certeza es **degradar funcionalidades no críticas y nunca desactivar la detección**, pero debe decidirse formalmente.

Además, "Zero-Knowledge" se usaba en el corpus anterior de forma imprecisa. Si no hay una prueba de conocimiento cero real, el término debe retirarse.

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

**Áreas designadas:** `Arquitectura` · `Agente` · `Red` · `Almacenamiento` · `Interfaz` · `Seguridad` · `Cumplimiento` · `Producto`

Todo reporte incluye tabla de metadatos, sección de trazabilidad indicando qué documentos reemplaza, y estado (`Borrador` / `En revisión` / `Canónico` / `Obsoleto`).

---

## 12. Puntos Abiertos

| ID | Punto | Bloquea |
|---|---|---|
| PA-01 | Frontera open-core: ¿`guardian-cc` y `motor-pqc` abiertos o propietarios? | Primer commit de código |
| PA-02 | Validación de licencias fuera de línea: vigencia, gracia, degradación | Diseño de `NUC-04` |
| PA-03 | Retirar o justificar el término "Zero-Knowledge" | Material comercial |
| PA-04 | Inicio de trámites Npcap OEM y atestación WHQL | Cronograma de `AGT-01` en Windows |
| PA-05 | Modelo de credenciales delegadas de switch (SNMP/NETCONF/802.1X) | Diseño de contención |
| PA-06 | Alojamiento del servidor STUN/DERP por defecto | Diseño de `RED-02` |

---

*Reporte Nº 2 — Arquitectura Consolidada · PremosCorp · 4 de agosto de 2026 · Estado: Canónico*
