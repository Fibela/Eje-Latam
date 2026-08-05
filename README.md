# Eje-Latam

**Plataforma Local-First de ciberseguridad, resiliencia poscuántica e inteligencia de amenazas para Latinoamérica.**

Entidad: PremosCorp · Arquitectura v2.0

---

## Qué es

Eje-Latam protege entornos corporativos, industriales (OT) y sanitarios partiendo
de una premisa: **soberanía del dato**. Cada instalación es un nodo autónomo con
capacidad plena de detección, cifrado poscuántico, persistencia y respuesta **sin
depender de conectividad a internet ni de infraestructura de PremosCorp**.

Tres diferenciadores frente a las suites globales:

1. **Dispositivos sin sistema operativo instalable** — PLC, cámaras, bombas de
   infusión. Cubiertos por inspección de red adyacente, no por agentes pesados.
2. **Transición poscuántica practicable** para empresas medianas, vía envoltorio
   híbrido local en lugar de reprogramar sistemas heredados.
3. **Inteligencia con foco latinoamericano** — troyanos bancarios regionales,
   fraude por mensajería, suplantación de plataformas de pago locales.

## Estructura

```
Eje-Latam/
├── crates/
│   ├── eje-agente/     Demonio local soberano (AGT-01 … AGT-07)
│   ├── guardian-cc/    Guardián de Confianza Cero — inspección pasiva L2/L3
│   ├── motor-pqc/      Motor Poscuántico — ML-KEM / ML-DSA + AES-256-GCM
│   ├── eje-almacen/    Registro de evidencia (Merkle) y sandbox del analista
│   ├── boveda/         Bóveda Aislada — cola cifrada ante apagón digital
│   └── eje-red/        Capa A (LAN) y Capa B (P2P / NAT)
├── apps/
│   └── eje-vision/     Interfaz TypeScript · React · Electron
│       ├── packages/eje-vision-base/          Apache-2.0 · VIS-01, VIS-03, VIS-04
│       ├── packages/eje-vision-empresarial/   Propietario · VIS-02, VIS-05, CON-SIM
│       └── proceso-principal/                 Apache-2.0 · puente IPC y carga firmada
├── xtask/              Herramientas de desarrollo (guardián de inconclusos)
└── docs/reportes/      Documentación canónica del proyecto
```

## Licenciamiento — Open-Core

Frontera **ratificada en firme** el 4 de agosto de 2026 (RPT-003 §2.7). El criterio
es *mecanismo vs. contenido*, no *núcleo vs. módulos*.

| Ámbito | Componentes | Licencia |
|---|---|---|
| **Mecanismo** | `eje-agente`, `guardian-cc`, `motor-pqc`, `eje-almacen` (incl. cadena Merkle), `boveda`, `eje-red`, SDK y esquemas | **Apache-2.0** |
| **Contenido y operación** | Suscripción de Inteligencia Regional, `VIS-02`, `VIS-05`, `SIM-01`, `NUC-*`, conectores de contención certificados | Propietaria PremosCorp |

**Por qué el motor criptográfico es abierto.** ML-KEM (FIPS 203) y ML-DSA
(FIPS 204) son estándares públicos del NIST: no hay propiedad intelectual que
proteger. Un motor criptográfico cuya seguridad dependa del secreto de su
implementación no es evaluable, y en una auditoría constituye hallazgo, no
diferenciador.

**Por qué el motor de inspección es abierto.** Un binario con autoridad para
aislar puertos de switch en una red hospitalaria debe ser auditable por quien lo
autoriza. El código de inspección es una mercancía a tres años; las firmas
regionales actualizadas cada semana no lo son.

Queda desestimada en firme toda propuesta de ocultar binarios del mecanismo
mediante crates cerradas o fronteras FFI/C: técnicamente inviable en Rust,
perjudicial para la seguridad de memoria y destructiva para la confianza del
cliente institucional.

## Compilar

```bash
cargo build --workspace
cargo test  --workspace
```

Requiere Rust 1.85 o superior. `rust-toolchain.toml` fija la versión.

## Verificaciones de calidad

Obligatorias en cada *push* y *pull request* (RPT-003 §9.4):

```bash
cargo fmt --all --check                      # Formato
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-features        # Pruebas
cargo deny check                             # Licencias, avisos, fuentes
cargo xtask verificar crates                 # todo!, mocks, endpoints inconclusos
gitleaks detect --config .gitleaks.toml      # Secretos en código e historia
```

`cargo deny check licenses` es el control que sostiene la frontera open-core:
impide que una dependencia copyleft contamine un crate Apache-2.0.

`cargo xtask` corre idéntico en Windows, Linux y CI, sin bash ni PowerShell. El
guardián vive en el crate `xtask` y **se prueba con `cargo test`** — un script
suelto no puede probarse, y dos guardianes de este proyecto ya pasaron en verde
con la violación presente antes de que una prueba negativa los delatara.

### Eje-Visión

```bash
cd apps/eje-vision
npm ci
npm run verificar    # tipos, frontera, prueba negativa de la frontera y pruebas
```

## Restricciones que no son negociables

Las siguientes derivan de análisis de riesgo documentado. No son preferencias de
estilo y no se relajan por conveniencia de implementación:

- **Prohibida la suplantación ARP para contención.** Es la técnica de un ataque de
  intermediario; en OT puede provocar un incidente de seguridad física. Solo
  SNMPv3 `authPriv`, NETCONF, 802.1X CoA o firewall local.
- **`SIM-01` carece de la capacidad de invocar contención.** No es que la invoque y
  sea rechazada: la operación no está en su superficie alcanzable. Un simulacro
  que aísle equipamiento médico real es un fallo con consecuencias de vida.
- **Ninguna condición comercial degrada una función de seguridad.** Una licencia
  vencida nunca desactiva detección ni contención.
- **En perfil OT, el descubrimiento es pasivo y la Capa B está deshabilitada** por
  defecto.
- **Sin puertos TCP locales** entre `eje-vision` y `eje-agente`. Solo IPC nativo.
- **Las credenciales de switch no residen en el registro de evidencia.** Rotan, y
  se exportarían junto con la evidencia en un proceso judicial.
- **La actualización automática está deshabilitada en modo OT, industrial y
  clínico.** Requiere aprobación por el flujo de gestión de cambios del cliente.

## Documentación

La documentación canónica vive en [`docs/reportes/`](docs/reportes/). Ante
discrepancia entre el código y un reporte canónico, **prevalece el reporte** hasta
que se emita una enmienda.

| Reporte | Tema |
|---|---|
| [RPT-002](docs/reportes/RPT-002_Arquitectura-Consolidada_2026-08-04_Arquitectura.md) | Arquitectura consolidada v2.0 |
| [RPT-003](docs/reportes/RPT-003_Gobernanza-y-Cierre-de-Puntos-Abiertos_2026-08-04_Gobernanza.md) | Gobernanza, licenciamiento y política de calidad |
| [RPT-004](docs/reportes/RPT-004_Especificacion-de-Eje-Vision_2026-08-04_Interfaz.md) | Especificación de Eje-Visión y frontera de licencia en la interfaz |
| [RPT-005](docs/reportes/RPT-005_Seleccion-de-Bibliotecas-Poscuanticas_2026-08-04_Seguridad.md) | Selección de bibliotecas ML-KEM y ML-DSA para `motor-pqc` |
| [RPT-006](docs/reportes/RPT-006_Contrato-IPC-y-Verificadores-Triestaticos_2026-08-05_Interfaz.md) | Contrato IPC y principio de verificación triestática |

**Regla de mantenimiento:** todo cambio funcional actualiza la documentación en el
mismo commit. Un *pull request* que modifique comportamiento sin tocar `docs/` se
rechaza.

## Licencia

Apache-2.0 para los componentes del mecanismo. Ver [LICENSE](LICENSE).
