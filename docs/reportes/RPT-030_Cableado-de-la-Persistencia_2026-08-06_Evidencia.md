# RPT-030 — Cableado de la persistencia

**Tema:** Que el módulo de PA-56 tenga quien lo llame
**Nº de reporte:** 030
**Fecha:** 6 de agosto de 2026
**Área designada:** Evidencia
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-58

- **Depende de:** RPT-029 (formato y política), RPT-006 §4 (principio triestático)
- **Cierra:** PA-58

---

## 1. Por qué esto era un punto y no una nota al pie

RPT-029 dejó el módulo escrito, probado y **sin usuarios**. Es el sexto caso de la misma forma en esta sesión —`disco.rs`, `ArchivoRevocaciones`, los centinelas de alerta, el ejecutable entero, la declaración de VLAN, y ahora la persistencia— y la única razón de que no se cerrara por decreto es que se le puso identificador.

Media hora de trabajo con identificador se hace. Media hora prometida en el §7 de un reporte, no.

## 2. La cota de lectura era del inventario

`disco::leer` fijaba ocho megabytes, que es `formato::LONGITUD_MAXIMA` — la cota **del inventario**. Un registro forense crece hasta sesenta y cuatro.

Reutilizarla sin más habría rechazado ficheros perfectamente válidos, y lo peor no es el rechazo: es **qué dice**. El operador ve `Excesivo` sobre su registro de evidencia y lo lee como manipulación, no como «alguien se equivocó de constante».

`leer_hasta(ruta, cota)` recibe la cota de quien lee, y `leer` queda como el caso del inventario. La alternativa era duplicar la función en `eje-almacen` con otra constante, que es como dos lectores con dos ideas de qué es demasiado grande acaban discrepando sobre el mismo fichero.

`ErrorDisco::Excesivo` gana el campo `cota` para que el mensaje diga contra qué se midió.

## 3. Un fichero que no se puede leer no es un fichero conforme

Si el registro excede la cota, no se lee. Devolver «conforme» porque no se pudo comprobar sería exactamente la mentira que RPT-006 §4 existe para impedir, así que sale como `ViolacionDetectada`.

Es discutible —un fichero enorme puede ser crecimiento legítimo y no un ataque— y la asimetría se resuelve hacia el lado seguro a propósito: la consecuencia de tratarlo como violación es que se aparta y se empieza otro, con la evidencia intacta. La de tratarlo como conforme es operar creyendo que hay un registro que nadie ha leído.

## 4. El fallo de disco sí impide arrancar; el registro dañado no

Dos fallos que parecen el mismo y no lo son:

- **Registro dañado** → `CargaRegistro` lo resuelve. El agente arranca, aparta el fichero y sigue observando. Negarse a arrancar apagaría la vigilancia por un daño ya ocurrido.
- **El disco no responde** → `ErrorAgente::Evidencia`, y el agente no arranca. Si no puede leer su almacén, tampoco podrá anexar, y **arrancar fingiendo que sí es peor que no arrancar**: el operador vería un sensor en marcha que no registra nada.

Si el renombrado del fichero apartado falla, se avisa y se continúa. Quedarse sin vigilancia por un fallo de disco sería un daño cierto para evitar uno ya ocurrido.

## 5. Se persiste una vez, al final, y eso tiene un precio

Escribir en cada alerta reescribiría el fichero entero por cada una (RPT-029 §5). Se persiste una vez, tras el recorrido.

La contrapartida está en el código y no sólo aquí: **si el proceso muere entre la última alerta y la escritura, esas alertas se pierden.** Es aceptable en un recorrido de comprobación que dura segundos y deja de serlo en un servicio que corre semanas. Ahí hará falta anexado incremental —escribir sólo lo nuevo, con la ventana de escritura parcial que RPT-029 §5 evitó— y eso es **PA-60**.

Decirlo importa porque el título de este reporte es «cableado de la persistencia» y alguien podría leer que las alertas ya están a salvo. Lo están frente a un reinicio ordenado. No frente a un `kill -9` en el momento equivocado.

## 6. Lo que sigue sin resolverse

1. **El extremo sigue sin anclar** (PA-57). Alterar el último asiento no se detecta.
2. **No hay retención** (PA-59). El registro crece hasta `ASIENTOS_MAXIMOS` y llegar ahí lo vuelve ilegible en lugar de rotarlo.
3. **Nada sale del equipo** (PA-42). Un registro persistido en un armario de planta sigue siendo un registro que nadie mira.
4. **El agente sigue siendo un recorrido, no un servicio.** Observa un número fijo de tramas y termina. El bucle de servicio y el canal IPC son PA-41 y PA-42.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-60** | **Anexado incremental del registro.** Hoy una muerte súbita entre la última alerta y la escritura las pierde | Operación como servicio continuo |
| PA-58 | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 30 — Cableado de la persistencia · PremosCorp · 6 de agosto de 2026*
