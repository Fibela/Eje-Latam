# RPT-031 — Salida de alertas fuera del equipo (Diseño)

**Tema:** Que la alerta llegue donde el cliente ya mira
**Nº de reporte:** 031
**Fecha:** 6 de agosto de 2026
**Área designada:** Comunicación
**Entidad:** PremosCorp
**Estado:** **Diseño — sin implementar.** Requiere ratificación

- **Depende de:** RPT-002 §9.2 (pasividad en OT), RPT-028 (manejadores), RPT-030 (persistencia cableada)
- **Cubre:** PA-42

---

## 1. El agente no tiene por dónde emitir, y es a propósito

Antes de decidir el formato de la alerta hay que decidir algo anterior: **este producto está construido para no transmitir**.

`eje-captura` tiene una prueba que no comprueba comportamiento sino **ausencia de mecanismo**:

```rust
fn no_existe_ninguna_via_de_transmision() {
    // La pasividad de RPT-002 §9.2 es por tipo: no basta con no llamar a
    // `send`, tiene que no haber `send` que llamar.
}
```

Y `eje-red` bloquea la capa B en perfil OT salvo autorización deliberada.

PA-42 pide sacar la alerta del equipo. Eso **no viola** ninguna de las dos —la prohibición es sobre el segmento vigilado, no sobre toda la máquina— pero sí obliga a decir por dónde sale, y esa respuesta no es libre.

## 2. La alerta no puede salir por la interfaz que vigila

Es la restricción que decide el resto, y es física antes que de diseño.

El modelo de despliegue es **Sensor Adyacente** (RPT-002 §5): puerto SPAN, TAP pasivo o gateway del segmento. Un TAP pasivo es, en muchos casos, **eléctricamente receive-only** — no hay par de transmisión conectado. Un puerto SPAN de destino suele estar configurado para descartar cualquier trama que reciba.

Luego:

> **El agente necesita una segunda interfaz.** Una que vigila y otra por la que habla.

Eso no es una preferencia de arquitectura: es un requisito de despliegue que hay que decirle al cliente antes de que compre el hardware, y que hoy no está escrito en ningún sitio. Un sensor con una sola tarjeta de red **no puede cumplir PA-42**, y descubrirlo en planta sería caro.

### 2.1 Y la segunda interfaz tiene su propio problema

Si la interfaz de gestión está en la misma VLAN que el segmento vigilado, la alerta viaja por donde está el atacante. Si está en la red de gestión —que es lo correcto— entonces el agente tiene un pie en la red de gestión, y **eso lo convierte en un objetivo con mejor posición que antes**.

No hay salida limpia. Lo que se puede hacer es que el agente **no acepte nada por esa interfaz**: sólo emite. Un emisor puro no amplía la superficie de ataque hacia la gestión, sólo la de exfiltración desde ella, que ya existía por la propia captura.

## 3. Syslog, y por qué no algo mejor

La recomendación es **syslog RFC 5424 sobre TCP**, y las razones son las de RPT-021 §2 —no elegimos el verificador— aplicadas aquí: **no elegimos el receptor**.

| Opción | A favor | En contra |
|---|---|---|
| **Syslog TCP** | ya está en el SIEM del cliente; nada que instalar | texto plano salvo TLS; sin acuse de aplicación |
| Syslog UDP | trivial | **pierde en silencio**, que es inaceptable para esto |
| HTTP a un colector nuestro | control total | infraestructura nuestra en la red del cliente; Local-First lo prohíbe |
| Fichero que otro recoge | ninguna dependencia | ya lo tenemos; no es «salir del equipo» |

**UDP queda descartado por la misma razón que el tercer estado de RPT-006 §4**: un envío que se pierde y no lo dice es indistinguible de un envío que llegó. Sobre TCP hay al menos confirmación de transporte.

TLS es deseable y **no bloqueante**: en la red de gestión de un hospital, un syslog en claro es lo que ya circula, y exigir TLS antes de emitir nada convertiría lo perfecto en enemigo de lo existente. Se anota.

## 4. Lo que se emite es la condición, no el registro

Un error tentador sería volcar cada asiento nuevo. Se rechaza:

- **Los sucesos ya están en ALM-01**, que es el registro auditable. Duplicarlos al SIEM crea dos fuentes de verdad con dos políticas de retención.
- **Lo que el operador necesita en tiempo real es que algo pasó**, no la evidencia completa. La evidencia se consulta después, por el canal que RPT-028 abrió.

Así que sale una línea por **amenaza incontenible** y una línea por **cambio de condición**, con el número de asiento como referencia cruzada. El volumen es el de las alertas: raro y grave.

### 4.1 Cambio de condición, no condición

Las condiciones son verdaderas hasta que alguien interviene (RPT-019 §2). Emitirlas en cada ciclo inundaría el SIEM con la misma noticia — el defecto exacto que aquel reporte evitó al no anexarlas.

Se emite **la transición**: cuando una condición pasa de falsa a verdadera, y cuando vuelve. Eso exige que el agente recuerde las condiciones del ciclo anterior, que es estado nuevo y hay que decirlo.

## 5. El fallo de envío es una condición más

Si el colector no responde, ¿qué hace el agente?

**No se detiene, y no lo oculta.** El envío fallido se convierte en una condición degradada más —`salidaNoDisponible`— que viaja por el canal IPC que VIS-04 ya consulta. Es la aplicación literal del principio triestático: «alerté», «no pude alertar» y «no había nada que alertar» son tres cosas.

Lo que **no** se hace es reintentar indefinidamente ni encolar sin límite. Una cola de alertas no enviadas que crece sin cota es el agotamiento de memoria de RPT-018 §6 con otro nombre. Se reintenta con espera acotada y, superado el intento, la alerta **sigue en ALM-01** — que es para lo que existe.

## 6. Lo que este diseño no resuelve

1. **La segunda interfaz es una condición de compra.** Ver §2. Debe entrar en la matriz de despliegue antes de que ningún cliente adquiera hardware.
2. **TLS.** Deseable, no bloqueante, y no está.
3. **El agente sigue siendo un recorrido.** Emitir tiene sentido en un servicio que corre; hoy termina tras un número fijo de tramas. PA-41.
4. **Nadie ha decidido el formato de los campos estructurados.** RFC 5424 admite `STRUCTURED-DATA` y elegir sus claves es una decisión de integración con el SIEM del cliente, no nuestra.
5. **La emisión no está firmada.** Quien esté en la red de gestión puede inyectar alertas falsas al SIEM haciéndose pasar por el agente. Firmar la línea de syslog es posible y ningún SIEM lo verificaría por defecto, así que sería teatro salvo que el cliente lo integre. Se anota sin resolver.

El punto 5 es el que más se parece a una promesa que nadie hizo.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-61** | **Segunda interfaz en la matriz de despliegue.** Un sensor con una sola tarjeta no puede cumplir PA-42 | Que PA-42 sea desplegable |
| **PA-62** | Syslog sobre TLS | Confidencialidad de la alerta en la red de gestión |
| **PA-63** | Autenticidad de la alerta emitida (§6.5) | Que el SIEM pueda distinguir una alerta nuestra de una inyectada |

---

## 8. Qué se pide ratificar

1. **§2** — el agente exige dos interfaces, y la de gestión es **sólo de emisión**: no acepta nada entrante.
2. **§3** — syslog RFC 5424 sobre TCP; UDP descartado por pérdida silenciosa; TLS anotado y no bloqueante.
3. **§4** — se emite el suceso y **la transición** de condición, no el registro ni la condición repetida.
4. **§5** — el fallo de envío es una condición degradada más, con reintento acotado y sin cola ilimitada.

---

*Reporte Nº 31 — Salida de alertas fuera del equipo (Diseño) · PremosCorp · 6 de agosto de 2026*
