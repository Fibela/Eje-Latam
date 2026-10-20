# RPT-106 — La barrera que no corrió

| | |
|---|---|
| **Tema** | La barrera que no corrió |
| **Número** | 106 |
| **Fecha** | 2026-10-20 |
| **Área designada** | Método |
| **Cierra** | PA-163 |
| **Abre** | PA-165, PA-166 |
| **Precedentes** | RPT-104 (PA-162, PA-163), RPT-094 (PA-53), PA-45, PA-160 |

---

## 0. Resumen

Se arregló un campo que no llegaba al registro. Al comprobar el arreglo con el
guardián de calidad del proyecto, el guardián no compiló. Al preguntar por qué CI
no había cazado eso, apareció que **CI no se había ejecutado ni una vez en 77
días**.

El arreglo de un punto menor destapó que la barrera más externa del proyecto
estaba desconectada. No es casualidad: es el tercer día consecutivo en que la
forma del hallazgo es la misma —un mecanismo correcto, bien documentado, que no
está enchufado a nada— y esta vez el mecanismo era el que vigila a los demás.

---

## 1. PA-163 — cerrado

El banner de arranque descartaba toda la carga de `EstadoArranque`. Entra
`detallar`, aparte de `describir` para que aquélla siga siendo `const` y total:

- `NoVerifica { detalle }` — el motivo del verificador, verbatim. Nueve causas
  posibles que antes se leían como una sola frase.
- `Supresion { secuencia_conocida }` — qué secuencia recuerda el centinela.
- `FormatoObsoleto { encontrada }` — qué versión traía el fichero.

Las dos últimas no estaban en el diagnóstico de RPT-104 §5, que hablaba sólo del
inventario. **El punto resultó más amplio que su enunciado:** no era «se pierde
el motivo del inventario», era «`describir` descarta toda carga». La fila del
tablero se amplió al cerrarla.

`match` sin comodín, y a propósito en este sitio: es el que derivó
`inventarioNoVerifica` durante toda la investigación de PA-162 (tarea #30).

Línea aparte y sólo cuando hay algo que decir. Un «detalle: ninguno» en cada
arranque sano es ruido, y un banner ruidoso se deja de leer — que es como se
pierde el aviso del día que importa (PA-45).

**Cinco pruebas, las cinco verdes**, y la que sostiene el arreglo es
`dos_causas_distintas_no_dan_el_mismo_detalle`: una versión que imprimiera una
constante pasaría las otras cuatro y caería sólo en ésa. Las pruebas construyen
el estado con el `Display` del `ErrorInventario` real y no con cadenas escritas a
mano, para que sigan afirmando algo el día que esos mensajes cambien.

`cargo clippy -p eje-agente --all-targets -- -D warnings` limpio.

### 1.1 La perturbación, ejecutada — y la predicción falló

Devolviendo una constante en la rama de `NoVerifica`:

```
test result: FAILED. 3 passed; 2 failed
  dos_causas_distintas_no_dan_el_mismo_detalle   left: "el inventario no verifica"
                                                right: "el inventario no verifica"
  el_motivo_del_verificador_llega_al_banner      left: "el inventario no verifica"
                                                right: "no hay centinela de frescura; …"
```

**Predije que caería una y cayeron dos.** `el_motivo_del_verificador_llega_al_banner`
compara con `assert_eq!` contra el `Display` real de `ErrorInventario`, así que una
constante también lo rompe. Lo escribí yo y aun así predije que no lo notaría.

Falló en la dirección segura —dos barreras independientes en lugar de una—, pero
una predicción equivocada es una predicción equivocada, y lo que revela es que mi
modelo de mis propias pruebas era más flojo que las pruebas. Es la misma
distracción que las tres órdenes de §6: no haber mirado lo que la orden (o la
aserción) realmente compara.

El arreglo queda verificado: **la perturbación se vio en rojo, por el motivo
correcto y en el sitio correcto.**

---

## 2. El hilo

El orden importa, porque ninguno de los dos hallazgos siguientes se buscaba.

1. Se pidió `cargo clippy --all-targets -- -D warnings` para cerrar PA-163.
2. Falló. **Pero no en `eje-agente`:** falló al compilar `eje-manifiesto`, con
   diez errores sobre `termios` y `std::os::fd`.
3. La pregunta obvia: CI corre `cargo clippy --workspace --all-targets` y
   `cargo test --workspace` con matriz `[ubuntu-latest, windows-latest]`. ¿Cómo
   ha pasado esto siete semanas?
4. La respuesta: no ha pasado. **CI no corrió.**

El paso 3 es el único que tiene mérito, y es un hábito, no una idea: cuando una
barrera no cazó algo que debería haber cazado, la pregunta no es «arreglemos el
defecto» sino «por qué calló la barrera». Las dos veces que este proyecto se la
ha hecho —PA-151 y hoy— ha encontrado algo mayor que el defecto original.

---

## 3. PA-165 — CI no se había ejecutado ni una vez

Medido:

```
refs/heads/Principal   55c47dd   4-ago-2026    ← la rama que vigilaba CI
refs/heads/principal   9c68f97   18-oct-2026   ← donde vive el desarrollo
```

Los dos ficheros de `.github/workflows/` disparaban en `branches: [Principal]`,
con mayúscula. El desarrollo vive en `principal`, minúscula. Y `Principal` no es
una rama vieja del mismo árbol: es un **commit raíz huérfano**, sin padre y **sin
antepasado común** con la historia de trabajo. LICENSE, README y la primera
versión de RPT-002, subidos el 4 de agosto, y nada más nunca.

Así que CI tenía cinco trabajos —formato, análisis estático, pruebas en dos
sistemas operativos, inconclusos—, una matriz de plataformas, `-D warnings` en el
entorno, y un comentario de cabecera que dice:

> *«Verificaciones obligatorias de RPT-003 §9.4. Ninguna es opcional ni
> posponible.»*

Durante 77 días y 67 commits, ese párrafo fue decorativo.

**La barrera no estaba mal escrita.** Estaba bien escrita y desconectada por la
mayúscula de un nombre. Es exactamente PA-162 y PA-161 y PA-108 otra vez
—mecanismo sin cablear— en la capa más externa que existe.

### 3.1 Arreglo aplicado

Las dos ramas nombradas en los dos ficheros:

```yaml
on:
  push:
    branches: [principal, Principal]
```

Las dos y no sólo la minúscula, para que una consolidación futura en cualquier
sentido no vuelva a dejarla muda. **Una barrera que depende de la mayúscula de un
nombre no es una barrera.**

`Principal` puede retirarse sin perder nada: su LICENSE es **idéntico byte a
byte** al de la historia de trabajo, y su README son dos líneas superadas. Pero
eso es un empujón forzado sobre una rama remota y no se hace de pasada: queda
como decisión aparte.

### 3.2 Por qué el punto se deja abierto

El disparador está arreglado; el punto no está cerrado. **Ninguna barrera se da
por buena en este proyecto hasta que se la ha visto funcionar**, y CI todavía no
ha corrido. Se cierra cuando la primera ejecución real se observe — y se predice
en §6 qué debe salir de ella.

Y queda la pregunta incómoda, que el arreglo no toca: **nada avisa de que una
barrera no se ha ejecutado.** Un fichero de workflow cuyo disparador no casa con
ninguna rama viva es, desde dentro del repositorio, indistinguible de uno que
pasa en verde. Es la forma de PA-160 —una barrera cuyo alcance es más estrecho
que su nombre— llevada al extremo: alcance cero.

---

## 4. PA-166 — `eje-manifiesto` no compila en Windows

`src/eco.rs` usa `libc::termios`, `tcgetattr`, `tcsetattr`, `TCSAFLUSH`, `ECHO` y
`std::os::fd::AsRawFd` **sin ninguna guarda `cfg(unix)`**, y `libc = "0.2"` es
dependencia incondicional. Diez errores de compilación.

Entró el 6 de septiembre con PA-53 (`cfc410a`, RPT-094), que cerró un problema
real y grave —la frase de paso se veía al teclearla, y el 31 de agosto una se dio
por comprometida por eso— y lo cerró **sólo para Unix, sin decirlo**. El módulo
documenta con cuidado sus tres llamadas `unsafe` y no menciona en ninguna línea
que la herramienta deje de construirse en la mitad de los sistemas operativos de
su propia matriz de CI.

**Dos motivos se taparon entre sí:** la ceremonia se corre en WSL, que es Linux, y
CI no corría. Cualquiera de los dos solo habría bastado para que esto se viera.

Lo que importa no es si la herramienta tiene que servir en Windows —puede que
no—, es que **el guardián de calidad del proyecto no se puede correr completo en
la máquina donde se escribe el código**. Una barrera que sale roja por algo que
no es tuyo se deja de correr en una semana, y eso es PA-45 textualmente.

Hay que decidir y **declarar**, que es lo que falta:

- Si la herramienta es Unix por política: acotar `eco.rs` con `cfg(unix)`, mover
  `libc` a `[target.'cfg(unix)'.dependencies]`, y dar a la frase de paso un
  camino en Windows o **negarse en claro** en vez de no compilar.
- Si no lo es: implementar el apagado de eco con la consola de Windows.

El estado actual no es ninguna de las dos cosas. Es un silencio.

### 4.1 Lo aplicado, y el lint que no era un lint

- `eco.rs` acotado a POSIX, con un **cuarto estado**: `Eco::SinSoporte`. Cuarto y
  no una rama de `NoSePudoApagar`, por la razón que el propio módulo da para no
  colapsar los otros tres: «falló en esta terminal» se responde investigando la
  terminal, «este sistema operativo no sabe hacerlo» se responde yéndose a otro.
- `libc` a `[target.'cfg(unix)'.dependencies]`.
- En `SinSoporte`, `pedir_frase` **se niega a leer** y devuelve
  `ErrorHerramienta::FraseNoProtegible`. Antes de leer, nunca después: después ya
  se vio. Un camino de Windows que leyera la frase con el eco encendido
  reintroduciría PA-53 en otra plataforma y en silencio.
- Las dos pruebas de descriptores, acotadas a POSIX.

**Y una barrera que disparó sola.** `el_eco_tiene_exactamente_tres_estados` dejó de
compilar al añadir el cuarto, que es exactamente para lo que estaba escrita. Se
actualizó a cuatro en lugar de relajarla a un comodín: una prueba que acepta
cualquier número de estados no ancla nada. Es la segunda barrera del proyecto
validada **por uso** y no por una perturbación fabricada —la primera fue PA-153 al
añadir `EstadoOperativa::Revocada` (RPT-102)— y la primera escrita con la
intención explícita de romperse así.

**Y un error mío que clippy cazó.** Acoté `impl Drop for Guardia` entero con
`cfg(unix)`, con lo que `Eco` dejaba de implementar `Drop` en Windows y
`clippy::drop_non_drop` denunció el `drop(eco)` de `pedir_frase`. **Tenía razón y
no era un lint:** esa línea devuelve la terminal *antes* de imprimir lo siguiente,
y la había convertido en una operación vacía en una plataforma dejando al
llamador creyendo que hace algo.

Lo cómodo era un `allow`. Lo correcto es lo contrario: el **contrato** de
`Guardia` —al soltarse, la terminal vuelve a como estaba— es cierto en todas las
plataformas; lo que varía es cuánto cuesta cumplirlo, y fuera de POSIX es nada.
Así que el `impl Drop` es incondicional y sólo el cuerpo está acotado. Un `Drop`
vacío no es una mentira: es la restauración de algo que nunca se tocó.

Que el primer rojo de clippy en esta máquina, el día que el guardián volvió a
poder correrse, fuera un defecto real introducido ese mismo día dice bastante
sobre lo que han sido estas siete semanas sin él.

---

## 5. Lo que no era: una falsa alarma que puse yo

Al ver `origin/Principal` parada en agosto y `ahead 67`, afirmé que 77 días de
trabajo vivían **en un solo disco sin copia**, y lo dije con urgencia.

Era falso. `git ls-remote` mostró `refs/heads/principal` en GitHub exactamente en
el HEAD local: **todo estaba empujado**. Lo que vi fue una referencia local de
seguimiento colapsada por la insensibilidad a mayúsculas de git en Windows —`git
for-each-ref refs/remotes/` sólo listaba `origin/Principal`— y de una ambigüedad
de herramienta saqué una conclusión sobre el mundo.

Queda escrito porque el coste de una falsa alarma no es cero: se pide un respaldo
de urgencia, se interrumpe el trabajo, y la siguiente alarma se cree un poco
menos. **La regla:** cuando el instrumento no puede distinguir dos estados, lo
que se reporta es que no puede distinguirlos, no el más alarmante de los dos.

---

## 6. Tres órdenes que me engañaron el mismo día

Las tres tienen la misma forma y conviene nombrarla.

| Orden | Cómo engañó |
|---|---|
| `cargo fmt` antes de comprobar | Predije que `cargo fmt --check` saldría rojo y en la misma orden mandé `cargo fmt`, que reescribe en silencio. **La predicción quedó inobservable por la orden que la acompañaba** |
| `git log -1 $(git merge-base A B)` | Sin antepasado común, `merge-base` no imprime nada, el `$(...)` queda vacío y `git log -1` devuelve **HEAD**. Leí «el antepasado común es HEAD» donde la respuesta real era «no hay ninguno» |
| `grep` de pruebas sin `^error` | Un filtro que devuelve vacío tanto si todo pasa como si no compila (ya corregido en su día) |

**El patrón:** escribo órdenes cuya salida vacía o cuyo fallo es indistinguible de
un resultado legítimo. No es un descuido aislado, son tres en una jornada.

La disciplina que adopto, y que es la misma que este proyecto exige al código:
**una orden de diagnóstico tiene que distinguir sus dos resultados posibles, y si
no puede, lo dice.** En la práctica: no sustituir el resultado de una orden que
puede fallar sin comprobarlo, no destruir la evidencia de una predicción en la
misma tanda que la comprueba, y preferir una orden que imprima el estado a una
que lo interprete.

Es, además, exactamente lo que PA-163 acaba de arreglar en el agente: un banner
que decía «el inventario NO VERIFICA» para nueve causas distintas. Pedirle al
código que distinga lo que yo no distingo en mis propias órdenes no se sostiene.

---

## 7. Qué queda

**Predicción de la primera ejecución de CI**, escrita antes de que ocurra:

| Trabajo | Predicción |
|---|---|
| `Formato` | 🟢 verde |
| `Análisis estático` (clippy, ubuntu) | 🟢 verde |
| `Pruebas (ubuntu-latest)` | 🟢 verde |
| `Pruebas (windows-latest)` | 🔴 **rojo**, por PA-166 |
| `Inconclusos` | 🟢 verde |
| `CI Eje-Vision` | 🟢 verde, si los filtros de ruta la disparan |

Cuatro verdes y un rojo conocido. **Si sale algún otro rojo es información nueva
y buena**, y lleva siete semanas esperando a que alguien mire.

**Pendiente inmediato:**

- Ejecutar la perturbación de PA-163 (§1).
- Empujar, y observar la primera ejecución de CI (cierra PA-165).
- Decidir la política de plataforma de `eje-manifiesto` (PA-166).
- Decidir el nombre de la orden de PA-162 (`sembrar`, propuesto en RPT-105 §5.1)
  y arrancar su construcción.
- Retirar o consolidar la rama `Principal`, con el empujón forzado que eso exige,
  como decisión deliberada y no de pasada.
