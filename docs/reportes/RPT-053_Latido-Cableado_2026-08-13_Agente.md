# RPT-053 — El latido cableado, y el décimo mecanismo sin cablear

**Tema:** PA-104 implementado en el ciclo. Cuatro estados, el reloj de pared, y una condición que llevaba desde PA-69 sin llegar al SIEM
**Nº de reporte:** 053
**Fecha:** 13 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado.** PA-104 quedó abierto aquí por decisión
de RPT-052 §6, y **se cerró por observación en RPT-057 §4** el mismo día, cuando
existió quien vigilara el latido. El §7 de este reporte se lee con esa fecha
delante: describe lo que era cierto al escribirlo.

- **Depende de:** RPT-052 (diseño del latido), RPT-038 (sello), RPT-044 (evidencia en riesgo), RPT-047 (degradación)
- **Aborda:** PA-104 (emisión). Descubre y cierra un defecto de PA-69

---

## 1. Qué se implementó

Los cinco puntos ratificados de RPT-052 §7, salvo el que por diseño no se cierra
con código:

| Punto de RPT-052 | Estado |
|---|---|
| §7.1 — el latido es el sello con cadencia, no un canal nuevo | Hecho: `Emisor::latir` junto a `Emisor::sellar` |
| §7.2 — lleva las condiciones vigentes | Hecho: `condiciones=` con los nombres activos |
| §7.3 — late en calma y degradado | Hecho, con prueba para cada caso |
| §7.4 — el intervalo viaja en el latido | Hecho: `intervaloMs=`. El **valor** sigue siendo hipótesis (PA-41) |
| §7.5 — PA-104 se cierra por observación | **Sin tocar.** Ver §7 |

`latir` vive **al lado** de `sellar`, no en su lugar. `sellar` calla cuando el
extremo no cambia, y está escrito así a propósito (RPT-032 §3): en un sensor
tranquilo el extremo no cambia nunca. Las dos conductas son correctas en su
contexto; cambiar una por otra habría arreglado PA-104 rompiendo PA-64.

## 2. Cuatro estados, porque tres de ellos suenan igual

`latir` devolvía un `bool`. Con él, «latí» y «todavía no tocaba» valían ambos
`true`, y el resumen por pantalla habría dicho **«latido enviado: sí»** en una
vuelta que no envió nada.

```
Latido::Emitido      salió al colector
Latido::NoTocaba     el intervalo no había vencido. Es lo normal
Latido::SinColector  este agente no tiene colector configurado
Latido::NoSePudo     tocaba latir y el despacho falló
```

Desde fuera del proceso, los tres últimos producen exactamente lo mismo:
**ninguna línea**. Por dentro, uno es funcionamiento normal, otro es una decisión
de despliegue y el tercero es un sensor mudo. Es RPT-006 §4 en su forma habitual:
*no se sabe* no es *no hay*, y *no toca* no es *no puedo*.

`SinColector` merece su variante propia porque es la única que se puede leer mal
en las dos direcciones:

- tomarlo por avería encendería `salidaNoDisponible` **de forma permanente** en
  todo agente sin colector;
- tomarlo por normalidad ocultaría que ese sensor **no está cubierto por
  PA-105** y que nadie fuera notará si se apaga.

Sólo `NoSePudo` cuenta como salida caída.

## 3. El reloj que retrocede dejaba al sensor mudo

El reloj que llega al ciclo es de pared, no monótono. La guarda del intervalo era

```rust
ahora_ms.saturating_sub(ultimo) < intervalo_ms
```

Un ajuste horario o un `ntpd` corrigiendo hacia atrás da un transcurrido
**negativo**, que esa comparación lee como «acabo de latir». El agente se
quedaría callado todo lo que durase el salto —horas, si el salto es de horas—
mientras la sala lo da por muerto. Un fallo de disponibilidad inventado por una
corrección de hora.

La guarda ahora exige que el transcurrido sea **positivo además de corto**:

```rust
(0..intervalo_ms).contains(&ahora_ms.saturating_sub(ultimo))
```

Ante la duda se late de más. Un latido sobrante es ruido; uno que falta es una
llamada de madrugada.

## 4. El décimo mecanismo sin cablear: `evidenciaEnRiesgo` nunca llegó al SIEM

Esto no estaba en el plan. Apareció al decidir qué condiciones viajan en el
latido, que obligó a preguntarse cuáles son **las vigentes** en ese punto del
ciclo.

`condiciones()` devuelve `evidenciaEnRiesgo` apagada por construcción —lo
documenta: es el resultado de intentar escribir, y quien llama lo rellena—. Y
quien llama lo rellenaba **al construir el `Resultado`, después de emitir**:

```rust
let base = condiciones(...);          // evidenciaEnRiesgo: false, siempre
emisor.emitir(&anexadas, &base, ...); // la transición se calcula aquí
...
Resultado { condiciones: Condiciones { evidencia_en_riesgo: ..., ..base } }
```

La transición se calculaba sobre un campo que **no cambiaba nunca**. La condición
existía, era emisible, tenía prueba propia y tenía canal; la pérdida de evidencia
de PA-69 **jamás salió al SIEM**. Sólo llegaba al panel local, que es
precisamente la consola que RPT-051 §2C dice que un operador de sala puede no
tener.

El campo se completa ahora **antes de emitir**, que es cuando ya se sabe:
`asegurar_durabilidad` corrió más arriba en la misma vuelta. `salidaNoDisponible`
sigue siendo el único que se pospone, y ese no viaja por syslog (RPT-032 §4), así
que el orden no altera lo que sale.

Es el décimo caso de la misma familia en el histórico del proyecto, y el segundo
seguido que **no lo encuentra una prueba**: PA-96 apareció fabricando datos para
otra cosa, y éste apareció al cablear una pieza distinta. Las pruebas cubren lo
que alguien pensó en comprobar; los mecanismos sin cablear se caen fuera de eso
por definición.

## 5. Las pruebas

Seis nuevas sobre el ciclo, no sobre el emisor. La distinción importa: `latir`
aislado ya funcionaba, y lo que faltaba era que **alguien lo llamara**.

| Prueba | Qué sujeta |
|---|---|
| `late_en_calma_aunque_no_ocurra_absolutamente_nada` | El caso entero de PA-104. Sin tramas ni alertas, sale una línea con `condiciones=ninguna` e `intervaloMs=` |
| `no_late_dos_veces_dentro_del_mismo_intervalo` | El borde exacto: un milisegundo antes no toca, en el intervalo sí |
| `un_reloj_que_retrocede_no_deja_al_sensor_mudo` | §3 |
| `el_sensor_ciego_sigue_latiendo_y_lo_dice_en_el_latido` | La que de verdad importa: `capturaNoDisponible` viaja **dentro** del latido |
| `un_latido_que_no_sale_no_marca_el_instante_y_se_reintenta` | Un colector caído no compra un intervalo extra de silencio |
| `sin_colector_no_se_late_y_el_agente_lo_declara` | §2 |
| `la_perdida_de_evidencia_llega_al_siem_y_no_solo_al_panel` | §4 |

`condiciones=ninguna` es una afirmación, no un hueco: dice que se miraron las
ocho condiciones emisibles y ninguna estaba activa. Omitir la lista cuando está
vacía dejaría al colector sin saber si se comprobó.

## 6. Lo que el latido rompió en las pruebas viejas, y por qué está bien que lo rompiera

Seis pruebas existentes empezaron a fallar. Ninguna por un defecto: todas
filtraban lo emitido **por subcadena**, y la línea del latido lleva `sello=` —el
mismo par (asiento, extremo) que el sello— y `condiciones=<nombre>` —los mismos
nombres que las transiciones—.

Ahora discriminan por **identificador de mensaje**: `sello-de-evidencia`,
`latido-de-sensor`, `condicion=`. Es la tercera vez que este proyecto tropieza
con lo mismo: un filtro por subcadena sobre un formato de texto se rompe en
cuanto aparece un mensaje nuevo que comparte un campo. La primera fue en
`las_alertas_anteriores_no_se_reemiten`, donde ya se dejó escrito el motivo.

Que se rompieran es la señal correcta. Un banco de pruebas que hubiera seguido
verde con un mensaje nuevo en el cable no estaba mirando el cable.

## 7. Lo que **no** cierra este reporte

**PA-104 sigue abierto**, y es deliberado. RPT-052 §6 lo dejó dicho antes de
escribir una línea: emitir sin vigilar es peor que no emitir, porque da el punto
por resuelto y deja a la sala igual de ciega con la sensación de estar cubierta.

Hoy el sensor late. **Nadie se da cuenta si deja de hacerlo.** Eso es PA-105, y
hasta que exista, lo construido aquí es la mitad de un mecanismo.

El intervalo sigue siendo `INTERVALO_LATIDO_MS = 60_000`, una **hipótesis
declarada, no una medida** (PA-41), y es el primer parámetro que justifica la
configuración firmada de PA-79.

## 8. Un identificador con dos significados

Al preparar el paso siguiente aparece un defecto de documentación, no de código.

**PA-84 está usado para dos cosas distintas.** Su única definición escrita en
forma de tabla está en `docs/Puesta-en-marcha-local.md` §5:

> | PA-84 | `--grupo-ipc` aceptaría un nombre de grupo y no un número |

RPT-051 y RPT-052 lo usan, en prosa, para **el empaquetado dual**. Nadie lo
notó porque los dos usos viven en documentos distintos y ninguno de los dos es
un registro central.

Se resuelve conservando el significado original —es el más antiguo y el que está
en la guía operativa, que es la que lee quien instala— y asignando **PA-107** al
empaquetado dual. RPT-051 y RPT-052 quedan corregidos con nota visible en lugar
de en silencio: un identificador que cambia de número sin dejar rastro es peor
que el choque.

**La causa es estructural y volverá a pasar.** Los identificadores se acuñan en
el reporte que los descubre, y el registro de RPT-002 §"puntos abiertos" dejó de
actualizarse hacia PA-65. Desde entonces hay más de cuarenta puntos nuevos y
ninguna lista única.

Peor: **PA-101, PA-102 y PA-103 no existen en ningún documento.** Se acuñaron en
sesión de trabajo y sólo PA-101 llegó a implementarse. Los otros dos son trabajo
acordado que hoy no está escrito en ninguna parte, y por tanto no existe. Quedan
recogidos en la tabla de abajo, que es lo único que los salva.

Un índice único de puntos abiertos es **PA-108**.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-104~~ | ✅ **Cerrado por observación** el 13-ago-2026: se apagó el sensor y la sala se enteró. RPT-057 §4 |
| ~~PA-105~~ | ✅ `eje-vigia`, RPT-057 |
| PA-41 | La cadencia sigue sin medirse |
| PA-79 | El intervalo es el primer parámetro que exige configuración firmada |
| PA-100 | El coste en el sensor sigue infiriéndose desde el cliente |
| **PA-106** | El latido no llega a VIS-04. La consola local ve las condiciones por IPC; la del colector las verá por syslog. Son dos caminos con la misma información y ninguna prueba de paridad entre ellos |
| ~~PA-102~~ | ✅ RPT-056 §3 |
| **PA-103** | La rama `noServido` del panel no se ha ejecutado nunca. Ídem |
| **PA-107** | Empaquetado dual: qué lleva el artefacto headless y qué el de diagnóstico (§8) |
| **PA-108** | Índice único de puntos abiertos. Hoy se acuñan en el reporte que los descubre y no hay lista (§8) |

---

*Reporte Nº 53 — El latido cableado · PremosCorp · 13 de agosto de 2026*
