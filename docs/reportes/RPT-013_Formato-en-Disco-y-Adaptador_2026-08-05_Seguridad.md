# RPT-013 — Formato en Disco, Analizador Defensivo y Recorrido de Extremo a Extremo

**Tema:** Camino de entrada del inventario firmado
**Nº de reporte:** 013
**Fecha:** 5 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** Canónico con reservas explícitas — véase §6

- **Depende de:** RPT-012 (frescura), RPT-011 (inventario firmado), RPT-010 (contratos), `eje-ipc` (lección del prefijo de longitud)
- **Cierra:** PA-24
- **Abre:** PA-29
- **Toca:** `motor-pqc`, que gana `FirmaHibrida::desde_bytes`

---

## 1. Por qué este reporte iba antes que la revocación

Cinco reportes —008 a 012— diseñaron un subsistema que **nunca se había ejecutado de extremo a extremo**. Todo lo verificado era de unidad. Seguir levantando torre de verificación sobre un componente sin camino de entrada es una forma cómoda de acumular confianza sin ganarla.

Y había un hueco concreto: **nadie había definido el formato en disco**. `Inventario`, `RaizAnclada` y `FirmaHibrida` eran tipos en memoria. Cómo llegan a un fichero era superficie canónica nueva, sin decidir.

## 2. El analizador es el primer frente, y es código no autenticado

Corre **antes** de que ninguna firma se verifique, sobre un fichero que el modelo de amenazas de RPT-012 asume manipulable. Toda la cadena de cinco eslabones se apoya en que este módulo no se caiga, no reserve memoria a petición del atacante y no admita dos lecturas del mismo fichero.

Disposición:

```text
magico       8 bytes  "EJE-INV1"
version      u16 BE
secuencia    u64 BE
entradas     u32 BE
  ── por entrada, 19 bytes de ancho fijo ──
  mac 6 | clase u8 | emitido_en u64 BE | vigencia_dias u32 BE
firma        longitud fija, ML-DSA-65 + Ed25519
```

### 2.1 La raíz **no** se almacena

Se recalcula a partir de las entradas. Guardarla crearía una pregunta que no debe existir: si la raíz escrita y la recalculada discrepan, ¿cuál vale? **Cualquiera de las dos respuestas es explotable.** Al no escribirla, alterar una entrada cambia la raíz recalculada y la firma deja de verificar — sin decisión que tomar.

`la_raiz_no_viaja_en_el_fichero_sino_que_se_recalcula` comprueba que los 32 bytes de la raíz no aparecen literalmente en el fichero.

### 2.2 Las entradas son de ancho fijo, y no es estilo

Con ancho fijo, el número declarado de entradas se valida **contra los bytes que quedan** antes de reservar nada. Con ancho variable habría que recorrer la lista para saber si cabe, y ese recorrido ya es trabajo a petición del atacante.

Es la misma lección de `eje-ipc`: un prefijo que declare cuatro gigabytes no debe provocar una reserva de cuatro gigabytes. Aquí el ataque equivalente son veintidós bytes que declaran `u32::MAX` entradas; `un_numero_de_entradas_absurdo_no_reserva_memoria` lo cubre.

### 2.3 Los bytes sobrantes se rechazan

Un fichero cuya cola no se interpreta admite dos lecturas: la del analizador y la de quien añadió los bytes. Misma clase de ambigüedad que `deny_unknown_fields` cierra en el contrato IPC.

### 2.4 Un código de clase desconocido se rechaza

Leerlo como «no crítico» daría al atacante una vía de degradación mediante un byte que el analizador no entiende. El rechazo es la ruta por defecto, coherente con el resto del proyecto.

### 2.5 Una versión futura se rechaza en lugar de interpretarse

Interpretar un formato que no se conoce es adivinar sobre entrada hostil.

## 3. El adaptador

`InventarioLocal::cargar` analiza y cierra los cinco eslabones **una vez**. Cada consulta posterior sólo construye la prueba de inclusión y comprueba los dos que dependen del marcado concreto.

Verificar la firma en cada consulta sería más lento y, peor, invitaría a saltársela «por rendimiento» en algún camino. Un `InventarioLocal` que existe es un inventario que ya pasó por todo.

Las dos capas de fallo se distinguen en el tipo: `ErrorCarga::Formato` para lo estructural —detectado antes de tocar criptografía— y `ErrorCarga::Verificacion` para firma, dominio y frescura. `alterar_una_entrada_del_fichero_invalida_la_firma` comprueba que un fichero alterado sigue **bien formado** y falla en la capa correcta.

Y la distinción que RPT-010 §4 exigía: **un dispositivo ausente del fichero devuelve `Ok(None)`, no un error.** La ausencia es legítima; el fallo de verificación no.

## 4. Añadido a `motor-pqc`

`FirmaHibrida::desde_bytes` y `longitud_serializada`. La longitud se **deriva del tipo** en lugar de escribirse a mano: una constante copiada se desincroniza en silencio si el parámetro cambia.

`desde_bytes` exige longitud **exacta**. Aceptar una entrada más larga y quedarse con el prefijo dejaría bytes sin interpretar en un dato que llega de un fichero manipulable.

## 5. Verificación

`crates/guardian-cc` pasa de 48 a **62 pruebas**; el workspace, de 159 a **173**.

`el_recorrido_completo_de_fichero_a_veredicto` es la prueba que faltaba desde RPT-008: serializa, escribe, carga, verifica, clasifica y evalúa, terminando en `Veredicto::Prohibida` con `es_amenaza_incontenible()`.

Siete mutaciones, todas atrapadas:

| Mutación | Prueba que falla |
|---|---|
| No se acota el número de entradas antes de reservar | `un_numero_de_entradas_absurdo_no_reserva_memoria` |
| Se admiten bytes sobrantes | `los_bytes_sobrantes_se_rechazan` |
| Un código de clase desconocido se lee como no crítico | `un_codigo_de_clase_desconocido_se_rechaza` |
| No se comprueba el mágico | `un_magico_ajeno_se_rechaza` |
| Se acepta cualquier versión | `una_version_futura_se_rechaza_en_lugar_de_interpretarse` |
| La raíz se escribe en el fichero | seis pruebas, incluida `la_raiz_no_viaja_en_el_fichero_sino_que_se_recalcula` |
| El adaptador confunde ausencia con fallo | `un_dispositivo_ausente_del_fichero_no_es_un_fallo` |

`un_fichero_vacio_o_minusculo_no_desborda` barre las 22 longitudes por debajo de la cabecera, que es donde vive el desbordamiento de índice si el analizador confía en que hay bytes.

## 6. Reservas explícitas

1. **Nada lee del sistema de ficheros.** `InventarioLocal::cargar` recibe `&[u8]`. Quién abre el fichero, con qué permisos, qué ocurre si no existe y cómo se escribe de forma atómica —para que un corte de energía no deje medio inventario— no está resuelto. Es PA-29.
2. **El centinela no se persiste.** `Centinela` se pasa como parámetro. Dónde vive entre arranques es parte de PA-28, y `InventarioLocal::secuencia()` existe para avanzarlo, pero nadie lo avanza todavía.
3. **`ENTRADAS_MAXIMAS = 200_000` y `LONGITUD_MAXIMA = 8 MiB` no están fundamentados en medición.** Son cotas holgadas puestas para que el límite exista. El inventario de un hospital grande debería medirse.
4. **No hay pruebas de fuzzing.** Las negativas del analizador son casos elegidos por quien escribió el analizador, que es exactamente el sesgo que el fuzzing corrige. Para un analizador de entrada hostil, esto es una carencia real y no una preferencia.
5. **`FicheroInventario` no deriva `Debug`.** `FirmaHibrida` no lo implementa, y añadírselo pondría material criptográfico en los registros de depuración. La consecuencia es que las pruebas comparan con `.err()` en lugar de con el `Result` completo.

La reserva 4 es la que más pesa: un analizador de entrada hostil sin fuzzing está probado contra la imaginación de su autor.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-29** | **Acceso al sistema de ficheros y escritura atómica.** Rutas, permisos, ausencia del fichero, escritura sin dejar estado a medias, y fuzzing del analizador | Despliegue real |

---

*Reporte Nº 13 — Formato en Disco, Analizador Defensivo y Recorrido de Extremo a Extremo · PremosCorp · 5 de agosto de 2026*
