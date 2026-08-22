# RPT-060 — El tablero abandonado: una fuente única que nadie alimentaba

**Tema:** PA-108. El índice existía, tenía lector, y llevaba dos semanas contando una parte del proyecto
**Nº de reporte:** 060
**Fecha:** 13 de agosto de 2026
**Área designada:** Método
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado.** Cierra PA-108

- **Depende de:** RPT-002 §12 (el tablero), RPT-003 §9.5 (el patrón de `xtask`)
- **Aborda:** PA-108

---

## 1. Lo que se iba a construir ya existía

PA-108 se acuñó como «hace falta un índice único de puntos abiertos». Al ir a
construirlo apareció que **el índice existe desde el primer día**: el tablero de
RPT-002 §12. Y no sólo existe — tiene lector, `cargo xtask tablero`, cuyo propio
encabezado dice:

> El tablero se ha resumido a mano cuatro veces y las cuatro reintrodujo puntos
> ya cerrados. La causa no es descuido: es que **se reescribe de memoria en lugar
> de leerse**. […] El tablero de RPT-002 §12 es la fuente de verdad; cualquier
> recuento sale de aquí.

Se escribió exactamente para el problema que PA-108 volvió a describir.

## 2. Y llevaba dos semanas diciendo una verdad parcial

El tablero se quedó en **PA-76**. Desde entonces los reportes acuñaron
**treinta y nueve identificadores** que no figuraban en él.

Así que `cargo xtask tablero` contaba una parte del proyecto y la presentaba como
el total. La herramienta no mentía sobre lo que leía: **el sitio que lee había
dejado de escribirse.**

Es una variante nueva de la familia dominante de este proyecto. No es un
mecanismo sin cablear —está cableado, funciona y se ejecuta— sino una **fuente
única que alguien dejó de alimentar**, y que por eso sigue *pareciendo* una
fuente única. La diferencia importa: un mecanismo muerto se nota al usarlo, y
éste respondía con una cifra creíble.

## 3. La corrección: escribir en el sitio que ya se lee

Treinta y nueve filas recuperadas, de PA-77 a PA-115, con su estado y su reporte
de cierre. **No se creó un fichero nuevo.** Añadir un segundo índice habría sido
crear una segunda cosa que mantener a mano — el defecto, otra vez, con mejor
nombre.

## 4. La barrera, y lo que cazó en su primera ejecución

`cargo xtask tablero` falla ahora si algún `PA-nn` citado en cualquier `.md` de
`docs/` no tiene fila en el tablero.

Habría cazado las tres cosas que nos pasaron esta semana:

- la colisión de **PA-84**, usado para dos puntos distintos en documentos
  distintos (RPT-053 §8);
- **PA-101, PA-102 y PA-103**, acuñados en sesión de trabajo y que no existían en
  ningún documento;
- este mismo desfase de treinta y nueve.

Y en su primera ejecución encontró uno que nadie buscaba: **PA-14b**. RPT-021
partió PA-14 en tres, y al llevar los hijos al tablero se escribieron `PA-14a` y
`PA-14c`. El de en medio —el único de los tres que estaba **resuelto**— se quedó
fuera durante ocho días.

## 5. La lección documental, y una corrección de método

Las tablas de puntos abiertos **dentro de cada reporte son instantáneas con
fecha**: dicen qué estaba abierto cuando se escribió aquel reporte.

Hoy, antes de llegar aquí, se parchearon a mano las tablas de seis reportes para
marcar como cerrados puntos que se habían cerrado después. **Fue el instinto
equivocado**, y conviene dejarlo escrito porque parecía diligencia:

- mantener N copias al día a mano **es** el defecto que llevamos el día entero
  arreglando en el código;
- y borra la naturaleza histórica del reporte, que es lo que permite leer una
  decisión con el contexto que tenía.

**El tablero es el único sitio que habla del presente.** Un reporte habla de su
día.

## 6. Lo verificado

```
Tablero de RPT-002 §12
  Identificadores : 114
  Cerrados        : 66
  Parciales       : 5
  Abiertos        : 43
  Pendientes      : 48 (parciales + abiertos)
```

Y tras añadir la fila de PA-14b, ningún identificador citado en `docs/` queda sin
fila.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-108~~ | ✅ Tablero recuperado y con barrera contra la deriva |
| PA-115 | El sello de RPT-038 no lleva interfaz. **Es el siguiente** |
| PA-112 | Firmar el latido |
| PA-107 | Empaquetado dual |

> El tablero de RPT-002 §12 lista los cuarenta y ocho pendientes. Esta tabla no
> los repite: repetirlos sería reabrir el defecto que este reporte cierra.

---

*Reporte Nº 60 — El tablero abandonado · PremosCorp · 13 de agosto de 2026*
