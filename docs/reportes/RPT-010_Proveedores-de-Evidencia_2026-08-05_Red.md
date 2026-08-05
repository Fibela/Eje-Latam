# RPT-010 — Proveedores de Evidencia de Clasificación

**Tema:** Contratos de alimentación del motor de clasificación
**Nº de reporte:** 010
**Fecha:** 5 de agosto de 2026
**Área designada:** Red
**Entidad:** PremosCorp
**Estado:** Canónico con reservas explícitas — véase §7

- **Depende de:** RPT-009 (clasificación triestática), RPT-008 (contrato de contención), RPT-006 §4
- **Cierra:** PA-24 **parcialmente** — contratos e integración; los proveedores concretos siguen abiertos
- **Abre:** PA-25, PA-26

---

## 1. Alcance

La especificación recibida definía cuatro proveedores con sus interfaces. Se implementaron los contratos, la composición hacia `Evidencia` y las pruebas. **No** se implementó ningún proveedor concreto: la huella pasiva depende de captura en `eje-red`, que no existe, y la base OUI tiene un problema de distribución propio (§8, PA-25).

Se aceptó la arquitectura de tres fuentes más el registro de segmentos, y se corrigieron cuatro puntos de la especificación. Los cuatro se documentan porque tres de ellos habrían pasado la revisión sin ruido.

## 2. Corrección 1 — ningún proveedor devuelve `bool`

La especificación proponía:

```rust
fn es_oui_critico(&self, mac: &[u8; 6]) -> Result<bool, ErrorProveedor>;
fn es_segmento_critico(&self, vlan_id: u16) -> Result<bool, ErrorProveedor>;
```

Dos problemas.

`Result<bool, _>` obliga a quien llama a convertir el error en `true` o en `false`, y `unwrap_or(false)` es la conversión que alguien escribirá. Ese `false` significa «no es crítico» — exactamente lo que ninguna fuente inferida puede afirmar (RPT-009 §3). La firma del método invita al error.

`es_segmento_critico -> bool` es peor: colapsa a dos los tres estados que RPT-009 §5 estableció. `NoDeclarado` y `SinDispositivosCriticos` **no son lo mismo** y la distinción es justamente la que impide que la ausencia de declaración se lea como declaración de ausencia.

Sustituido por tipos con variante de desconocimiento propia:

```rust
pub enum Indicio {
    SugiereCriticidad(ClaseExcluida),
    SinIndicio,      // esta fuente no aporta — NO «no es crítico»
    Indeterminado,   // no se pudo consultar
}
```

`Indicio` no tiene variante «no es crítico». La asimetría deja de depender de la disciplina de quien implemente el proveedor y pasa a estar en el tipo.

## 3. Corrección 2 — el marcado lleva clase

`es_critico: bool` no distingue soporte vital de seguridad funcional ni de camino de gestión. Las tres clases existen desde RPT-008 y el veredicto las nombra. Sustituido por `Option<ClaseExcluida>`.

## 4. Corrección 3 — la firma por entrada no protege contra la supresión

Es la importante, y es un fallo de seguridad, no de estilo.

La especificación firmaba cada marcado por separado: `MarcadoAdministrativoFirmado { … , firma: FirmaDeclaracion }`. Con ese diseño, **un atacante que borre la entrada «esta bomba es soporte vital» no rompe ninguna firma**. Las entradas restantes verifican perfectamente. El dispositivo pasa a ser contenible y nada protesta.

La supresión es precisamente el ataque que una lista de exclusión debe resistir: quitar un nombre de la lista es más fácil y más útil que falsificar uno.

Corrección: el inventario se ancla **completo** a una raíz Merkle, que es lo que `eje-almacen` ya construye (`merkle::raiz`, `prueba_inclusion`, `verificar_inclusion`). Verificar un marcado exige firma **y** prueba de inclusión contra la raíz anclada. De ahí `ErrorProveedor::InclusionNoProbada`, que existe para nombrar exactamente este síntoma.

Y una consecuencia que merece su propia regla: **una firma inválida no es «marcado ausente»**. La ausencia permite contener en un segmento declarado limpio; la manipulación nunca. Por eso ambas producen `MotivoAmbiguedad::EvidenciaNoVerificable` y no un `Ok(None)`.

## 5. Corrección 4 — las interfaces sin estado no ven el equipo rodante

La especificación pedía, en su §3, degradar a ambiguo cuando un dispositivo cambia de segmento. Sus interfaces del §2 son consultas puntuales sin memoria: `fn(&mac) -> respuesta`. **Una consulta sin historial no puede saber que hubo un cambio.** El requisito no era satisfacible con las interfaces propuestas.

Corrección: `ProveedorSegmento` devuelve historial, no estado.

```rust
pub struct HistorialSegmento {
    pub actual: DeclaracionSegmento,
    pub visto_en_segmento_critico: bool,  // pegajoso
}
```

Un carro de telemedicina que pasó por la VLAN clínica y aparece luego en la administrativa **no vuelve a ser contenible por haberse movido**. Una vez cierto, el bit permanece hasta que un humano lo limpie.

## 6. Cómo falla cada fuente, y por qué no todas igual

La regla no es «ante cualquier fallo, bloquear». Eso haría el producto frágil: bastaría tumbar la captura para inutilizar la contención en toda la red, lo que convierte una fuente auxiliar en un único punto de fallo explotable.

| Fuente | Naturaleza | Si falla |
|---|---|---|
| Inventario | declarativa | **bloquea** |
| Segmento | declarativa | **bloquea** |
| OUI | inferida | se ignora |
| Huella | inferida | se ignora |

La regla se deriva de RPT-009 §3 en lugar de añadirse: **el permiso para contener procede siempre de una fuente declarativa. La inferencia nunca lo concede, así que su ausencia tampoco puede retirarlo.** Un fallo declarativo, en cambio, significa que no sabemos si el dispositivo está marcado como crítico, y eso sí obliga a escalar.

### 6.1 La prohibición no puede ser silenciosa

Hueco detectado en el trabajo propio de RPT-008 y RPT-009: `Veredicto::Prohibida` bloqueaba y no escalaba nada.

Eso convierte la lista de exclusión en una vía de evasión cómoda. El inventario se indexa por dirección de enlace, que se falsifica trivialmente, y el fallo es asimétrico: **quien suplanta la MAC de un equipo crítico no consigue que se contenga a un tercero por error, consigue volverse incontenible.** Si además el bloqueo es silencioso, la jugada óptima del atacante es declararse crítico y quedar archivado.

No se corrige haciendo infalsificable la MAC, porque no puede serlo. Se mitiga con identidad por certificado 802.1X donde exista —poco frecuente en OT— y sobre todo con que la detección **grite**:

```rust
veredicto.exige_alerta()             // todo lo que no sea Ejecutar
veredicto.es_amenaza_incontenible()  // solo Prohibida
```

«Detectamos una amenaza sobre un dispositivo que no podemos contener» es lo más urgente que este producto puede comunicar. No existe respuesta automática posible: aislar la bomba no es una opción, aislar lo que la rodea y avisar a ingeniería clínica o a planta sí. Y `RequiereAprobacion` **no** activa ese nivel: confundirlas ahogaría la señal urgente entre las ordinarias.

## 7. Verificación

`crates/guardian-cc` pasa de 22 a **32 pruebas**. La matriz PA24-UT propuesta está cubierta, con una salvedad en UT-03 (§7.1).

Probadas por negativa, mutando el código y el manifiesto:

| Mutación | Prueba que falla |
|---|---|
| La firma inválida se lee como marcado ausente | `una_firma_invalida_no_se_lee_como_marcado_ausente` **y** `el_ataque_de_supresion_no_produce_permiso` |
| La ambigüedad de segmento deja de ser pegajosa | `la_ambiguedad_de_segmento_es_pegajosa` |
| Un fallo inferido bloquea | `un_fallo_inferido_no_bloquea` |
| El reloj atrasado extiende la vigencia | `un_reloj_atrasado_caduca_el_marcado_en_lugar_de_extenderlo` |
| La prohibición es silenciosa | `toda_prohibicion_exige_alerta_maxima` |

### 7.1 Desviación sobre PA24-UT-03

La matriz pedía: *marcado `NoCritico` en VLAN que admite críticos → `ConflictoEntreFuentes` → `Ambiguo`*.

No se implementó así, y conviene decir por qué. Un marcado de dispositivo es **más específico** que una declaración de segmento, y ambos son declaraciones humanas firmadas. Una impresora en una sala clínica es un caso legítimo y frecuente: alguien la marcó no crítica sabiendo perfectamente dónde vive. Forzar ambigüedad ahí devuelve la parálisis que RPT-009 §5 resolvió, porque en un hospital la mayoría de las VLAN son clínicas.

Lo que sí es sospechoso no es el **estado** sino el **cambio**: el dispositivo que se mueve. Eso lo cubre `visto_en_segmento_critico`, que sí degrada a ambiguo. El resultado práctico de UT-03 se obtiene para el caso que importa —el equipo rodante— sin romper el caso estático legítimo.

### 7.2 Política de reloj

Un agente Local-First puede tener el reloj desviado. `vigente_en` falla hacia **caducado**: un `ahora` anterior a la emisión cuenta como caducado en lugar de tratarse como «aún no empieza». Un marcado caducado degrada a ambiguo y escala; uno indebidamente vigente permitiría contener un equipo crítico.

## 8. Reservas explícitas

1. **No existe ningún proveedor concreto.** Este reporte define contratos. Los dobles de prueba son bancos de la propia lógica, no simulaciones de equipo de terceros, así que no violan RPT-008 §2 — pero tampoco demuestran que un proveedor real se comporte así.
2. **El anclaje Merkle está especificado, no implementado.** `ErrorProveedor::InclusionNoProbada` existe y las pruebas comprueban que se trata correctamente. Quien implemente `ProveedorInventario` **debe** verificar inclusión; hoy nada en el tipo se lo obliga. Es la clase de exigencia que este proyecto prefiere hacer cumplir con una prueba, y todavía no la tiene.
3. **`Indicio::es_concluyente` no se usa.** Se conserva porque distinguir `SinIndicio` de `Indeterminado` es el objeto del tipo, pero un consumidor futuro podría necesitar la distinción y hoy nadie la consume.
4. **La ambigüedad pegajosa no se limpia.** El diseño dice «hasta que un humano lo limpie». No existe la operación de limpieza. Sin ella el bit sólo crece y, con el tiempo suficiente, todo dispositivo móvil queda permanentemente ambiguo.

La reserva 4 es la que se degrada con el uso, no con el tiempo de desarrollo.

## 9. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-25** | **Distribución de la base OUI en Local-First.** El registro IEEE cambia; un agente sin red no puede actualizarlo. Hace falta decidir empaquetado, cadencia y qué hace un OUI desconocido — que debe ser `SinIndicio` y no un fallo | Proveedor OUI |
| **PA-26** | **Limpieza de la ambigüedad pegajosa.** Operación humana auditada para retirar `visto_en_segmento_critico`, con registro en ALM-01 | Uso prolongado en parques con equipo móvil |

PA-24 permanece abierto para los proveedores concretos: huella pasiva (bloqueado por captura en `eje-red`), OUI (bloqueado por PA-25) e inventario firmado (implementable ya, sobre `eje-almacen` y `motor-pqc`).

---

*Reporte Nº 10 — Proveedores de Evidencia de Clasificación · PremosCorp · 5 de agosto de 2026*
