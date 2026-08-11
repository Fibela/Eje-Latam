# RPT-035 — Escucha local y forma de la petición

**Tema:** Que VIS-04 pueda preguntar, y que el contrato diga cómo
**Nº de reporte:** 035
**Fecha:** 6 de agosto de 2026
**Área designada:** Arranque
**Entidad:** PremosCorp
**Estado:** **Implementado parcialmente.** PA-67, mitad de protocolo y escucha

- **Depende de:** RPT-034 (diseño ratificado), RPT-002 §9.3 (transporte), RPT-006/007 (contrato IPC)
- **Cubre:** PA-67 §§2 y 4 parcialmente (no PA-41: ver RPT-034 §0). **No cubre** el bucle — ver §6

---

## 1. Al contrato le faltaba cómo viaja el nombre del canal

`contrato-ipc.toml` declaraba **qué** canales existen y **qué** campos lleva cada carga. No declaraba **cómo** viaja el nombre del canal por el cable, y `enmarcar`/`desenmarcar` sólo manejan el prefijo de longitud.

Mientras no hubo transporte, no se notó. Al escribir el servicio, **cada extremo habría tenido que inventarlo** — que es exactamente lo que este manifiesto existe para impedir, y el defecto que RPT-006 documentó como motivo de su existencia.

Se añaden `[peticion]` y `[respuesta]`, con prueba de paridad:

```text
peticion:   [u16 BE longitud_nombre][nombre][carga]
respuesta:  [u8 codigo][resto]
```

El nombre va **prefijado en longitud y no delimitado por un separador**: un separador obliga a decidir qué pasa si aparece dentro del nombre, y esa decisión no debería existir.

## 2. El rechazo tiene código, y por qué eso importa

Un canal que devolviera bytes vacíos ante un rechazo sería **indistinguible de uno que devuelve una lista vacía**. Es el tercer estado de RPT-006 §4 llevado al cable: «no hay nada» y «no pude decírtelo» no son lo mismo.

Y el rechazo **nunca falla al componerse**: el motivo se recorta en lugar de devolver error. Quien rechaza ya está en el camino de error, y un fallo al construir el mensaje de fallo dejaría al otro extremo sin respuesta ninguna, que es lo único inaceptable.

## 3. El agente pasa a escuchar

Hasta hoy el diseño era **no transmitir y no escuchar**. `eje-captura` no tiene método de envío; la salida de RPT-032 es de emisión pura. Un socket que acepta conexiones es superficie nueva y conviene decirlo sin adornos antes de enumerar lo que la acota:

- **No es una red.** Un socket de dominio Unix vive en el sistema de ficheros. Ninguna página web puede conectarse — que es exactamente por lo que RPT-002 §9.3 eliminó el WebSocket local.
- **Permisos `0600`** sobre el socket.
- **La lista de permitidos ya existía** y rechaza canal desconocido y carga excesiva antes de interpretar nada.
- **Los dos canales son de consulta.** Hay prueba de que `ordenar-contencion` no es alcanzable por este camino.

Lo que **no** acota: quien ya ejecute código como el usuario del agente puede consultar alertas. Ese atacante ya tiene el registro en disco, así que no gana nada nuevo — y queda escrito para que nadie lo descubra en una auditoría.

### 3.1 Una ventana que el código no cierra

Entre `bind` y `set_permissions` el socket existe con la máscara del proceso. La ventana es de microsegundos y **no se cierra desde aquí**: se acota con los permisos del directorio, que son de quien despliega.

Está en el comentario del código además de aquí, porque es el tipo de cosa que se lee como resuelta si sólo aparece en un reporte.

## 4. El socket huérfano se retira al abrir

Sin captura de señales (RPT-034 §1), un apagado abrupto deja el fichero en su sitio y `bind` falla con «dirección en uso». Retirarlo al arrancar es lo que sustituye al apagado limpio.

Pero **sólo si no hay nadie escuchando**: si otro agente está vivo sobre esa ruta, borrarlo lo dejaría sordo sin que se enterase. Se comprueba conectando antes de borrar.

El destructor también lo retira, y es mejor esfuerzo: con `panic = "abort"` no corre. Por eso la comprobación al abrir es la que cuenta.

## 5. Dos cotas que protegen el hilo único

Con un solo hilo (RPT-034 §3), **un cliente lento es un cliente que apaga la vigilancia**. De ahí:

- `PLAZO_CONEXION` de 250 ms para leer y escribir. Una conexión que no habla no detiene el ciclo.
- `CONEXIONES_POR_CICLO` de 16. Sin cota, un cliente que reconecta en bucle mantendría el agente sirviendo y sin capturar.

Lo que no se atiende este ciclo se atiende el siguiente. Y un fallo de una conexión no tumba el ciclo: se ignora y se pasa a la siguiente, porque la observación no puede detenerse por un cliente roto.

## 6. Lo que falta de PA-67

**El bucle de servicio.** `Escucha` sabe aceptar y `atender_peticion` sabe responder; el `main` sigue siendo un recorrido que termina tras N tramas. Falta encadenar el ciclo de RPT-034 §4 y conectar `Atiende` con los manejadores de RPT-028.

Se separa a propósito y con identificador —**PA-66**— en lugar de dejarlo prometido en un §7, que es la lección que PA-58 dejó: media hora con identificador se hace; media hora prometida en un reporte, no.

También falta, y sigue en PA-65: unidad de `systemd`, arranque automático y reinicio ante fallo.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-66** | **Bucle de servicio.** `Escucha` y el protocolo existen; nadie los llama en ciclo | Que PA-67 sirva de algo |
| PA-65 | Unidad de servicio y arranque automático | Que el sensor vigile sin que nadie lo lance |

---

*Reporte Nº 35 — Escucha local y forma de la petición · PremosCorp · 6 de agosto de 2026*
