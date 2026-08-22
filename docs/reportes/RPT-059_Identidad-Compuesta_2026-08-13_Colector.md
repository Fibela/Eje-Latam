# RPT-059 — Identidad compuesta: la máquina no es el sensor

**Tema:** PA-113. Varios agentes en un mismo servidor dejan de ser uno solo
**Nº de reporte:** 059
**Fecha:** 13 de agosto de 2026
**Área designada:** Colector
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado en ejecución real con dos agentes.** Cierra PA-113

- **Depende de:** RPT-058 (la identidad era la interfaz), RPT-057 (el vigía)
- **Aborda:** PA-113

---

## 1. El mismo error, un escalón más arriba

RPT-058 corrigió que el sensor se identificara con **la interfaz**: dos hospitales
con `eth0` eran el mismo sensor para la sala. La corrección fue usar la máquina.

Y ahí quedó la otra mitad del error. Un servidor perimetral con **un agente por
segmento** —despliegue normal en una planta grande— tiene varios agentes que
comparten `HOSTNAME`. Con la máquina como clave, **el latido de uno tapa la
muerte del otro**: exactamente lo que PA-104 existe para impedir.

Allí se tomó la parte por el todo; aquí, el todo por la parte. La identidad es el
par `(máquina, interfaz)`, y ninguna de las dos por separado lo es.

## 2. La interfaz es opcional, y eso no es un descuido

Un agente anterior a este cambio no la declara. Su identidad es **la máquina
sola**: distinta de cualquier par con interfaz y perfectamente estable, así que se
le vigila igual.

Descartar su latido por incompleto lo dejaría **fuera** de la vigilancia, que es
lo contrario de lo que se busca. Y rellenarla con una cadena vacía lo haría
indistinguible de un agente que declara una interfaz sin nombre — la clase de
mentira pequeña que este proyecto persigue.

## 3. El censo se nombra igual que se imprime

`maquina/interfaz`, o `maquina` a secas. Hay prueba de que leer y escribir usan la
misma notación, y no es cosmética: si el censo se escribiera de una forma y el
vigía imprimiera de otra, habría entradas que **nunca casan** y se leerían como
«ese sensor no ha hablado nunca». Es el defecto de RPT-058 §2 en forma de
notación, y ya nos costó una prueba de fuego entera.

**El censo es estricto a propósito.** Una entrada `maquina` no casa con
`maquina/eth0`. Si casara con cualquier interfaz, un sensor vivo satisfaría la
entrada del compañero muerto: el colapso reintroducido por la puerta del censo.

Como eso es fácil de escribir mal y el síntoma llega tarde, el vigía **avisa al
arrancar** de toda entrada sin interfaz.

## 4. `DatosLatido`, y por qué no fue sólo callar a clippy

`too_many_arguments` a los ocho parámetros. La corrección no fue un `#[allow]` ni
un alias: los datos van en un registro con campos nombrados.

El motivo real no es el recuento. **`maquina` e `interfaz` son los dos `&str`**:
invertirlos compila sin una queja y vuelve a colapsar la identidad —esta vez al
revés, todos los sensores llamándose `eth0`— sin que ninguna comprobación de
tipos lo note. Es el defecto que acabábamos de arreglar, esperando a que alguien
se equivocara de orden. Los nombres lo impiden.

## 5. La observación que cierra PA-113

Dos agentes en la **misma máquina**, uno sobre `lo` y otro sobre un `veth`, con
almacenes separados y el vigía esperando a los dos.

Primer intento — se apagaron los dos:

```
AUSENTE  LapTap-AF/veth-eje: sin latir desde hace 31212 ms
AUSENTE  LapTap-AF/lo: sin latir desde hace 33708 ms
```

Eso prueba que la contabilidad está partida —dos relojes propios, dos alarmas a
dos segundos y medio— pero **no prueba lo que hacía daño**. El enmascaramiento
sólo se ve con uno vivo.

Segundo intento, matando **sólo** el de `lo`:

```
AUSENTE  LapTap-AF/lo: sin latir desde hace 32933 ms (se le permitian 30000)
```

Y nada más. Ni una línea sobre `veth-eje`, que siguió latiendo. **El vivo no tapó
la muerte del compañero.**

La predicción se escribió antes de ejecutar, con las tres formas de salir mal
enumeradas —los dos ausentes, ninguno, o cruzados— para que el resultado valiera
como observación y no como interpretación a posteriori.

## 6. Lo que sigue sin resolver

**El sello de RPT-038 no lleva interfaz.** El latido sí. Dos agentes en una
máquina siguen entrelazando sus series de sellos ante el testigo externo, y ahí
la consecuencia no es una muerte no detectada sino una acusación de manipulación
mal dirigida. Es **PA-115**.

No se arregló en este reporte por disciplina: el sello tiene su propia
correlación en el colector y tocarlo sin una prueba de fuego que lo ejercite
sería cambiar a ciegas la pieza que detecta el recorte del registro.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-113~~ | ✅ Cerrado por observación con dos agentes en la misma máquina (§5) |
| **PA-115** | El sello de RPT-038 no lleva interfaz: dos agentes de una máquina entrelazan sus series ante el testigo (§6) |
| PA-112 | Firmar el latido. El contador obliga a seguir emitiendo, no impide la suplantación |
| PA-107 | Empaquetado dual. El instalador es quien fijará el nombre y la interfaz |
| PA-79 | Nombre e intervalo, los dos parámetros que piden configuración firmada |

---

*Reporte Nº 59 — Identidad compuesta · PremosCorp · 13 de agosto de 2026*
