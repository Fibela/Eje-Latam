# RPT-029 — Persistencia del registro de evidencia

**Tema:** Que una alerta sobreviva a un reinicio
**Nº de reporte:** 029
**Fecha:** 6 de agosto de 2026
**Área designada:** Evidencia
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-56

- **Depende de:** RPT-028 (manejadores), RPT-006 §4 (principio triestático), RPT-013 (por qué no se almacenan los resúmenes)
- **Cierra:** PA-56

---

## 1. El hueco

RPT-028 cableó los manejadores y dejó el registro **en memoria**. Un sensor se reinicia por una actualización, por un corte de luz o porque alguien lo reinicia, y con él se iba la única constancia de que hubo una amenaza incontenible.

Es el mismo patrón de la sesión entera: mecanismo completo, sin la pieza que lo hace útil. La diferencia es que esta vez estaba anotada antes de escribirla.

## 2. Los resúmenes no se almacenan; la numeración sí

**Ni el resumen propio ni el del anterior.** Mismo argumento que el inventario firmado (RPT-013): guardarlos crearía una pregunta sin respuesta segura —si el escrito y el recalculado discrepan, ¿cuál vale?— y ambas respuestas son explotables. La cadena se **reconstruye** al cargar, encadenando cada asiento igual que hizo el agente al anexarlo.

Pero el **número de asiento sí se guarda**, y también es derivable. Ahí está el motivo:

> Si no se guardara, **borrar un asiento intermedio pasaría desapercibido**, porque la reconstrucción renumeraría los supervivientes y la cadena cuadraría consigo misma.

Con el número escrito, la reconstrucción compara lo que asigna con lo que el fichero declara y la supresión sale como `NumeracionAlterada`. Es la lección de RPT-010 §4 en otro sitio: firmar entrada por entrada no protege contra la supresión, y aquí el papel de la firma lo hace la numeración consecutiva.

### 2.1 Lo que esto no detecta

Alterar el **último** asiento sin cambiar longitudes produce un registro que se reconstruye y verifica consigo mismo. Lo que cambia es su extremo, así que **sólo lo detecta quien conserve el extremo anterior** — y hoy nadie lo conserva.

Está probado como lo que es, no como lo que quisiéramos: `alterar_el_detalle_de_un_asiento_se_detecta` afirma que la cadena sigue siendo coherente y que el extremo difiere. La detección real exige anclar el extremo fuera del fichero, que es el mismo problema del centinela de PA-28 y se registra como **PA-57**.

## 3. Tres estados al cargar, y ninguno se colapsa

RPT-006 §4 aplicado a la evidencia:

| Estado | Qué ocurrió | Qué hace el agente |
|---|---|---|
| `Conforme` | verifica, o el fichero no existe | continúa la serie |
| `Truncado` | corte de energía durante la escritura | continúa, avisando |
| `ViolacionDetectada` | alguien tocó el fichero | no lo toca, empieza otro |

**Truncado no es violación.** Un corte de luz durante la escritura es lo esperable; colapsarlo en «alguien tocó esto» haría que cada apagón pareciera una intrusión, que es la fatiga contra la que lleva media sesión escribiéndose.

**Ausente no es violación.** A diferencia del inventario, aquí **no hay centinela** que atestigüe que hubo algo antes. Inventar uno sería cómodo y equivocado: afirmar manipulación sin testigo es acusar sin pruebas.

## 4. Las dos decisiones difíciles

**Ante una violación no se carga nada, ni siquiera lo que sobrevivió.** Cargar los asientos que sí verificaban dejaría que **quien borró evidencia eligiera qué se conserva**, y el operador vería un registro que parece íntegro. Un registro vacío y una alerta son honestos; un registro parcial sin marcar, no.

**El fichero dañado no se borra ni se sobrescribe.** Es lo que más importa de este reporte. Un registro que no verifica es evidencia de que alguien intervino, y **esa evidencia vale más que la que contiene**. Quien lo pisara para «arrancar limpio» destruiría la única prueba de la manipulación. Se aparta con el instante en el nombre para que dos incidentes no se pisen.

## 5. Se reescribe entero en lugar de anexar

Anexar sería más barato y abre una ventana: un corte a mitad de la escritura deja un asiento parcial en la cola de un fichero por lo demás válido. Con reescritura atómica el fichero es siempre el de antes o el de después.

El coste es lineal en el tamaño del registro y se paga en cada alerta. Es asumible mientras las alertas sean lo que son —raras y graves— y **deja de serlo el día que se anexe cada trama**. Queda escrito para ese día.

## 6. Lo que sigue sin resolverse

1. **El agente no lo usa todavía.** El módulo carga, valida y aparta; el recorrido de `eje-agente` sigue construyendo el registro en memoria en cada ejecución. Cablearlo exige decidir dónde vive el fichero dentro de `RutasAlmacen` y es media hora de trabajo, no un diseño. Se registra como **PA-58** para no cerrarlo por decreto.
2. **El extremo no está anclado** (PA-57, §2.1).
3. **No hay retención.** El registro crece sin límite hasta `ASIENTOS_MAXIMOS`, y llegar ahí lo vuelve ilegible en lugar de rotarlo. La Bóveda tiene política de retención (30 días / 5 GB) y esto no.
4. **libSQL sigue sin usarse.** ALM-01 se diseñó sobre él; esto es un fichero plano. Para el volumen de las alertas basta, y decir «ALM-01 implementado» seguiría siendo falso.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-57** | **Anclaje del extremo de la cadena fuera del fichero.** Sin él, alterar el último asiento no se detecta | Detección de manipulación del último asiento |
| **PA-58** | **Cablear la persistencia en el recorrido del agente** | Que PA-56 tenga efecto |
| PA-59 | Política de retención y rotación del registro | Operación más allá de `ASIENTOS_MAXIMOS` |
| PA-56 | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 29 — Persistencia del registro de evidencia · PremosCorp · 6 de agosto de 2026*
