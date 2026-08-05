# RPT-011 — Inventario Firmado de Marcados Administrativos

**Tema:** Verificación criptográfica del marcado de dispositivo
**Nº de reporte:** 011
**Fecha:** 5 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** Canónico con reservas explícitas — véase §7

- **Depende de:** RPT-010 (contratos de proveedores), RPT-009 (clasificación), RPT-005 (firma híbrida), `eje-almacen` (Merkle y `Absorbedor`)
- **Cierra:** la reserva 2 de RPT-010 §8 y el primero de los tres proveedores de PA-24
- **Abre:** PA-27
- **Toca:** frontera de custodia de claves, que roza PA-14

---

## 1. Qué se cerró

RPT-010 §8 dejó escrito: *«Quien implemente `ProveedorInventario` debe verificar inclusión; hoy nada en el tipo se lo obliga.»* Era una obligación en prosa. `MarcadoVerificado` tenía campos públicos: cualquiera podía fabricar uno sin verificar nada, y el nombre mentía.

Ahora los campos son privados y la única vía de construcción es `verificar_e_instanciar`. Un valor de ese tipo **es**, por construcción, un marcado perteneciente a un inventario firmado por el administrador del cliente.

## 2. Codificación canónica: se reutiliza, no se reinventa

El lineamiento especificaba prefijos de longitud, campos de ancho fijo, clase como escalar cerrado y etiqueta de dominio. `Absorbedor` de `eje-almacen` **ya hace exactamente eso**: prefija en longitud la etiqueta de dominio y cada campo absorbido.

Escribir un codificador nuevo habría significado mantener dos mecanismos equivalentes y auditar ambos, con la garantía de que sólo uno recibiría atención. Se reutiliza el que ya tiene 25 pruebas detrás, incluida `los_prefijos_de_longitud_impiden_la_ambiguedad`.

Dos dominios distintos, y la separación importa:

```rust
const DOMINIO_MARCADO: &[u8] = b"eje-latam/agt-01/marcado-inventario/v1";
const DOMINIO_RAIZ:    &[u8] = b"eje-latam/agt-01/raiz-inventario/v1";
```

Sin `DOMINIO_RAIZ`, una firma sobre 32 bytes cualesquiera —el resumen de un asiento de ALM-01, por ejemplo— podría reutilizarse como firma de raíz de inventario.

La clase viaja como escalar cerrado (`u8`), no como cadena: dos textos distintos para la misma clase producirían hojas distintas.

## 3. La cadena de cuatro eslabones

| Eslabón | Qué ata | Ataque que impide |
|---|---|---|
| 1. El resumen del marcado coincide con el de la prueba | prueba ↔ **este** marcado | presentar una prueba válida de **otra** entrada |
| 2. La prueba de inclusión verifica contra la raíz | marcado ↔ inventario | inventar una entrada que nunca estuvo |
| 3. La firma híbrida verifica sobre la raíz | inventario ↔ administrador | sustituir el inventario entero |
| 4. La clave pertenece al dominio del cliente | administrador ↔ custodia | firmar marcados con la clave de PremosCorp |

### 3.1 El eslabón 1 es el que se olvida

`verificar_inclusion` comprueba que la prueba es internamente consistente con la raíz. **Nada en ella la ata al marcado que se está verificando.** Quien presente la prueba legítima de la entrada 1 la pasaría como si fuera la entrada 0, y el eslabón 2 no protestaría.

No es hipotético: la firma de la función invita al error, porque recibe la prueba y la raíz pero no el dato. `una_prueba_de_otra_entrada_se_rechaza` existe por eso.

De ahí sale también `alterar_el_marcado_rompe_la_cadena`, que cubre el ataque útil de verdad: degradar «soporte vital» a «no crítico». El marcado alterado produce otro resumen, así que deja de coincidir con la prueba.

### 3.2 Errores distintos por eslabón

`PruebaAjenaAlMarcado` e `InclusionNoVerifica` describen ataques diferentes. Colapsarlos en un genérico «no verifica» ocultaría cuál se intentó, y esa información es forense.

## 4. Frontera de custodia: `DominioClave` es un tipo, no un comentario

```rust
pub enum DominioClave { Cliente, PremosCorp }
```

Las dos claves tienen custodios distintos y confundirlas es grave en ambas direcciones:

- **PremosCorp no debe poder declarar qué equipos del cliente son críticos.** Con la infraestructura de PA-14 reutilizada «por comodidad», el proveedor podría marcar como incontenible cualquier equipo del parque de su cliente.
- **El cliente no debe poder firmar nada que el agente cargue como código.** Es la dirección que suele revisarse; la otra no.

El dominio se declara **al aprovisionar**, no al usar: una clave que llega sin dominio no puede adquirirlo más tarde. Y el eslabón 4 se comprueba **antes** que ningún otro: una clave del dominio equivocado no debe llegar a tocar dato alguno.

## 5. Verificación

`crates/guardian-cc` pasa de 32 a **39 pruebas**. Las del inventario recorren la cadena real: generan par de claves, construyen el árbol, firman la raíz y piden la prueba de inclusión. **No existe atajo para fabricar un `MarcadoVerificado`**, que es exactamente el punto del reporte.

Seis mutaciones, todas atrapadas:

| Mutación | Prueba que falla |
|---|---|
| Se omite el eslabón 1 | `una_prueba_de_otra_entrada_se_rechaza` **y** `alterar_el_marcado_rompe_la_cadena` |
| Se omite el eslabón 2 | `una_raiz_ajena_no_verifica` |
| Se omite el eslabón 3 | `suprimir_una_entrada_invalida_la_firma_de_la_raiz` |
| Se omite el eslabón 4 | `la_clave_de_premoscorp_no_firma_inventarios` |
| La clase se codifica como texto libre | `alterar_el_marcado_rompe_la_cadena` **y** `el_resumen_del_marcado_separa_su_dominio` |
| El mensaje de raíz pierde su dominio | `el_resumen_del_marcado_separa_su_dominio` |

`suprimir_una_entrada_invalida_la_firma_de_la_raiz` comprueba de extremo a extremo lo que RPT-010 §4 razonó: se borra la entrada «esta bomba es soporte vital», se recalcula la raíz del inventario mutilado, y la firma del administrador deja de verificar contra ella.

### 5.1 Generador determinista, no `rand`

Las pruebas usan un generador determinista propio en lugar de `rand::rng()`. Dos razones: `ThreadRng` de rand 0.9 no implementa `CryptoRng` de rand_core 0.10, que es la versión que usa `motor-pqc`; y tres versiones de `rand_core` conviviendo ya nos costaron una sesión. El generador replica el de `motor-pqc` en vez de exportarse desde allí: un generador de pruebas en la API pública de un crate criptográfico es una invitación a usarlo fuera de las pruebas.

## 6. Lo que este mecanismo no cubre

1. **La reversión (*rollback*) del inventario.** Un inventario **anterior**, legítimamente firmado, verifica los cuatro eslabones sin problema. Un atacante con acceso al almacén local puede restaurar la versión de antes de que se marcara un equipo como crítico. Hace falta número de versión monótono dentro del mensaje firmado y rechazo de versiones que retrocedan. Es PA-27, y es una omisión de este reporte, no una limitación de alcance.
2. **La revocación de la clave del administrador.** Si la clave del cliente se compromete, no existe vía de retirarla.
3. **La distribución del inventario.** Quién lo escribe, cómo llega al agente y con qué periodicidad no está definido.
4. **La MAC sigue siendo el índice.** RPT-010 §6.1 ya lo documenta: quien suplanta la MAC de un equipo crítico se vuelve incontenible. Esto no lo corrige; por eso la prohibición alerta.

## 7. Reservas explícitas

- `ProveedorInventario` **sigue sin implementarse**. Este reporte entrega la verificación; falta el adaptador que lea el inventario del almacén local y lo exponga por el trait. Es mecánico, pero no está hecho, y decir «PA-24 cerrado» sería falso.
- El árbol Merkle usa las hojas en el orden en que se pasan. Nada fija el **orden canónico** de las entradas del inventario, así que dos productores que ordenen distinto producirán raíces distintas para el mismo contenido. Debe fijarse —por MAC ascendente es lo natural— antes de que exista más de un productor.

La segunda reserva es del mismo tipo que el eslabón 1: algo que funciona hoy porque sólo hay un implementador.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-27** | **Reversión del inventario y revocación de clave.** Versión monótona firmada, rechazo de retrocesos y vía de retirada de la clave del administrador | Despliegue en sitio con almacén local escribible |

---

*Reporte Nº 11 — Inventario Firmado de Marcados Administrativos · PremosCorp · 5 de agosto de 2026*
