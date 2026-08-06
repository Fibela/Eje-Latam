# RPT-018 — Captura Pasiva y Almacén de Observación (Diseño)

**Tema:** Del razonamiento sobre ficheros al razonamiento sobre una red
**Nº de reporte:** 018
**Fecha:** 5 de agosto de 2026
**Área designada:** Red
**Entidad:** PremosCorp
**Estado:** **Diseño — sin implementar.** Requiere ratificación

- **Depende de:** RPT-003 §5.3 (rutas de captura), RPT-010 (contratos de proveedor), RPT-009 (clasificación)
- **Abre:** PA-37, PA-38, PA-39

---

## 1. Lo que hay que probar, no lo que hay que construir

Diez reportes han verificado una máquina de decisión que nunca ha visto un dispositivo. Sus entradas son traits con dobles de prueba escritos por quien escribió los traits.

El objetivo de esta fase no es «tener captura». Es **someter las interfaces a datos que nadie inventó**. Si `ProveedorHuella::indicio(&mac)` resulta insostenible, es mejor saberlo ahora que después de construirle encima.

## 2. Decisión 1 — la captura va en un crate aparte

`guardian-cc` declara `#![forbid(unsafe_code)]`. AF_PACKET con `PACKET_MMAP` —la ruta de referencia de RPT-003 §5.3— exige `mmap`, anillos y FFI: `unsafe` inevitable.

Hay dos salidas y sólo una es aceptable:

- Relajar `forbid(unsafe_code)` en `guardian-cc`. **No.** El motor de decisión es lo que un auditor lee primero, y su garantía de seguridad de memoria no debe depender de revisar un módulo de sockets.
- **Crate nuevo, `eje-captura`**, con el `unsafe` confinado y auditado por separado.

La frontera no es de estilo: son dos posturas de auditoría distintas. `guardian-cc` se audita leyendo lógica; `eje-captura` se audita leyendo llamadas al núcleo.

`eje-captura` no depende de `guardian-cc`. Emite observaciones; no clasifica.

## 3. Decisión 2 — pasivo por tipo, no por disciplina

RPT-002 §9.2 prohíbe emitir tráfico en perfil OT. Hoy eso es una regla que alguien debe recordar.

En su lugar: **el tipo que representa la captura no expone forma alguna de transmitir.** No «no llamamos a `send`», sino que no hay `send` que llamar. Misma disciplina que `MarcadoVerificado`, que no se puede construir sin verificar.

Un socket que puede transmitir y no transmite depende de que nadie escriba la línea. Uno que no puede, no.

## 4. Decisión 3 — el descarte tiene que ser visible

Una captura sobre red cargada descarta paquetes. Es normal y no es el problema.

El problema sería descartarlos **en silencio**: el clasificador vería menos protocolos, inferiría menos criticidad, y la ausencia de indicio se leería como ausencia de riesgo. Un guardián que informa verde porque no miró — RPT-006 §4, otra vez.

Luego el almacén de observación debe poder decir **«puedo haber perdido cosas»**, y esa condición debe llegar a la clasificación como `Indicio::Indeterminado` y no como `SinIndicio`. La distinción existe desde RPT-010 §2; aquí adquiere su segundo uso.

## 5. La pregunta que motiva todo esto: ¿aguanta `ProveedorHuella`?

`indicio(&mac) -> Indicio` es una consulta puntual, síncrona y sin coste. La huella real no es nada de eso: se acumula con el tiempo, mejora con más paquetes y no está disponible al primer paquete.

**La interfaz aguanta, pero sólo si algo mantiene estado detrás.**

```text
eje-captura  ──paquetes──►  AlmacenObservacion  ◄──consulta──  ProveedorHuella
                             (tabla por MAC)
```

El trait no cambia. Lo que cambia es que deja de ser una función pura y pasa a ser una lectura de un almacén vivo. Eso tiene tres consecuencias que conviene aceptar por escrito:

1. **Un dispositivo no observado todavía no es un dispositivo sin indicios.** Es `Indeterminado`. Y por RPT-010 §6 una fuente inferida indeterminada no bloquea, así que un equipo recién visto en un segmento declarado limpio sigue siendo contenible — que es coherente: la autoridad ahí es la declaración de segmento, no la huella.
2. **La respuesta a la misma pregunta cambia con el tiempo.** Ninguna prueba actual lo contempla.
3. **El almacén es estado compartido entre el hilo de captura y el de decisión.**

## 6. Hallazgo: la expulsión de la tabla borra la ambigüedad pegajosa

Una tabla por MAC sin límite es agotamiento de memoria a petición: basta emitir tramas con direcciones inventadas. Luego hace falta expulsión.

Pero `HistorialSegmento::visto_en_segmento_critico` —la ambigüedad pegajosa de RPT-010 §5— vive precisamente en ese tipo de estado por dispositivo. **Expulsar una entrada olvida que el equipo pasó por la VLAN clínica**, y el carro de telemedicina vuelve a ser contenible por el camino que RPT-010 cerró.

Es un blanqueo: llenar la tabla hasta que el dispositivo interesante sea expulsado.

La corrección va en la dirección conocida: **lo pegajoso no se expulsa**. La tabla se parte en dos —observación volátil, que caduca; y hechos pegajosos, que no— y sólo la primera tiene política de expulsión. La segunda crece mucho más despacio, porque sólo anota dispositivos que estuvieron en un segmento crítico.

Esto convierte PA-26 —la limpieza auditada de la pegajosidad— de mejora en requisito: si nada la limpia y nada la expulsa, crece para siempre.

## 7. Alcance de la primera entrega

Deliberadamente estrecho:

- Captura AF_PACKET en Linux, **sólo lectura**, con contador de descartes.
- Extracción de dirección de enlace y de un puñado de indicadores baratos de cabecera.
- Almacén de observación con las dos mitades del §6.
- `ProveedorHuella` real leyendo del almacén.

**Fuera:** Windows —Npcap y su licencia OEM, RPT-003 §5.4—, macOS, `PACKET_MMAP` con anillos, y todo análisis L7. La primera versión puede usar el AF_PACKET simple: si el diseño no aguanta ahí, tampoco aguantará con anillos.

## 8. Lo que este diseño no resuelve

1. **Ninguna huella concreta está decidida.** Qué protocolos delatan un equipo clínico o industrial es una cuestión de dominio, no de arquitectura, y merece su propio trabajo.
2. **Los privilegios.** AF_PACKET exige `CAP_NET_RAW`. Cómo se concede, si el proceso los suelta después, y qué hace el agente sin ellos, no está decidido.
3. **La correlación entre captura y segmento.** El almacén sabrá qué MAC habla qué; quién le dice en qué VLAN está es `ProveedorSegmento`, que tampoco tiene implementación.

El punto 3 es el que puede obligar a rediseñar: si la VLAN sólo se conoce por la captura misma, `ProveedorSegmento` y `ProveedorHuella` dejan de ser independientes y comparten almacén.

## 9. Estado tras la primera entrega

El crate `eje-captura` existe, entra en el workspace y aporta **8 pruebas**; el total pasa a **228**.

### 9.1 Lo que está verificado, y no es la captura

| Verificado | Sin verificar |
|---|---|
| Los guardianes de frontera y de pasividad | **Todo `linux.rs`** |
| Los ayudantes de `Trama` ante tramas cortas | La apertura del socket |
| La visibilidad de la pérdida | La lectura de tramas |
| El retorno de `PlataformaNoSoportada` | Los contadores del núcleo |

`linux.rs` vive tras `#[cfg(target_os = "linux")]` y la verificación corrió en Windows. **Ni `clippy` ni `cargo test` lo han compilado nunca.** Las suposiciones sobre `libc::tpacket_stats`, `SOL_PACKET`, `PACKET_STATISTICS` y los campos de `sockaddr_ll` siguen siendo suposiciones.

Decirlo importa porque «228 en verde» invita a leer que la captura funciona, y lo que funciona es la mitad portable del crate.

### 9.2 Los guardianes leen texto, no árbol sintáctico

`solo_un_modulo_admite_unsafe` busca una cadena en los ficheros de `src`; `no_existe_ninguna_via_de_transmision`, otras cuatro. No analizan el código.

Consecuencias:

- Un `unsafe` en un submódulo dentro de `linux.rs` no se vería como frontera nueva: el fichero ya tiene el permiso.
- Una transmisión construida por alias o a través de otro crate pasaría desapercibida.

Siguen valiendo la pena porque cubren el caso realista —alguien añade un fichero con permiso propio, o llama a `sendto` directamente—, pero son **barreras contra el descuido, no contra alguien decidido**. La garantía dura es el lint del workspace: `unsafe_code = "warn"` más `-D warnings` impide que ningún `unsafe` compile sin permiso explícito. Los guardianes sólo acotan dónde puede vivir ese permiso.

### 9.3 El guardián falló en su primera ejecución, y estuvo bien

Ambos guardianes se detectaron a sí mismos: su literal de búsqueda estaba en el propio fichero. La salida cómoda era excluir `pruebas.rs` del escaneo; se descartó porque dejaría un punto ciego permanente donde nadie vería un permiso de `unsafe` puesto ahí «por ser sólo pruebas». Se partieron las agujas con `concat!` y se añadió `el_escaner_no_se_encuentra_a_si_mismo` para que nadie las vuelva a juntar.

El fallo espontáneo hizo de prueba negativa: demostró que el escáner lee ficheros reales y que la aserción dispara. Un verde a la primera no habría distinguido «funciona» de «no mira».

## 10. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-37** | **Crate `eje-captura` con AF_PACKET de sólo lectura.** Frontera de `unsafe`, tipo sin transmisión, contador de descartes | Todo lo demás de esta fase |
| **PA-38** | **Almacén de observación partido.** Volátil con expulsión acotada, pegajoso sin expulsión | `ProveedorHuella` real |
| **PA-39** | **Privilegios de captura.** `CAP_NET_RAW`, si se sueltan, y qué hace el agente sin ellos | Despliegue |
| **PA-40** | **Compilar y ejecutar `linux.rs`.** `cargo check --target x86_64-unknown-linux-gnu` valida los tipos sin enlazador; ejecutar contra una interfaz real es lo que queda después | Cerrar PA-37 |

---

*Reporte Nº 18 — Captura Pasiva y Almacén de Observación (Diseño) · PremosCorp · 5 de agosto de 2026*
