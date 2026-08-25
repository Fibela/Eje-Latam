# RPT-079 — Protocolo de la conversación

**Tema:** PA-78. El agente y la consola nunca se han hablado
**Nº de reporte:** 079
**Fecha:** 21 de agosto de 2026
**Área designada:** Producto
**Entidad:** PremosCorp
**Estado:** **Protocolo emitido. Predicciones escritas antes de ejecutar.** Sin observación todavía

- **Depende de:** RPT-045 (los vectores de paridad), RPT-046 (el enlace y el guardián del puente), RPT-068 (las reglas de higiene, que aquí valen igual), RPT-067 (dónde nace el socket)
- **Aborda:** PA-78. Al cerrarse, resuelve también **PA-40** y **PA-65**, y toca **PA-103**

---

## 1. Por qué esto se escribe antes

Dos veces en esta plataforma la predicción escrita evitó leer una salida como
confirmación de lo que se esperaba: en PA-118 el fallo real no era el previsto —no
rompía, **mentía**— y en RPT-064 §6 quedaron anotadas tres lecturas de evidencia
del proceso equivocado en media hora.

Y hay un motivo propio de esta prueba: **es la primera vez que estas dos piezas se
ven las caras**, y las dos han cambiado mucho en ocho días. Sin predicción, cualquier
cosa que salga se leerá como «bueno, era de esperar».

## 2. Lo que ya está roto antes de empezar

Preparando este documento, sin ejecutar nada, aparecieron dos defectos. Van aquí y
no escondidos en una nota, porque **son exactamente lo que PA-78 dice que pasa**.

### 2.1 La consola por omisión llama a una puerta que no existe

| Dónde | Qué dice | Qué es |
|---|---|---|
| `guardian_cc::arranque` (el agente) | `/run/eje-latam/agente.sock` | La verdad |
| `arranque.ts` (la consola, por omisión) | `/run/eje/agente.sock` | ❌ **El defecto** |
| `package.json` y `arrancar-agente.sh` | `/tmp/eje/agente.sock` | Anulación deliberada de desarrollo |

**Aquí hay una corrección a lo que dije al encontrarlo.** Anuncié «tres sitios y dos
mal». No es exacto: el tercero es una **anulación deliberada y coherente** —crear
`/run/eje-latam` exige root y obligar a `sudo` para levantar la consola de
diagnóstico haría que nadie la levantara (RPT-067 §4)—, y el guion del agente y el
de la consola pasan el mismo valor. Sitios de verdad hay **dos**, y uno estaba mal.

La exageración importa. Este método descansa en que las descripciones sean exactas;
un hallazgo inflado hoy es un hallazgo del que se desconfía mañana.

Lo que sí es cierto, y sigue siendo grave: **con los valores de fábrica, un sensor
sano y una consola sana no se encuentran.** El agente movió su socket en RPT-067
(PA-120) y la consola no se enteró. Y el defecto era invisible precisamente porque
la anulación de desarrollo se usa siempre: **sólo aparecía en un despliegue de
verdad, que es donde no hay nadie mirando**.

Es el índice escrito a mano otra vez. Los cinco anteriores se quedaban cortos —falta
algo y se nota—; éste **apunta a otro sitio**, que es peor, porque produce dos
piezas sanas y un `ECONNREFUSED`.

**Lo que se acepta y no se arregla:** que la anulación de desarrollo viva en dos
guiones que podrían divergir entre sí. Divergir ahí cuesta cinco minutos a quien
desarrolla; divergir en el valor de fábrica cuesta un despliegue. No toda
duplicación merece la misma barrera, y decir cuál no la merece es parte de decidir
dónde ponerlas.

### 2.2 Un comentario promete un arreglo que ya no va a llegar

`arranque.ts` dice, sobre esa ruta:

> *PA-79 queda anotado porque esta ruta está fijada y debería salir de
> configuración.*

**PA-79 se cerró hoy** y no tocó esto — la configuración firmada dicta el socket
del *agente*, nunca dijo nada de la consola. El comentario quedó apuntando a un
punto cerrado, que es la forma más silenciosa de que un pendiente desaparezca: no
se borra, se queda esperando a alguien que ya se fue.

### 2.3 Qué se hizo con esto — PA-132, cerrado antes de ejecutar

**Se arregló antes de la prueba**, no después. Medir la conversación con
`EJE_SOCKET` puesto a mano sería medir una configuración que nadie va a desplegar.

Y no bastaba con cambiar la cadena: eso es el mismo defecto una vuelta más tarde.

**La causa raíz no era el valor, era dónde vivía.** `arranque.ts` importa Electron,
así que ninguna prueba podía leer esa constante sin levantar un escritorio — y una
constante que ninguna prueba puede mirar es una constante que se queda atrás. Por
eso sobrevivió a RPT-067.

Lo hecho:

1. El punto de encuentro pasa a `contrato-ipc.toml`, sección `[socket]`, que **ya
   era la fuente común de los dos lados**.
2. La constante sale de `arranque.ts` a `punto-de-encuentro.ts`, **un módulo que no
   importa nada**. Ni Electron, ni `node:fs`.
3. Barrera **a los dos lados**, porque media barrera es como llegamos aquí:
   `el_punto_de_encuentro_del_contrato_es_el_que_el_agente_abre` en `xtask` ata el
   manifiesto a la constante del agente, y *«la consola busca al agente donde el
   contrato dice que está»* lo ata a la de la consola.

### 2.4 Y un tercer defecto que salió al arreglar el segundo

`process.env["EJE_SOCKET"] ?? valor` no protege de nada. `??` sólo sustituye
`undefined` y `null`, así que **`EJE_SOCKET=` —definida y vacía, que systemd y los
guiones de shell producen sin querer— pasaba tal cual**: la consola habría intentado
abrir la cadena vacía y habría presentado el fallo como «el agente no responde».

Es literalmente PA-118 un piso más abajo. Allí costó dos rondas de diagnóstico por
lo mismo: no rompía, **mentía**. Una cadena vacía no es un destino, ni para el
colector del agente ni para el socket de la consola, y ahora hay prueba de las dos.

## 3. Lo que se va a afirmar, y lo que no

| # | Afirmación | Cómo se comprueba |
|---|---|---|
| 1 | Los dos procesos intercambian bytes por un socket real | Petición y respuesta, con el PID de los dos anotado |
| 2 | Las trece condiciones que el agente calcula son las que la consola lee | Comparar el latido del diario con lo que responde `obtener-condiciones` |
| 3 | Un marco troceado por el núcleo se reensambla | Consulta de alertas sobre un registro sembrado, mayor que un `read()` |
| 4 | Sin agente, la consola lo dice en lenguaje de operador | Parar el servicio con la consola abierta |
| 5 | El permiso del socket decide de verdad quién entra | Consultar desde un usuario fuera del grupo |

**Lo que este protocolo no afirma:**

- **Nada sobre detección.** Se vigila `lo`. Meter la clasificación en la ecuación
  añade una forma más de que la prueba falle por algo que no es lo que se prueba
  (misma decisión que RPT-068 §2).
- **Nada sobre la configuración firmada.** El sensor corre **sin ella**, declarando
  `configuracionSinFirmar`. Es el estado en que está toda la flota, y además hace
  la prueba independiente del emisor. La conversación con configuración firmada es
  otra sesión.
- **Nada sobre rendimiento.** PA-100 sigue abierto y no se toca aquí.

## 4. Escenario y estado inicial

### 4.1 Sobre el bloque que el equipo técnico propuso como «vector de ataque»

Se conserva el bloque y se le cambia el nombre a **§5, «Lo que se lanza,
exactamente»**. En Eje-Latam «ataque» y «avería» están separados a propósito y con
consecuencias: RPT-055 §3 y PA-45 existen porque confundirlos es como se enseña a
un operador a ignorar la alerta que importa. Una consola pidiendo condiciones no es
un ataque, y llamarlo así aquí entrenaría el reflejo equivocado en el sitio donde
más caro sale. La comprobación de permisos de §5.5 **sí** es adversaria, y va
marcada como tal.

### 4.2 La máquina

La VM de RPT-068, o una igual. El protocolo se bifurca según lo que haya.

### Cómo se comprueba, y cómo NO — observado el 24-ago-2026

Se preguntó por `systemctl get-default` y por el paquete `ubuntu-desktop`, y la
respuesta fue **engañosa**: `graphical.target`, que suena a escritorio. No lo era.
No hay gestor de sesiones —`display-manager.service` no existe—, ni socket de
Wayland en `/run/user/1000`, ni servidor X. `graphical.target` es sólo el objetivo
por omisión; nadie lo satisface.

Lo decisivo es preguntar por lo que se va a usar, no por lo que se declara:

```bash
systemctl status display-manager --no-pager | head -3
ls /run/user/1000/ | grep -Ei 'wayland|X11'
```

Es la misma lección que atraviesa el proyecto —lo declarado no es lo cableado—
aplicada esta vez al sistema operativo de la máquina de pruebas.

**Y una consecuencia de producto, no de la prueba:** montar Electron ahí exigió
GTK**3** —26.04 trae GNOME sobre GTK4, así que ningún otro programa lo usa— y
poner `chrome-sandbox` en `root:root` modo `4755`, que npm no puede hacer. Ninguna
de las dos cosas está declarada en ningún sitio. Van a PA-46: un instalador de
`eje-vision` que no las entregue fallará en cada máquina limpia.

**Se rechazó `--no-sandbox`** como atajo. Desactivar el aislamiento del
renderizador para que arranque la consola de un producto de seguridad es medir una
configuración que nadie debe desplegar; RPT-004 §6.1 sujeta las capas de arriba y
ésta es la de abajo.

**Con escritorio** (Ubuntu Desktop) — se hace la prueba entera, incluida la vista.

**Sin escritorio** (Ubuntu Server) — se hace **la mitad A** con `node` conduciendo
el `enlace.js` compilado. Conviene ser exacto sobre qué es eso: **no es un
simulacro**. Es el código de enlace que se despliega, el mismo `marcos.ts`, el
mismo `puente.ts`, sin la ventana. Lo que queda fuera es la **mitad B** —que el
operador vea la verdad en pantalla—, y ésa no se puede afirmar sin pantalla.

Si se hace sólo la mitad A, **PA-78 no se cierra**: se cierra su primera mitad y se
deja escrito qué falta. Un punto cerrado a medias es peor que uno abierto.

### 4.3 Cómo llega la consola a la máquina

**Este bloque faltaba en la primera versión de este protocolo**, y la omisión no es
menor: da por hecho el paso que más formas tiene de salir mal. Se anota como
corrección, no se disimula.

El agente viaja empaquetado; la consola **no tiene empaquetado todavía** —eso es
PA-14a y PA-46— así que va por el repositorio:

```bash
git clone https://github.com/Fibela/Eje-Latam.git && \
cd Eje-Latam/apps/eje-vision && \
npm ci && \
npm run compilar
```

`npm ci` y no `npm install`: instala exactamente lo que dice `package-lock.json`.
`install` puede resolver versiones distintas de las probadas, y entonces lo que se
observa no es lo que se verificó.

**Y una condición de orden que no es opcional:** el arreglo de PA-132 tiene que
estar **empujado** antes de clonar. Un clon anterior trae la consola con
`/run/eje`, la conversación falla, y la prueba «encontraría» un defecto ya
arreglado. Es el primer modo de fallo de §7, y ocurre por descuido de secuencia,
no de código.

```bash
git log --oneline -1        # en la VM: tiene que ser el commit de PA-132
```

### 4.4 Comprobaciones previas, en orden y encadenadas

```bash
cd /mnt/c/Eje-latam && \
cargo build --release --bin eje-agente && \
cargo xtask empaquetar && \
cargo xtask probar-instalador && \
md5sum target/release/eje-agente
```

Y en la VM, **antes de observar nada**:

```bash
md5sum /usr/local/bin/eje-agente
```

Los dos resúmenes se comparan **a mano y a ojo**. En tres días se leyeron cuatro
veces observaciones de un binario del 14 de agosto creyendo que era el nuevo
(RPT-068 §5). Esta línea no es ceremonia: es la que evita esa media hora.

### 4.5 Estado inicial que hay que confirmar, no suponer

| Qué | Orden | Lo que tiene que salir |
|---|---|---|
| El servicio corre | `systemctl is-active eje-agente` | `active` |
| Y su PID | `systemctl show -p MainPID --value eje-agente` | un número, que se **anota** |
| El socket existe y dónde | `ls -l /run/eje-latam/agente.sock` | fichero de tipo `s` |
| Con qué permisos y grupo | `stat -c '%A %U:%G' /run/eje-latam/agente.sock` | `srw-rw---- root:<grupo>` |
| Qué versión dice ser | `journalctl -u eje-agente --no-pager | head -20` | el banner, con `Configuracion : SIN FIRMAR` |
| Que no hay agente viejo | `pgrep -a eje-agente` | **una sola** línea |

La última existe porque un segundo agente sobre el mismo almacén se acusa a sí
mismo de recorte del registro (`dos_agentes_en_una_maquina_se_acusan_entre_ellos`),
y esa acusación se leería aquí como un defecto del IPC.

## 5. Lo que se lanza, exactamente

### 5.1 La conversación mínima

**Sin `EJE_SOCKET`, a propósito.** Desde PA-132 el valor de fábrica es correcto, y
pasarlo a mano volvería a medir una configuración que nadie despliega — que es
exactamente cómo el defecto sobrevivió tanto tiempo.

```bash
cd apps/eje-vision && npm run compilar && \
  electron proceso-principal/dist/principal.js
```

Si eso no encuentra al agente, **es un hallazgo**, no un montaje mal hecho.

Con escritorio. Sin él, la mitad A:

```bash
cd apps/eje-vision && npm run compilar && \
EJE_SOCKET=/run/eje-latam/agente.sock \
  node -e 'import("./proceso-principal/dist/enlace.js").then(async m => { ... })'
```

Hizo falta. El guion es **`apps/eje-vision/scripts/conversar.mjs`**, escrito y
versionado antes de ejecutarlo:

```bash
cd ~/Eje-Latam/apps/eje-vision && node scripts/conversar.mjs
```

Importa `enlace.js`, `cable.js`, `puente-ipc.js` y `punto-de-encuentro.js` de
`dist/`: el enmarcado, el vencimiento, la interpretación de la respuesta y la
lista de canales son **los que se despliegan**. Recorre `CANALES_PERMITIDOS` en
vez de pedir seis a mano, que sería el séptimo índice escrito a mano de la serie.

Lo que **sí** duplica, y se dice en su cabecera, son dos piezas de fontanería
atrapadas dentro de `arranque.ts` porque ese fichero arrastra Electron:
`abrirConducto` —ocho líneas sobre `net.connect`— y `cargaDe`. El protocolo no se
reimplementa; sólo el pegamento. Y un `cargaDe` equivocado produciría un rechazo
con motivo, que es un fallo ruidoso.

### 5.2 Los seis canales permitidos, uno por uno

`obtener-estado-agente`, `obtener-inventario`, `obtener-estado-boveda`,
`consultar-sandbox`, `consultar-alertas`, `obtener-condiciones`.

Se piden **los seis**, no una muestra. El contrato declara seis y las pruebas de
paridad afirman que los seis existen a ambos lados; lo que ninguna afirma es que
los seis **respondan**. Es la distinción de siempre entre estar declarado y estar
cableado.

### 5.3 Un marco que no cabe en una lectura

Con `cargo xtask sembrar` se deja un registro de evidencia con asientos suficientes
para que la respuesta de `consultar-alertas` supere holgadamente lo que un `read()`
entrega de una vez.

Es lo único de esta prueba que los vectores **no pueden** cubrir: el troceado lo
decide el núcleo, no el codificador.

### 5.4 El agente se va a media conversación

Con la consola abierta y consultando:

```bash
sudo systemctl stop eje-agente
```

### 5.5 Y una comprobación adversaria, ésta sí

Desde un usuario **fuera** del grupo del socket:

```bash
sudo -u nobody EJE_SOCKET=/run/eje-latam/agente.sock node ...
```

## 6. Predicciones de éxito

Escritas antes de ejecutar. Si alguna falla, **no se ajusta**: se escribe qué se
esperaba, qué ocurrió, y qué punto abierto nace.

| # | Predicción |
|---|---|
| 1 | Los seis canales responden. Ninguno rechaza |
| 2 | `obtener-condiciones` devuelve **trece**, con `configuracionSinFirmar` en `true` y `capturaNoDisponible` en `false` |
| 3 | Esas trece coinciden **campo a campo** con las del último latido del diario |
| 4 | `consultar-alertas` sobre el registro sembrado llega **entero**, y el acumulador de marcos no descarrila |
| 5 | `obtener-inventario` responde **vacío**, y la consola lo presenta como *«no hay»*, no como *«no se sabe»* — hay inventario ausente, que es una observación (RPT-048 §1) |
| 6 | Al parar el servicio, la cabecera pasa a lenguaje de operador y **no** muestra una excepción de Node |
| 7 | Desde fuera del grupo: `EACCES` al abrir, traducido como *«sin permiso»* y **sin reintento** (PA-93) |

**Predicción sobre el conjunto:** al menos una de las siete fallará. No es
pesimismo — es que esta es la primera ejecución de un camino con siete piezas que
sólo se han probado por separado, y la tasa de esta semana no da para más optimismo.

## 7. Modos de fallo anticipados

Se escriben para que, si ocurren, no se confundan con el mecanismo.

| Hipótesis | Cómo se distingue de un defecto real |
|---|---|
| **La ruta por omisión** (§2.1). Si el arreglo de PA-132 no llegó al binario que corre, `ECONNREFUSED` sobre una ruta que no existe | El mensaje nombra `/run/eje` sin `-latam`. Sería el defecto **ya arreglado** reapareciendo, es decir: se está observando un artefacto viejo. Ver §4.4 |
| **Permisos**: la consola corre como el usuario y el socket es `0660` de un grupo al que no pertenece | `EACCES`, no `ECONNREFUSED`. Son causas distintas y `enlace.ts` las separa |
| **`ProtectSystem=strict`** impidiendo al agente crear el socket | No habría socket **en absoluto**; se ve en §4.5 antes de empezar |
| **Electron sin GUI**: falta NSS, GBM o ALSA en la VM | Falla al **arrancar Electron**, antes de tocar el socket. No es un defecto del IPC |
| **El plazo de 5 s** (`ESPERA_MAXIMA_MS`) venciendo porque el agente está en mitad de una vuelta de captura | Se distinguiría por reproducirse **sólo** con `--ciclos 0` y tramas en curso. Sería un hallazgo real y grave: el agente atiende consultas al final del ciclo (RPT-034 §4) |
| **Un agente viejo** en el `PATH`, o dos corriendo | §4.4 y la última fila de §4.5 |
| **El registro sembrado no verifica** porque lo escribió otro binario | El agente lo apartaría al arrancar y lo diría en el banner |

El quinto es el que más me interesa. Es el único de la lista que, de ocurrir,
significa que el diseño del ciclo tiene un problema de verdad y no un montaje mal
hecho.

## 8. Criterios para PA-40 y PA-65

El tablero puede estar por detrás de la realidad en estas dos filas: se
escribieron antes de la prueba de fuego de RPT-069. **No se cierran por recuerdo.**

### PA-40 — «compilar `linux.rs` y ejecutarlo contra una interfaz»

Se cierra si, y sólo si:

- el diario muestra `Captura : ...` **sin** `NO DISPONIBLE`, y
- `Descartes del nucleo: N (vista completa)` aparece con un número —lo que exige
  que `estadisticas()` haya hablado con el núcleo de verdad—, y
- se genera tráfico en `lo` (`ping -c 20 127.0.0.1`) y `Tramas observadas` sube
  entre dos vueltas.

Las tres. La primera sola no basta: un `abrir()` que devuelve `Ok` sobre una
interfaz muerta seguiría dando cero tramas para siempre.

Si alguna falla, la fila **se corrige** con lo observado, no se deja como está.

### PA-65 — «unidad de servicio y arranque automático»

Se cierra si:

- `systemctl is-enabled eje-agente` dice `enabled`, y
- tras `sudo reboot`, el servicio está `active` sin que nadie lo arranque, y
- el socket vuelve a existir en `/run/eje-latam` tras ese reinicio.

El tercero importa: `/run` es `tmpfs` y se vacía, así que un socket que reaparece
demuestra que `RuntimeDirectory=` hizo su trabajo en un arranque de verdad y no
sólo en un `systemctl start`.

## 9. Reglas de higiene

Las de RPT-068 §5 valen enteras, y se repiten porque son las que se olvidan:

- **Encadenar con `&&`.** Un montaje que sigue tras un fallo produce salidas que
  parecen resultados.
- **`--no-pager` siempre.**
- **Comprobar el PID antes de leer un diario.**
- **Si un paso falla, no seguir al siguiente.**
- **Y una nueva, de esta prueba:** anotar el PID **de los dos** procesos. Aquí hay
  dos diarios, y confundirlos es más fácil que con uno.

## 10. Al terminar

Si las siete salen como se predice, se cierra **PA-78** y se resuelven **PA-40** y
**PA-65** con la evidencia de §8. Los resultados se anexan aquí, **incluidas las
predicciones que fallen, que son las que más valen**.

Si alguna sale distinta: qué se esperaba, qué ocurrió, qué punto nace.

## 11. Resultados de la mitad A — 25 de agosto de 2026

**Artefacto comprobado antes de observar:** `md5 37d036a5e9883f820e5ffb450346b952`,
`ExecStart` sólo con `--almacen` y `--ciclos 0`, y banner declarando
`Configuracion : FIRMADA y verificada (secuencia 1)`.

| Canal | Resultado | Tiempo |
|---|---|---|
| `obtener-estado-agente` | ❌ rechazo: *sin manejador* | 444 ms |
| `obtener-inventario` | ❌ rechazo: *sin manejador* | 559 ms |
| `obtener-estado-boveda` | ❌ rechazo: *sin manejador* | 549 ms |
| `consultar-sandbox` | ❌ rechazo: *sin manejador* | 787 ms |
| `consultar-alertas` | ✅ `{"primerDisponible":1,"hayMas":false,"sucesos":[]}` | 983 ms |
| `obtener-condiciones` | ✅ **trece** condiciones | 870 ms |

**Ningún fallo de transporte.** Los seis viajaron, se enmarcaron, y volvieron. El
enmarcado, el prefijo de longitud, el código de respuesta y el de rechazo
funcionan contra un socket real y no sólo contra vectores.

### 11.1 Cuatro de seis canales están declarados y no cableados — PA-135

`obtener-estado-agente`, `obtener-inventario`, `obtener-estado-boveda` y
`consultar-sandbox` **no tienen manejador**. El agente los rechaza con un motivo
exacto, que es el comportamiento correcto y está probado
(`un_canal_sin_manejador_se_rechaza_con_motivo_y_no_con_lista_vacia`).

Lo que **no** existía es un punto abierto que dijera que faltan. La barrera de
paridad con `contrato-ipc.toml` afirma que los seis canales están **declarados a
los dos lados**; ninguna afirma que los seis **respondan**. Se anotó en §5.2 antes
de ejecutar —«lo que ninguna afirma es que los seis respondan»— y así ha sido.

Es el defecto dominante del proyecto, esta vez con una vuelta más: el hueco estaba
**probado como correcto**. Una prueba que fija «si no hay manejador, rechaza con
motivo» es buena y necesaria, y a la vez hace que la ausencia se sienta resuelta.

### 11.2 El plano de control espera al de datos — PA-136

Entre 444 y 983 ms por petición, sobre un socket de dominio Unix local. Debería
ser de un dígito.

No es el IPC: es que el agente **atiende consultas al final del ciclo**
(RPT-034 §4), y cada ciclo espera hasta `PLAZO` = 500 ms por trama. La latencia de
la consola la fija la ventana de observación de la captura.

Es la hipótesis 5 de §7, y la que más me interesaba. No venció el plazo de 5 s
—no rompió—, pero **el mecanismo está confirmado**: el plano de control va detrás
del de datos. Con `--tramas 200` en un segmento con tráfico, un ciclo dura más, y
esos 5 s dejan de parecer holgados.

### 11.3 Las trece condiciones, leídas por el otro proceso

```json
{"inventarioSuprimido":false,"inventarioNoVerifica":false,
 "observacionSaturada":false,"capturaConPerdida":false,
 "capturaNoDisponible":false,"accionAdministrativa":false,
 "salidaNoDisponible":false,"sinColector":true,
 "escuchaNoDisponible":false,"configuracionSinFirmar":false,
 "configuracionNoVerifica":false,"registroSaturado":false,
 "evidenciaEnRiesgo":false}
```

Trece campos, en el orden del contrato. `sinColector` encendida —el sensor se
emitió sin colector a propósito—, y **`configuracionSinFirmar` apagada**, que es la
primera vez que un proceso ajeno confirma que este sensor obedece una
configuración firmada.

### 11.4 El marco troceado — §5.3

Registro sembrado de **1 245 846 bytes**. `consultar-alertas` devolvió **1 038 208
bytes** con `hayMas: true`, justo por debajo del techo de marco de 1 048 576.

Llegó **entero**. El acumulador reensambló alrededor de un megabyte repartido en
tantas lecturas como quisiera el núcleo, y el troceado lo decidió el sistema
operativo, no nuestro codificador. **Es la única afirmación de toda esta sesión que
los vectores de RPT-045 no podían hacer.**

De paso quedó ejercitada la cota por bytes y `hayMas`, que PA-97 dice que la
consola todavía no lee.

### 11.5 Sin permiso y sin agente son cosas distintas — §5.4 y §5.5

Con el socket desnudo, desde un usuario fuera del grupo: `Permission denied`,
salida 2. El `0660` protege de verdad.

Con la consola, desde ese mismo usuario:

```
[sin-permiso] sin permiso sobre el socket: el agente lo creó para su propio
              usuario y esta consola corre como otro (PA-82)
```

Seis veces, en **0 y 1 ms**. Instantáneo, es decir: **no reintentó**. Reintentar
contra un `EACCES` es girar el pomo de una puerta cerrada con llave, y PA-93 lo
dejó fijado hace semanas. Aquí se ve ocurrir.

Con el agente parado, el mismo guion:

```
[sin-socket] no existe el socket: el agente nunca llegó a abrirlo en esta ruta
```

Tres causas distintas —sin permiso, sin socket, sin respuesta— con tres mensajes
distintos, sobre un sistema real. RPT-046 §11 las separó en el tipo; esto es la
primera vez que se comprueba que llegan separadas hasta arriba.

**Lo que NO se ejecutó:** cortar el servicio **a mitad de una petición en vuelo**.
Se hizo la variante limpia —agente ya parado— y no la de dos sesiones simultáneas.
`colgar a media respuesta menciona los bytes que faltaban` sigue siendo una prueba
unitaria sin observación. Queda escrito.

### 11.6 PA-40 cerrado: la captura habla con el núcleo — §5.3 de §8

Agente a mano, `--ciclos 3 --tramas 40`, con `ping -c 40 127.0.0.1` de fondo. Los
tres criterios de §8, los tres:

| Criterio | Observado |
|---|---|
| `Captura` sin `NO DISPONIBLE` | ✅ no aparece |
| `Descartes del nucleo: N (vista completa)` | ✅ `0 (vista completa)` — la llamada a `estadisticas()` respondió |
| `Tramas observadas` > 0 y moviéndose | ✅ vueltas de 0, **4** y 0 |

Los tres hacían falta. Un `abrir()` que devuelve `Ok` sobre una interfaz muerta
habría dado el primero y cero tramas para siempre.

**Y algo que no se buscaba:** en la vuelta con tráfico apareció un dispositivo con
dirección `00:00:00:00:00:00` —`lo` no tiene MAC real— clasificado `SinIndicio` y
contado como **«requiere humano»**. Es correcto: sin marcado firmado y sin segmento
declarado limpio, nada es contenible por sí solo. Pero es la primera vez que el
camino de clasificación corre sobre tráfico de verdad y no sobre una trama
fabricada.

### 11.7 Una condición que nadie había visto encenderse

Ejecutado a mano, fuera de `systemd`, el agente declaró:

```
Escucha local      : NO disponible (/run/eje-latam/agente.sock: No such file...)
  !  El directorio del socket no existe: /run/eje-latam
     Bajo systemd lo crea RuntimeDirectory=. A mano, usa --directorio-socket.
...
    escuchaNoDisponible   : true
```

`escuchaNoDisponible` es la undécima condición, de RPT-070 (PA-125), y **se
encendió sola en la primera situación real que la produce**. Además el agente
siguió observando: perder la escucha no apaga la captura, que es RPT-047 (PA-81)
en la dirección contraria a la que se escribió.

El aviso sobre `RuntimeDirectory=` también hizo su trabajo: explica el directorio
que falta, no el socket, que es lo que el mensaje del sistema no dice.

### 11.8 Las predicciones de §6, sin ajustar

| # | Predicción | Resultado |
|---|---|---|
| 1 | Los seis responden, ninguno rechaza | ❌ **Falló.** Cuatro rechazaron |
| 2 | Trece condiciones, `configuracionSinFirmar` en `true` | ⚠️ Trece ✅; el valor es `false` porque el escenario cambió al camino A por acuerdo. **No cuenta como acierto** |
| 3 | Coinciden campo a campo con el latido del diario | 🔵 **Parcial.** El modo servicio no imprime la lista (RPT-072); la ejecución a mano sí, y coincide salvo `escuchaNoDisponible`, que difiere **porque la situación difiere** (§11.7) |
| 4 | Marco troceado se reensambla | ✅ **Acertó.** 1 038 208 bytes enteros, `hayMas: true` |
| 5 | `obtener-inventario` responde **vacío** | ❌ **Falló.** Rechazó por falta de manejador |
| 6 | Sin agente, lenguaje de operador | ✅ **Acertó** en la variante limpia. La de «a mitad de frase» no se ejecutó |
| 7 | Sin permiso: `EACCES` y sin reintento | ✅ **Acertó.** Seis fallos en 0–1 ms, sin reintento |

Tres aciertos, dos fallos, una superada y una parcial.

**La predicción sobre el conjunto —«al menos una de las siete fallará»— acertó**,
y fallaron dos. Las dos por lo mismo, que es lo que hace que valga la pena: yo
sabía que la paridad del contrato estaba probada y **la leí como si probara que
los canales funcionan**. Es exactamente el error que este protocolo existe para
sacar a la luz.

### 11.9 Qué se cierra y qué no

**PA-40 cerrado** (§11.6): los tres criterios, observados.

**PA-78: se cierra la mitad A.** Los dos procesos se hablan por un socket real,
con marcos de un megabyte, y las tres causas de fallo llegan separadas hasta
arriba. Lo que queda es la **mitad B** —que el operador vea esto en una pantalla—
y no se pudo hacer porque la VM no tiene escritorio (§4.2). Un punto cerrado a
medias es peor que uno abierto, así que **PA-78 sigue abierto** con su mitad
resuelta y escrita.

**PA-103 (`noServido`) se ejercitó cuatro veces por rechazo y seis por ausencia**,
pero sólo se vio el motivo, nunca la presentación. Sigue abierto por lo mismo.

**PA-65 cerrado** (§11.10).

### 11.10 El reinicio, y lo que confirmó de paso

```
systemctl is-enabled  → enabled
[reboot]
systemctl is-active   → active
stat                  → srw-rw---- root:vboxeruser /run/eje-latam/agente.sock
conversar.mjs         → Los dos procesos se hablan.
```

Los tres criterios de §8 para PA-65. El tercero es el que decide: `/run` es
`tmpfs` y se vacía en cada arranque, así que un socket que **reaparece** demuestra
que `RuntimeDirectory=` hizo su trabajo en un arranque de verdad y no sólo en un
`systemctl start`. Con el grupo correcto, además, que es lo que costó PA-124.

**Y algo que no se iba a buscar aquí.** Tras el reinicio, `configuracionSinFirmar`
sigue en `false`: el sensor releyó su configuración firmada del disco, la verificó
otra vez, y **la comparó contra la marca de agua que él mismo había avanzado a 1 en
el arranque anterior** (RPT-078 §7).

Ahí se ejercitó por primera vez, sobre una máquina real, la decisión de comparar
con `<` y no con `<=`. Con la comparación estricta, la secuencia 1 no habría
superado a la marca 1 y **este sensor no habría vuelto a arrancar nunca**. Quedó
escrito el 21 de agosto como razonamiento; el 25 se vio ocurrir.

## 12. Puntos abiertos

| ID | Punto |
|---|---|
| PA-78 | Este protocolo. Sin ejecutar |
| ~~PA-132~~ | ✅ **Cerrado antes de ejecutar** (§2.1). El punto de encuentro vive en `contrato-ipc.toml`, con paridad probada en `xtask` y en `contrato.prueba.ts`, y la constante de la consola salió a un módulo sin Electron para que una prueba pueda mirarla |
| PA-40 | Lo resuelve §8, en un sentido o en el otro |
| PA-65 | Lo resuelve §8, en un sentido o en el otro |
| PA-103 | La rama `noServido` se ejercita en §5.4, pero cerrarla exige mirar la vista: sólo con escritorio |
| PA-100 | Fuera de alcance a propósito (§3) |

---

*Reporte Nº 79 — Protocolo de la conversación · PremosCorp · 21 de agosto de 2026*
