# RPT-073 — El paquete llega entero

**Tema:** PA-126. Formato de distribución e integridad, con la autenticidad declarada ausente
**Nº de reporte:** 073
**Fecha:** 17 de agosto de 2026
**Área designada:** Producto
**Entidad:** PremosCorp
**Estado:** Construido y probado. **La mitad de autenticidad queda bloqueada por PA-14a**

- **Depende de:** RPT-054 §9 (el formato sin decidir), RPT-069 §5 (por qué apareció el punto), RPT-021 §8 (PA-14a), RPT-006 §4
- **Aborda:** PA-126

---

## 1. El punto llevaba días gritando y no tenía fila

`cargo xtask empaquetar` imprimía en **cada ejecución**:

```
El formato del paquete sigue sin decidirse (RPT-054 §9).
```

Nadie lo contaba. Apareció al cerrar PA-107 y se acuñó entonces (RPT-069 §3).

## 2. Lo que el diseño encontró antes de escribir código

La propuesta era `.tar.gz` firmado con ML-DSA y verificado por el instalador. Al
mirar dónde viviría la clave de verificación aparecieron dos hechos:

**`verificarPaquete` no sirve.** Es TypeScript, vive en la consola y verifica
módulos empresariales de VIS-02. El sensor es headless, tiene `sh` y **no tiene
Node**.

**`DominioClave::PremosCorp` ya existe** en `guardian-cc`, y su documentación
dice literalmente *«firma binarios, reglas e imágenes de release (PA-14)»*. Pero
`RutasAlmacen` sólo tiene sitio para dos claves —la operativa del cliente y la de
recuperación—: **el dominio existe en el vocabulario y no tiene dónde vivir en el
sensor**.

Y PA-14a está en rojo con esta frase en el tablero:

> *No se puede entregar a un cliente un agente sin firmar que además le pide
> confiar en firmas.*

Añadir la ruta de la clave hoy sería un mecanismo sin cablear: un sitio donde
guardar algo que nadie puede emitir. **Se documenta y no se acuña punto propio**,
porque no se puede trabajar sin PA-14a y un punto que sólo puede esperar es un
punto que estorba en el recuento.

## 3. Lo que sí se cerró: formato e integridad

El artefacto pasa a ser `target/paquete/eje-agente.tar.gz`, con un `MANIFIESTO`
dentro que lista cada fichero con su SHA-256.

**El formato del manifiesto está prestado a propósito**: `<resumen>  <nombre>`,
que es exactamente lo que come `sha256sum -c`. Un formato propio obligaría al
instalador a llevar un analizador escrito en `sh`, y un analizador en `sh` dentro
del guion que decide si se instala es la última pieza que este proyecto quiere
escribir.

**El manifiesto sale del disco**, no de la lista que el empaquetador creyó
escribir. Es PA-107 otra vez: allí la revisión se movió de la lista al artefacto
por el mismo motivo.

## 4. El instalador comprueba antes de tocar nada

```sh
if ! sha256sum -c MANIFIESTO >/dev/null 2>&1; then
    echo "!! EL PAQUETE NO LLEGO ENTERO."
    …
    exit 1
fi
```

Y **falla cerrado sin la herramienta**: si no hay `sha256sum`, no se instala. No
poder comprobar no es haber comprobado (RPT-006 §4); un instalador que sigue
adelante porque le falta el instrumento es el verde que no afirma nada.

Hay prueba de que la comprobación va **antes** de la primera escritura.
Comprobar después de copiar dejaría media instalación hecha con ficheros que no
son los que se enviaron, que es peor que no comprobar.

## 5. Y dice a gritos lo que **no** ha comprobado

```
!! ESTE PAQUETE NO ESTA FIRMADO.
   Se comprobo que llego entero, NO que venga de PremosCorp.
   Quien pueda sustituir el paquete puede recalcular los resumenes.
   La firma de release es PA-14a y exige custodia en hardware.
```

Es la disciplina de `EJE_COLECTOR=` vacío (RPT-054 §4.1): el hueco se envía
declarado, no escondido. Un instalador que dijera «verificado» habiendo mirado
sólo resúmenes sería **peor** que uno que no mira nada, porque el operador creería
tener una cadena de confianza que no tiene.

## 6. El paquete es reproducible, y eso es para PA-14a

`tar` guarda fecha, usuario y grupo; `gzip` guarda otra fecha. Con los del
sistema, dos empaquetados del mismo árbol dan ficheros distintos — y el día que
haya firma, **la firma cambiaría sin que cambie nada de lo firmado**.

Las cabeceras se escriben a mano con fecha, uid y gid a cero. El contenido es lo
único que decide los bytes de salida. El manifiesto va ordenado por la misma
razón.

## 7. La prueba que convierte la comprobación en algo más que adorno

La caja de arena copia el artefacto, **le añade un byte al binario** y ejecuta el
instalador. Exige dos cosas, no una:

- que se niegue, y
- que **no quede nada instalado**.

Sin la segunda, un instalador que detectara el daño después de copiar el binario
pasaría la prueba dejando el sistema con un fichero que no es el que se envió.

Sin esta prueba entera, un `sha256sum -c` que siempre dijera «OK» habría pasado
en verde hasta el día del despliegue.

## 8. Dos dependencias nuevas en `xtask`, y por qué no se llamó al `tar` del sistema

`tar` y `flate2`. La alternativa era invocar el `tar` del sistema, y eso rompería
lo que `xtask` existe para garantizar: que corre idéntico en Windows, Linux y CI
sin bash ni PowerShell (RPT-003 §9.5). Rompería el día que alguien empaquete desde
Windows, que es el escenario normal en esta máquina.

Las dos son MIT/Apache-2.0 y caen dentro de la lista permitida de `deny.toml`.

## 9. Lo que este reporte **no** puede afirmar

Que el paquete venga de PremosCorp. No lo afirma nadie todavía, y el instalador
lo dice.

Tampoco que el `.tar.gz` se descomprima limpio en una máquina de verdad: está
probado en unidad, y la observación —desempaquetar, instalar desde el tarro y ver
la línea de integridad— sigue pendiente.

## 11. El aviso que pasó a mentir al revés

`cargo xtask empaquetar` seguía imprimiendo, con el `.tar.gz` ya en el disco:

```
El formato del paquete sigue sin decidirse (RPT-054 §9).
```

Es **la línea que acuñó PA-126** (RPT-069 §3), y al cerrarse el formato pasó a
mentir en la dirección contraria: declaraba indeciso lo que estaba decidido y
callaba lo único que de verdad falta.

Ahora dice lo que falta hoy:

```
El paquete lleva resumenes y NO firma: se comprueba que llega entero,
no de donde viene (PA-14a).
```

Merece quedar escrito porque es una forma de envejecimiento que no habíamos
visto: no es un aviso que se queda corto, es uno que **sigue disparando después
de resolverse** y que, por seguir ahí, tapa el hueco siguiente. Un instrumento
que dice algo falso enseña a ignorarlo igual que uno que calla.

## 12. Lo observado

```
  MANIFIESTO
  agente.conf.ejemplo
  eje-agente
  eje-agente.service
  instalar.sh
  -> eje-agente.tar.gz

  PASA   un paquete alterado NO se instala, y no deja nada a medias
```

Nueve afirmaciones en la caja de arena, frente a ocho. El manifiesto sale con sus
cuatro líneas ordenadas y el paquete pesa 392 KiB.

## 13. Puntos abiertos

| ID | Punto |
|---|---|
| PA-126 | 🔵 Parcial: formato e integridad cerrados; **la autenticidad se transfiere a PA-14a** |
| PA-14a | Firma de release: custodia en hardware y sellado de tiempo. Y una ruta en `RutasAlmacen` para la clave pública de PremosCorp, que hoy no existe |

---

*Reporte Nº 73 — El paquete llega entero · PremosCorp · 17 de agosto de 2026*
