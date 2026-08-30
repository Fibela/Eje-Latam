# RPT-089 — El cable que afirmaba de más

**Tema:** PA-139 reabierto y cerrado. `ClaseConocida` pasa a ser el espejo de `Clasificacion`
**Nº de reporte:** 089
**Fecha:** 28 de agosto de 2026
**Área designada:** Visión
**Entidad:** PremosCorp
**Estado:** **Cerrado.** Todo verde, sin cambio de cifras

- **Corrige:** RPT-088 §5, emitido esta misma tarde
- **Depende de:** RPT-009 §3 (una fuente inferida no afirma), RPT-006 §4
- **Aborda:** PA-139 (cerrado de nuevo). Desbloquea PA-138b de verdad

---

## 1. Qué estaba mal

RPT-088 cerró PA-139 con un `ClaseConocida` de ocho valores, dos de ellos
`inferidaSoporteVital` e `inferidaSeguridadFuncional`.

**El dominio nunca produce eso.** `guardian_cc::clasificacion::clasificar`:

```rust
if evidencia.inferencia.is_some() {
    return Clasificacion::Ambiguo {
        motivo: MotivoAmbiguedad::InferenciaSugiereCriticidad,
    };
}
```

Cuando la huella apunta a criticidad sin marcado que la respalde, el motor **declara
ambigüedad y no afirma la clase**. Es doctrina desde RPT-009 §3: una fuente inferida no
puede afirmar, sólo levantar la mano.

El cable llevaba una afirmación que el motor se niega a hacer. El productor de PA-138b
habría tenido que **inventarla**.

## 2. Y faltaba el estado que más importa

```rust
// Marcado vigente que declara no critico.
return Clasificacion::Clasificado { clase: None, fuente: MarcadoAdministrativo };
```

**«No crítico, y hay un humano que lo firma.»** Es el único estado que permite acción
automática (`permite_accion_automatica`). En el enumerado anterior se habría leído como
`sinIndicio`, que significa lo contrario: que nada apunta.

También se perdían `MarcadoCaducado` y `SegmentoPuedeAlojarCriticos`, colapsados dentro de
un genérico.

## 3. El enumerado nuevo es un espejo, no un modelo

Diez valores, uno por resultado alcanzable de `clasificar`. Ni más —sería inventar dato—
ni menos —sería colapsar estados.

| `Clasificacion` | En el cable |
|---|---|
| `Clasificado { Some(SoporteVital), MarcadoAdministrativo }` | `declaradaSoporteVital` |
| `Clasificado { Some(SeguridadFuncional), … }` | `declaradaSeguridadFuncional` |
| `Clasificado { Some(CaminoDeGestion), … }` | `declaradaCaminoDeGestion` |
| `Clasificado { None, MarcadoAdministrativo }` | **`declaradaNoCritica`** |
| `Clasificado { None, DeclaracionDeSegmento }` | `segmentoDeclaradoSinCriticos` |
| `Ambiguo { MarcadoCaducado }` | `ambiguaMarcadoCaducado` |
| `Ambiguo { ConflictoEntreFuentes }` | `ambiguaConflictoEntreFuentes` |
| `Ambiguo { InferenciaSugiereCriticidad }` | `ambiguaInferenciaSugiereCriticidad` |
| `Ambiguo { SegmentoPuedeAlojarCriticos }` | `ambiguaSegmentoPuedeAlojarCriticos` |
| *(fuente ilegible — hoy `escalados` en el ciclo)* | `indeterminada` |

Las cuatro ambigüedades no se colapsan: mandan a mirar sitios distintos, y es lo único que
le dice al operador por dónde empezar. `ResumenRespaldo` sí las agrega **para una
cabecera**, y eso vive en TypeScript a propósito — si mañana hay que separarlas, se cambia
la vista y no se recompila el sensor.

## 4. Cómo apareció, que es lo único que salva el episodio

No lo cazó ninguna prueba. Todo estaba en verde y el contrato en paridad con los dos lados
— porque la paridad comprueba que los tres digan **lo mismo**, no que digan **la verdad**.

Apareció al ir a escribir el productor y preguntar de dónde saldría cada variante. Tres no
salían de ninguna parte.

Ése es exactamente el momento en que tenía que aparecer, y la razón por la que PA-138b se
diseñó después de PA-139 y no al revés. Pero deja una lección más estrecha que «mirar el
código»: **cuando un tipo del cable refleja un cálculo del dominio, se escribe desde el
`match` que lo produce — no desde lo que uno cree que produce.**

## 5. Dos identificadores inventados hoy, y los dos míos

- `Protocolo::clase_sugerida` en cinco sitios. Se llama `sugiere()`. Lo cazó el compilador.
- `ClaseConocida::InferidaSoporteVital`, que no corresponde a ningún resultado. No lo cazó
  nada; lo cazó ir a buscar su productor.

Llevo el día rechazando este patrón en propuestas ajenas —`ClaseSugerida`, `Corporativo`,
`PerfilSegmento::Aislado`, los canales `ping` y `estadisticas`—. **No es un patrón de quien
propone: es de quien escribe de memoria.** Hoy fui yo dos veces, y la segunda pasó por
todas las barreras.

## 6. Puntos abiertos

| ID | Punto |
|---|---|
| PA-139 | **Cerrado.** §3 |
| PA-138b | Desbloqueado. El mapeo es ahora un `match` de diez brazos que el compilador sujeta |
| PA-142 | Los ficheros del renderer siguen ciegos |

---

*Reporte Nº 89 — El cable que afirmaba de más · PremosCorp · 28 de agosto de 2026*
