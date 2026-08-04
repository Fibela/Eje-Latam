# Reporte de Construcción Nº 4 — Especificación de Eje-Visión

| Campo | Valor |
|---|---|
| **Tema documentado** | Arquitectura y frontera de licencia de la interfaz multiplataforma |
| **Número de reporte** | 004 |
| **Fecha** | 4 de agosto de 2026 |
| **Área designada** | Interfaz |
| **Entidad / Firma** | PremosCorp |
| **Estado** | Canónico |

## Trazabilidad

- **Depende de:** RPT-002 §4 (numeración de módulos), RPT-003 §2.7 (frontera ratificada), RPT-003 §3.4 (política de licencia vencida)
- **Origen del insumo:** propuesta de arquitectura modular remitida por equipo externo, agosto 2026
- **Abre:** PA-11 a PA-14

---

## 1. Dictamen sobre la Propuesta Recibida

| Elemento | Dictamen |
|---|---|
| Separación en paquetes en lugar de `if (isEnterprise)` | 🟢 **Aceptada** — §3 |
| Carga dinámica para que el código comercial no exista en la instalación base | 🟢 **Aceptada con requisito de firma** — §5 |
| Renderer pasivo tras el context bridge | 🟢 **Aceptada, especificada** — §6 |
| Desacoplamiento de equipos y CI por paquete | 🟢 Aceptada |
| Asignación de códigos `VIS-01`, `VIS-04`, `VIS-05` | 🔴 **Incorrecta** — §2 |
| `SIM-01` como vista de la interfaz | 🔴 **Incorrecta y peligrosa** — §2.2 |
| "Riesgo de ingeniería inversa: Nulo" | 🔴 **Falsa** — §4 |
| Frontera sostenida por `package.json` | 🟡 **Insuficiente sin verificación automática** — §7 |
| Nombres de paquete en inglés (`core`, `enterprise`) | 🟡 Contradicen RPT-002 §2 — §3.1 |

---

## 2. Corrección de Numeración

### 2.1 Códigos mal asignados

RPT-002 §4 sustituyó la numeración `M1`–`M12` precisamente porque los diagramas y las descripciones se habían desincronizado. La propuesta reintroduce el mismo defecto.

| Propuesta recibida | Correcto (RPT-002 §4) |
|---|---|
| `VIS-01` Vista Operativa / Estado de Agente | **`VIS-01` Consola Eje-Almacén** — cliente SQL, visor de esquemas, importación y exportación |
| `VIS-04` Visor de Eventos Forenses (ALM-01) | **`VIS-04` Panel de Confianza Cero e Inventario Vivo** |
| `VIS-05` Simulador de Crisis e Impacto Financiero | **`VIS-05` Mapa de Calor Regional** |

`VIS-02` y `VIS-03` estaban correctos.

### 2.2 `SIM-01` no es una vista

`SIM-01` (Simulador de Crisis) reside en **`eje-agente`, en Rust** (RPT-002 §4). No es un componente de interfaz.

La distinción no es formal. RPT-003 §8.1 exige que `SIM-01` y la ruta de contención residan en **dominios de capacidad separados**: el simulador no posee la capacidad de invocar contención, no es que la invoque y sea rechazada. Si el simulador se reimplementa dentro de la interfaz, esa garantía arquitectónica desaparece y queda reducida a una comprobación de UI — exactamente el modo de fallo que puede desconectar equipamiento médico durante un simulacro.

**Resolución:** el paquete comercial contiene la **Consola de Simulación** (`CON-SIM`), una vista que ordena y observa simulacros ejecutados por `SIM-01` en el agente. El motor permanece en Rust.

---

## 3. Estructura de Paquetes

```
apps/eje-vision/
├── package.json                     # Raíz del workspace pnpm
├── pnpm-workspace.yaml
├── .dependency-cruiser.cjs          # Frontera verificada (§7)
├── packages/
│   ├── eje-vision-base/             # ── Apache-2.0 ──────────────
│   │   ├── LICENSE                  #    Apache-2.0
│   │   ├── package.json
│   │   └── src/
│   │       ├── componentes/         #    UI común: botones, diseño, tablas
│   │       ├── ipc/                 #    Cliente IPC tipado
│   │       └── vistas/
│   │           ├── vis-01-consola-almacen/
│   │           ├── vis-03-lanzador/
│   │           └── vis-04-panel-confianza-cero/
│   │
│   └── eje-vision-empresarial/      # ── Propietaria PremosCorp ──
│       ├── LICENSE                  #    Propietaria
│       ├── package.json
│       └── src/vistas/
│           ├── vis-02-tablero-directivo/
│           ├── vis-05-mapa-calor-regional/
│           └── con-sim-consola-simulacion/
│
└── proceso-principal/               # Proceso principal de Electron
    ├── indice.ts
    ├── puente-ipc.ts                #  Enlace con eje-agente
    └── cargador-firmado.ts          #  Verificación de firma antes del import (§5)
```

### 3.1 Nombres

La propuesta usa `eje-vision-core` y `eje-vision-enterprise`. RPT-002 §2 establece nomenclatura en español; se corrigen a **`eje-vision-base`** y **`eje-vision-empresarial`**.

Se evita deliberadamente `eje-vision-nucleo`: `eje-nucleo` ya designa el backend Go de Fase 2, y la colisión de nombres produciría exactamente la confusión que RPT-002 §2.1 retiró al eliminar `origin/Origen` y el doble branding `PREMOS-*`.

### 3.2 Directorios de vista

La propuesta nombra los directorios `VIS-01/`, `VIS-02/`. Un directorio llamado `VIS-01` no le dice nada a quien entra al repositorio por primera vez. Se adopta el formato **`vis-01-consola-almacen/`**: conserva la trazabilidad con los reportes y añade legibilidad.

---

## 4. Límite Real de la Protección del Código

La propuesta afirma, en su tabla comparativa, que el riesgo de ingeniería inversa es **"Nulo"**. Es cierto únicamente para la instalación base.

**En cuanto un cliente de pago instala el paquete empresarial, ese código es JavaScript dentro de una aplicación Electron.** El formato `asar` no es cifrado: es un archivo contenedor. `npx asar extract` recupera el árbol completo. Cualquier cliente, competidor con una licencia, o analista con acceso a la máquina puede leer el código comercial íntegro.

### Consecuencia operativa

**Ningún secreto puede residir en el frontend empresarial.** No algoritmos de valoración de impacto que se consideren propietarios, no umbrales de detección, no credenciales, no lógica de negocio que se pretenda confidencial.

Esto es coherente con la frontera ya ratificada, no una excepción a ella: RPT-003 §2.5 establece que el activo protegido es **el contenido y la operación**, no el código. Los paquetes separados existen para **claridad de licencia y auditabilidad de la compilación abierta**, no para secreto.

Debe consignarse así en la documentación del equipo. Si se deja escrito que el riesgo es "nulo", alguien colocará ahí lógica valiosa bajo una protección que no existe.

---

## 5. Carga de Módulos: Firma Obligatoria

### 5.1 Superficie de inyección no contemplada

La carga dinámica desde disco introduce un vector que la propuesta no menciona: **quien pueda escribir en el directorio de módulos obtiene ejecución de código dentro del proceso principal de Electron**, que es el mismo que se comunica con `eje-agente` y puede solicitar órdenes de contención sobre la red del cliente.

Los *fuses* de integridad de asar protegen el archivo principal empaquetado; **no cubren módulos externos cargados en tiempo de ejecución**.

### 5.2 Requisitos

| Requisito | Detalle |
|---|---|
| Firma del paquete | Ed25519, misma cadena de confianza que los tokens de licencia (RPT-003 §3) |
| Momento de verificación | En `cargador-firmado.ts`, **antes** de resolver el `import` dinámico |
| Ubicación | Directorio no escribible por el usuario sin elevación |
| Comportamiento ante fallo | **Fallo cerrado**: firma ausente, ilegible o inválida ⇒ no se carga, se registra en `ALM-01` y se notifica en `VIS-04` |
| Alcance | Se verifica el paquete completo, no ficheros individuales |

El criterio de fallo cerrado es el mismo que rige `SIMULATION_ONLY` en RPT-003 §8.1: ante marca ausente o inválida, no se actúa.

---

## 6. Modelo de Seguridad de Electron

La propuesta describe correctamente el flujo. Se especifica para que sea verificable:

```
[ Vista React — proceso de renderizado ]
        │  API tipada y cerrada, expuesta por preload
        ▼
[ Proceso principal de Electron ]
        │  IPC nativo del sistema operativo
        ▼
[ eje-agente — Rust ]
```

### 6.1 Configuración obligatoria de `BrowserWindow`

| Opción | Valor | Motivo |
|---|---|---|
| `contextIsolation` | `true` | Aísla el contexto del preload del de la página |
| `nodeIntegration` | `false` | El renderer no accede a Node |
| `sandbox` | `true` | El renderer corre en sandbox del sistema operativo |
| `webSecurity` | `true` | No se desactiva bajo ninguna circunstancia |
| Contenido remoto | Prohibido | Toda la interfaz se sirve desde disco local |
| CSP | Estricta, sin `unsafe-inline` ni `unsafe-eval` | |

### 6.2 Superficie del puente

El preload expone una **API tipada y cerrada** — un método por operación permitida. Queda **prohibido** exponer un pasamanos genérico del tipo `invoke(canal, argumentos)`: eso traslada la decisión de autorización al renderer, que es justo la capa que no debe tenerla.

`shell.openExternal` solo con lista de permitidos. Un enlace recibido por telemetría o mostrado en un evento no se abre sin validación de destino.

### 6.3 Transporte hacia el agente

Socket de dominio Unix con ACL en Linux y macOS; named pipe con descriptor de seguridad en Windows. **Sin puerto TCP local** (RPT-002 §9.3): un servicio en `localhost` es alcanzable por cualquier proceso local y por cualquier página que el usuario visite.

---

## 7. Cumplimiento Automatizado de la Frontera

La propuesta sostiene la frontera con "límites físicos de archivos y dependencias de `package.json`". Eso no impide que alguien añada `"eje-vision-empresarial": "*"` a las dependencias del paquete base. Una frontera que depende de la disciplina se erosiona.

Se requiere el equivalente npm de lo que `cargo deny` hace en Rust:

| Verificación | Herramienta | Impide |
|---|---|---|
| Base no importa de empresarial | **dependency-cruiser** con regla de dirección, en CI | Contaminación de código propietario hacia el lado abierto |
| Base no depende de copyleft | Verificador de licencias npm (`license-checker` o equivalente) | Que una dependencia GPL/AGPL contamine el paquete Apache-2.0 |
| Cabecera de licencia por fichero | Verificación en CI | Ficheros sin licencia declarada |
| Árbol de dependencias del instalador base | Verificación en CI | Que el artefacto abierto incluya algo del paquete empresarial |

**El verificador de licencias npm no figuraba en la propuesta.** El ecosistema npm contiene paquetes GPL y AGPL, y la contaminación en esa dirección es al menos tan probable como la que la propuesta sí contempla.

---

## 8. Conflicto con la Política de Licencia Vencida

### 8.1 El conflicto

Si la carga dinámica del módulo empresarial está condicionada al estado de la licencia, **una licencia vencida impediría cargar `VIS-02`**.

Eso viola directamente RPT-003 §3.4, que exige que durante un **incidente activo** el Tablero Directivo siga mostrando el estado operativo en vivo aunque la licencia esté vencida. Dejar a un comité de crisis hospitalario sin tablero por una fecha de facturación es un fallo de producto con consecuencias reales.

### 8.2 Resolución

**La licencia no controla si el módulo carga. Controla qué hace el módulo una vez cargado.**

| Estado | Carga del módulo | Comportamiento |
|---|---|---|
| Vigente | Sí | Completo |
| Vencida — sin incidente | **Sí** | Visualización en vivo activa. Se deshabilita exportación de reportes, comparativas históricas y `CON-SIM`. Aviso discreto persistente. |
| Vencida — incidente activo | **Sí** | **Completo, sin restricción.** Aviso en segundo plano. Uso en gracia registrado en `ALM-01`. |
| Nunca licenciado | No | El paquete no está instalado |

La condición de carga es **haber sido licenciado alguna vez**, no estarlo ahora.

---

## 9. Requisitos por Vista

| Código | Vista | Paquete | Requisitos derivados de reportes previos |
|---|---|---|---|
| `VIS-01` | Consola Eje-Almacén | base | Opera **solo contra ALM-02**. La consola no puede emitir DDL contra ALM-01 (RPT-002 §5) |
| `VIS-02` | Tablero Directivo | empresarial | Impacto operativo, financiero y reputacional. Acciones estratégicas. Política de licencia §8.2 |
| `VIS-03` | Lanzador GUI | base | Términos y licencia, modo de esquema, modo de red. **Opción de servidor STUN/DERP propio prominente y sin coste** (RPT-003 §7) |
| `VIS-04` | Panel de Confianza Cero e Inventario Vivo | base | Inventario IoT/OT, postura por nodo, **alerta obligatoria de Bóveda al límite de capacidad** (RPT-002 §5, AGT-04), alerta de firma inválida (§5.2) |
| `VIS-05` | Mapa de Calor Regional | empresarial | **Sin comparativa sectorial en Fase 1**: requiere agregación multiinquilino (`NUC-01`). Debe comunicarse así (RPT-002 §9.6) |
| `CON-SIM` | Consola de Simulación | empresarial | Ordena y observa simulacros de `SIM-01`. **No ejecuta el motor** (§2.2) |

La alerta de Bóveda en `VIS-04` no figuraba en la propuesta y es un requisito ya establecido: un disco lleno en un nodo hospitalario es una interrupción, no un detalle.

---

## 10. Puntos Abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-11** | Mecanismo único del guardián de inconclusos. El `.ps1` propuesto tiene falso negativo silencioso; ver RPT-003 §9.5 | Adopción del guardián en local |
| **PA-12** | Gestor de paquetes y empaquetador: pnpm + Vite, o alternativa | Primer commit de `eje-vision` |
| **PA-13** | Biblioteca de componentes de la capa base, y su licencia | Diseño de `componentes/` |
| **PA-14** | Cadena de firma del paquete empresarial: ¿reutiliza la de licencias de RPT-003 §3 o es independiente? | Diseño de `cargador-firmado.ts` |

---

*Reporte Nº 4 — Especificación de Eje-Visión · PremosCorp · 4 de agosto de 2026 · Estado: Canónico*
