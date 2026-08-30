# Manual de comandos de Eje-Latam

Todo comando que interviene en el proceso de creación: qué hace, cuándo se usa y
qué **no** puede afirmar. RPT-065, PA-119.

Este documento es una referencia, no un tutorial. Para levantar el agente y la
consola por primera vez, [`Puesta-en-marcha-local.md`](Puesta-en-marcha-local.md)
va paso a paso.

> **Convención de rutas.** La raíz del repositorio se escribe `/mnt/c/Eje-latam`
> porque es donde vive en la máquina de desarrollo actual (WSL). Sustitúyela por
> la tuya. Todos los `cargo` se ejecutan **desde la raíz**; los `npm`, desde
> `apps/eje-vision`.

---

## 0. Índice

| § | Fase |
|---|---|
| [1](#1-entorno) | Entorno — comprobar que la caja de herramientas está |
| [2](#2-compilar) | Compilar |
| [3](#3-las-seis-verificaciones-obligatorias) | Las seis verificaciones obligatorias |
| [4](#4-cargo-xtask--las-verificaciones-propias) | `cargo xtask` — las verificaciones propias |
| [5](#5-eje-visión-la-consola) | Eje-Visión, la consola |
| [6](#6-ejecutar-en-desarrollo) | Ejecutar en desarrollo |
| [7](#7-empaquetar-e-instalar) | Empaquetar e instalar |
| [8](#8-observar-un-servicio-instalado) | Observar un servicio instalado |
| [9](#9-comandos-por-usar-pa-117) | Comandos **por usar** — PA-117 |
| [10](#10-higiene-de-observación) | Higiene de observación |
| [11](#11-comandos-que-no-se-usan-y-por-qué) | Comandos que **no** se usan, y por qué |

---

## 1. Entorno

```bash
rustup show                    # qué toolchain se va a usar de verdad
cargo --version                # debe decir 1.85.x
node --version                 # 22 o superior
ps -p 1 -o comm=               # quién es el PID 1: `systemd` o no
```

`rust-toolchain.toml` **fija** el canal en 1.85 con `rustfmt`, `clippy` y
`llvm-tools-preview`. `rustup show` no informa de una preferencia tuya: informa
de lo que el fichero impone. Si dice otra cosa, la instalación está a medias.

**Por qué 1.85 y no la última.** Es la MSRV declarada. Escribir `let … && …`
—cadenas de `let`— compila desde 1.88 y **rompería la compilación del cliente**
sin que este repositorio se entere. Se usa `is_some_and` en su lugar.

`ps -p 1 -o comm=` parece fuera de sitio en una lista de herramientas de
compilación. Está aquí porque decide si una observación vale: cuatro de las
comprobaciones del artefacto son afirmaciones sobre `systemd`, y sin `systemd`
como PID 1 no se pueden hacer (§9).

---

## 2. Compilar

```bash
cargo build --workspace                    # todo, en depuración
cargo build --bin eje-agente               # sólo el sensor — lo que usa el guion de arranque
cargo build --release --bin eje-agente     # obligatorio ANTES de empaquetar
cargo build --workspace --release          # lo que corre CI
cargo check --workspace                    # ¿compila? sin generar binario. El bucle rápido
```

`cargo check` es el que conviene mientras se escribe: comprueba tipos y préstamos
sin enlazar. `cargo build` sólo cuando algo tiene que ejecutarse.

**`--release` no es opcional para empaquetar.** `cargo xtask empaquetar` **falla
cerrado** si no encuentra `target/release/eje-agente`: prefiere no producir
artefacto a producir uno con el binario de depuración dentro. Ya ocurrió lo
contrario y costó una conclusión falsa (RPT-064 §6).

---

## 3. Las seis verificaciones obligatorias

Obligatorias en cada *push* y cada *pull request* (RPT-003 §9.4). En orden, de la
más barata a la más cara:

```bash
cargo fmt --all --check                                     # ¿está formateado?
cargo clippy --workspace --all-targets -- -D warnings       # ¿hay advertencias?
cargo test --workspace                                      # ¿pasan las pruebas?
cargo deny check                                            # licencias, avisos, fuentes
cargo xtask verificar crates                                # todo!, mocks, endpoints a medias
gitleaks detect --config .gitleaks.toml                     # secretos en el código y en la historia
```

| Comando | Qué afirma | Qué **no** afirma |
|---|---|---|
| `cargo fmt --all --check` | El árbol está formateado. **No toca nada**; sólo falla | Nada sobre corrección |
| `cargo clippy … -D warnings` | Ninguna advertencia. `-D warnings` las convierte en errores | Que el código haga lo que dice |
| `cargo test --workspace` | Pasan las pruebas **registradas** | Que estén todas registradas → §4, `cobertura` |
| `cargo deny check` | Ninguna dependencia copyleft contamina un crate Apache-2.0 | Nada sobre el código propio |
| `cargo xtask verificar` | No hay marcadores de implementación inconclusa en ruta de producción | Que lo implementado sea correcto |
| `gitleaks detect` | No hay llaves ni credenciales, **tampoco en commits anteriores** | Que no las haya en otra rama |

Para arreglar el formato en lugar de sólo comprobarlo:

```bash
cargo fmt --all                # escribe. Sin --check
```

Variantes útiles cuando algo falla y se quiere acotar:

```bash
cargo test -p eje-vigia                     # sólo un crate
cargo test -p eje-agente sellos             # sólo las pruebas cuyo nombre contenga «sellos»
cargo test --workspace -- --nocapture       # deja ver los println! de las pruebas
cargo clippy -p eje-agente --all-targets -- -D warnings
cargo deny check licenses                   # sólo la frontera open-core
```

**Las advertencias no se silencian.** Un `#[allow(dead_code)]` sobre un aviso
correcto es apagar el instrumento: el aviso decía la verdad —ese campo no lo lee
nadie— y la respuesta es borrar el campo, no callar al compilador.

---

## 4. `cargo xtask` — las verificaciones propias

`cargo xtask` es un alias de `.cargo/config.toml` que ejecuta el crate `xtask`.
**Sustituye a los guiones de shell** (RPT-003 §9.5, PA-11): corre idéntico en
Windows, Linux y CI, y —lo que ningún `.sh` puede— **se prueba con `cargo test`**.

```bash
cargo xtask ayuda              # la lista, desde el propio binario
```

### 4.1 Las órdenes

```bash
cargo xtask verificar [ruta]           # guardián de inconclusos. Por defecto: crates
cargo xtask tablero                    # recuento de puntos abiertos, LEÍDO de RPT-002 §12
cargo xtask cobertura                  # ¿ejecuta alguien todas las pruebas escritas?
cargo xtask manual                     # ¿dice este manual lo mismo que el binario?
cargo xtask conformidad                # suites PQC + atestado CONFORMIDAD.lock
cargo xtask empaquetar [ruta]          # artefacto headless, revisado sobre el disco
cargo xtask probar-instalador          # caja de arena del instalador
cargo xtask vectores [--actualizar]    # vectores ACVP y Wycheproof de motor-pqc
cargo xtask vectores-ipc               # regenera los vectores del formato de cable
cargo xtask sembrar <ruta> [n] [bytes] # fabrica un registro de evidencia de prueba
```

La lista de arriba **no está escrita a mano**: sale de la misma tabla `ORDENES`
de la que salen el despacho y `cargo xtask ayuda`. Si las tres dejan de coincidir,
`cargo xtask manual` se pone en rojo.

**`verificar`** busca marcadores de implementación inconclusa y datos simulados
en ruta de producción. Bloquea el *build* de release. No se relaja: dos guardianes
de este proyecto ya pasaron en verde con la violación presente hasta que una
prueba negativa los delató.

**`tablero`** cuenta los puntos abiertos **leyendo** RPT-002 §12, no de memoria.
El resumen a mano se hizo cuatro veces y las cuatro reintrodujo puntos ya
cerrados. Desde PA-108 lleva además una barrera: **todo `PA-nn` citado en
cualquier `.md` de `docs/` tiene que tener fila en el tablero**, o el comando sale
en rojo. Cazó a PA-14b en su primera ejecución.

**`cobertura`** compara las pruebas que hay en el árbol con las que `cargo test`
registra. Existe porque dos pruebas quedaron anidadas dentro de otra función y la
suite siguió verde con dos menos. La comparación es una **desigualdad**: falla si
hay más escritas que registradas, nunca al revés — las pruebas de documentación se
registran sin llevar `#[test]`.

**`empaquetar`** construye el artefacto headless en `target/paquete/eje-agente` y
**lo revisa sobre el disco**, no sobre la lista de lo que pretendía escribir. Falla
cerrado sin el binario de release. Produce además `eje-agente.tar.gz`
—**reproducible**: dos empaquetados del mismo árbol dan bytes idénticos— con un
`MANIFIESTO` de resúmenes SHA-256 dentro.

Ese manifiesto afirma **integridad, no autenticidad**: que el paquete llegó
entero, no que venga de PremosCorp. Quien pueda sustituirlo puede recalcular los
resúmenes. La firma de release es PA-14a, y el instalador lo dice a gritos
(RPT-073).

**`conformidad`** ejecuta las tres suites poscuánticas de `motor-pqc` —ACVP,
Wycheproof y la diferencial contra libcrux— y **sólo si las tres pasan** emite
`CONFORMIDAD.lock`: las versiones exactas de las dependencias de `motor-pqc`, el
resumen de `FUENTES.lock`, el canal del toolchain y una huella SHA-256 sobre todo
ello.

La huella se calcula sobre las **entradas** y no sobre el evento de compilación.
Si alguien sube `ml-dsa` o cambia un vector sin volver a ejecutar esta orden, la
huella deja de cuadrar y `cargo test -p xtask` se pone rojo solo: el atestado se
autoinvalida. Ese fichero **no se edita a mano**; se regenera con esta orden.

Lo que **no** garantiza: ata *qué* se probó, no *que* se probó. Componer la huella
sin ejecutar nada es posible, y cerrar eso exige que la CI sea el único productor
de confianza con una clave que sólo ella tenga — el alcance de PA-14.

**`probar-instalador`** corre `instalar.sh` **dos veces** contra un destino
desechable en `/tmp`. Comprueba que respeta las rutas que se le dan, que deja el
binario ejecutable, que **una reinstalación no machaca la configuración** del
operador y que una instalación recién hecha grita que no hay colector.

**`manual`** compara este documento con las órdenes que `xtask` acepta de verdad,
en las dos direcciones: ninguna orden sin anunciar aquí, y ningún `cargo xtask X`
citado en cualquier `.md` de `docs/` que no exista. La segunda es la que importa:
un comando documentado y retirado manda a teclear algo que falla. En su primer
barrido encontró que RPT-005 §9.3 llevaba diez días mandando teclear una orden
que **todavía no se ha construido**.

Para citar una orden diseñada y no construida sin que la barrera acuse, el
documento tiene que decirlo **en la misma línea**:

```markdown
- `cargo xtask atestar-release` — **NO EXISTE TODAVIA**, es diseño; se sigue en PA-14
```

El aviso vale para su línea y no para el resto del fichero. Así no se apagan las
comprobaciones: se les amplía el alcance hasta que no ven nada.

**`sembrar`** fabrica un registro de evidencia grande para ejercitar la
fragmentación de marcos. Vive en `xtask` a propósito: es herramienta de
desarrollo y no se distribuye.

```bash
cargo xtask sembrar /tmp/eje/evidencia.alm 300 4000
```

### 4.2 Los códigos de salida son tres, no dos

```
0   Conforme
1   ViolacionDetectada
3   ComprobacionImposible     ← ni verde ni rojo: no se sabe
```

`probar-instalador` devuelve **3** cuando no hay `sh`, cuando el sistema no es
Unix o cuando falta el artefacto; `manual`, cuando no puede leer `docs/`. Es la regla que gobierna medio proyecto
(RPT-006 §4): *«no se pudo comprobar» no es «pasó»*. Un arnés que no encuentra su
herramienta y devuelve verde es peor que uno que no existe. **Si automatizas esto
en un guion, trata el 3 como distinto del 0.**

---

## 5. Eje-Visión, la consola

Desde `apps/eje-vision`:

```bash
npm ci                       # instala EXACTAMENTE el lock. Nunca `npm install` en CI
npm run compilar             # tsc --build
npm run limpiar              # tsc --build --clean
npm run verificar            # las cuatro de abajo, en orden
```

`verificar` encadena:

```bash
npm run verificar:tipos              # tsc --build
npm run verificar:frontera           # dependency-cruiser: la frontera de licencia
npm run verificar:frontera:negativa  # comprueba que la barrera DETECTA una violación
npm run probar                       # node --test sobre lo compilado
npm run verificar:licencias          # aparte: sólo licencias permitidas en el paquete abierto
```

**La prueba negativa de la frontera es la que importa.** `verificar:frontera` en
verde puede significar «no hay violaciones» o «la barrera no funciona».
`verificar:frontera:negativa` introduce una violación a propósito y exige que la
barrera la vea. Sin ella, la primera no afirma nada.

Y para verla funcionando:

```bash
npm run diagnostico          # puesto de observación. NO es el producto
npm run vis04                # VIS-04
```

Se llama `diagnostico` y no `start` a propósito: es un puesto deliberadamente
feo. Si alguien lo confunde con el producto, el problema es que se parece
demasiado.

---

## 6. Ejecutar en desarrollo

### 6.1 El agente, por el guion

```bash
scripts/arrancar-agente.sh            # interfaz `lo` por omisión
scripts/arrancar-agente.sh eth0
scripts/arrancar-agente.sh --parar
```

El guion existe porque el arranque manual costó **cuatro rondas de diagnóstico en
una sola sesión, ninguna por un fallo del producto**: la redirección del log la
ejecuta tu shell y no `sudo`; `sudo` en segundo plano recibe `SIGTTIN` y el
trabajo queda *suspendido* —no fallido—; un socket huérfano sobrevive al proceso
y da `ECONNREFUSED` sobre un fichero que existe; y `sleep 2` es correcto hasta el
día que la máquina va lenta. Un guion no las evita por disciplina: las evita por
construcción.

Después:

```bash
tail -f /tmp/eje/agente.log
ls -ln /tmp/eje/agente.sock      # debe decir  srw-rw----  root  TU_GRUPO
id -g                            # tu grupo, para comparar con la cuarta columna
```

Si falta el segundo `rw`, la consola no conectará sin `sudo`.

### 6.2 El agente, a mano

```bash
sudo ./target/debug/eje-agente --interfaz lo --almacen /tmp/eje --ciclos 0 --grupo-ipc "$(id -g)"
```

| Opción | Para qué | Por omisión |
|---|---|---|
| `--interfaz NOMBRE` | La interfaz que se vigila | **obligatoria** |
| `--almacen RUTA` | Dónde vive lo persistente: inventario, centinela, evidencia | `/var/lib/eje-latam` |
| `--directorio-socket RUTA` | Directorio volátil donde se abre el socket | `/run/eje-latam` |
| `--ciclos N` | Vueltas antes de salir. **`0` = sin fin** | `1` |
| `--grupo-ipc GID` | Grupo numérico que puede consultar por el socket | ninguno |
| `--perfil corporativo\|ot` | Perfil de segmento | `corporativo` |
| `--tramas N` | Tramas por vuelta | interno |
| `--syslog HOST:PUERTO` | Colector al que se emiten alertas, sellos y latidos | ninguno |
| `--nombre TEXTO` | Identidad en la sala | el `hostname` de la máquina |
| `--intervalo-latido MS` | Cada cuánto late | 10 000 |

**`--ciclos 0` es el modo demonio.** Con el valor por omisión el agente da una
vuelta y sale, que es lo correcto para inspeccionar y lo inútil para observar.

**`--syslog ""` no es un colector.** Una cadena vacía se descarta en la frontera y
el agente declara `sinColector`. Fue un defecto real: `systemd` sustituye
`${VARIABLE}` como un argumento **aunque esté vacía**, y el agente tomaba «este
sensor no informa a nadie» por «el colector está caído» — las dos cosas que
mandan al técnico a sitios distintos (RPT-064).

**`--grupo-ipc` toma un número, no un nombre.** `$(id -g)`, no `$(id -gn)`. Que
acepte un nombre es PA-84.

**El socket ya no vive con la evidencia.** `/run` es tmpfs y se vacía en cada
arranque, así que el socket huérfano —el fichero que sobrevive al proceso y hace
que el cliente reciba `ECONNREFUSED` sobre algo que existe— deja de ser posible.
Bajo `systemd` el directorio lo crea `RuntimeDirectory=`; a mano no lo crea
nadie, y por eso el guion de desarrollo pasa `--directorio-socket /tmp/eje`. Se
mueve el **directorio**, nunca el nombre del fichero (RPT-067, PA-120).

### 6.3 El vigía, el colector de referencia

```bash
cargo run --bin eje-vigia -- --escuchar 127.0.0.1:5514 --esperar LapTap-AF/lo
```

| Opción | Para qué |
|---|---|
| `--escuchar DIR:PUERTO` | Dónde se expone el colector. **Obligatoria** |
| `--esperar MAQUINA[/INTERFAZ]` | Censo. Repetible |

**`--escuchar` no tiene valor por omisión a propósito** (RPT-075). Decide en qué
interfaz escucha un servicio de red: un `127.0.0.1` por omisión funciona en la
máquina de quien lo escribió y se convierte en `0.0.0.0` el día que alguien
quiere recibir tráfico de otro equipo — y entonces el colector queda expuesto a
toda la red del cliente sin que nadie lo haya decidido.

**Sin censo sólo se cubre «se apagó».** Con censo se cubre además «nunca
arrancó», que es el caso que no puede deducirse de lo que se oye: un sensor que
jamás emitió no deja hueco en ninguna serie.

Y el vigía distingue tres cosas que se parecen: `APARECE` (nunca visto, ahora
habla), `VUELVE` (estuvo ausente y regresa) y `AUSENTE` (lleva sin latir más de
tres intervalos).

---

### 6.4 `eje-manifiesto`, la herramienta del administrador del cliente

**No se despliega en el sensor**, y ésa es la decisión que sostiene la cadena de
confianza: si viviera dentro del agente, cada sensor llevaría encima la capacidad
de firmar (RPT-025). Vive en la máquina del administrador, con la semilla.

```bash
eje-manifiesto generar      --semilla <fichero> --almacen <directorio>
eje-manifiesto emitir       --semilla <fichero> --entrada <toml> --salida <inv> [--anterior <inv>]
eje-manifiesto configurar   --semilla <fichero> --entrada <toml> --salida <cfg> [--anterior <cfg>]
eje-manifiesto recuperacion --fragmentos <prefijo> --almacen <directorio>
eje-manifiesto revocar      --fragmento-uno <frg> --fragmento-dos <frg> --almacen <dir> --sucesora <pub> --corte <n>
```

**`configurar`** emite la configuración firmada del sensor (RPT-074, PA-79). El
administrador escribe un TOML y sale un binario firmado que sólo vale en la
máquina que declara:

```toml
maquina = "planta-3"          # hostname donde esta configuración es válida
nombre = "sensor-planta-3"    # identidad en la sala. NO es lo mismo que maquina
interfaz = "eth0"
perfil = "ot"                 # corporativo u ot
colector = "siem.hospital:514"
intervalo_latido_ms = 60000
grupo_ipc = 1000
almacen = "/var/lib/eje-latam"
directorio_socket = "/run/eje-latam"
```

Casi nada tiene valor por omisión, y es deliberado: **leer el TOML dice
exactamente qué hará el sensor**, sin valores escondidos en un binario. Sólo
`colector` y `grupo_ipc` pueden faltar, y en los dos la ausencia significa algo —
«no informa a ninguna sala» y «socket en `0600`».

**La secuencia no se escribe.** Sale de `--anterior`, que se **verifica** antes de
creerle el número: un fichero editado no decide qué se emite después, y si está
corrupto la herramienta falla en lugar de empezar la serie de nuevo — si no,
bastaría con borrarlo para rebobinar.

## 7. Empaquetar e instalar

**Encadenado con `&&`, siempre.** Si `empaquetar` falla y el `cd` sigue adelante,
`instalar.sh` copia el binario anterior y la observación siguiente describe un
código que no está instalado. Pasó (RPT-064 §6).

```bash
cargo build --release --bin eje-agente && \
cargo xtask empaquetar && \
cargo xtask probar-instalador
```

Luego, en la máquina destino:

```bash
cd target/paquete/eje-agente && sudo sh instalar.sh
```

El instalador respeta cuatro variables, y por eso la caja de arena puede
probarlo sin tocar el sistema:

| Variable | Por omisión |
|---|---|
| `DESTINO_BIN` | `/usr/local/bin` |
| `DESTINO_CONF` | `/etc/eje-latam` |
| `DESTINO_DATOS` | `/var/lib/eje-latam` |
| `DESTINO_UNIDAD` | `/etc/systemd/system` |

```bash
# Instalación de juguete, sin privilegios y sin tocar nada del sistema
DESTINO_BIN=/tmp/prueba/bin DESTINO_CONF=/tmp/prueba/conf \
DESTINO_DATOS=/tmp/prueba/datos DESTINO_UNIDAD=/tmp/prueba/unidad \
  sh instalar.sh
```

Después de instalar, **el sensor todavía no vigila nada**. La unidad ya no lleva
`EnvironmentFile` ni pasa parámetros: la interfaz, el colector, el intervalo del
latido, el grupo del socket y la identidad en la sala salen de
`/etc/eje-latam/agente.conf.firmado`, y pasarlos por la línea de órdenes teniendo
configuración firmada **impide arrancar** (RPT-077). Dos sitios donde decidir lo
mismo es un sitio donde ganarle a la firma.

El paquete deja la plantilla en `/etc/eje-latam/configuracion-sensor.toml.ejemplo`.
Se rellena con el `hostname` **de esa máquina** y se firma en la máquina de
emisión del administrador — ver §6.4:

```bash
hostname                                  # el campo `maquina` tiene que ser este
eje-manifiesto configurar --semilla clave.sem \
    --entrada configuracion-sensor.toml \
    --salida agente.conf.firmado \
    [--anterior /etc/eje-latam/agente.conf.firmado]
```

Un sensor arrancado sin ella lo declara en cada latido con
`configuracionSinFirmar`, y uno con una configuración que no verifica **arranca
igual pero no vigila nada**: no cae a la línea de órdenes, porque a quien pudo
tocar el fichero le bastaría romperlo para recuperar el mando por argumentos.

```bash
sudo systemctl daemon-reload            # tras tocar la unidad. Se olvida siempre
sudo systemctl enable --now eje-agente  # arranca y deja arrancado en el reinicio
```

---

## 8. Observar un servicio instalado

```bash
systemctl status eje-agente --no-pager
systemctl show -p ExecStart --no-pager eje-agente
systemctl show -p MainPID --value eje-agente
journalctl -u eje-agente --no-pager -n 50
journalctl -u eje-agente -f                       # en vivo
journalctl _PID=4518 --no-pager                   # UNA ejecución concreta
sudo systemctl restart eje-agente
sudo systemctl stop eje-agente
sudo systemctl disable eje-agente
```

**`--no-pager` en todas.** Sin él, `systemctl show -p ExecStart` sale truncado por
el paginador, y la línea cortada es exactamente la que decide.

Para dejar la máquina como estaba:

```bash
sudo systemctl disable --now eje-agente && \
sudo rm -f /usr/local/bin/eje-agente /etc/systemd/system/eje-agente.service && \
sudo rm -rf /etc/eje-latam /var/lib/eje-latam && \
sudo systemctl daemon-reload
```

---

## 9. Comandos **por usar** — PA-117

Las dos comprobaciones que faltan del artefacto. **No se pueden hacer en WSL** ni
en un directorio de `/tmp`: son afirmaciones sobre `systemd`, y exigen `systemd`
como PID 1 en máquina limpia o contenedor privilegiado.

### 9.1 ¿Vuelve tras una muerte a traición?

```bash
PID=$(systemctl show -p MainPID --value eje-agente) && echo "antes: $PID" && \
sudo kill -9 "$PID" && sleep 5 && \
systemctl show -p MainPID --value eje-agente
```

Se espera un PID **distinto y no cero**.

**Tiene que ser `kill -9`, no `systemctl stop`.** Un `stop` es una parada
ordenada y `Restart=always` no reinicia tras ella: probaríamos lo contrario de lo
que queremos y saldría verde.

### 9.2 ¿`ProtectSystem=strict` impide de verdad escribir fuera de sitio?

```bash
sudo systemd-run --unit=prueba-aislamiento \
  --property=ProtectSystem=strict \
  --property=ReadWritePaths=/var/lib/eje-latam \
  /bin/sh -c 'echo x > /etc/eje-prueba-de-fuga; echo "salida=$?"'
```

Se espera que **falle**. Leer la unidad y ver la directiva no afirma nada: la
directiva puede estar escrita y no aplicarse. Se comprueba intentándolo.

### 9.3 De paso, que la separación de PA-120 sea real

```bash
ls -ldn /run/eje-latam /var/lib/eje-latam
stat -c '%a %U:%G %n' /run/eje-latam/agente.sock /var/lib/eje-latam/*
```

Se espera el socket en `/run/eje-latam` y **ningún** `.sock` en
`/var/lib/eje-latam`. Y al parar el servicio, que `/run/eje-latam` desaparezca:

```bash
sudo systemctl stop eje-agente && ls -ld /run/eje-latam
```

Se espera `No such file or directory`. Eso es lo que hace imposible el socket
huérfano, y es una afirmación sobre `systemd` — no se puede comprobar en WSL ni
en un directorio de `/tmp`.

---

## 10. Higiene de observación

Tres veces en media hora se leyó evidencia del proceso equivocado, y estuvo a
punto de costar dos conclusiones falsas (RPT-064 §6). Las tres se evitan igual:

**Encadena con `&&`.** Un montaje que sigue adelante tras un fallo produce
salidas que **parecen resultados**. Fue así como se leyó una observación tomada
con el binario anterior.

**Comprueba el PID antes de leer un diario.** Líneas del PID 4397 —un arranque
*con* colector— se leyeron como si fueran del 4518. Con un colector configurado
que no responde, `salidaNoDisponible: true` con `sinColector: false` **es la
respuesta correcta**: la evidencia parecía confirmar el defecto y no lo mostraba.

**`--no-pager` siempre.** Ver §8.

Y una cuarta, que no es de comandos pero cuesta lo mismo:

**Escribe la predicción antes de ejecutar**, incluyendo las formas en que puede
salir mal. En PA-118 la predicción fue *«`systemd` dejará `--syslog` colgando y el
agente saldrá con error de uso»*. Fue errónea, y el comportamiento real era peor:
no rompía, **mentía**. Sin la predicción escrita, la salida se habría leído como
confirmación.

---

## 11. Comandos que **no** se usan, y por qué

```bash
npm install                  # usa `npm ci`: instala el lock exacto
cargo update                 # no en un cambio funcional. Mueve el lock por su cuenta
cargo build --offline        # oculta que falta una dependencia hasta el día del despliegue
```

**`cargo fuzz` corre aparte.** `fuzz/` es un workspace propio porque `cargo-fuzz`
exige *nightly*, y meterlo en el principal obligaría a **todo** el proyecto a
compilar con nightly, saltándose la MSRV de §1:

```bash
cd fuzz && cargo +nightly fuzz run analizar
```

**Ningún `curl`, `wget` ni descarga por script en las verificaciones.** Los
vectores de prueba entran por `cargo xtask vectores`, que los **ancla**: un
vector que cambia de contenido sin que nadie lo note convierte la suite
criptográfica en decoración.

**Ningún `test-instalador.sh`.** Sería un guion que verifica cosas y al que nadie
verifica. Es `cargo xtask probar-instalador` justamente por eso.

---

## 12. La secuencia completa, antes de un commit

```bash
cd /mnt/c/Eje-latam && \
cargo fmt --all && \
cargo clippy --workspace --all-targets -- -D warnings && \
cargo test --workspace && \
cargo xtask verificar crates && \
cargo xtask cobertura && \
cargo xtask manual && \
cargo xtask tablero && \
cargo xtask conformidad
```

Y si se tocó la interfaz:

```bash
cd /mnt/c/Eje-latam/apps/eje-vision && npm run verificar
```

**La documentación va en el mismo commit.** Un *pull request* que cambie
comportamiento sin tocar `docs/` se rechaza (RPT-003). Y si el cambio acuñó un
`PA-nn`, `cargo xtask tablero` no pasará hasta que tenga fila en RPT-002 §12.

---

## 13. Lo que este documento no puede afirmar

**Lo que sí puede, desde hoy:** que la lista de órdenes de §4.1 corresponde con
las que `xtask` acepta. Lo comprueba `cargo xtask manual` en las dos direcciones,
y la lista sale de la misma tabla que el despacho y la ayuda. PA-119, RPT-066.

**Lo que no:** todo lo demás. Que las banderas del agente de §6.2 sean las que
`main.rs` acepta, que los `npm run` de §5 existan en `package.json`, que las
opciones del vigía sean esas. Hoy los comprobé a mano y coincidían; mañana es
costumbre, no prueba.

Ampliar la barrera a esas tres tablas es trabajo pendiente y no tiene número
todavía. Queda dicho aquí porque un documento que sólo declara lo que verifica
—callando lo que no— se lee como si lo verificara todo.

---

*PremosCorp · Manual de comandos · 14 de agosto de 2026 · RPT-065*
