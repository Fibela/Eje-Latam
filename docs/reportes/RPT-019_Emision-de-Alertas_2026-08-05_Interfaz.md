# RPT-019 — Emisión de Alertas (Diseño)

**Tema:** Que lo que el agente sabe llegue a alguien
**Nº de reporte:** 019
**Fecha:** 5 de agosto de 2026
**Área designada:** Interfaz
**Entidad:** PremosCorp
**Estado:** Canónico — **contrato implementado, manejadores pendientes**

- **Depende de:** RPT-006/007 (contrato IPC), RPT-017 (arranque), RPT-018 (observación), `eje-almacen` (ALM-01)
- **Abre:** PA-41, PA-42

---

## 1. El punto ciego

Tres mecanismos saben cuándo hay que gritar y ninguno tiene a quién:

| Centinela | Dónde | Consumidor |
|---|---|---|
| `Veredicto::es_amenaza_incontenible()` | RPT-010 §6.1 | ninguno |
| `EstadoArranque::exige_alerta()` | RPT-017 | ninguno |
| `AlmacenObservacion::pegajoso_saturado()` | RPT-018 §6 | ninguno |

Un producto que detecta correctamente y no se lo dice a nadie tiene el mismo efecto operativo que uno que no detecta. Este es hoy el mayor hueco del proyecto, por encima de PA-36 y PA-40.

## 2. Decisión 1 — son dos formas, no una

Conflarlas produce o un registro inundado por una condición persistente, o una condición que sólo se ve como un evento rancio.

**Sucesos.** Ocurren una vez y quedan. «Se detectó una amenaza sobre un equipo incontenible.» Van a un registro que crece.

**Condiciones.** Son verdaderas hasta que dejan de serlo. «El almacén pegajoso está saturado», «el inventario fue suprimido». No se anotan una y otra vez; se consultan.

De los tres centinelas, el primero es suceso y los otros dos son condiciones. Que dos de tres sean condiciones no es casualidad: los estados degradados de este producto persisten hasta que alguien interviene.

## 3. Decisión 2 — los sucesos van a ALM-01, no a memoria

Una alerta que sólo existe en RAM muere con el proceso. Es la misma clase de defecto que PA-34, y en un agente de planta que pasa trimestres sin reiniciarse, la memoria es donde la evidencia se pierde en el reinicio que sigue al incidente.

`eje-almacen` ya tiene lo que hace falta: registro por anexado, encadenado por resumen, con prueba de inclusión. Una alerta es exactamente un asiento de evidencia.

No hay que construir nada nuevo. Hay que usar lo que existe.

## 4. Decisión 3 — **no** se añade empuje al contrato IPC

Los cuatro canales de `contrato-ipc.toml` son consultas del renderer al agente. Añadir un canal de empuje sería la respuesta obvia y es la equivocada:

- Amplía la superficie del proceso privilegiado en la dirección que RPT-004 §6.2 restringe. Un canal de empuje implica que el agente inicia comunicación hacia el renderer, y eso es una capacidad nueva, no un mensaje nuevo.
- Obliga a resolver entrega, orden y reintento sobre un transporte que hoy es petición-respuesta.
- Y no compra lo que parece: si el renderer no está abierto, la alerta se pierde igual. La durabilidad la da §3, no el empuje.

En su lugar, **dos canales de consulta nuevos**:

```toml
[[canal]]
nombre = "consultar-alertas"      # sucesos desde un punto del registro
[[canal]]
nombre = "obtener-condiciones"    # estado degradado actual
```

VIS-04 pregunta. El agente responde. Mismo modelo que los cuatro que ya existen.

### 4.1 El coste, dicho claro

Consultar implica latencia. Entre que se detecta una amenaza sobre un equipo incontenible y que el operador la ve pasa, como mucho, el intervalo de consulta.

Para la condición más urgente que este producto puede comunicar, eso es una decisión que hay que tomar con los ojos abiertos y no heredarla del transporte. Un intervalo de segundos es defendible; uno de minutos, no. Queda como PA-41 porque depende de lo que VIS-04 pueda sostener.

Lo que **no** cambia con la latencia: la alerta ya está en ALM-01 desde el instante en que se detectó. La consulta afecta a cuándo se ve, no a si se conserva.

## 5. Decisión 4 — el agente no decide qué es urgente

Los tres centinelas ya devuelven booleanos con significado fijado en sus reportes. La emisión no vuelve a juzgar: traduce.

Añadir una política de severidad aquí duplicaría la decisión y crearía el escenario en el que `es_amenaza_incontenible()` devuelve cierto y el emisor lo clasifica como informativo. Una sola autoridad por decisión.

## 6. Lo que este diseño no resuelve

1. **Nada sale hacia fuera del equipo.** Syslog, SIEM, correo: no está contemplado. Un agente aislado en planta que grita hacia una interfaz que nadie abre sigue sin llegar a nadie. Es PA-42 y es más operativo que técnico.
2. **La condición «suprimido» no se limpia sola.** Si alguien restaura el inventario, ¿la condición desaparece al siguiente arranque, o exige reconocimiento explícito? Lo segundo es más seguro y más molesto.
3. **El volumen.** Un segmento comprometido puede generar amenazas incontenibles a ritmo alto, y ALM-01 tiene cuota (30 días, 5 GB por RPT-003). Agrupar por dispositivo es lo razonable, pero agrupar es perder detalle.

El punto 1 es el que más se parece a un hueco real: los otros dos son afinado.

## 7. Estado tras la implementación

El contrato queda declarado en los seis sitios que la fricción de RPT-006 exige, y las pruebas de paridad lo vigilan: **236 pruebas en Rust** —24 en `eje-ipc`— y **31 en TypeScript**.

### 7.1 Lo que existe y lo que no

| Existe | No existe |
|---|---|
| Los dos canales en el manifiesto | Manejadores que respondan |
| `SucesoAlerta`, `Condiciones`, `ClaseAlerta` en Rust y TS | Nada que anexe alertas a ALM-01 |
| Las pruebas de paridad sobre los tres registros nuevos | La conversión desde el asiento de evidencia |

Decir «alertas implementadas» sería falso. Lo implementado es **por dónde saldrán**.

### 7.2 El compilador nombró mejor que el diseño

`Alerta` colisionó: VIS-04 ya tenía un tipo con ese nombre —**algo que mostrar**, con severidad— y el nuevo era **el registro de un hecho**. No fue un problema de re-exportación: consolidar bloques sólo habría movido el conflicto.

Renombrado a `SucesoAlerta`, que carga en el nombre la distinción del §2 entre sucesos y condiciones. La colisión era el síntoma de que «alerta» significaba dos cosas en dos capas, y el compilador lo vio antes que el diseño.

### 7.3 El asiento no puede fabricarse

`SucesoAlerta` lleva `asiento: u64`, y ese número **no es un dato del suceso**: lo asigna ALM-01 al anexar. La conversión debe ir **desde el asiento de evidencia hacia el DTO, nunca al revés** — convertir en el otro sentido significaría que alguien puede construir un `SucesoAlerta` con un asiento inventado.

Es la misma disciplina que `MarcadoVerificado` y `CertificadoVerificado`: el tipo existe sólo si el hecho ocurrió. **Está acordado y no está escrito**, y hasta que lo esté, nada impide fabricar uno.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-41** | **Intervalo de consulta de alertas.** Cuánto puede tardar VIS-04 en enterarse de una amenaza incontenible | Utilidad operativa |
| **PA-42** | **Salida fuera del equipo.** Syslog o equivalente para que la alerta llegue donde ya mira el cliente | Despliegue en sitio sin operador delante |
| **PA-43** | **Manejadores de los dos canales.** Anexar sucesos a ALM-01, derivar las condiciones de los tres centinelas, y la conversión unidireccional del §7.3 | Que las alertas lleguen a alguien |

---

*Reporte Nº 19 — Emisión de Alertas (Diseño) · PremosCorp · 5 de agosto de 2026*
