# RPT-037 — El ciclo, fuera del `main` y bajo prueba

**Tema:** Extraer el bucle de servicio a la biblioteca para poder ejercitar N vueltas
**Nº de reporte:** 037
**Fecha:** 6 de agosto de 2026
**Área designada:** Arranque
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-68, abre PA-69

- **Depende de:** RPT-034 (diseño del servicio), RPT-036 (bucle e implementación)
- **Cierra:** PA-68
- **Abre:** PA-69

---

## 1. Qué se movió, y qué se quedó

El cuerpo de la vuelta pasa de `crates/eje-agente/src/main.rs` a
`crates/eje-agente/src/ciclo.rs`. En `main.rs` queda únicamente lo que exige una
tarjeta de red de verdad —abrir la captura, esperar tramas, leer las estadísticas
del núcleo— y lo que exige una consola: presentar.

El corte no es estético. `main.rs` es el único fichero del workspace sin pruebas,
y no por descuido: **no se puede probar lo que sólo se ejecuta con `CAP_NET_RAW` y
una NIC en modo promiscuo**. Todo lo que viva ahí es, por construcción, código sin
red de seguridad.

## 2. Las dos costuras que hacen probable el ciclo

**La captura sale del ciclo.** `Ciclo::vuelta` recibe un `&[Observacion]` en lugar
de observar. Es la misma disciplina que ya aplicaban `Despacho` (RPT-032) y
`Atiende` (RPT-035): el I/O detrás de un parámetro.

**El reloj es un parámetro.** `ahora_s` y `ahora_ms` se pasan; dentro del módulo no
hay ninguna llamada a `SystemTime::now`. Esto es lo que convierte el defecto de
RPT-036 §3 en algo que ya no puede ocurrir por descuido: congelar el reloj ahora
exige **pasar el mismo valor dos veces a propósito**, que es una decisión visible
en el sitio donde se toma.

## 3. Lo que apareció al hacerlo

Extraer el ciclo destapó un segundo defecto de exactamente la misma familia que el
reloj congelado.

`main.rs` calculaba las alertas a emitir así:

```rust
let sucesos = consultar(registro, &PeticionAlertas { desde_asiento: 0 });
```

**En cada vuelta.** Con `--ciclos 1` es correcto: hay una vuelta y las alertas de
esa vuelta son todas las que hay. En servicio continuo, el emisor reenvía al SIEM
del cliente **el historial completo de alertas una vez por ciclo, indefinidamente**.
Un sensor con veinte alertas acumuladas y un ciclo de un minuto le entrega al
colector veintiocho mil ochocientas entradas al día, todas duplicadas.

Es el mismo error que el reloj: código escrito para ejecutarse una vez,
ejecutándose muchas. Y del mismo modo, ninguna prueba podía verlo, porque la
prueba que lo ve necesita dos vueltas.

La marca de agua se toma ahora del **número del último asiento** antes de anexar, y
no de la longitud del registro. Cuando PA-59 introduzca la poda, longitud y número
dejarán de coincidir, y consultar por longitud volvería a reemitir alertas viejas.

## 4. La cota de `consultar` protege el canal, no la salida

`consultar` acota a `SUCESOS_POR_CONSULTA = 256` para que la respuesta quepa en un
marco de IPC. Al reutilizarla para la emisión, esa cota se convertía en un límite
de **cuántas alertas de una misma vuelta llegan a salir**: a partir de la 257, la
marca de agua de la vuelta siguiente pasaría por encima de ellas y no saldrían
nunca.

Se resuelve consultando por lotes hasta agotar. El bucle progresa porque
`consultar` filtra por `> desde_asiento` estrictamente.

**Esto no está cubierto por una prueba, y conviene decirlo en lugar de dejarlo
implícito.** Producir más de 256 amenazas incontenibles en una sola vuelta exige un
inventario firmado con 257 dispositivos de clase excluida, y `eje-agente` no puede
construirlo: con `PrimerArranque` ningún veredicto es `Prohibida`. La corrección es
correcta por lectura, no por verificación — que es el tercer estado de RPT-006 §4
aplicado a mi propio trabajo.

## 5. PA-69 — la evidencia en riesgo no tiene por dónde salir

Si una vuelta anexa alertas y `persistir` falla, esas alertas existen sólo en
memoria. `Resultado::evidencia_en_riesgo()` lo expone y `main.rs` lo imprime, pero
**ninguna de las seis condiciones lo dice**, así que VIS-04 no puede enterarse y el
SIEM tampoco.

No se resuelve aquí. Añadir un séptimo campo a `Condiciones` toca los seis sitios
del contrato (RPT-011), y hacerlo de paso mientras se cierra otro punto es
justamente cómo se cuelan los cambios que nadie revisa. Queda como **PA-69**.

## 6. Las siete pruebas, y qué defecto vigila cada una

| Prueba | Qué impide que vuelva |
|---|---|
| `el_almacen_recuerda_entre_vueltas_y_no_se_reinicia` | Que alguien recree el almacén por vuelta y se pierda la ambigüedad pegajosa de RPT-010 §5 |
| `las_alertas_anteriores_no_se_reemiten_en_cada_vuelta` | El defecto del §3 |
| `una_condicion_estable_se_emite_una_sola_vez_en_muchas_vueltas` | RPT-032 §3 estaba probado sobre `transiciones()` en aislamiento, nunca a través del bucle — que es donde la fatiga de alertas se produce |
| `el_reloj_de_la_vuelta_llega_al_cable_y_no_se_queda_en_el_de_la_primera` | El defecto de RPT-036 §3, en su forma observable sin inventario firmado |
| `sin_alertas_no_se_escribe_el_disco_por_muchas_vueltas_que_den` | RPT-034 §1.1; diez vueltas tranquilas no deben crear el fichero |
| `el_registro_cargado_del_disco_continua_su_serie_a_traves_del_ciclo` | Que el ciclo reinicie una serie que el registro sabe continuar |
| `el_estado_administrativo_se_declara_en_cada_vuelta_y_no_solo_en_la_primera` | Que una condición derivada se convierta en una condición guardada |

## 7. Lo que sigue sin resolverse

1. **La cadencia sigue sin medir** (PA-41). Nada de esto la mide.
2. **El lote de más de 256 no está probado** (§4).
3. **La evidencia en riesgo no tiene canal** (PA-69).
4. **Sin `systemd`** (PA-65): el binario corre en bucle, pero nadie lo lanza ni lo reinicia.
5. **El agente sigue sin contener nada** (PA-22).
6. **Las condiciones se imprimen en cada vuelta.** En un demonio eso es ruido. Es la lección de RPT-032 §3 aplicada a la salida por pantalla, y sigue pendiente.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-69** | **La evidencia en riesgo no tiene canal.** Alertas anexadas que no llegaron al disco no aparecen en ninguna condición | Que el operador sepa que perdió evidencia |
| PA-68 | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 37 — El ciclo, fuera del `main` y bajo prueba · PremosCorp · 6 de agosto de 2026*
