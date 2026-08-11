# RPT-034 — Servicio continuo (Diseño)

**Tema:** Que el agente deje de ser un recorrido y pase a vigilar
**Nº de reporte:** 034
**Fecha:** 6 de agosto de 2026
**Área designada:** Arranque
**Entidad:** PremosCorp
**Estado:** **Diseño — sin implementar.** Requiere ratificación

- **Depende de:** RPT-002 §9.3 (transporte IPC), RPT-029/030 (persistencia), RPT-032 (salida)
- **Cubre:** PA-67 (ver §0)
- **Reformula:** PA-60

---

## 0. Fe de erratas: el demonio no es PA-41

Las primeras versiones de este reporte y de la matriz del equipo llamaban PA-41 al servicio continuo. **PA-41 ya estaba ocupado** desde RPT-019 §8: es «intervalo de consulta de alertas — cuánto puede tardar VIS-04 en enterarse de una amenaza incontenible».

Son cosas distintas y relacionadas. El demonio es el **mecanismo**; PA-41 es la **cifra**, y §5.4 de este reporte dice expresamente que no está medida. Cerrar el demonio no cierra PA-41.

El servicio continuo pasa a ser **PA-67**. La colisión se coló porque los identificadores se asignan escribiendo prosa y no derivándolos de ningún sitio — el mismo defecto que `cargo xtask tablero` corrige para los recuentos y que no cubre para la asignación.

## 1. La durabilidad no se resuelve capturando señales

Es la decisión central y va primero porque cambia todo lo demás.

El instinto al escribir un demonio es capturar `SIGTERM` para vaciar antes de salir. Aquí no sirve, y por tres motivos que se acumulan:

1. **No cubre `SIGKILL`**, ni un corte de luz, ni que el núcleo mate el proceso por memoria. El operador que reinicia un sensor de planta rara vez lo hace con delicadeza.
2. **El perfil de release declara `panic = "abort"`.** Un pánico no desenrolla la pila, así que ningún destructor corre — ya está escrito en `disco.rs` y vale igual aquí.
3. Capturar señales sin dependencia exige `libc` y bloques `unsafe`, y este workspace los tiene confinados a un módulo de `eje-captura` con siete llamadas enumeradas. Ampliarlo para esto sería mal cambio.

**La respuesta es persistir en cada ciclo**, no al salir. Con eso, una muerte súbita pierde como mucho el ciclo en curso, y lo pierde igual sea `SIGTERM`, `SIGKILL` o un apagón. La cobertura es uniforme en lugar de depender de cómo muera el proceso.

### 1.1 Y eso reformula PA-60

PA-60 se abrió como «anexado incremental para tolerar fallos catastróficos». Con persistencia por ciclo, la tolerancia ya está; lo que queda es **rendimiento**, que es otra cosa.

RPT-029 §5 dejó escrito que reescribir el fichero entero es lineal en su tamaño. Un ciclo cada `T` segundos sobre un registro de `n` asientos da coste cuadrático a lo largo de una ejecución larga.

Se resuelve con una condición y no con un mecanismo: **sólo se escribe si el registro cambió**. Las alertas son raras y graves; en un sensor tranquilo no se escribe nunca. PA-60 pasa de correctitud a optimización, y su disparador deja de ser «cuando desplegemos el demonio» y pasa a ser «cuando un cliente genere alertas con frecuencia suficiente para que se note».

## 2. El transporte ya está decidido y no se reabre

RPT-002 §9.3 eliminó explícitamente los WebSockets locales: un servicio en `localhost` es alcanzable por cualquier proceso local **y por cualquier página que el usuario visite** —*DNS rebinding* y origen cruzado son vectores conocidos y explotados—.

Único transporte autorizado: **socket de dominio Unix con ACL** en Linux, named pipe con descriptor de seguridad en Windows.

En la práctica esto es Linux. `eje-captura` sólo funciona ahí, así que el demonio no corre en Windows y el named pipe es una previsión, no una entrega. `std::os::unix::net::UnixListener` está en la biblioteca estándar y **no exige `unsafe` ni dependencia nueva**, que es la razón de que esto sea abordable ahora.

### 2.1 El agente pasa a escuchar, y eso es nuevo

Conviene decirlo sin adornos: hasta hoy el diseño entero era **no transmitir y no escuchar**. `eje-captura` no tiene método de envío; la salida de RPT-032 es de emisión pura. Un socket que acepta conexiones es superficie nueva.

Lo que la acota:

- **No es una red.** Un socket de dominio Unix vive en el sistema de ficheros y no es alcanzable desde otra máquina. Ninguna página web puede conectarse a él.
- **Permisos `0600` sobre el propio socket**, y directorio propio. Sólo el usuario del agente conecta.
- **La lista de permitidos de `eje-ipc` ya existe** y rechaza canal desconocido y carga excesiva antes de interpretar nada.
- **Los dos canales son de consulta.** No hay ninguno que ordene contención; RPT-004 §6.2 lo prohíbe y hay prueba.

Lo que **no** acota: quien ya ejecute código como el usuario del agente puede consultar alertas. Es aceptable —ese atacante ya tiene el registro en disco— y merece quedar escrito.

## 3. Un solo hilo, y por qué

La tentación es un hilo para capturar y otro para servir IPC. Se rechaza:

`FuentePasiva::siguiente` ya recibe un plazo. Un bucle único puede capturar durante una ventana corta, atender lo que haya en el socket, y volver. Con dos hilos aparecen el registro compartido y el almacén de observación compartidos, y con ellos cerrojos en la ruta que decide si un equipo médico se contiene.

**Un cerrojo en el camino de la contención es un modo de fallo nuevo** —bloqueo, inversión de prioridad, envenenamiento tras pánico— a cambio de latencia que nadie ha pedido. El coste real es que una consulta IPC espera como mucho una ventana de captura.

## 4. El ciclo

```text
    ┌─ arrancar: claves, inventario, registro + ancla, socket
    │
    ├─► capturar durante VENTANA
    ├─► clasificar lo observado
    ├─► anexar amenazas incontenibles
    ├─► si el registro cambió: persistir registro y ancla
    ├─► emitir sucesos nuevos y transiciones de condición
    ├─► atender conexiones pendientes del socket, sin bloquear
    └─◄ repetir
```

**El orden importa en dos sitios.** Persistir va antes de emitir: si el proceso muere entre ambos, la alerta está en disco y el SIEM no se enteró —recuperable—; al revés, el SIEM sabe de una alerta que el registro no tiene, y eso es peor que no saber.

Y atender IPC va al final: una consulta responde con lo que ya está persistido, nunca con lo que aún vive sólo en memoria.

## 5. Lo que este diseño no resuelve

1. **El agente sigue sin contener nada.** PA-22.
2. **No hay unidad de `systemd` ni gestor de servicio.** El binario correrá en bucle; que arranque solo, se reinicie al fallar y escriba a `journald` es empaquetado, y es PA-12 con PA-39.
3. **Sin `SIGTERM` capturado, el apagado no es limpio**: el socket queda en el sistema de ficheros y hay que retirarlo al arrancar si está huérfano. Se resuelve con una comprobación al abrir, no con una señal.
4. **La cadencia no está decidida.** Una ventana corta responde antes y come CPU; una larga al revés. Sin PA-40 —una máquina real con tráfico real— cualquier número es inventado, así que se propone configurable con un valor por defecto y se anota que **no está medido**.
5. **PA-64 sigue abierto** y este diseño lo hace más caro: firmar el ancla en cada ciclo, aunque sólo se escriba cuando algo cambia, es una firma híbrida por alerta. Merece medirse antes de exigirse.

## 6. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-65** | **Unidad de servicio y arranque automático.** Va con PA-12 y PA-39 | Que el sensor vigile sin que nadie lo lance |
| PA-60 | — | Se **reformula**: de correctitud a rendimiento. Ver §1.1 |

---

## 7. Qué se pide ratificar

1. **§1** — la durabilidad se resuelve persistiendo **en cada ciclo con cambio**, no capturando señales. Sin `libc` ni `unsafe` nuevos.
2. **§1.1** — PA-60 se reclasifica de correctitud a rendimiento, con disparador medible en lugar de calendario.
3. **§2** — socket de dominio Unix con permisos `0600`, sólo en Linux. Windows queda declarado no soportado en lugar de fingido.
4. **§3** — un solo hilo. Ningún cerrojo en la ruta que decide una contención.
5. **§4** — persistir antes de emitir; atender IPC al final, sobre lo ya persistido.

---

*Reporte Nº 34 — Servicio continuo (Diseño) · PremosCorp · 6 de agosto de 2026*
