# RPT-040 — Segmentación del registro de evidencia

**Tema:** Cómo se corta el registro en segmentos sin romper la cadena, el ancla ni el sello
**Nº de reporte:** 040
**Fecha:** 8 de agosto de 2026
**Área designada:** Evidencia
**Entidad:** PremosCorp
**Estado:** **Ratificado e implementado.** Cierra PA-59 (vía C), abre PA-74

- **Depende de:** RPT-029 (persistencia), RPT-033 (ancla), RPT-038 (testigo), RPT-039 (vía C ratificada)
- **Aborda:** PA-59
- **Abriría:** PA-74

---

## 1. El hallazgo que simplifica todo: el ancla no cambia al rotar

Empecé asumiendo que rotar exigía mover tres ficheros —el segmento, su ancla y el
activo nuevo— y que cualquier corte de energía entre medias dejaría al arranque
siguiente acusando de manipulación. Estuve un rato buscando un orden seguro.

No hace falta. Si el segmento nuevo arrastra como génesis el extremo del anterior,
entonces **el ancla que describe el final del segmento cerrado describe también,
palabra por palabra, el estado inicial del segmento nuevo**: mismo número de
asiento, mismo extremo. Un segmento recién abierto está vacío, y lo último que
consta sigue siendo el asiento *N* del anterior.

De ahí que la rotación sean dos escrituras y no cuatro:

```text
1. escribir  evidencia-000001.alm   (el segmento que se cierra, completo)
2. sustituir evidencia.alm          (vacío, base = N+1, genesis = extremo)
```

Y el ancla **no se toca**. Un corte entre 1 y 2 deja el activo intacto y la
rotación se reintenta; un corte después de 2 deja un activo vacío que el ancla
vieja valida sin objeción.

Esto no es elegancia por gusto: la mitad de los defectos de esta semana han
salido de secuencias de escritura con estados intermedios. Una que no los tiene no
puede tenerlos.

## 2. El formato: dos campos nuevos y una versión que no rompe nada

Cabecera actual: mágico(8) + versión(2) + asientos(4) = 14 bytes.
Cabecera propuesta: mágico(8) + versión(2) + **base(8)** + **génesis(32)** + asientos(4) = 54.

`verificar_cadena` pasa de exigir `numero == indice + 1` a `numero == base + indice`,
y el primer asiento enlaza con `génesis` en lugar de con `Resumen::GENESIS`.

**Un fichero de la versión 1 se lee sin ceremonia**, y esto importa: es evidencia
real de un cliente. Un registro v1 *es* un segmento con `base = 1` y
`génesis = GENESIS`; no hay que migrarlo, hay que interpretarlo. Se lee como tal y
se reescribe en v2 la primera vez que se anexe algo.

Rechazarlo como formato desconocido lo habría convertido en `ViolacionDetectada`
—una acusación de manipulación por haber actualizado el agente—, que es
exactamente el error de RPT-039 §1 con otro disfraz.

## 3. El umbral, y por qué no es el que parece

Pensaba justificar el tamaño de segmento por granularidad de purga. Es un
argumento real pero secundario. El que manda es otro:

**RPT-029 §5 reescribe el fichero entero en cada persistencia.** Con segmentos de
500 000 asientos eso son unos 100 MB reescritos por cada alerta anexada. El tamaño
de segmento no es una preferencia de organización: es **la cota del coste de
escritura**.

Propongo `ASIENTOS_POR_SEGMENTO = 10_000`. A unos 200 bytes por asiento son ~2 MB
por reescritura, y da granularidad de purga fina para cuando llegue la vía B.

Es una **hipótesis de diseño, no una medida**: la cadencia real de eventos es
PA-41 y sigue sin medir porque depende de PA-40. La constante queda declarada y
revisable, y este párrafo existe para que nadie la lea como calculada.

`ASIENTOS_MAXIMOS = 500_000` se queda como techo duro del formato. Con rotación no
debería alcanzarse nunca, y justamente por eso la comprobación de PA-72 sigue
haciendo falta: las cotas que nunca se tocan son las que se rompen en silencio.

## 4. Qué verifica qué

| Se altera… | Lo detecta… |
|---|---|
| El segmento activo, en el medio | La cadena (`verificar_cadena`) |
| El segmento activo, el último asiento | El ancla (RPT-033) |
| Un segmento archivado, en cualquier punto | El génesis del segmento siguiente, que ya no cuadra |
| El conjunto entero, con el agente parado | El testigo externo (RPT-038) |

**Los segmentos archivados no llevan ancla propia y no la necesitan**: el génesis
del siguiente cumple ese papel, y añadir una sería un fichero más que mantener
coherente sin ganar nada.

## 5. PA-74 — la consulta después de rotar deja de ser completa

Ésta no la había previsto y es la única parte del diseño que me incomoda.

El agente cargará **sólo el segmento activo**: cargar el histórico entero
reintroduciría el consumo de memoria que la segmentación existe para acotar. Pero
`consultar` responde con lo que tiene, y un cliente que pida `desdeAsiento: 0`
después de una rotación recibirá las alertas del segmento activo **y creerá que
son todas**.

Es la lección de RPT-036 §6 otra vez: «no hay nada» y «esto ya no lo tengo aquí»
no son lo mismo, y colapsarlas es cómo un operador concluye que un incidente no
ocurrió.

No lo resuelvo en este reporte porque la solución honesta toca el contrato IPC
—hace falta que la respuesta pueda decir «incompleta desde el asiento *X*»— y
acabamos de pagar ese peaje dos veces esta semana. Queda como **PA-74**, y con él
anotado la vía C se puede implementar sin que el hueco quede sin nombre.

Mientras tanto el agente no empeora: hoy tampoco tiene histórico, porque hoy no
rota.

## 6. Lo que propongo ratificar

1. La rotación de dos pasos del §1, con el ancla intacta.
2. La cabecera del §2, con lectura transparente de la versión 1.
3. `ASIENTOS_POR_SEGMENTO = 10_000` como constante declarada y revisable, con el
   motivo del §3 escrito en el código y no sólo aquí.
4. Sin ancla por segmento archivado (§4).
5. **PA-74 abierto** antes de escribir la primera línea, no después (§5).

## 6.1. Lo que faltaba y no estaba en el diseño

Al escribir las pruebas apareció que **§4 no tenía código detrás**. El reporte
afirmaba que alterar un segmento archivado se detecta porque el génesis del
siguiente deja de cuadrar, y esa comprobación no existía en ninguna parte: la
propiedad era cierta y no la ejecutaba nadie, que es la única forma que tiene una
garantía de no existir.

Se añade `RegistroEvidencia::continua_a`, y dos pruebas la ejercitan leyendo del
disco lo que la rotación acaba de escribir.

## 6.2. Lo que sigue sin poder probarse

**El corte de energía entre los dos pasos.** Hace falta inyectar un fallo de
escritura y la costura para eso no existe: `escribir_atomico` es una función
concreta, no un rasgo. Es un cambio limpio —el mismo patrón de `Despacho` y
`Atiende`— pero es otro punto, no éste.

La propiedad que hace segura la rotación **sí** está probada
(`el_ancla_anterior_sigue_valiendo_para_el_segmento_recien_abierto`): lo que falta
es ejercitar el corte, no la invariante que lo hace inofensivo.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-74** | **Una consulta tras rotar parece completa y no lo es.** El contrato IPC no puede decir «incompleta desde el asiento X» | Que un consumidor confunda «no hay» con «aquí ya no está» |
| ~~PA-59~~ | — | ✅ **Cerrado por la vía C.** Trece pruebas nuevas: ocho de formato en `eje-almacen`, cinco de rotación en `eje-agente` |
| PA-41 | La cadencia sigue sin medir; el umbral del §3 es hipótesis | Calibrar `ASIENTOS_POR_SEGMENTO` |

---

*Reporte Nº 40 — Segmentación del registro de evidencia · PremosCorp · 8 de agosto de 2026*
