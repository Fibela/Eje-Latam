# RPT-017 — Cableado de Persistencia y Semántica de Arranque (Diseño)

**Tema:** Qué hace el agente al arrancar, y cuándo escribe
**Nº de reporte:** 017
**Fecha:** 5 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** Canónico — diseño ratificado e implementado

- **Depende de:** RPT-016 (persistencia de revocaciones), RPT-014 (E/S atómica), RPT-012 (centinela), RPT-009 (clasificación)
- **Cierra:** PA-35
- **Abre:** PA-36

---

## 1. La deuda que cierra

Es la tercera vez que el proyecto acumula mecanismo verificado sin cableado: `disco.rs` en RPT-014, `ArchivoRevocaciones` en RPT-016, y `InventarioLocal::cargar` sigue recibiendo `&[u8]` de nadie. Cada pieza está probada por separado y ninguna corre en un agente.

Atacarlo entero, y no por partes, porque los huecos que importan aparecen al montar el conjunto — como pasó con el recorrido de extremo a extremo de RPT-013.

## 2. Hallazgo: borrar el inventario es un ataque

Es la razón de que esto sea un reporte de diseño y no un commit.

Sin inventario, ningún dispositivo tiene marcado. Y por RPT-009 §5, un dispositivo sin marcar **en un segmento declarado limpio es contenible**. Luego:

> **Un atacante que borre el fichero de inventario vuelve contenible un equipo de soporte vital** que estaba protegido por su marcado.

El ataque no requiere claves, ni firmas, ni entender el formato. Requiere `del`.

Y el fichero ausente es, a primera vista, indistinguible de un primer arranque legítimo.

### 2.1 El centinela ya es el testigo

No hace falta estado nuevo. `Centinela` distingue exactamente lo que se necesita:

| Inventario | Centinela | Lectura | Qué hacer |
|---|---|---|---|
| ausente | `SinEstablecer` | primer arranque legítimo | degradar con elegancia (§3) |
| ausente | `Establecido(n)` | **hubo un inventario y ya no está** | manipulación: bloquear contención |
| presente, no verifica | cualquiera | manipulación | bloquear contención |
| presente, verifica | — | normal | operar |

Un agente que alguna vez aceptó un inventario tiene el centinela establecido. Si el fichero desaparece, eso no es un primer arranque: es una supresión.

Es el mismo razonamiento de RPT-012 §3 con `FrescuraNoEstablecida` —la ausencia de centinela no se lee como «primera vez»— aplicado en la otra dirección.

## 3. Sin inventario, el agente **no** se niega a arrancar

La tentación es abortar. Sería un error.

El agente hace dos cosas: **observar** y **contener**. Un inventario ausente sólo afecta a la segunda. Negarse a arrancar apagaría también la observación, y en un hospital eso significa quedarse sin vigilancia — un daño cierto para evitar uno hipotético.

Arranca, observa, y la contención automática queda deshabilitada por el camino que ya existe: sin marcados, la clasificación resuelve por segmento, y donde pueda haber críticos sale `Ambiguo` → aprobación humana.

**El mecanismo no es nuevo. Es el de RPT-009 funcionando como se diseñó.**

## 4. Ausencia frente a fallo de verificación

RPT-010 §4 estableció que no deben confundirse, y hasta ahora nadie estaba en posición de confundirlos porque nadie leía ficheros. Al cablear, la distinción se vuelve operativa:

- **Ausente y primer arranque** → `ProveedorInventario::marcado` devuelve `Ok(None)` para todo. Los dispositivos caen a las reglas de segmento.
- **Ausente con centinela establecido**, o **presente y no verifica** → `Err(ErrorProveedor::FirmaInvalida)` → `EvidenciaNoVerificable` → ambigüedad → ninguna contención automática.

## 5. Orden de escritura, y por qué una decisión vieja lo salva

Al aceptar un inventario de secuencia N hay que avanzar el centinela. Si el proceso muere entre ambas cosas, ¿qué queda?

- **Centinela primero, luego usar el inventario.** Si muere en medio: centinela en N, inventario en N presente en disco. Al rearrancar, `secuencia == aceptada`, y RPT-012 §4.4 eligió `secuencia < aceptada` para el rechazo, **no** `<=`. El inventario se vuelve a aceptar sin problema.
- **Inventario primero, centinela después.** Si muere en medio: se actuó sobre un inventario cuya secuencia no quedó registrada, y se reabre la ventana de reversión.

Luego: **centinela primero.** Y funciona porque hace cuatro reportes se decidió admitir la reemisión de la misma secuencia. Esa decisión se tomó por otro motivo —evitar incrementar el contador en cada relectura— y resulta ser lo que hace segura esta secuencia de escritura.

## 6. Rutas y permisos

```text
<datos>/inventario.inv       inventario firmado
<datos>/revocaciones.rev     certificados de revocación
<datos>/centinela.dat        marca de agua
```

Tres decisiones:

1. **Un directorio de datos, no rutas sueltas.** Se pasa al agente al arrancar; no se codifica.
2. **Sin permiso de escritura, el agente arranca en sólo lectura y avisa.** No poder persistir el centinela significa no poder protegerse de reversiones, pero sigue pudiendo observar. Misma lógica del §3.
3. **El centinela no se firma.** Es estado local propio, no dato recibido. Que sea rebobinable por quien controle el disco es PA-28, y sigue abierto.

## 7. Lo que este diseño no resuelve

1. **La ausencia del fichero de revocaciones nunca es sospechosa.** A diferencia del inventario, no hay testigo equivalente al centinela. RPT-015 §5 ya lo acepta: perderlo devuelve al estado previo, y el certificado se puede volver a presentar. Pero conviene tenerlo escrito junto al §2, porque la asimetría entre ambos ficheros no es evidente.
2. **Nada vigila el directorio de datos en caliente.** Un borrado con el agente en marcha no se detecta hasta el siguiente arranque.
3. **La rotación y el tamaño del almacén** —cuántos inventarios históricos conservar, si alguno— no se tocan.

El punto 2 es el que más se parece a un hueco real: el §2 protege el arranque, no la operación.

## 8. Verificación

`crates/guardian-cc` pasa de 97 a **107 pruebas**; el workspace, de 208 a **218**. Clippy limpio.

### 8.1 El estado de arranque **es** el proveedor

`EstadoArranque` implementa `ProveedorInventario`. No es un detalle de estilo: elimina la capa de traducción donde alguien podría convertir una supresión en «no hay marcado». La distinción de RPT-010 §4 deja de depender de la disciplina de quien cablee y pasa a estar en el tipo.

### 8.2 Las dos pruebas van en pareja

- `la_supresion_no_desemboca_en_contencion` mide el **veredicto**, no el estado, y sobre un segmento declarado limpio — que es exactamente donde el borrado convertiría un equipo crítico en contenible.
- `un_primer_arranque_si_permite_contener_en_segmento_limpio` es la otra mitad. Sin ella, un mecanismo que bloqueara **siempre** pasaría por bueno, y una instalación nueva no contendría nada nunca: la parálisis que RPT-009 §5 resolvió.

Leerlas por separado da una impresión equivocada de cualquiera de las dos.

### 8.3 El centinela corrupto no se degrada

Decisión tomada al escribir el formato, no al diseñar: si corromper dieciocho bytes simulara un primer arranque, borrar el inventario volvería a funcionar — el ataque del §2 entrando por otra puerta. `un_centinela_corrupto_no_se_degrada_a_primer_arranque` cubre cuatro formas de corrupción: vacío, ceros, truncado y versión desconocida.

## 9. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-36** | **Vigilancia del directorio de datos en caliente.** Detectar la supresión del inventario mientras el agente corre, no sólo al arrancar | Cobertura completa del ataque del §2 |

---

*Reporte Nº 17 — Cableado de Persistencia y Semántica de Arranque (Diseño) · PremosCorp · 5 de agosto de 2026*
