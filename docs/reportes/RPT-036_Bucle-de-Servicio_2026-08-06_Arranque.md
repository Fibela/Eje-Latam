# RPT-036 — Bucle de servicio

**Tema:** Que el agente vigile en lugar de pasar una vez
**Nº de reporte:** 036
**Fecha:** 6 de agosto de 2026
**Área designada:** Arranque
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-66 y PA-67

- **Depende de:** RPT-034 (diseño ratificado), RPT-035 (escucha y protocolo)
- **Cierra:** PA-66, y con él PA-67

---

## 1. El modo por defecto no cambia

`--ciclos 1` es el valor por omisión y es el recorrido de comprobación de siempre. `--ciclos 0` es el servicio continuo.

Convertir el agente en demonio por omisión habría **alterado lo que hace un binario ya existente sin que nadie lo pidiera**. Quien lo invoque como hasta ahora obtiene lo de hasta ahora.

## 2. Dos cosas que vivían fuera del ciclo y tenían que quedarse fuera

**El almacén de observación.** Recrearlo en cada vuelta borraría la ambigüedad pegajosa de RPT-010 §5, y con ella la protección del carro de telemedicina que pasó por la VLAN clínica. Un demonio que olvida cada minuto es peor que un recorrido que recuerda una vez.

**La escucha.** Reabrirla en cada ciclo dejaría una ventana en la que VIS-04 no encuentra a nadie, y con `CONEXIONES_POR_CICLO` acotado eso convierte un cliente que reintenta en un cliente que nunca acierta.

## 3. Y una que estaba fuera y debía estar dentro

`let instante = ahora();` se calculaba **una sola vez, antes del bucle**.

En un recorrido de segundos eso es correcto. En un demonio de días congela el reloj, y con él **ningún marcado caduca nunca** — exactamente lo contrario de la política de vigencia de RPT-011, que hace expirar los marcados porque un parque cambia y un marcado de hace tres años describe un equipo que quizá ya no está.

Apareció al mover el bucle, no revisando el código. Es el tipo de defecto que sólo existe cuando algo pasa de correr una vez a correr muchas, y por eso no lo tenía ninguna prueba: **ninguna prueba ejecutaba dos ciclos**.

## 4. Sólo se escribe si el registro cambió

RPT-034 §1.1. `registro.longitud() != anexados_antes` es toda la condición.

En un sensor tranquilo no se escribe nunca, y eso es lo que evita el coste cuadrático de reescribir el fichero entero en cada vuelta. PA-60 sigue siendo optimización y no correctitud.

## 5. El orden del ciclo, y por qué cada pieza está donde está

```text
observar → clasificar → anexar → persistir (si cambió) → emitir → atender IPC
```

**Persistir antes de emitir**: si el proceso muere entre ambos, la alerta está en disco y el SIEM no se enteró — recuperable. Al revés, el SIEM sabe de una alerta que el registro no tiene, y eso es peor que no saber.

**IPC al final**: una consulta responde con lo que ya está persistido, nunca con lo que aún vive sólo en memoria. Eso permite además que `Manejadores` tome referencias **compartidas**, y con ello no hace falta ningún cerrojo — que era el argumento entero del hilo único.

## 6. Los cuatro canales sin manejador se rechazan con motivo

`obtener-estado-agente`, `obtener-inventario`, `obtener-estado-boveda` y `consultar-sandbox` están declarados y pertenecen a módulos que aún no existen.

Se rechazan **con motivo** en lugar de devolver una lista vacía. Una lista vacía haría creer a VIS-04 que el inventario está vacío, y «no hay nada» y «esto todavía no lo sirve nadie» no son lo mismo: es RPT-006 §4 otra vez, esta vez en el puente.

Nota de estilo con consecuencia: el texto dice «aún no tiene manejador» y **no** «no implementado». El escáner de `xtask` prohíbe ese literal en ruta de producción, y la prohibición es correcta — pero conviene saber que forzó redactar el mensaje de otra forma, no que el mensaje se omitiera.

## 7. Lo que sigue sin resolverse

1. **La cadencia sigue sin medir.** El ciclo dura lo que dure la ventana de captura, y `--tramas` la gobierna. Sin PA-40 cualquier número es inventado; PA-41 —la cifra de latencia— sigue abierto y este trabajo no lo cierra.
2. **Ninguna prueba ejecuta dos ciclos.** El bucle vive en `main.rs`, que no tiene pruebas, y por eso el defecto del §3 llegó a existir. Cerrarlo exige extraer el ciclo a la biblioteca o probar el proceso; se anota como **PA-68**.
3. **Sin `systemd`** (PA-65): el binario corre en bucle, pero nadie lo lanza ni lo reinicia.
4. **El agente sigue sin contener nada** (PA-22).
5. **Las condiciones se imprimen en cada vuelta.** En un demonio eso es ruido; hace falta imprimir sólo lo que cambia, que es la misma lección de RPT-032 §3 aplicada a la salida por pantalla.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-68** | **Probar el ciclo, no sólo sus piezas.** Ninguna prueba ejecuta dos vueltas, y ahí vivía el defecto del §3 | Confianza en el modo continuo |
| PA-66 | — | ✅ **Cerrado por este reporte** |
| PA-67 | — | ✅ **Cerrado**: diseño en RPT-034, escucha en RPT-035, bucle aquí |

---

*Reporte Nº 36 — Bucle de servicio · PremosCorp · 6 de agosto de 2026*
