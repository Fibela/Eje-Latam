# RPT-099 — Dos hashes para la misma clave

**Tema:** PA-146b fase 2. El centinela ancla la clave operativa, y el ancla que diseñé hacía imposible la rotación
**Nº de reporte:** 099
**Fecha:** 12 de septiembre de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** Fase 2 cerrada en código. `guardian-cc` 181, `eje-manifiesto` 46

- **Depende de:** RPT-096 (el centinela v3), RPT-098 (la fase 1), RPT-016 (`IdentificadorClave`), RPT-015 §4
- **Aborda:** PA-146b (sigue parcial). Corrige el ancla de PA-150

---

## 1. La puerta que estaba abierta

Después de PA-150 el sensor vigilaba su clave de **recuperación** —la que se usa una
vez cada varios años— y no vigilaba la **operativa**, que firma el inventario y la
configuración que obedece cada día. Se cargaba del disco y se creía.

El 1 de septiembre reinstalamos la identidad de `eje-prueba` a mano: clave nueva, fichero
sustituido. **El sensor no dijo una palabra.**

## 2. El ancla que diseñé hacía imposible la rotación

Al ir a escribir la máquina de estados apareció esto:

| Función | Qué resume | Bajo qué dominio |
|---|---|---|
| `IdentificadorClave::de` | los **bytes crudos** de la clave | `DOMINIO_IDENTIFICADOR` |
| `huella_de_recuperacion` (mía, PA-150) | la **forma serializada** | etiqueta propia |

Dos valores distintos del mismo objeto. Los certificados nombran a la sucesora con
`IdentificadorClave`; el ancla guardaba la otra. Y **los hashes no se invierten**: desde
el ancla no hay camino a la clave para recalcular el otro resumen.

Para evaluar una rotación hay que buscar en el registro de revocaciones **por la clave
vieja**, que ya no está en disco. Con mi huella, esa búsqueda era imposible.

La respuesta cómoda habría sido guardar las dos en el centinela. Dos hashes del mismo
objeto en el mismo fichero, cada uno hablando con una mitad del sistema.

**Y la pieza correcta existía desde RPT-016**, con esta documentación:

> *«Se usa en lugar de la clave completa porque el registro de revocaciones debe poder
> nombrar una clave que ya no se conserva.»*

Eso es la definición de un ancla. Inventé una segunda sin leer la que había — la misma
falta que RPT-098 §5 documenta, pero aquí el coste no habría sido un identificador
fantasma sino un formato de disco equivocado en todas las máquinas desplegadas.

Se retira `huella_de_recuperacion`. Las dos anclas son `IdentificadorClave`.

## 3. El formato: apéndice, no refactor

**94 bytes.** Los desplazamientos existentes no se mueven —`10..19`, `19..28`, `28..61`—
y el ancla operativa va en `61..94`. Reordenar por estética movería los tres a cambio de
nada, y `escribir_ancla`/`leer_ancla` ya son funciones: se llaman dos veces, no hay
duplicación que extraer.

`vacio()` mira ahora los cuatro campos, por el mismo argumento de RPT-096 §3.

## 4. Cinco estados, y el quinto es el punto entero

| Ancla | Fichero | Certificado | Lectura |
|---|---|---|---|
| ausente | cualquiera | — | `NoAprovisionada` |
| presente | ausente | — | `Suprimida` |
| presente | la misma | — | `Anclada` |
| presente | **otra** | **la nombra sucesora** | **`Rotada`** |
| presente | **otra** | no, o nombra a otra | `NoVerifica` |

Las dos últimas filas son el mismo hecho en el disco —un `.pub` distinto del que había— y
lo único que las separa es un certificado verificado, firmado por una clave que vive fuera
de esa máquina. **El mismo mecanismo que acusa el secuestro autoriza la rotación**, que es
lo que PA-146 pedía desde el principio.

`Rotada` no cuenta como manipulación. `NoAprovisionada` tampoco: nadie hizo nada, y eso es
el problema.

## 5. La perturbación, otra vez

Se implementó a mano la versión cómoda y equivocada —«hay un certificado sobre esta clave,
luego la rotación está autorizada», sin comprobar que nombre *a esta* sucesora— y:

```
test un_certificado_para_otra_sucesora_no_autoriza_esta_clave ... FAILED
  hay certificado sobre esta clave, pero nombra a otra sucesora
```

Ese error convertiría **cualquier revocación legítima del pasado en un permiso permanente**
para instalar la clave del atacante. Es el que se escribe con prisa, y ahora tiene una
prueba que lo espera.

## 6. Un literal que envejeció en tres días

`un_centinela_ya_migrado_dice_que_es_de_otra_version` se puso roja al subir a v4. No porque
nada se hubiera roto: llevaba `encontrada: 3` **escrito a mano** en una prueba cuyo
propósito es decir «la versión que el sensor escribe hoy no la lee el migrador».

Ahora compara contra `VERSION_CENTINELA`. Décimo-primer índice a mano de la serie, y el más
pequeño: un solo número, dentro de una prueba, con tres días de vida.

## 7. Dos costes aceptados, dichos en voz alta

**El registro de revocaciones se carga dos veces** en `arrancar_con_almacen`, porque
`arrancar` lo consume por dentro y cambiarle la firma movía diez llamantes. Dos lecturas del
mismo fichero con microsegundos de diferencia, ambas verificadas contra la clave de
recuperación. Si discreparan, alguien estaría reescribiéndolo durante el arranque — un
problema mayor que el coste de leerlo dos veces.

**Y el migrador acumula versiones legibles**: sabe leer la 2 y la 3. Cada versión que se
retire de esa lista es un sensor desplegado que deja de poder migrarse y hay que
reaprovisionar a mano. La prueba que lo cubre lleva escrita esa pregunta.

## 8. Lo que falta para cerrar PA-146b

La fase 3: llevar `EstadoOperativa` al contrato como condiciones, pintarlo en VIS-04, y
observar los cinco estados en `eje-prueba` — incluida una **rotación de verdad**, con
certificado emitido por dos custodios.

Hoy el estado se calcula y **no lo consume nadie**. Es, exactamente, el defecto dominante
del proyecto: hay que cablearlo antes de decir que el punto está hecho.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| PA-146b | **Parcial.** Fases 1 y 2 hechas; falta cablear y observar |
| PA-145, PA-147, PA-148 | Sin cambios |
| PA-142 | `diagnostico.js` sigue ciego |

---

*Reporte Nº 99 — Dos hashes para la misma clave · PremosCorp · 12 de septiembre de 2026*
