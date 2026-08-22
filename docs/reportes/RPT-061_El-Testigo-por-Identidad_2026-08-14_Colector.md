# RPT-061 — El testigo por identidad, y un defecto construido a propósito

**Tema:** PA-115. El sello no llevaba interfaz, y el cotejo acusaba de manipulación a dos sensores intactos
**Nº de reporte:** 061
**Fecha:** 14 de agosto de 2026
**Área designada:** Colector
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado en ejecución real.** Cierra PA-115

- **Depende de:** RPT-038 (el testigo externo), RPT-059 (identidad compuesta en el latido), RPT-057 (el vigía)
- **Aborda:** PA-115

---

## 1. La mitad que faltaba

RPT-038 dice que el colector guarda la serie de extremos y que **la discrepancia
se ve fuera del equipo comprometido**, que es el único sitio donde puede verse.

Nadie la guardaba. El vigía descartaba los sellos: `analizar` exigía que el
identificador de mensaje fuera `latido-de-sensor`, y había una prueba explícita
de que una línea de sello devolvía `None` — escrita para que el latido y el sello
no se confundieran, y correcta.

Así que la acusación falsa de PA-115 **no tenía observador**. Emitir sellos sin
cotejarlos es la misma media pieza que era el latido sin vigía (RPT-052 §6).

## 2. Se construyó con el defecto intacto, a propósito

Éste es el punto de método del reporte.

El módulo `sellos` se escribió primero **indexando por `HOSTNAME` sin interfaz**,
que era lo único que el sello decía. Se sabía que estaba mal. Se escribió así
para que el defecto fuera observable.

Y llevó dos pruebas que **describían el defecto en vez de garantizar algo**, con
el aviso en el comentario de que tenían que dejar de pasar al corregir PA-115:

```rust
#[test]
fn dos_agentes_en_una_maquina_se_acusan_entre_ellos() {
    // ESTA PRUEBA DESCRIBE UN DEFECTO, NO UNA GARANTIA.
```

Una prueba que fija el comportamiento roto es normalmente un error grave. Aquí
era el instrumento, y por eso llevaba escrito cuándo dejaba de serlo.

## 3. La observación, antes de tocar el sello

Dos agentes en la **misma máquina**, uno sobre `lo` con 100 asientos sembrados y
otro sobre un `veth` con 40. Registros independientes, los dos íntegros, los dos
verificando.

```
SELLO BASE  LapTap-AF: extremo anotado en el asiento 100. Nada que cotejar todavia.
LINEA BASE  LapTap-AF/lo: primer latido (numero 1).
!! RECORTE  LapTap-AF: el registro tenia 100 asientos y ahora declara 40.
   El ancla local no ve esto: quien recorta puede recalcularla (RPT-038).
LINEA BASE  LapTap-AF/veth-eje: primer latido (numero 1).
```

**Nadie había tocado nada.** La acusación más seria que este sistema sabe emitir,
dirigida a un servidor que funcionaba bien.

La misma pantalla enseña el contraste completo: el latido ya distinguía
`LapTap-AF/lo` de `LapTap-AF/veth-eje` —RPT-059— y el sello seguía viendo un solo
`LapTap-AF`. El mecanismo arreglado y el roto, con los mismos dos agentes.

## 4. La corrección, y la prueba que la protege

El sello declara `interfaz=` y el testigo lleva la serie por identidad completa.
Las dos pruebas del §2 se sustituyeron por sus contrarias.

Y se añadió una tercera que importa más que las dos: **`y_el_recorte_de_verdad_se_sigue_viendo`**.
Lo peligroso de arreglar un falso positivo es silenciar el verdadero. El mismo
agente —misma máquina, misma interfaz— con un registro que encoge sigue acusando.
Sin esa prueba, «ya no acusa a nadie» y «ya no detecta nada» son indistinguibles.

`DatosSello` con campos nombrados por el mismo motivo que `DatosLatido`
(RPT-059 §4): `maquina` e `interfaz` vuelven a ser dos `&str` adyacentes, e
invertirlos compilaría sin una queja.

## 5. La observación que cierra PA-115

Mismo banco, con la corrección:

```
SELLO BASE  LapTap-AF/lo: extremo anotado en el asiento 100. Nada que cotejar todavia.
LINEA BASE  LapTap-AF/lo: primer latido (numero 1).
APARECE  LapTap-AF/lo: informa por primera vez. Ya no falta del censo.
SELLO BASE  LapTap-AF/veth-eje: extremo anotado en el asiento 40. Nada que cotejar todavia.
LINEA BASE  LapTap-AF/veth-eje: primer latido (numero 1).
APARECE  LapTap-AF/veth-eje: informa por primera vez. Ya no falta del censo.
```

Dos líneas base con interfaz. **Ningún `RECORTE`.**

La predicción se escribió antes de tocar el código, con las tres formas de salir
mal enumeradas: si aparecía la acusación, la corrección no había llegado al
cable; si faltaba el segundo sello, habríamos silenciado el testigo en vez de
arreglarlo; si los dos salían sin interfaz, el binario en ejecución era el viejo.

## 6. Dos montajes fallidos, y ninguno era el mecanismo

Entre la observación del §3 y la del §5 hubo dos intentos que no salieron:

- **Orden invertido**: los agentes antes que el vigía. Los latidos llegaron
  igualmente —el contador sólo se consume tras un envío correcto— pero eso no
  explicaba la falta de sellos.
- **`/tmp` vaciado al reiniciar la máquina.** Los almacenes sembrados
  desaparecieron, los agentes arrancaron con el registro vacío, y la guarda de
  PA-64 —«un registro vacío no sella nada»— hizo lo correcto: callar.

El segundo se diagnosticó **sin tocar código**, preguntando por la salida del
agente antes de suponer. Ya van seis montajes fallidos en dos días de pruebas de
fuego y ninguno ha sido la pieza; conviene anotarlo para no perder la confianza
en el mecanismo cada vez que una pantalla sale en blanco.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-115~~ | ✅ **Cerrado por observación** (§5) |
| PA-112 | Firmar el latido. Ni el contador ni la interfaz impiden la suplantación |
| PA-107 | Empaquetado dual |

> El tablero de RPT-002 §12 lleva el estado del presente. Esta tabla no lo
> repite (RPT-060 §5).

---

*Reporte Nº 61 — El testigo por identidad · PremosCorp · 14 de agosto de 2026*
