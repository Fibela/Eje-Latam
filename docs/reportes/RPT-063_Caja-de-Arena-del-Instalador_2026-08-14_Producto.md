# RPT-063 — La caja de arena del instalador, y el colector de mentira

**Tema:** PA-116. Comprobaciones 2 y 5 de RPT-054 §8, y un aviso que el ejemplo silenciaba
**Nº de reporte:** 063
**Fecha:** 14 de agosto de 2026
**Área designada:** Producto
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado por observación.** Cierra PA-116

- **Depende de:** RPT-062 (el artefacto), RPT-054 §8 (las cinco comprobaciones), RPT-003 §9.5 (por qué `xtask` y no shell)
- **Aborda:** PA-116

---

## 1. El arnés no es un guion de shell, y eso ya estaba decidido

`instalar.sh` **tiene** que ser shell: corre en la máquina del cliente. El arnés
que lo comprueba, no.

Un `test-instalador.sh` sería un guion que verifica cosas y que nadie verifica.
`xtask` existe exactamente para eso, y su encabezado lo dice desde RPT-003 §9.5:
«punto de entrada único para las verificaciones propias del proyecto, **en
sustitución de scripts de shell**. Corre idéntico en Windows, Linux y CI, y **se
prueba con `cargo test`**».

`cargo xtask probar-instalador`.

## 2. Lo que el aviso descubrió, que es más que lo que confirmó

Al escribir la comprobación de la frase del colector apareció que **no salía
nunca en una instalación normal**.

El ejemplo de configuración traía `EJE_COLECTOR=127.0.0.1:5514`. El instalador
comprueba que ese campo *tenga valor*, así que una dirección de ejemplo **lo hace
callar**: el técnico que instala y no edita se lleva un sensor apuntando a un
colector inexistente, con `salidaNoDisponible` encendida para siempre y el
instalador sin decir una palabra.

La decisión ratificada en RPT-054 §4 era «instala aunque no haya colector, y **lo
declara a gritos**». Una dirección de mentira la anulaba en silencio, que es la
peor forma de anularla: parecía cumplida.

El ejemplo va ahora **vacío a propósito**, con su prueba de que no lleva un
colector inventado. Así la frase sale en una instalación recién hecha, que es
justo cuando alguien puede decidir.

> Una prueba que sólo confirma lo que ya sabíamos vale poco. Ésta descubrió un
> comportamiento **ausente**, y eso justifica el arnés entero.

## 3. Tres estados, otra vez

`Resultado::{Conforme, ViolacionDetectada, ComprobacionImposible}`, y el comando
sale con código 3 en el tercero.

Si no hay `sh`, si el sistema no es tipo Unix, o si no se ha empaquetado todavía,
el arnés **no dice verde**. Uno que no encuentra lo que iba a probar y devuelve
éxito es peor que uno que no existe: convierte la ausencia de comprobación en una
afirmación.

## 4. El aislamiento se comprueba leyendo el guion

La arena demuestra que el instalador **usa** las variables `DESTINO_*`. No
demuestra que no escriba **además** en otro sitio: una línea que copiara a
`/etc/algo` pasaría desapercibida, porque la arena no mira ahí.

Y mirar `/etc` tampoco serviría — un fichero legítimo del sistema es
indistinguible de uno recién puesto.

Lo que sí se puede afirmar es que **ninguna línea de instalación nombra una ruta
absoluta**, leyendo el texto del guion. Con su prueba de que la comprobación
comprueba algo: se cuela una ruta absoluta y aparece.

Es una afirmación más fuerte que la observación. Dice que el instalador **no
tiene la capacidad** de salirse del corral, no que esta vez no lo hiciera.

## 5. Lo observado

```
Instalador contra un destino desechable
  PASA   el binario aterriza en .../bin/eje-agente
  PASA   la unidad aterriza en .../unidad/eje-agente.service
  PASA   la configuracion aterriza en .../conf/agente.conf
  PASA   el directorio de datos se crea
  PASA   el binario queda ejecutable
  PASA   declara a gritos que no hay colector
  PASA   una segunda instalacion NO machaca la configuracion
  PASA   todo destino del guion sale de una variable DESTINO_*
```

La séptima es la que salva al cliente de una reinstalación: sin ella, actualizar
el sensor borraría la interfaz y el colector que alguien configuró a mano.

## 6. La frontera, repetida donde no se puede pasar por alto

RPT-062 §5 la escribió y aquí se repite, porque el sitio donde se olvida una
frontera es justo el reporte que la cruza:

**Esto cubre las comprobaciones 2 y 5. No dice nada de la 4.**

«Matar el servicio y ver que vuelve» es una afirmación sobre `systemd`, y
`systemd` no corre en un directorio de `/tmp`. El comando lo dice en su ayuda y
al terminar en verde, para que no haga falta leer un reporte para saberlo.

Es **PA-117**, y exige contenedor o máquina virtual con `systemd` como PID 1.

## 7. Dos decoraciones que cazó el compilador

En una hora escribí dos cosas que no usaba nadie: un campo `raiz` en un informe
—quien llama ya sabe dónde pidió el artefacto— y una función `arena_por_omision`
«por si alguien quiere mirar la arena después», que es la forma educada de decir
que no la necesita nadie.

Las dos venían con la sugerencia de `#[allow(dead_code)]`, y las dos se borraron.
La regla de RPT-062 §6 se aplicó sola: **un `#[allow]` sobre una advertencia
correcta es apagar el instrumento.**

Anotado porque es un patrón mío, no del código: escribir mirando a un futuro
imaginado en lugar de al presente. El compilador lo cazó las dos veces.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-116~~ | ✅ **Cerrado por observación** en la caja de arena (§5) |
| PA-117 | La comprobación 4. **No se puede simular** |
| PA-107 | Cubiertas 1, 2, 3 y 5 de las cinco. Se cierra con la que falta |
| PA-79 | La configuración instalada sigue siendo un fichero de texto editable |

---

*Reporte Nº 63 — La caja de arena del instalador · PremosCorp · 14 de agosto de 2026*
