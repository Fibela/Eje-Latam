# RPT-068 — Protocolo de la prueba de fuego

**Tema:** PA-117. Ciclo de vida y confinamiento del servicio en máquina limpia
**Nº de reporte:** 068
**Fecha:** 14 de agosto de 2026
**Área designada:** Producto
**Entidad:** PremosCorp
**Estado:** **Protocolo emitido. Predicciones escritas antes de ejecutar.** Sin observación todavía

- **Depende de:** RPT-054 §8 (las cinco comprobaciones), RPT-062 §5 (por qué exige `systemd` como PID 1), RPT-067 (la separación que hace exacta la medición)
- **Aborda:** PA-117. Al cerrarse, cierra también PA-120 y PA-107

---

## 1. Por qué esto se escribe antes

En PA-118 la predicción fue *«`systemd` dejará `--syslog` colgando y el agente
saldrá con error de uso»*. Fue **errónea**, y el comportamiento real era peor: no
rompía, mentía. Sin la predicción escrita, la salida se habría leído como
confirmación de lo que se esperaba.

Y en RPT-064 §6 quedaron anotadas tres lecturas de evidencia del proceso
equivocado en media hora. De ahí las reglas de §5.

## 2. Lo que se va a afirmar, y lo que no

| # | Afirmación | Cómo se comprueba |
|---|---|---|
| 1 | El servicio arranca en un `systemd` real | `systemctl start` + banner en el diario |
| 2 | El socket nace en `/run/eje-latam` y **no** en `/var/lib` | `stat` de los dos |
| 3 | `Restart=always` devuelve el proceso tras una **muerte a traición** | `kill -9` al `MainPID` |
| 4 | El directorio volátil **desaparece** al parar | `ls -ld /run/eje-latam` |
| 5 | `ProtectSystem=strict` impide de verdad escribir fuera | intentándolo, no leyendo la unidad |

**Lo que este protocolo no afirma:** nada sobre detección. Se vigila `lo`, que no
tiene tráfico interesante, a propósito: lo que se mide es el servicio, y meter la
captura en la ecuación añade una forma más de que la prueba falle por algo que no
es lo que se prueba.

## 3. Preparación

En la máquina de desarrollo:

```bash
cd /mnt/c/Eje-latam && \
cargo build --release --bin eje-agente && \
cargo xtask empaquetar && \
cargo xtask probar-instalador
```

Encadenado con `&&` por lo de siempre: si `empaquetar` falla y el resto sigue, se
instala el binario anterior y la observación describe un código que no está ahí.

El artefacto viaja entero, con su `instalar.sh`. Con Multipass:

```bash
multipass launch --name eje-prueba && \
multipass transfer -r /mnt/c/Eje-latam/target/paquete/eje-agente eje-prueba:/home/ubuntu/ && \
multipass shell eje-prueba
```

Se copia el **directorio**, no sólo el binario: el instalador, la unidad y la
configuración de ejemplo son parte de lo que se está probando. Instalar a mano el
binario suelto mediría otra cosa —lo que uno recuerda que hay que hacer— en lugar
del artefacto.

### Con VirtualBox en lugar de Multipass

Sirve igual: lo único que el protocolo exige del entorno es `systemd` como PID 1.
Cambia el transporte, y conviene fijar dos cosas antes.

**La versión de Ubuntu de la VM debe ser la misma que la de la máquina que
compila.** Se averigua con `lsb_release -ds` en las dos. Si no coinciden, la
comprobación de humo de más abajo lo dirá, pero es más barato elegir bien el ISO
que descubrirlo con la VM ya instalada.

**VirtualBox y WSL2 se disputan el hipervisor.** VirtualBox 7 convive con Hyper-V,
pero en modo lento. Es irrelevante para lo que se mide aquí —arrancar, matar y
comprobar permisos— y sólo se anota para que la lentitud no se lea como un
síntoma del agente.

Instalado Ubuntu Server con OpenSSH, en red NAT con redirección del puerto
`2222 → 22`, desde la máquina que compila:

```bash
scp -P 2222 -r /mnt/c/Eje-latam/target/paquete/eje-agente ubuntu@127.0.0.1:~/ && \
ssh -p 2222 ubuntu@127.0.0.1
```

La carpeta compartida de VirtualBox también vale, pero **no para instalar desde
ella**: monta con permisos y propietario impuestos por el anfitrión, y el
instalador hace `install -m 0755`. Copiar a `~` primero y ejecutar allí evita
medir el sistema de ficheros compartido en lugar del instalador.

**Y se comprueba que llegó, no que se copió.** RPT-070 §8: tres observaciones
seguidas se tomaron con el binario del día anterior, y la conclusión inmediata
habría sido que el código nuevo no funcionaba. Cuesta veinte segundos:

```bash
md5sum /mnt/c/Eje-latam/target/paquete/eje-agente/eje-agente   # en la maquina que compila
md5sum $HOME/eje-agente/eje-agente                             # en la VM
```

Si no coinciden, **parar aquí**. Cualquier cosa medida a partir de este punto
describe otro código.

Ya dentro de la VM:

```bash
ps -p 1 -o comm=              # tiene que decir systemd. Si no, parar aquí
cd ~/eje-agente && ls
./eje-agente                  # sin argumentos: se espera un error de uso, no un error de carga
```

La segunda es barata y descarta de golpe la incompatibilidad de bibliotecas entre
la máquina que compila y la que ejecuta. **Un error de uso es el resultado
bueno**; `error while loading shared libraries` significa que hay que compilar en
la VM.

Después:

```bash
sudo sh instalar.sh
ip -br link                   # el nombre real de la interfaz de esta VM
sudo nano /etc/eje-latam/agente.conf
```

En la configuración: `EJE_INTERFAZ=lo` y `EJE_GRUPO_IPC=` el gid de tu usuario
(`id -g`). `EJE_COLECTOR` se deja **vacío** — este protocolo no mide el colector,
y con la corrección de PA-118 el agente lo declara sin mentir.

```bash
sudo systemctl daemon-reload && sudo systemctl enable --now eje-agente
```

## 4. Las cinco observaciones, con su predicción

### 4.1 Arranca

```bash
systemctl status eje-agente --no-pager && \
journalctl -u eje-agente --no-pager -n 30
```

**Se predice:** `active (running)`, y en el diario `sinColector: true` con
`salidaNoDisponible: false` — las dos como quedaron en RPT-064 §4.

**Puede salir mal así:** si `EJE_INTERFAZ` quedó en `eth0` y esa interfaz no
existe, se espera `Captura : NO DISPONIBLE` **y el servicio vivo** (RPT-047,
PA-81). Si en cambio el servicio muere y reintenta en bucle, eso es un hallazgo y
no un error de montaje.

### 4.2 El socket está donde debe y no donde no

```bash
stat -c '%A %U:%G %n' /run/eje-latam/agente.sock && \
ls -la /var/lib/eje-latam
```

**Se predice:** `srw-rw---- root:<tu grupo> /run/eje-latam/agente.sock`, y en
`/var/lib/eje-latam` **ningún** `.sock` — sólo `evidencia.alm`, `centinela.dat` y
compañía.

**Puede salir mal así:** si el socket no aparece pero el servicio está vivo, lo
más probable es que `/run/eje-latam` no exista, y eso significaría que
`RuntimeDirectory=` no llegó a la unidad instalada. El agente lo dice él mismo
desde RPT-067: *«El directorio del socket no existe»*.

### 4.3 Vuelve tras una muerte a traición

```bash
ANTES=$(systemctl show -p MainPID --value eje-agente) && echo "antes: $ANTES" && \
sudo kill -9 "$ANTES" && sleep 8 && \
DESPUES=$(systemctl show -p MainPID --value eje-agente) && echo "despues: $DESPUES" && \
journalctl -u eje-agente --no-pager -n 15
```

**Se predice:** `$DESPUES` distinto de `$ANTES` y distinto de `0`, con
`RestartSec=5` de por medio — de ahí el `sleep 8`.

**Tiene que ser `kill -9`.** Un `systemctl stop` es una parada ordenada y
`Restart=always` no reinicia tras ella: probaríamos lo contrario de lo que
queremos, **y saldría verde**.

**Puede salir mal así:** si `$DESPUES` es `0`, el servicio no volvió; conviene
mirar si `systemd` lo puso en `failed` por exceso de reinicios, que sería un
hallazgo distinto (`StartLimitBurst`) y no la ausencia de `Restart=always`.

### 4.4 El directorio volátil desaparece al parar

```bash
sudo systemctl stop eje-agente && ls -ld /run/eje-latam
```

**Se predice:** `No such file or directory`.

Esto es lo que cierra PA-120 y lo que erradica el socket huérfano por
construcción: no queda fichero que sobreviva al proceso, así que el cliente no
puede recibir `ECONNREFUSED` sobre algo que existe.

**Puede salir mal así:** si el directorio sigue ahí, es que alguien lo creó fuera
de `systemd` — y el sospechoso sería el instalador, aunque hoy no lo toca.

### 4.5 El confinamiento es real

```bash
sudo systemd-run --unit=prueba-aislamiento --wait --collect \
  --property=ProtectSystem=strict \
  --property=ReadWritePaths=/var/lib/eje-latam \
  /bin/sh -c 'echo x > /etc/eje-prueba-de-fuga; echo "codigo=$?"'
```

**Se predice:** falla, con `Read-only file system`. Y la contraprueba, que importa
igual:

```bash
sudo systemd-run --unit=prueba-permitida --wait --collect \
  --property=ProtectSystem=strict \
  --property=ReadWritePaths=/var/lib/eje-latam \
  /bin/sh -c 'echo x > /var/lib/eje-latam/prueba-permitida && echo escribio'
```

**Se predice:** escribe.

**Sin la segunda, la primera no afirma nada.** Un `ProtectSystem` que lo prohíba
todo también haría fallar la primera, y el servicio no funcionaría — pero la
prueba saldría verde. Es la misma guarda que la prueba de la unidad de RPT-067 §7:
una comprobación que sólo puede salir bien de una manera no comprueba nada.

Y hay que decir qué **no** prueba esto: es un servicio de juguete con las mismas
directivas, no el agente. Que el agente esté sujeto a ellas lo dice el fichero de
la unidad, que ya tiene su propia prueba.

## 5. Reglas de higiene, que aquí no son opcionales

- **Encadenar con `&&`.** Un montaje que sigue tras un fallo produce salidas que
  parecen resultados.
- **`--no-pager` siempre.** `systemctl show -p ExecStart` sale truncado sin él, y
  la línea cortada es la que decide.
- **Comprobar el PID antes de leer un diario.** Ya se leyeron líneas del PID 4397
  como si fueran del 4518, y la evidencia parecía confirmar un defecto que no
  mostraba.
- **Si un paso falla, no seguir al siguiente.** Seis montajes fallidos de la
  prueba de PA-104 y ninguno era el mecanismo.

## 6. Al terminar

Si las cinco salen como se predice, se cierran **PA-117**, **PA-120** y —por ser
la cuarta de las cinco comprobaciones— **PA-107**. Los resultados se anexan a
este reporte, incluidas las predicciones que hayan fallado, que son las que más
valen.

Si alguna sale distinta, no se ajusta la predicción: se escribe qué se esperaba,
qué ocurrió y qué punto abierto nace de ahí.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| PA-117 | Este protocolo. Sin ejecutar |
| PA-120 | Lo cierran §4.2 y §4.4 |
| PA-107 | Lo cierra §4.3, que es su comprobación 4 |
| PA-79 | La configuración sigue siendo un fichero de texto editable |

---

*Reporte Nº 68 — Protocolo de la prueba de fuego · PremosCorp · 14 de agosto de 2026*
