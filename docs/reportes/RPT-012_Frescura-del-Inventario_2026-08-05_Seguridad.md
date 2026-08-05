# RPT-012 — Frescura del Inventario y Orden Canónico

**Tema:** Protección contra reversión de estado y determinismo de la raíz
**Nº de reporte:** 012
**Fecha:** 5 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** Canónico con reservas explícitas — véase §5

- **Depende de:** RPT-011 (inventario firmado), `eje-almacen` (Merkle y `Absorbedor`)
- **Cierra:** PA-27 **parcialmente** — véase §5.1; cierra la reserva 2 de RPT-011 §7
- **Abre:** PA-28

---

## 1. El ataque

La firma de un inventario de la semana pasada es perfectamente válida. Lo que no es válido es que describa un estado del parque ya superado.

Quien comprometa el almacén local no puede falsificar la firma del administrador, pero sí restaurar el fichero legítimo emitido **antes** de que la bomba de infusión se marcara como soporte vital. Los cuatro eslabones de RPT-011 se cierran sin objeción: firma correcta, inclusión probada, dominio correcto, resumen coincidente. Y el dispositivo vuelve a ser contenible.

La cadena pasa a tener **cinco eslabones**. El quinto ata el inventario a un **momento**.

## 2. Corrección 1 — dónde vive el orden canónico

La propuesta pedía que el árbol Merkle de `eje-almacen` ordenase sus hojas por dirección. **Sería un error de capa, y destructivo.**

Ese árbol sirve al registro forense ALM-01, donde el orden es **cronológico y significativo**. Existe una prueba, `reordenar_asientos_rompe_la_cadena`, cuyo propósito es exactamente impedir que se reordene. Ordenar allí corrompería la evidencia que el producto promete conservar.

El orden canónico es una propiedad **del inventario**, no del árbol. Vive en `Inventario::construir`, que ordena por dirección ascendente y —añadido no pedido— **rechaza duplicados**: sin ese control un lector indulgente elegiría entre dos entradas contradictorias, y una de las dos elecciones favorece a quien añade una segunda entrada «no crítico».

## 3. Corrección 2 — la marca de agua no es la solución completa

La propuesta decía: *«almacenar y comparar el último número de secuencia validado en el estado local»*. Eso lo derrota el mismo atacante que motiva la defensa: **si el centinela vive en el almacén que el atacante controla, restaura ambos de forma consistente y no queda rastro.**

La protección completa contra reversión exige un ancla fuera del almacén escribible —contador monótono en TPM o elemento seguro—, y eso no está disponible en todos los destinos. Decir lo contrario sería vender una garantía que no tenemos.

Lo que sí se consigue, y no es poco: **que la reversión no sea silenciosa.**

```rust
pub enum Centinela {
    Establecido(u64),
    SinEstablecer,   // legítimo SOLO durante el aprovisionamiento
}
```

Un centinela ausente **no se lee como «primera vez»**, sino como `FrescuraNoEstablecida`. Si la ausencia significara «primera vez», bastaría borrarlo para desactivar toda la protección. Borrarlo debe costar lo mismo que rebobinarlo: un rechazo visible.

El aprovisionamiento inicial es una operación distinta y explícita, `RaizVerificada::aprovisionar`, que se invoca con un humano presente durante la instalación.

## 4. Diseño

### 4.1 La secuencia viaja dentro del mensaje firmado

```rust
pub struct RaizAnclada { pub raiz: Resumen, pub secuencia: u64 }
```

Firmar la raíz por un lado y la secuencia por otro permitiría recombinar la raíz vieja con la secuencia nueva. `recombinar_raiz_vieja_con_secuencia_nueva_no_verifica` cubre exactamente eso.

### 4.2 `RaizVerificada`: se verifica una vez, sirve para muchos marcados

Los eslabones 3, 4 y 5 —firma, dominio y frescura— son propiedades del inventario, no de cada entrada. Se comprueban una vez al construir `RaizVerificada`; los eslabones 1 y 2 se comprueban por marcado.

`MarcadoVerificado::verificar_e_instanciar` **exige** una `RaizVerificada`. No existe forma de llegar a un marcado verificado partiendo de una raíz sin firmar, de otro dominio o revertida: el tipo lo impide.

### 4.3 La frescura se comprueba antes que la firma

Deliberado. Un inventario revertido tiene firma válida; verificar primero la firma dejaría un «firma correcta» engañoso en el registro antes del rechazo real. El orden es dominio → frescura → firma.

### 4.4 Reemitir sin cambios no es un ataque

La comparación es `secuencia < aceptada`, no `<=`. Retroceder se rechaza; repetir la misma secuencia se admite. Exigir estricto crecimiento obligaría a incrementar el contador en cada relectura del mismo fichero, lo que multiplica el estado sin comprar seguridad.

## 5. Verificación

`crates/guardian-cc` pasa de 39 a **48 pruebas**. Seis mutaciones, todas atrapadas:

| Mutación | Prueba que falla |
|---|---|
| Se omite la comprobación de reversión | `un_inventario_anterior_se_rechaza_pese_a_firma_valida` |
| El centinela ausente se lee como primera vez | `borrar_el_centinela_no_se_lee_como_primera_vez` |
| La secuencia sale del mensaje firmado | `la_secuencia_viaja_dentro_del_mensaje_firmado` **y** `recombinar_raiz_vieja_con_secuencia_nueva_no_verifica` |
| El centinela retrocede al avanzar | `el_centinela_nunca_retrocede` |
| El inventario no ordena sus hojas | `el_orden_de_entrada_no_altera_la_raiz` **y** `un_dispositivo_declarado_dos_veces_se_rechaza` |
| Se admiten dispositivos duplicados | `un_dispositivo_declarado_dos_veces_se_rechaza` |

`el_orden_de_entrada_no_altera_la_raiz` pasa los marcados **desordenados a propósito**: el orden lo impone `Inventario::construir`, no quien los escribe.

### 5.1 Por qué PA-27 cierra sólo parcialmente

Lo que queda fuera es §3: **el centinela sigue siendo tan rebobinable como el almacén donde viva.** Este reporte entrega el mecanismo de detección y la política de fallo seguro; no entrega el ancla de confianza. Declarar PA-27 cerrado sería afirmar una garantía que el código no da.

Se abre PA-28 para el ancla: contador monótono en TPM 2.0 donde exista, y decisión explícita sobre qué hacer en los destinos donde no exista —que en OT serán muchos, porque el parque es antiguo.

## 6. Reservas explícitas

1. **La revocación de la clave del administrador sigue sin resolver** (RPT-011 §6.2). Si la clave del cliente se compromete, no hay vía de retirarla, y la secuencia no ayuda: el atacante con la clave emite secuencias crecientes.
2. **`ClaveInventario::dominio()` no se usa** fuera de la verificación interna. Se conserva porque un consumidor futuro querrá auditarlo, pero hoy nadie lo llama.
3. **`ProveedorInventario` sigue sin implementarse.** Este reporte y RPT-011 entregan la verificación; falta el adaptador que lea del almacén local. PA-24 continúa parcial.
4. **La secuencia no está atada al reloj.** Un inventario con secuencia alta y fecha de emisión antigua se acepta. Atarlas exigiría confiar en el reloj local, que RPT-011 §5.1 ya trata como poco fiable. Se deja así conscientemente.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-28** | **Ancla de confianza para el centinela.** Contador monótono fuera del almacén escribible —TPM 2.0 donde exista— y política explícita para los destinos sin él | Protección completa contra reversión en sitio comprometido |

---

*Reporte Nº 12 — Frescura del Inventario y Orden Canónico · PremosCorp · 5 de agosto de 2026*
