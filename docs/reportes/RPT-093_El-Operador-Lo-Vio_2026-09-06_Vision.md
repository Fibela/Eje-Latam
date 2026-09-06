# RPT-093 — El operador lo vio

**Tema:** PA-78 mitad B. Cinco semanas, y lo que faltaba no era código
**Nº de reporte:** 093
**Fecha:** 6 de septiembre de 2026
**Área designada:** Visión
**Entidad:** PremosCorp
**Estado:** **Cerrado por observación en máquina real.** Captura en `docs/evidencia/RPT-093_VIS-04_2026-09-06.png`

- **Depende de:** RPT-079 (mitad A), RPT-090 (el tercer canal), RPT-091 (la vista tipada), RPT-092 (la rotación de identidad que lo desbloqueó)
- **Aborda:** PA-78 (cerrado). Abre PA-148

---

## 1. Lo observado

VIS-04 sobre XFCE en `eje-prueba`, hablando con el agente **como servicio**, por el socket
por omisión del contrato —`/run/eje-latam/agente.sock`, sin `EJE_SOCKET`—, con el usuario
en el grupo `eje-ipc` y el socket en `srw-rw---- root eje-ipc`.

En pantalla, y esto es lo que PA-78 pedía desde el 1 de agosto:

```
Este sensor no informa a ninguna sala
No tiene colector configurado: vigila el segmento, pero nada sale de
este equipo y nadie fuera notará si se apaga.

Condiciones      trece «no» y un «sí»: Sin colector
Inventario       1 equipos · 0 con marcado firmado · 0 por declaración de
                 segmento · 1 sin respaldo suficiente · 0 sin veredicto
                 00:00:00:00:00:00 ambiguaSegmentoPuedeAlojarCriticos
                 segmento noDeclarado · visto en segmento crítico
```

## 2. La predicción, y dónde falló

Se escribió antes de lanzar. Acertó **carácter a carácter** la fila del inventario y los
cinco contadores del respaldo, y acertó las catorce condiciones.

Falló en la cabecera. Se predijo `sensor-eje-prueba · eje-prueba`; salió
`Eje-Agente 0.1.0 · perfil corporativo · contiene sin aprobación`. **Ni el nombre del
sensor ni la máquina aparecen en la pantalla.** Es PA-148, y no es cosmético: `nombre`
existe para correlacionar sellos y detectar ausencia de latidos, y con tres consolas
abiertas ninguna diría cuál es cuál.

Se anota porque una predicción que acierta entera no enseña nada. Ésta enseñó dónde no
habíamos mirado.

## 3. Cuatro paredes, y ninguna era el código

Entre «el código está listo» y «el operador lo ve» hubo cuatro bloqueos, y **ninguno
estaba en Eje-Latam**:

| Pared | Qué era |
|---|---|
| Sin escritorio | XFCE y LightDM sin instalar en la VM |
| El saludo no aceptaba teclado | `accountsservice` sin instalar; se rodeó con entrada automática |
| Sin `$DISPLAY` | Electron lanzado por SSH; se resolvió leyendo el entorno de `xfce4-session` en `/proc` |
| Electron moría al cerrar la sesión | Corría en primer plano de un SSH; `nohup` y registro a fichero |

La lección operativa: **un punto que sólo se puede cerrar observando depende de un entorno
que nadie ha presupuestado.** PA-78 estuvo cinco semanas abierto y su código llevaba dos
listas.

## 4. La causa real, y es del catálogo de la casa

Al arrancar por fin, VIS-04 dijo:

```
el agente rechazó «obtener-estado-agente»: el canal está declarado
y aún no tiene manejador en el agente
```

Ese es el motivo de `servido = false`. Pero los dos canales se cablearon —PA-135 y
PA-138b— y **se observaron respondiendo en esta misma VM el 28 de agosto**.

`/usr/local/bin/eje-agente` tenía md5 `37d036a5…`, del 25 de agosto. El binario probado es
`8b547ec…`, y vivía en `/tmp`. **Nunca se instaló.**

Es el defecto dominante del proyecto en la capa que faltaba: no un mecanismo sin cablear en
el código, sino **un mecanismo cableado, probado y observado que no está en el artefacto
desplegado**. Ninguna prueba podía cazarlo: todas corren contra el árbol, y el árbol estaba
bien. Lo cazó comparar dos md5.

Y una nota que vale más que el defecto: `cargo build --release` produjo `8b547ec…`, **el
mismo md5 que el 28 de agosto**. La construcción es reproducible entre sesiones y entre
semanas.

## 5. Dos cosas que la pantalla demostró y no eran su cometido

**El rechazo llegó entero hasta el DOM.** Cuando el binario era el viejo, la vista no pintó
una lista vacía ni ceros: escribió el motivo literal, en castellano, con el nombre del
canal. RPT-006 §4 sostenido desde el contrato en Rust hasta la pantalla, que es el recorrido
completo y es la primera vez que se ve entero.

**Y antes de eso, con el módulo caído, dijo «EL MÓDULO DE LA VISTA NO ARRANCÓ»** en lugar de
quedarse en blanco con aspecto de normalidad. El fallo de carga era real —`vista/dist/`
importa `packages/eje-vision-base/dist/indice.js` por ruta relativa, y en la VM había una
copia del 24 de agosto que aún exportaba `resumirPostura`—, y la vista lo declaró en vez de
fingir.

### 5.1 El error de copia fue mío, y tiene forma conocida

Dije «serán tres ficheros, no el árbol entero». Miré los `import` del `.ts`, donde pone
`@eje/vision-base`, y no los del `.js` emitido, que es lo que el navegador ejecuta y que
`tsc` reescribe a `../../../packages/eje-vision-base/dist/indice.js`.

**El grafo de dependencias real no está en el fuente: está en lo emitido.** Es la misma
trampa de RPT-091 §3 vista desde el otro lado.

## 6. Puntos abiertos

| ID | Punto |
|---|---|
| PA-78 | **Cerrado.** §1 |
| PA-148 | La cabecera no dice qué sensor es. §2 |
| PA-142 | Sigue parcial: `diagnostico.js` |
| PA-146 | La rotación de identidad sigue sin procedimiento |
| PA-147 | La plantilla del instalador contradice al código |

---

*Reporte Nº 93 — El operador lo vio · PremosCorp · 6 de septiembre de 2026*
