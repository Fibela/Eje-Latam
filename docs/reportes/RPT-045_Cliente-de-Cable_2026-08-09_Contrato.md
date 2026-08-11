# RPT-045 — El cliente del protocolo, antes que la ventana

**Tema:** El salto que separa a VIS del agente, y cómo se verifica byte a byte
**Nº de reporte:** 045
**Fecha:** 9 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** **Diseño. Requiere ratificación antes de implementar.**

- **Depende de:** RPT-002 §9.3 (transporte), RPT-035 (protocolo), RPT-042 y RPT-043 (paridad de uso)
- **Abriría:** PA-77, PA-78

---

## 1. Son dos saltos y sólo uno existe

«IPC» nombra dos cosas distintas en este proyecto y conviene separarlas antes de
escribir nada:

```text
renderer ──[contextBridge / ipcRenderer.invoke]──▶ proceso principal   (existe a medias)
proceso principal ──[socket Unix + marco de eje-ipc]──▶ eje-agente     (NO existe)
```

El primero es mecanismo interno de Electron y está declarado: `puente-ipc.ts`
tiene la lista de permitidos y `seguridad-ventana.ts` la configuración de la
ventana. El segundo es un protocolo binario propio y **no hay una sola línea de
TypeScript que lo hable**.

Empezar por la ventana daría una ventana conectada a nada, y el problema real
aparecería al final en lugar de al principio. De ahí el orden invertido.

## 2. El formato de cable, tal como es hoy

Del lado de Rust, en `eje-ipc`:

```text
marco    = longitud(u32 big-endian, 4 bytes) ‖ carga
peticion = longitud_nombre(u16 BE, 2 bytes) ‖ nombre ‖ carga_util
respuesta= codigo(1 byte: 0 = respuesta, 1 = rechazo) ‖ cuerpo
```

Cotas: marco ≤ 1 048 576 bytes; nombre de canal ≤ 64 bytes.

No hay nada exótico, y ése es justamente el riesgo: **es lo bastante simple como
para reimplementarlo de memoria y lo bastante específico como para equivocarse en
un byte**. Un prefijo escrito en little-endian pasaría las 33 pruebas de
TypeScript y las 417 de Rust sin inmutarse, y fallaría la primera vez que los dos
procesos se hablen de verdad.

## 3. La barrera: vectores generados por Rust, leídos por los dos

Las barreras de PA-75 y PA-76 comprueban **formas y esquemas**. Ninguna mira los
bytes. Añadir un tercer sitio donde el contrato vive —el codificador de
TypeScript— sin una barrera nueva sería repetir el error que llevamos toda la
semana corrigiendo.

La propuesta reutiliza un patrón que este proyecto ya tiene: los vectores de
`motor-pqc` (ACVP, Wycheproof) con su anclaje en `FUENTES.toml`.

1. `cargo xtask vectores-ipc` genera `vectores-ipc.json`: una lista de casos
   `(canal, carga, marco esperado en hexadecimal)`, producidos **por el
   codificador de Rust**, que es el que manda.
2. Una prueba de Rust comprueba que regenerar el fichero da lo mismo. Si alguien
   toca `enmarcar` o `componer_peticion`, el fichero deja de cuadrar y hay que
   regenerarlo **a propósito**.
3. Una prueba de TypeScript lee el mismo fichero y exige que su codificador
   produzca esos bytes exactos, y que su decodificador recupere lo que toca.

Ninguno de los dos lados puede moverse sin que el otro lo note, y no hay tabla que
mantener a mano: los vectores salen del código que ya existe.

**Los casos límite entran en los vectores desde el principio**: carga vacía, carga
en el límite exacto, nombre de canal más largo, respuesta de rechazo con motivo.
Son los que un reimplementador acierta por casualidad o falla en silencio.

## 4. Lo que el cliente tiene que hacer bien y es fácil hacer mal

**Leer del socket no es leer un mensaje.** Un `socket.on("data")` entrega trozos
arbitrarios: puede llegar medio marco, o dos marcos juntos. El cliente necesita un
acumulador que lea el prefijo, espere a tener esos bytes y sólo entonces entregue.
Es el fallo clásico de todo cliente de protocolo con longitud, y no lo detecta
ninguna prueba que use mensajes pequeños en localhost — aparece en producción.

**La cota se comprueba antes de reservar.** Igual que en Rust: un prefijo de
longitud llega del otro extremo, y `Buffer.alloc(prefijo)` sobre un valor absurdo
es una reserva de memoria dictada por quien habla.

**El rechazo no es un error de transporte.** `CODIGO_RECHAZO` es una respuesta
válida que lleva un motivo. Convertirlo en una excepción genérica perdería el
motivo, que es justo lo que RPT-036 §6 puso ahí para que VIS-04 no confunda «no
hay nada» con «esto no lo sirve nadie».

## 5. El transporte queda decidido, y con él una pregunta de producto

Ratificado: **socket de dominio Unix, co-localizado**. La consola remota no habla
con el agente; consume del colector, adonde ya llegan las alertas y los sellos
(PA-42, PA-64). El IPC del agente no cruza la red nunca.

Eso es coherente y cierra el asunto del *named pipe*, que queda sin implementar y
sin necesidad mientras VIS no corra en Windows.

Pero tiene una consecuencia que no está resuelta: **si VIS-04 corre en el sensor,
el sensor necesita escritorio**. Un equipo en un rack de fábrica no suele tenerlo,
y Electron sin servidor gráfico no arranca.

Puede ser correcto —hay appliances con consola local para el técnico que se planta
delante con un monitor— pero si el sensor va a ser una caja sin cabeza, Electron es
la tecnología equivocada para VIS-04 y conviene saberlo antes de construirla, no
después. Se anota como **PA-77**: no es código, es una decisión de producto sobre
quién mira el tablero y desde dónde.

## 6. Lo que no cubre este trabajo

**El agente no está corriendo.** Las pruebas de integración del §3 comprueban el
codificador contra vectores, no contra un proceso vivo. Probar los dos procesos
hablando exige levantar `eje-agente` con su socket, y eso es Linux — PA-40 otra
vez. Queda como **PA-78**, y hasta entonces la garantía es de formato, no de
conversación.

Decirlo ahora evita que el verde de las pruebas de vectores se lea como «VIS habla
con el agente».

## 7. Lo que propongo ratificar

1. El orden invertido: cliente de cable primero, ventana después.
2. Los vectores generados por Rust como barrera de bytes (§3), con los casos
   límite dentro desde el principio.
3. El acumulador de marcos como requisito explícito, no como detalle de
   implementación (§4).
4. **PA-77** abierto: dónde corre VIS-04 y si el sensor tiene escritorio.
5. **PA-78** abierto: la conversación real entre procesos, tras PA-40.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-77** | **¿Tiene escritorio el sensor?** Si VIS-04 es co-localizada y la caja no tiene servidor gráfico, Electron no arranca | Elegir la tecnología de VIS-04 antes de construirla |
| **PA-78** | **Nadie ha visto a los dos procesos hablarse.** Los vectores prueban el formato, no la conversación | Depende de PA-40 |

---

*Reporte Nº 45 — El cliente del protocolo, antes que la ventana · PremosCorp · 9 de agosto de 2026*
