# RPT-024 — Clave aprovisionada en el agente

**Tema:** Que el sensor sepa con qué clave verificar lo que le llega
**Nº de reporte:** 024
**Fecha:** 6 de agosto de 2026
**Área designada:** Arranque
**Entidad:** PremosCorp
**Estado:** **Implementado.** PA-49

- **Depende de:** RPT-011 §4 (`DominioClave`), RPT-015 §4 (clave de recuperación), RPT-017 (`EstadoArranque`)
- **Cierra:** PA-49
- **Habilita:** PA-48

---

## 1. La mitad que faltaba

`arrancar` recibía las dos claves **como parámetros**, y nadie se las daba. `eje-agente` construía `EstadoArranque::PrimerArranque` a mano porque no tenía de dónde sacarlas.

Es la misma forma que el hallazgo de RPT-022 §1: cinco eslabones criptográficos completos, verificados por separado, y ninguna vía por la que el sensor pudiera usarlos. Emitir manifiestos firmados (PA-48) no sirve de nada si el agente no puede comprobarlos, así que este punto tenía que ir antes.

## 2. El dominio viaja en el fichero, no en la ruta

Es la decisión que sostiene todo lo demás.

`DominioClave` existe desde RPT-011 §4 para que la clave con la que PremosCorp firma binarios no pueda declarar qué equipos del cliente son críticos, y al revés. Si el dominio se dedujera de la ruta —«este fichero es el operativo porque está en `clave-cliente.pub`»—, **la separación dependería de que nadie confunda dos rutas**.

Al viajar dentro, un fichero de recuperación colocado donde va el operativo se rechaza **por lo que es**, no por dónde está. La prueba se llama `el_dominio_viaja_en_el_fichero_y_no_en_la_ruta`.

Y las dos claves van en ficheros separados aunque ambas sean material público: un solo fichero con las dos obligaría a elegir cuál se usa **en el código**, y RPT-015 §4 las separa precisamente para que esa elección no exista.

## 3. Este fichero no está firmado, y no puede estarlo

Conviene decirlo antes de que alguien lo suponga. Es el **ancla de confianza**: firmarlo exigiría otra clave para verificar la firma, y esa otra habría que aprovisionarla igual. La regresión no termina.

Lo que protege a estos bytes no es criptografía sino **el momento**: se escriben durante la instalación, con un humano presente, y a partir de ahí el centinela detecta que desaparezcan. Quien pueda sustituirlos después ya tiene escritura en el almacén del agente, que es un compromiso más grave que el que este fichero podría evitar.

## 4. Un estado nuevo, y por qué no se colapsa

Sin clave, `InventarioLocal::cargar` **no se puede ni intentar**. No es que el inventario falle la verificación: es que no hay con qué verificarlo.

Colapsarlo en `PrimerArranque` diría que la instalación está completa cuando le falta la mitad, y el administrador no se enteraría hasta emitir un manifiesto que el agente ignora en silencio. De ahí `SinClaveAprovisionada`, con el perfil que la Fase 1 de PA-45 hizo posible: **alerta sí, manipulación no**.

Pero sólo se alcanza con el centinela **sin establecer**. Si el centinela existe, alguien aceptó un inventario alguna vez, luego hubo clave y ahora no:

| clave | inventario | centinela | estado |
|---|---|---|---|
| ausente | — | sin establecer | `SinClaveAprovisionada` — instalación a medias |
| ausente | — | establecido | **`Supresion`** — borrar la clave es borrar el inventario por otra puerta |
| corrupta | — | cualquiera | **error de arranque**, no se degrada |
| presente | … | … | lógica de RPT-017 |

La segunda fila es la que importa. Sin ella, **borrar un fichero de dos kilobytes conseguiría lo mismo que borrar el inventario**: ningún marcado, y un equipo de soporte vital vuelto contenible. El centinela ya era el testigo de esa clase de ataque y no hizo falta inventar nada.

La tercera aplica el argumento de `un_centinela_corrupto_no_se_degrada_a_primer_arranque`: corromper el fichero no puede ser una vía para simular el estado que exime de verificar.

## 5. El agujero que me encontré escribiendo esto

El primer borrador de `arrancar_con_almacen` resolvía la ausencia de la clave de recuperación sustituyéndola por la operativa:

```rust
let lectora = recuperacion.unwrap_or_else(|| {
    ClaveInventario::nueva(operativa.verificacion().clone(), DominioClave::ClienteRecuperacion)
});
```

Parece inofensivo —es la misma clave pública, sólo cambia la etiqueta— y es un agujero directo. Quien tuviera la privada operativa habría podido **forjar un certificado de revocación que verificase**, y con él bajar el centinela por `Centinela::reiniciar_por` para después reponer un inventario anterior. Es el ataque de PA-27 servido por la puerta que RPT-015 §4 cerró: las dos claves están separadas justamente para que quien roba la operativa no pueda revocar.

La corrección obligó a cambiar la firma pública: `arrancar` y `cargar_revocaciones` toman ahora `Option<&ClaveInventario>`, y sin clave de recuperación el registro queda **vacío** en lugar de leerse con una clave inventada. Mismo resultado que un fichero de revocaciones ausente, que RPT-015 §5 ya acepta.

Vale la pena registrar cómo apareció: no revisando el código, sino al escribir el comentario que justificaba el `unwrap_or_else`. La frase «no puede verificar ningún certificado» no se sostenía al terminar de escribirla.

## 6. El agente ya usa lo que verifica

`resumir_veredictos` construía `marcado: None` por decreto, con el comentario «es lo que el agente ve hoy». Ya no: el marcado sale del inventario firmado si lo hay, y **un fallo de esa fuente declarativa escala** en lugar de leerse como ausencia (RPT-010 §4). El resumen distingue ahora cuántos dispositivos llevan marcado firmado.

El agente acepta `--almacen <ruta>` y anuncia su estado de arranque con dos avisos distintos: acción administrativa frente a respuesta a incidente.

## 7. Lo que este reporte no resuelve

1. **Nadie genera las claves.** `aprovisionar_clave` escribe el fichero, pero el par sale de `generar_par` y no hay herramienta que lo produzca ni custodie la privada. Es PA-48, y ahora tiene una interfaz concreta contra la que trabajar: producir estos dos ficheros.
2. **El aprovisionamiento no está instrumentado.** Copiar dos ficheros a un directorio es el procedimiento entero, y no hay nada que verifique que se hizo bien salvo arrancar y mirar el estado. Para un despliegue en planta eso es poco, y se anota en **PA-51**.
3. **La rotación de la clave del cliente sigue sin escribirse** (PA-50).

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-51** | **Procedimiento e instrumentación del aprovisionamiento.** Hoy son dos ficheros copiados a mano sin comprobación | Despliegue repetible en planta |
| PA-49 | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 24 — Clave aprovisionada en el agente · PremosCorp · 6 de agosto de 2026*
