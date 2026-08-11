# RPT-027 — Clave de recuperación repartida

**Tema:** Que comprometer la semilla del cliente tenga remedio
**Nº de reporte:** 027
**Fecha:** 6 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-54

- **Depende de:** RPT-015 §§4 y 8.1 (PA-32, custodia 2-de-3 ratificada), RPT-026 (emisor)
- **Cierra:** PA-54

---

## 1. El mecanismo llevaba una sesión sin dueño

RPT-015 dejó la revocación entera: certificado, registro, sexto eslabón, reinicio del centinela. RPT-026 dejó el emisor. Y entre los dos quedaba un hueco: **la clave que firma los certificados no existía**, porque generarla en el mismo comando que la operativa habría anulado la separación que RPT-015 §4 establece.

Sin ella, comprometer la semilla del cliente no tiene remedio. No hay forma de revocar.

## 2. Por qué se escribe Shamir en lugar de traerlo

El reparto de secreto es de los pocos esquemas donde escribirlo es defendible: **es incondicionalmente seguro** si los coeficientes son aleatorios, y no depende de suposiciones sutiles de implementación como las que hacen peligroso escribir una curva o un AEAD a mano. La alternativa era una dependencia más cuya API no puedo verificar contra el compilador.

La multiplicación en GF(2⁸) va **sin tablas de logaritmos**. Con tablas sería más corta y filtraría por caché; el bucle usa `0u8.wrapping_sub(bit)` como máscara, que vale `0xFF` o `0x00` y sustituye al `if` sin introducir un salto que dependa del operando.

## 3. Shamir no autentica, y eso es lo que casi se nos escapa

Un custodio que entregue un fragmento alterado **no hace fallar la reconstrucción**. Produce otro secreto, en silencio.

El modo de fallo es concreto y malo: quien reúne cree tener la clave de recuperación, firma un certificado de revocación, y el agente lo rechaza — **en mitad del incidente que motivó la reconstrucción**, que es el peor momento imaginable para descubrir que dos custodios no cuadran.

Cada fragmento lleva la **huella de la clave pública** que se deriva del secreto original. Tras reunir se re-deriva y se compara. No es material secreto —es la misma clave pública que se aprovisiona en el agente— y cierra el hueco.

**Lo que no hace: decir quién mintió.** Detecta que el conjunto no cuadra, no cuál de los dos fragmentos está mal. Distinguirlo exigiría un compromiso por fragmento y no lo tenemos. Queda escrito para que nadie lo suponga en una auditoría.

## 4. Tres cosas pequeñas que sostienen el umbral

**El índice 0 no es un fragmento.** `f(0)` **es** el secreto, así que un fichero con índice 0 sería el secreto entero. Se rechaza.

**Dos fragmentos del mismo custodio se rechazan.** Dos puntos con la misma abscisa no determinan una recta, y aceptarlo significaría dividir por cero. Pero el efecto real es peor que un error numérico: **el umbral de dos dejaría de ser dos**, porque un custodio podría reconstruir presentando su fragmento dos veces.

**El fichero declara umbral y custodios.** Redundante con el formato, y a propósito: dentro de unos años, quien encuentre uno de estos ficheros en una caja fuerte necesita saber cuántos hermanos tiene sin depender de que el procedimiento siga escrito en algún sitio.

## 5. `recuperacion` y `revocar` son comandos separados de `generar`

Producir las dos claves en el mismo comando las dejaría juntas en la misma máquina y en el mismo instante. **La separación criptográfica no sobrevive a una separación operativa que nadie hace.**

`recuperacion` no escribe el secreto entero en ningún sitio: sólo salen los tres fragmentos y la clave pública. Y avisa de dos cosas —que hay que repartirlos y borrarlos de esa máquina, y de la salvedad de RPT-015 §8.1 sobre custodios de la misma organización.

`revocar` **anexa** al registro existente en lugar de sustituirlo. Reescribirlo borraría revocaciones anteriores, y una revocación que desaparece es exactamente lo que RPT-015 impide que ocurra en silencio.

## 6. La salvedad de RPT-015 §8.1 sigue en pie

Dos de los tres fragmentos viven dentro de la misma organización —seguridad del cliente y operaciones de TI—; sólo el de custodia bancaria queda fuera. **El umbral efectivo frente a un compromiso profundo o a un interno es menor que 2-de-3 nominal.**

El código no puede arreglar eso, así que el comando lo dice por pantalla al generar. Si se quiere el umbral real, dos de los tres custodios deben ser externos, o hay que subir a 3-de-5 — y eso último es un cambio de constantes, no de diseño.

## 7. Lo que sigue sin resolverse

1. **Nadie detecta el compromiso.** RPT-015 §9.3 ya lo decía y sigue igual: toda esta mecánica se activa por un hecho que ocurre fuera del sistema. Es lo que más pesa de todo lo escrito hoy.
2. **Cómo llega el certificado al agente** sigue siendo PA-31, y es operativo.
3. **La calidad de los coeficientes es la seguridad del esquema.** Vienen de `getrandom`; si esa fuente falla el programa se niega a continuar, pero nada comprueba que los bytes sean buenos, porque no se puede.
4. **Reunir exige juntar dos fragmentos en una máquina.** Ese instante es el punto más frágil del esquema y no está instrumentado. Debería ser una máquina aislada, y eso es procedimiento (PA-51).

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| PA-54 | — | ✅ **Cerrado por este reporte** |
| PA-55 | Elevar el reparto a 3-de-5, o exigir dos custodios externos | El umbral **real**, no el nominal |

---

*Reporte Nº 27 — Clave de recuperación repartida · PremosCorp · 6 de agosto de 2026*
