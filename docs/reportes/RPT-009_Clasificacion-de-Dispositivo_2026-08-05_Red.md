# RPT-009 — Clasificación de Dispositivo para la Exclusión Permanente

**Tema:** Cómo se determina que un dispositivo no puede contenerse
**Nº de reporte:** 009
**Fecha:** 5 de agosto de 2026
**Área designada:** Red
**Entidad:** PremosCorp
**Estado:** Canónico con reservas explícitas — véase §7

- **Depende de:** RPT-008 (`ClaseExcluida`, contrato de contención), RPT-006 §4 (principio triestático)
- **Cierra:** PA-23
- **Abre:** PA-24
- **Extiende:** `contrato-contencion.toml`

---

## 1. El hueco

RPT-008 dejó tres clases que Guardian-CC no puede contener por ninguna vía, y una regla dura: ningún humano levanta esa exclusión. Lo que no dejó es **cómo se determina que un dispositivo pertenece a ellas**. `evaluar()` recibía la clase como parámetro y nadie la calculaba.

Una lista de exclusión con clasificación no fiable protege sobre el papel. Es la misma familia de defecto que la suite de vectores que sólo comprobaba la presencia de los ficheros.

## 2. Desviación respecto a la propuesta: sin ponderación ni umbral

El lineamiento pedía *«ponderar las fuentes de evidencia»* y *«niveles de confianza»*. Se implementó otra cosa, y conviene justificarlo porque es la decisión central del reporte.

**Una confianza numérica con umbral configurable es un mando.** Alguien lo bajará para reducir falsos positivos —una petición razonable y frecuente— y el día que lo baje la protección desaparece **sin que ninguna prueba falle**. Un umbral es exactamente el tipo de artefacto que este proyecto lleva ocho reportes retirando.

Hay además un defecto técnico independiente: **las fuentes no son independientes**. OUI, protocolo observado y VLAN pueden derivar los tres del mismo hecho —«es un PLC de Siemens»—. Una suma ponderada trata tres señales correlacionadas como tres confirmaciones y puede hacerlas superar a una señal fuerte. Y hace que la *ausencia* de evidencia se parezca a evidencia *negativa*, que es justo la confusión que hay que impedir.

En su lugar: **fuentes ordenadas por autoridad, reglas discretas, sin números que ajustar.**

## 3. La asimetría de la inferencia

Es el hallazgo del reporte y no estaba en el planteamiento inicial.

> Una huella pasiva puede **sugerir** que un dispositivo es crítico. **No puede demostrar que no lo es.**

Una bomba de infusión y una impresora de red hablan HTTP y DHCP. Muchos equipos médicos e industriales usan módulos de red comerciales, con lo que el OUI identifica al fabricante del módulo y no al del equipo. No existe firma pasiva que permita afirmar con seguridad «esto **no** es soporte vital».

De ahí tres reglas:

1. La inferencia sólo mueve la clasificación **hacia** la exclusión, nunca al revés.
2. Sólo un marcado humano firmado puede declarar que un dispositivo es contenible.
3. La inferencia **nunca** produce prohibición permanente, sino `Ambiguo`.

La tercera importa tanto como las otras dos. Un falso positivo permanente e irrevocable dejaría un dispositivo comprometido incontenible para siempre, sin vía de corrección. Sería un modo de fallo tan malo como el que se quiere evitar, sólo que en la otra dirección.

### 3.1 El humano manda para prohibir, no para permitir

De lo anterior sale una asimetría que parece contradictoria y no lo es:

| Situación | Resultado |
|---|---|
| Marcado dice **crítico**, inferencia no lo respalda | **Prohibida.** Añadir una prohibición con una firma es legítimo |
| Marcado dice **no crítico**, inferencia lo contradice | **Ambiguo.** O el marcado está obsoleto o el equipo fue sustituido |

Un administrador puede añadir una prohibición con su sola firma. No puede levantarla contra la evidencia observada.

## 4. Los dos «no» son distintos, y confundirlos sería un defecto

| | `Veredicto::Prohibida` | `Veredicto::RequiereAprobacion` |
|---|---|---|
| Origen | el dispositivo **es** de clase excluida | la evidencia no basta |
| ¿Se puede levantar? | **nadie**, nunca | un operador puede proceder |
| Fuente que puede producirlo | sólo marcado declarativo | también la inferencia |

Tratar una ambigüedad como prohibición permanente inmoviliza dispositivos por un falso positivo. Tratar una prohibición como ambigüedad permite aislar una bomba de infusión con un clic. `Veredicto::RequiereAprobacion` lleva ahora el motivo, para que el operador sepa **qué mirar** y no sólo que algo falta.

## 5. La declaración de segmento, y por qué sin ella no habría producto

Aplicadas sólo las reglas de §3, todo dispositivo sin marcar queda `Ambiguo`. En una oficina con cinco mil equipos eso significa que **nada se contiene nunca**: teatro en la dirección contraria, y más difícil de detectar porque todas las pruebas de seguridad seguirían en verde.

La salida es mover la responsabilidad humana al nivel donde es tratable. El administrador declara la naturaleza de un **segmento** —decenas, no miles—:

- VLAN declarada `SinDispositivosCriticos` → un equipo sin marcar es contenible
- VLAN clínica, de planta, o **sin declarar** → un equipo sin marcar es `Ambiguo`

`NoDeclarado` se resuelve como `PuedeAlojarCriticos`. **La ausencia de declaración no es una declaración de ausencia.**

Por eso `ninguna_evidencia_dudosa_desemboca_en_ejecucion` termina afirmando `ejecutadas > 0`: si esa cuenta bajara a cero, la política sería inaplicable y todas las demás pruebas seguirían pasando.

## 6. Verificación

`crates/guardian-cc` pasa de 11 a **22 pruebas**. `ninguna_evidencia_dudosa_desemboca_en_ejecucion` barre las 30 combinaciones de marcado × segmento × inferencia por enumeración y no por argumento, y comprueba además que el perfil OT no ejecuta en ninguna de ellas.

Probadas por negativa, mutando código y manifiesto:

| Mutación | Prueba que falla |
|---|---|
| La inferencia produce prohibición permanente | `la_inferencia_nunca_produce_prohibicion_permanente` |
| El marcado «no crítico» vence a la huella contradictoria | `un_marcado_no_critico_contradicho_por_la_huella_es_ambiguo` |
| Un segmento sin declarar se presume limpio | `sin_evidencia_alguna_no_se_contiene_automaticamente` **y** `un_segmento_no_declarado_se_trata_como_si_alojara_criticos` |
| El marcado caducado sigue valiendo | `un_marcado_caducado_no_vale_como_marcado` |
| El manifiesto permite que la huella descarte criticidad | `solo_las_fuentes_declarativas_descartan_criticidad` |

La última cierra el círculo: la asimetría de §3 está declarada en el manifiesto **y** comprobada contra el código, no sólo escrita en un comentario.

## 7. Reservas explícitas

1. **No existe todavía el productor de evidencia.** Este reporte define cómo se *combina* la evidencia, no cómo se *obtiene*. Quién calcula la huella pasiva, quién resuelve el OUI, dónde vive el marcado firmado y cómo se verifica esa firma: nada de eso está implementado. `MarcadoDispositivo::vigente` es un booleano que alguien deberá calcular contra un reloj y una política de vigencia. Se abre PA-24.
2. **`Clasificacion::NoClasificado` es inalcanzable desde `clasificar()`.** El segmento siempre aporta algo, aunque sea su ausencia de declaración. Se conserva en el tipo para que ningún consumidor futuro pueda asumir que la evidencia siempre llega, y `evaluar()` lo trata como ambigüedad. Es deuda deliberada, no descuido.
3. **La vigencia por defecto de 365 días no está fundamentada.** Es un número redondo puesto para que exista el mecanismo. La cadencia real de rotación de parque en un hospital o una planta debería fijarla alguien que la conozca.
4. **La clasificación no cubre el dispositivo móvil entre segmentos.** Un equipo que hoy está en la VLAN clínica y mañana en la administrativa cambia de clasificación sin que nada lo registre. En hospitales esto no es hipotético: el equipo rodante se mueve.

La reserva 4 es la que más se parece a un fallo real y no a una limitación de alcance.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-24** | **Productores de evidencia.** Huella pasiva, resolución de OUI, almacén de marcados firmados y verificación de esa firma, cálculo de vigencia. Sin ellos la clasificación es una función sin entradas | Uso real de la contención |

---

*Reporte Nº 9 — Clasificación de Dispositivo para la Exclusión Permanente · PremosCorp · 5 de agosto de 2026*
