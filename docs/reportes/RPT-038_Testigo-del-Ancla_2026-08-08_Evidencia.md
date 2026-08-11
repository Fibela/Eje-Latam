# RPT-038 — El ancla no se cierra firmándola

**Tema:** Qué protege realmente el ancla de evidencia, y qué haría falta para que protegiera de quien escribe en el disco
**Nº de reporte:** 038
**Fecha:** 8 de agosto de 2026
**Área designada:** Evidencia
**Entidad:** PremosCorp
**Estado:** **Ratificado e implementado.** Cierra PA-64 (reformulado), abre PA-70 y PA-71

- **Depende de:** RPT-029 (persistencia), RPT-033 (ancla, PA-57), RPT-032 (salida, PA-42)
- **Reformula:** PA-64
- **Abriría:** PA-70, PA-71

---

## 1. El hueco es real y está bien identificado

`Ancla` es `{ numero, extremo }` y `ancla_de` es una función pura del registro:

```rust
pub fn ancla_de(registro: &RegistroEvidencia) -> Option<Ancla> {
    registro.asientos().last().map(|asiento| Ancla {
        numero: asiento.numero,
        extremo: asiento.resumen_propio,
    })
}
```

`serializar_ancla` escribe mágico, versión, número y extremo. Nada más. Quien pueda escribir en el directorio de datos puede:

1. Recortar el registro a N asientos.
2. Recalcular el ancla de ese registro recortado.
3. Escribirla.

`cotejar` devuelve `Conforme`. La evidencia desaparece sin dejar rastro.

Las pruebas de RPT-033 cubren **borrar** el ancla y **corromperla**, que es lo que haría un atacante torpe. No cubren **reescribirla coherentemente**, que es lo que haría cualquiera que lea el código — y el código es abierto.

Hasta aquí, PA-64 está bien planteado.

## 2. Dónde deja de estarlo

El objetivo registrado dice «sellar criptográficamente el ancla» para «cerrar el último vector de manipulación local directo en disco **para usuarios con privilegios de escritura**».

Una firma local no cierra eso.

Firmar el ancla en cada vuelta exige que el agente tenga una **clave privada de firma**, disponible sin intervención humana, en la máquina. El mismo atacante que reescribe el ancla lee esa clave y firma la suya. Se pasa de «recortar y recalcular» a «recortar, recalcular y firmar»: un paso más de guion, ninguna barrera.

Esto contradice además el modelo de confianza que RPT-015 y RPT-024 construyeron con cuidado: el agente sostiene **claves de verificación**, nunca de firma. `EJE-PUB1` está ahí precisamente porque un agente que puede firmar es un agente cuyo compromiso permite fabricar inventarios. Darle ahora una clave de firma para el ancla abriría por la puerta de atrás lo que aquella decisión cerró por la de delante.

**Ninguna máquina se audita a sí misma frente a quien la controla.** Es una propiedad del problema, no una carencia de la implementación, y no hay criptografía que la levante mientras la clave viva en el disco que el atacante escribe.

## 2.1. Una precisión que me debo a mí mismo

El §2 discute la firma **con clave del agente**. El enunciado original de PA-64 en
RPT-033 §7 era más cuidadoso que eso: decía «firmar el ancla **con la clave del
cliente**», y resistiría «a quien tenga escritura en el almacén **pero no la
clave**». Esa versión no comete el error que le atribuí.

Comete otro. Una clave del cliente que el agente no tiene sólo puede firmar
anclas cuando el cliente está delante, y el ancla se actualiza cada vez que se
anexa una alerta. O el agente tiene la clave —y volvemos al §2— o el anclaje es
ceremonial y periódico, y entre ceremonia y ceremonia queda un tramo sin firmar
que es exactamente donde alguien recortaría.

El testigo externo resuelve las dos versiones a la vez, y por eso lo propongo en
lugar de cualquiera de ellas. Pero conviene que quede escrito que el enunciado de
RPT-033 era más preciso de lo que dio a entender mi §2.

## 3. Lo que sí cierra el hueco

Tres candidatos, y sólo uno cabe en el producto hoy.

**Elemento seguro (TPM / secure element).** Sella la clave fuera del alcance del sistema de ficheros. Funciona de verdad. Es hardware: entra en la misma cola que PA-61, y convierte una decisión de código en una condición del BOM. No lo descarto — lo aplazo, y quedaría como **PA-71**.

**Fichero de sólo-anexado del sistema operativo** (`chattr +a`, `CAP_LINUX_IMMUTABLE`). Detiene a un usuario con escritura que no sea root. No detiene a root, y el agente necesita esa capacidad al arrancar. Es defensa en profundidad barata, no una solución. Sería **PA-70**, y depende de PA-40 como todo lo demás en Linux.

**Testigo externo.** El extremo del registro se emite al colector del cliente por el canal que ya existe desde PA-42. El SIEM guarda una serie de extremos con su número de asiento. Si alguien recorta el registro local y reescribe el ancla, el extremo que la máquina declara para el asiento N **no coincide con el que el colector anotó** cuando ese asiento se creó.

La detección ocurre fuera de la máquina comprometida, que es el único sitio donde puede ocurrir.

## 4. Por qué el testigo encaja sin forzar nada

No hace falta canal nuevo, ni clave nueva, ni formato nuevo en disco. Es una entrada de syslog más, con la misma estructura que las demás.

Encaja además con la disciplina de RPT-032 §3 sin excepción alguna: **el extremo cambia exactamente cuando el registro cambia**, así que emitir en cada cambio es emitir sólo lo que cambia. No es un latido periódico —eso sí inundaría— sino la misma regla ya vigente aplicada a un dato nuevo.

Forma propuesta, alineada con `linea_de_suceso`:

```text
sello=<hex del extremo> asiento=<numero>
```

Gravedad informativa: no es una alerta, es una constancia.

## 5. Lo que el testigo no hace, dicho ahora y no en campo

**Sin `--syslog` no hay testigo.** Un despliegue sin colector conserva el hueco entero. El agente ya dice por pantalla que las alertas no salen del equipo; esto añade una consecuencia que antes no tenía.

**El testigo no impide la manipulación, la delata.** El registro local se puede seguir recortando. Lo que deja de poderse es recortarlo *sin que quede constancia en otro sitio*.

**La detección exige que alguien mire.** Correlacionar la serie de extremos es trabajo del SIEM, no del agente. Sin una regla en el colector, el dato está y nadie lo lee. Eso hay que entregarlo como parte del producto —una regla de correlación de ejemplo— o el mecanismo existe y nadie lo llama, que es el error que este proyecto lleva encontrándose desde RPT-022.

**Un atacante que corta la red antes de recortar** se lleva por delante el testigo de ese momento. Deja, eso sí, el hueco de `salidaNoDisponible` en el SIEM justo antes del salto en la serie — que es una señal, no una prueba.

## 6. Lo que propongo ratificar

1. **Reformular PA-64**: de «firmar el ancla» a «anclar el extremo fuera de la máquina». La firma local se descarta con el motivo del §2 escrito, no se deja pendiente.
2. **Implementar el testigo** (§3, §4) como cierre de PA-64.
3. **Abrir PA-70**: fichero de sólo-anexado en Linux, defensa en profundidad, tras PA-40.
4. **Abrir PA-71**: sellado en elemento seguro, condición de BOM junto a PA-61.
5. **Añadir a PA-46** —el repositorio firmado— la regla de correlación de ejemplo para el colector, sin la cual el §5 se cumple en su peor forma.

Si el equipo prefiere la firma local pese al §2, se puede implementar; lo que pido es que no se registre como cierre del vector que nombra, porque no lo cierra y quedaría escrito que sí.

## 7. Lo implementado

`linea_de_sello` en `salida.rs` compone `sello=<hex> asiento=<n>` con gravedad
informativa. `Emisor::sellar` lo emite sólo si el par cambió. `Ciclo::vuelta` lo
llama al final, después de persistir.

Dos decisiones que no son obvias y que costarían caro si se tocan sin leer esto:

**El sello describe el disco, no la memoria.** Si una vuelta anexó y la escritura
falló, no se sella. Anunciar un extremo que no sobrevive al reinicio haría que el
arranque siguiente pareciera un recorte, y el testigo acusaría de manipulación a
un fallo de disco. Cuando no se anexó nada, en cambio, el extremo vigente **sí**
viene del disco, y sellarlo es lo que da la línea base tras cada arranque.

**Esa línea base es el mecanismo, no un extra.** El ataque que el ancla no ve es:
parar el agente, recortar, recalcular el ancla, arrancar. El cotejo local dice
`Conforme` porque el atacante lo hizo bien. Lo único que lo delata es que el
colector tenía anotado un asiento más alto para esa máquina.

**El sello sí se reintenta; las transiciones no.** Es la única asimetría del
emisor y es deliberada. Reemitir una transición pasada le mostraría al operador un
incidente que ya ocurrió. Reenviar el extremo vigente no cuenta nada falso: cuenta
lo que sigue siendo cierto. Si no se reintentara, un colector caído un minuto
dejaría un tramo sin atestiguar para siempre — y ese tramo es justo donde alguien
podría recortar sin que nadie lo notara.

Se comparan **número y extremo**, no sólo el número: alterar el último asiento sin
cambiar su número es precisamente la mutación que RPT-029 §2.1 dejó escrita como
limitación.

Seis pruebas nuevas. Lo que **no** está cubierto: la rama «se anexó y la
persistencia falló», porque producir una amenaza incontenible desde `eje-agente`
exige un inventario firmado que sus pruebas no pueden construir. Correcta por
lectura, no por verificación.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| PA-64 | **Anclar el extremo fuera de la máquina** (reformulado) | Detectar el recorte del registro por quien escribe en el disco |
| PA-70 | Fichero de sólo-anexado en Linux | Depende de PA-40 |
| PA-71 | Sellado del ancla en elemento seguro | Condición de BOM, con PA-61 |

---

*Reporte Nº 38 — El ancla no se cierra firmándola · PremosCorp · 8 de agosto de 2026*
