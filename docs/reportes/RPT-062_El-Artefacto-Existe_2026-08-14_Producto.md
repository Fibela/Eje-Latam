# RPT-062 — El artefacto existe, y la comprobación corre sobre él

**Tema:** PA-107, primera porción. `cargo xtask empaquetar`, y qué se puede afirmar sin `systemd`
**Nº de reporte:** 062
**Fecha:** 14 de agosto de 2026
**Área designada:** Producto
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado.** PA-107 avanza; **no se cierra** (§5)

- **Depende de:** RPT-054 (especificación del empaquetado dual), RPT-025 (por qué la prueba sobre el `Cargo.toml` no basta), RPT-011 (la cadena de cinco eslabones)
- **Aborda:** PA-107, PA-12

---

## 1. Ocho días de una promesa, y por fin algo detrás

RPT-025 dejó escrito el 6 de agosto:

> Eso no impide que el empaquetador copie el binario del emisor al instalador.
> Sólo lo cierra una comprobación **sobre el artefacto**, y esa es **PA-12** y no
> existe. La prueba de aquí es necesaria y no suficiente, y presentarla como
> suficiente sería el tipo de garantía de papel que este proyecto lleva veinte
> reportes desmontando.

Hasta hoy la única defensa era una prueba sobre el `Cargo.toml`: que `eje-agente`
no declara `eje-manifiesto` como dependencia. Cierta, y ciega ante una línea de
copiado.

`cargo xtask empaquetar` produce el árbol y **después lo recorre en disco**.

```
Empaquetando el sensor headless en target/paquete/eje-agente
  agente.conf.ejemplo
  eje-agente
  eje-agente.service
  instalar.sh
Artefacto revisado sobre el disco: nada prohibido.
```

Hay una prueba que fija la diferencia: mete un `eje-manifiesto` que `empaquetar`
nunca puso, y `revisar` lo encuentra igual. Si sólo mirara la lista del propio
módulo, no.

## 2. Tres decisiones que fallan cerrado

**Se exige el binario de `release`.** Empaquetar el de depuración y llamarlo
artefacto sería mentir sobre lo que es —otro binario, otro tamaño, otras
garantías—. Se rechaza con el comando que falta.

**Un árbol que no se puede recorrer no se declara limpio.** `revisar` devuelve
error, no lista vacía. «No se pudo comprobar» no es «no había nada» (RPT-006 §4).
Un nombre de fichero que no es UTF-8 tampoco se absuelve: se declara.

**El destino se vacía antes de escribir.** Un artefacto con restos de otra
ejecución es exactamente lo que la revisión no puede distinguir de uno correcto.

## 3. Lo que la unidad de servicio dice, y por qué

`Restart=always` no es comodidad de operación. Desde RPT-053 el agente late, y un
proceso que muere y no vuelve es un sensor que la sala da por apagado (RPT-054
§7). El supervisor tiene que **reiniciar**, no sólo lanzar.

`AmbientCapabilities=CAP_NET_RAW` con `CapabilityBoundingSet` a lo mismo y
`NoNewPrivileges`. RPT-051 §1 daba por supuesto que el agente no necesita root
entero, y hasta hoy eso no estaba escrito en ninguna parte ejecutable.

La unidad **no arranca sin fichero de configuración**, y no hay interfaz por
omisión. Es preferible que no arranque a que vigile el segmento equivocado.

## 4. El instalador dice la frase

RPT-054 §4 ratificó «instala aunque no haya colector, y **lo declara a gritos**».
La mitad que ve VIS-04 es la condición `sinColector` (RPT-055). La mitad que ve
la persona delante de la máquina es esto, y tiene prueba:

```
!! ESTE SENSOR NO TIENE COLECTOR CONFIGURADO.
   Vigila el segmento, pero nada sale de este equipo y
   nadie fuera notara si se apaga.
```

## 5. La frontera que no se puede cruzar simulando

RPT-054 §8 enumeró cinco comprobaciones que sólo existen sobre el artefacto. Con
esto queda cubierta la **primera**. De las cuatro restantes, **dos y cinco** se
pueden ejercitar en una caja de arena y **cuatro no**, y la distinción es de
hormigón:

| Comprobación | Dónde se puede afirmar |
|---|---|
| 1. El artefacto no contiene el emisor ni material de clave | ✅ Cubierta, sobre el disco |
| 2. El binario arranca con la configuración que instala el instalador | Caja de arena, con `DESTINO_*` |
| 3. Sin colector, el instalador imprime la frase | ✅ Cubierta, con prueba |
| 4. **El servicio se reinicia tras matarlo** | **Máquina real con `systemd` como PID 1** |
| 5. El grupo del socket existe y el operador pertenece a él | Caja de arena, con `DESTINO_*` |

`instalar.sh` acepta `DESTINO_BIN`, `DESTINO_CONF`, `DESTINO_DATOS` y
`DESTINO_UNIDAD` por entorno, y eso **no fue comodidad para probar**: un
instalador cuyas rutas sólo se pueden ejercitar escribiendo en `/usr/local/bin`
es un instalador que nadie prueba hasta que ya es tarde.

Pero la comprobación 4 **no se puede simular**. «Matar el servicio y ver que
vuelve» es una afirmación sobre `systemd`, y `systemd` no corre en un directorio
de `/tmp`. Bajo WSL hay un agravante: el PID 1 tradicionalmente no es `systemd`,
así que exige contenedor o máquina virtual.

**Si se juntaran sin decirlo**, tendríamos un verde que cubre la 2 y la 5 y que
se lee como si cubriera la 3 —perdón, la 4—: creer que se probó la resiliencia
del servicio cuando sólo se probó que el fichero `.service` existe. Es la misma
forma del error que este proyecto lleva dos días desmontando, y por eso queda
escrito antes de montar la caja de arena y no después.

## 6. Un `#[allow]` que no se puso

Al quitar un campo que nadie leía, clippy lo señaló y la sugerencia inmediata fue
`#[allow(dead_code)]`. Se rechazó.

**Un `#[allow]` sobre una advertencia correcta es apagar el instrumento.** El
compilador no se equivocaba: nadie leía `raiz` porque quien llama ya sabe dónde
pidió el artefacto. No era código muerto que tolerar, era decoración.

Los `#[allow]` legítimos de este proyecto están todos en módulos de prueba y
dicen por qué. Uno aquí habría sido el primero que tapa un hecho en lugar de
declarar una excepción — la misma familia que `lexico.js` sumando al recuento sin
comprobar nada (RPT-056 §4) y que el tablero contando 76 identificadores y
llamándolo el total (RPT-060 §2).

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| PA-107 | Avanza. Se cierra con las cinco comprobaciones del §5, no con una |
| **PA-116** | Caja de arena del instalador: comprobaciones 2 y 5 con `DESTINO_*` |
| **PA-117** | Prueba de fuego del ciclo de vida: comprobación 4, en contenedor o VM con `systemd` como PID 1 |
| PA-79 | La configuración instalada es un fichero de texto editable. Sigue siendo el primer parámetro que pide firma |
| PA-46 | El formato del paquete depende del repositorio firmado, que no existe |

---

*Reporte Nº 62 — El artefacto existe · PremosCorp · 14 de agosto de 2026*
