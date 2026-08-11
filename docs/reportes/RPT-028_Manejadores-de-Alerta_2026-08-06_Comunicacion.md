# RPT-028 — Manejadores de alerta

**Tema:** Que lo que el agente detecta llegue a alguien
**Nº de reporte:** 028
**Fecha:** 6 de agosto de 2026
**Área designada:** Comunicación
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-43

- **Depende de:** RPT-019 (contrato de alertas), RPT-022 y RPT-024 (estados nuevos de arranque)
- **Cierra:** PA-43
- **Amplía:** `Condiciones` de cuatro campos a cinco

---

## 1. Los manejadores viven en el agente, no en `guardian-cc`

`SucesoAlerta` está en `eje-ipc` —es el contrato de cable— y `RegistroEvidencia` en `eje-almacen`. `guardian-cc` no conoce ninguno de los dos: **decide, no comunica**.

El agente es el único que tiene las dos mitades. Meter la traducción en `guardian-cc` habría obligado a que la biblioteca de decisión dependiera del formato de cable, y con eso un cambio de la interfaz podría arrastrar a la lógica de contención.

## 2. El hallazgo: dos estados que alertan y no tenían por dónde salir

Al escribir `condiciones()` apareció que **`FormatoObsoleto` y `SinClaveAprovisionada` no cabían en ningún sitio**.

Los dos devuelven `exige_alerta() == true`. Ninguno es un suceso —no ocurren, **son**— y ninguno encajaba en las cuatro condiciones que RPT-019 definió, porque RPT-019 es anterior a los dos. Así que el agente los habría calculado y descartado en silencio: exactamente el defecto que PA-43 existía para cerrar, una capa más arriba.

`Condiciones` gana `accionAdministrativa`, declarado en los seis sitios que la fricción de RPT-006 exige. Y se **deriva** en lugar de enumerar variantes:

```rust
accion_administrativa: estado.exige_alerta() && !estado.es_manipulacion(),
```

Si mañana aparece un tercer estado con ese perfil, llega solo. Enumerar `FormatoObsoleto | SinClaveAprovisionada` habría vuelto a dejar fuera al siguiente.

`Condiciones` gana también `hay_manipulacion()`, por la misma razón que `EstadoArranque` separó `es_manipulacion` de `exige_alerta`: VIS-04 debe poder presentar distinto «hay que reemitir el inventario» y «alguien borró el inventario». Presentarlos igual produce la fatiga que la Fase 1 de PA-45 existía para evitar.

## 3. La conversión es de una sola dirección, y su garantía es más débil de lo que parece

RPT-019 §7.3 dejó escrito que `SucesoAlerta::asiento` **no es un dato del suceso** —lo asigna ALM-01 al anexar— y que la conversión debe ir del asiento al DTO y nunca al revés. Sólo existe `suceso_desde`.

Pero conviene ser exacto sobre qué garantiza eso. `SucesoAlerta` tiene campos públicos porque serde los necesita, así que **cualquiera puede construir uno con un asiento inventado**. No es como `MarcadoVerificado`, cuya invariante vive en el tipo.

La garantía real es de este módulo: el agente no fabrica asientos. Lo que la sostiene es una prueba —`todo_suceso_devuelto_corresponde_a_un_asiento_real`— que comprueba la propiedad en lugar de confiar en el nombre.

## 4. Sólo una clase de evento se comunica

La correspondencia entre `ClaseEvento` y `ClaseAlerta` es explícita y **no exhaustiva a propósito**: añadir una clase a ALM-01 no debe convertirla en alerta por omisión.

Comunicar de más es la otra cara de la fatiga. Un operador que recibe diez avisos rutinarios por cada uno real deja de mirarlos, y entonces el canal está peor que vacío — porque parece que funciona.

## 5. La consulta lleva cota

`SUCESOS_POR_CONSULTA = 256`. Sin cota, pedir el registro entero chocaría contra el límite de marco de `eje-ipc` y el consumidor **no recibiría nada** en lugar de recibir un lote. Quien quiera más continúa desde el último asiento, que es justo para lo que existe `desdeAsiento`.

Y `desdeAsiento` es **exclusivo**: quien pide «desde el 3» ya tiene el 3. Incluirlo haría que un consumidor que continúa donde lo dejó viera la misma alerta dos veces, y una alerta repetida enseña a ignorarlas.

## 6. La dirección se presenta como una MAC

`{:02x?}` produce `[00, 1b, 21, ...]`, que no es lo que un operador reconoce. Es un detalle de una línea y va en un reporte porque **una alerta que no se entiende no sirve**, y el resto de este documento trata de que las alertas sirvan.

## 7. Lo que sigue sin resolverse

1. **El canal no está cableado.** Los manejadores existen y responden; nadie los invoca desde IPC porque el bucle de servicio del agente no existe todavía. Hoy el agente imprime por pantalla lo que respondería. Decirlo así es peor que entregarlo y mucho mejor que calcularlo y tirarlo.
2. **PA-41 sigue abierto**: cuánto puede tardar VIS-04 en enterarse. Un canal de consulta sin cadencia declarada no tiene latencia acotada.
3. **PA-42 sigue abierto**: nada sale del equipo. En un armario de planta sin operador delante, una alerta que sólo vive en el registro local no es una alerta.
4. **El registro es en memoria.** `RegistroEvidencia` se construye en cada ejecución del recorrido. La persistencia en libSQL está diseñada (ALM-01) y no cableada, así que las alertas de una ejecución no sobreviven a la siguiente.

El punto 4 es el que más se parece a algo terminado sin estarlo.

## 8. La fricción de RPT-006 cobró su factura, que es lo que se le pide

Añadir el quinto campo rompió la compilación de `eje-ipc` en tres sitios, todos dentro de `las_constantes_estan_atadas_a_los_structs`. No es un inconveniente del cambio: **es el mecanismo funcionando.**

Esa prueba desestructura `Condiciones` campo por campo en lugar de usar `..`, precisamente para que ampliar el registro no pueda pasar sin tocarla. Si hubiera usado `..`, el campo nuevo habría entrado con `CAMPOS_CONDICIONES` desactualizado y la paridad con el manifiesto se habría roto en silencio — que es el defecto que los seis sitios existen para impedir.

`las_condiciones_distinguen_lo_degradado_de_lo_normal` gana el caso del campo nuevo, y se añade `la_manipulacion_no_se_confunde_con_la_accion_administrativa`: las tres condiciones que **no** acusan a nadie —acción administrativa, saturación y pérdida— son límites del propio agente, no huellas de un tercero.

### 8.1 La suite de TypeScript no se invoca con `npm test`

Este proyecto nombra sus tareas en español, como todo lo demás:

```
npm run verificar   # tipos + frontera + frontera negativa + pruebas
npm run probar      # solo las pruebas
```

Queda escrito aquí porque la instrucción equivocada ya se dio una vez y `npm error Missing script: "test"` no dice cuál es la correcta.

## 9. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-56** | **Persistencia del registro de evidencia.** Hoy las alertas mueren con el proceso | Que una alerta sobreviva a un reinicio |
| PA-43 | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 28 — Manejadores de alerta · PremosCorp · 6 de agosto de 2026*
