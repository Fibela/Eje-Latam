# RPT-022 — Declaración firmada de segmentos

**Tema:** Que una VLAN pueda declararse limpia, y que sólo un humano pueda declararlo
**Nº de reporte:** 022
**Fecha:** 6 de agosto de 2026
**Área designada:** Clasificación
**Entidad:** PremosCorp
**Estado:** **Implementado.** Fase 2 de PA-45

- **Depende de:** RPT-009 §5 (`DeclaracionSegmento`), RPT-011 (cadena de eslabones), RPT-013 (formato en disco), RPT-017 (`EstadoArranque`)
- **Cierra:** PA-45
- **Sube:** `formato::VERSION` 1 → 2

---

## 1. El mecanismo estaba entero y sin cablear

`DeclaracionSegmento` existe desde RPT-009 y `clasificar` la consume. Es la **única fuente declarativa que no exige marcar equipo por equipo**, y por tanto la única que hace tratable un parque de miles de dispositivos: decenas de segmentos en lugar de miles de equipos.

Nadie la producía. `eje-agente` traía esto:

```rust
const fn segmento_de(vlan: Option<u16>) -> DeclaracionSegmento {
    match vlan {
        None => DeclaracionSegmento::NoDeclarado,
        Some(_) => DeclaracionSegmento::PuedeAlojarCriticos,
    }
}
```

Léase con cuidado: **ninguna VLAN podía declararse limpia**. Y como un dispositivo sin marcado sólo es contenible en un segmento declarado limpio (RPT-009 §5), la consecuencia es que **ningún equipo sin marcado era contenible jamás**. El producto observaba y escalaba, y nunca contenía nada por sí solo.

Es la quinta vez en este proyecto que aparece el mismo defecto —mecanismo completo, sin cablear— después de `disco.rs`, `ArchivoRevocaciones`, los tres centinelas de alerta y el propio ejecutable. Vale la pena nombrarlo: **las pruebas unitarias de una pieza no detectan que nadie la llame.** Lo que lo detecta es preguntar quién produce cada entrada de la función que se está probando.

## 2. Por qué la declaración viaja dentro del manifiesto firmado

La alternativa natural era un fichero de configuración aparte. Se rechaza, y el motivo es concreto y no genérico.

Un fichero aparte es editable sin romper ninguna firma. Y la edición útil para un atacante es **de una sola línea**: declarar limpia la VLAN clínica. Hecho eso, todo equipo sin marcado de ese segmento pasa a ser contenible automáticamente, que es exactamente el daño que toda la cadena de RPT-011 existe para impedir. Habríamos construido cinco eslabones criptográficos sobre los marcados y dejado la puerta de al lado abierta.

Así que la tabla se resume y **su resumen entra en el mismo mensaje que firma el administrador**, junto a la raíz Merkle y la secuencia:

```rust
DOMINIO_RAIZ = b"eje-latam/agt-01/raiz-inventario/v2"

mensaje = Absorbedor(DOMINIO_RAIZ)
    .resumen(raiz_merkle_de_marcados)
    .resumen(resumen_de_la_tabla_de_segmentos)
    .entero(secuencia)
```

Los dos bloques quedan cubiertos por caminos distintos pero equivalentes: alterar un marcado cambia la raíz, alterar una declaración cambia el resumen, y en ambos casos la firma deja de verificar. Es el mismo argumento por el que RPT-010 §4 firma la raíz y no cada entrada.

### 2.1 El contador va antes que el contenido

El resumen de la tabla absorbe **el número de declaraciones y después las declaraciones**. Sin ese contador, una tabla vacía y la ausencia de tabla producirían el mismo resumen, y **borrar el bloque entero no rompería la firma**.

No es hipotético: una tabla vacía es un estado legítimo y frecuente —un cliente que aún no ha declarado sus segmentos—, así que no se puede cerrar el hueco prohibiendo el caso vacío. Hay que hacerlo distinguible.

## 3. `NoDeclarado` no se codifica

`NaturalezaSegmento` tiene **dos** variantes, no tres. La ausencia de declaración se representa con la **ausencia de registro**.

Si existiera un código para «no declarado», el mismo estado tendría dos representaciones —registro ausente y registro presente con ese código— y volvería la ambigüedad que el rechazo de bytes sobrantes y `deny_unknown_fields` cierran en el resto del proyecto.

El código `0` queda reservado y se rechaza, de modo que **un bloque de ceros no se analiza como una tabla de declaraciones válidas**. Relleno, sector sin escribir o fichero recién creado fallan en lugar de leerse como política.

## 4. La declaración caduca, y por un motivo asimétrico

La declaración peligrosa es «este segmento está limpio»: es la única que concede contención automática a un equipo sin marcado. Y es justo la que envejece mal, porque el día que alguien conecte un carro de telemedicina a la VLAN administrativa **nadie va a volver a emitir el manifiesto para corregirla**.

La política de reloj es idéntica a la de `MarcadoVerificado::vigente_en`, deliberadamente: ante duda, **caducada**. Un `ahora` anterior a la emisión —reloj atrasado, fecha futura— también cuenta como caducada.

Los tres caminos a `NoDeclarado` degradan en la misma dirección segura:

| Situación | Resultado |
|---|---|
| Trama sin etiqueta | `NoDeclarado` |
| VLAN sin registro | `NoDeclarado` |
| Declaración caducada | `NoDeclarado` |

`NoDeclarado` admite críticos, así que **ninguno de los tres concede contención**. Sólo una declaración presente, en rango y vigente puede declarar limpio un segmento.

## 5. El rango declarable excluye el VID 0

En 802.1Q el VID `0` significa «trama etiquetada sólo por prioridad, sin pertenencia a VLAN», y `eje-captura` lo entrega como `Some(0)` porque enmascara los doce bits bajos. El `4095` está reservado por la norma para uso de implementación.

Admitir la declaración del `0` permitiría escribir «el segmento 0 está limpio» y con ello **conceder contención automática a cualquiera que emita tramas con prioridad y sin VLAN**, que es una condición trivial de cumplir. Rango declarable: `1..=4094`.

## 6. Lo que este mecanismo no resuelve, y su dirección sorprende

La etiqueta VLAN de una trama es la palabra del conmutador **sólo si el agente observa un espejo de puertos de acceso**: allí el equipo reescribe la etiqueta y el emisor no la elige. Sobre un espejo de **troncal**, quien está en el cable etiqueta lo que quiera.

La asimetría merece escribirse porque su dirección es contraria a la intuición:

- Fingir estar en una VLAN **limpia** te vuelve contenible. Nadie lo hace.
- Fingir estar en una VLAN **crítica** te vuelve ambiguo, y por tanto incontenible sin un humano. **Ésa** es la jugada.

Luego la declaración de segmento **no es un vector para contener de más: es un vector de evasión**, exactamente igual que la suplantación de MAC descrita en `ProveedorInventario`. No se corrige aquí —no puede—; se mitiga con espejo de acceso, con identidad por certificado 802.1X donde exista, y sobre todo con que ninguna prohibición sea silenciosa.

## 7. La comprobación que hoy no puede fallar

`TablaVlanVerificada::verificar_e_instanciar` compara el resumen recalculado con el que la raíz verificada ancla. Por el camino de `InventarioLocal::cargar` **esa comparación no puede fallar**: `analizar` derivó el resumen anclado de la misma tabla que después se presenta, y la firma ya lo cubrió.

Conviene decirlo en lugar de presentarlo como una defensa activa. Lo que sí aporta es que `TablaVlanVerificada` **no tiene otro constructor**, así que ningún módulo futuro puede fabricar una tabla «verificada» a partir de otra fuente —un fichero de configuración, una opción de línea de órdenes— sin pasar por aquí. Es la misma disciplina que hace inconstruible un `MarcadoVerificado` sin su prueba de inclusión, y su valor es de mañana, no de hoy.

## 8. Subir la versión ya no parece un ataque

Esta es la deuda que la Fase 1 pagó por adelantado. `formato::VERSION` pasa de 1 a 2, lo que deja **obsoletos de golpe todos los inventarios existentes**. Antes de la Fase 1, el estado resultante habría sido `EstadoArranque::NoVerifica`, que RPT-017 §2.1 trata como manipulación: alerta máxima y contención automática detenida en todas las instalaciones a la vez, por una actualización rutinaria.

Con `FormatoObsoleto` en su sitio, el mismo hecho produce alerta administrativa sin alerta de incidente, y la protección degrada al perfil del primer arranque. **La Fase 1 no era preparación burocrática: era la condición para que esta Fase 2 fuera desplegable.**

## 9. Deuda que este reporte crea

1. **Nadie emite manifiestos.** `serializar` existe y las pruebas lo ejercitan, pero no hay herramienta administrativa que construya un inventario, declare segmentos y los firme. Sin ella el mecanismo funciona y **no tiene usuarios** — es el mismo defecto del §1 desplazado un nivel. Se registra como **PA-48**.
2. **El agente sigue en primer arranque.** `eje-agente` construye `EstadoArranque::PrimerArranque` en lugar de llamar a `arrancar`, porque no hay aprovisionamiento de claves que lo permita (PA-14b/PA-48). La traducción de VLAN a declaración ya **no** vive en el agente, que era lo que impedía firmarla; pero mientras no cargue manifiesto, toda VLAN sigue saliendo `NoDeclarado`.
3. **`VLAN_MAXIMA` no cubre QinQ.** Un despliegue con etiquetado apilado presenta la etiqueta externa, y este modelo no distingue las dos. No se ha visto en ningún destino de Fase 1; se anota para no descubrirlo en campo.

## 10. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-48** | **Herramienta de emisión de manifiestos.** Construir inventario, declarar segmentos, firmar con la clave del cliente y entregar el `.inv` | Que el mecanismo tenga usuarios |
| PA-45 | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 22 — Declaración firmada de segmentos · PremosCorp · 6 de agosto de 2026*
