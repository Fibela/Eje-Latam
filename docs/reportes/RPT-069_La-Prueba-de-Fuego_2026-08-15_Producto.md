# RPT-069 — La prueba de fuego

**Tema:** Ejecución de RPT-068 en máquina limpia. PA-117
**Nº de reporte:** 069
**Fecha:** 15 de agosto de 2026
**Área designada:** Producto
**Entidad:** PremosCorp
**Estado:** **Cierra PA-117, PA-120, PA-124 y PA-107.** Acuña PA-122, PA-123, PA-125 y PA-126

- **Depende de:** RPT-068 (el protocolo y las predicciones), RPT-067 (la separación de rutas), RPT-062 (la unidad)
- **Aborda:** PA-117
- **Acuña:** PA-122, PA-123, PA-124, PA-125

Entorno: VirtualBox, Ubuntu Server 26.04, `systemd` como PID 1, misma versión que
la máquina que compila. Artefacto transferido entero, instalado con su propio
`instalar.sh`.

---

## 1. Resultado de las cinco observaciones

| # | Predicción | Observado |
|---|---|---|
| 4.1 | `active (running)`, `sinColector: true`, `salidaNoDisponible: false` | ✅ exacto |
| 4.2 | `srw-rw---- root:1000 /run/eje-latam/agente.sock` | ❌ **no hay socket** |
| 4.3 | PID distinto y no cero tras `kill -9` | ✅ 1645 → 2518 |
| 4.4 | `/run/eje-latam` desaparece al parar | ✅ `No such file or directory` |
| 4.5 | falla en `/etc`, escribe en `/var/lib` | ✅ las dos mitades |

**PA-117 queda cerrado.** Sus dos afirmaciones —que `Restart=always` devuelve el
proceso tras una muerte a traición, y que `ProtectSystem=strict` impide de verdad
escribir fuera de `ReadWritePaths`— están observadas, con la contraprueba que
demuestra que es bisturí y no martillo.

## 2. Lo que descubrió, que es más de lo que confirmó

Cuatro puntos abiertos en veinte minutos de máquina real, **ninguno visible desde
el código**:

- **PA-122** — la línea de uso no menciona `--directorio-socket`. Salió en la
  comprobación de humo, que existía para detectar incompatibilidad de
  bibliotecas.
- **PA-123** — el agente escribe el informe completo cada vuelta: dos bloques
  entre las 02:17:19 y las 02:17:20, unas 50 líneas por segundo a `journald`, en
  un segmento sin tráfico.
- **PA-124** — el sensor arranca **sin escucha local** (§3).
- **PA-125** — y ninguna condición lo declara (§4).

## 3. PA-124 — dos mecanismos correctos que juntos se anulan

```
Escucha local : NO disponible (/run/eje-latam/agente.sock: no se pudo asignar
el grupo 1000 al socket: Operation not permitted (os error 1))
```

El servicio corre como root, pero `CapabilityBoundingSet=CAP_NET_RAW` le quita
todo lo demás, incluida **`CAP_CHOWN`**. Cambiar el grupo de un fichero a uno al
que el proceso no pertenece exige esa capacidad. Sin ella, el socket no se crea.

Las dos piezas son correctas por separado:

- Endurecer la unidad al mínimo de capacidades es lo que RPT-062 buscaba.
- Restringir el socket a un grupo es PA-82, y existe para que la consola no
  necesite `sudo`.

Juntas dejan al cliente **sin consola**. Y no había forma de verlo antes: las
pruebas del empaquetado leen el texto de la unidad, y el texto era correcto en
las dos directivas por separado. Hacía falta un `systemd` que aplicara el
conjunto acotado de verdad.

La corrección es una capacidad más en las dos directivas, con prueba que exige
ambas —conceder por encima del techo no otorga nada, así que una sola en verde no
afirmaría nada.

**El precio queda escrito:** `CAP_CHOWN` está ahí por PA-82. Si el socket no
llevara grupo, sobraría — y la consola necesitaría `sudo`.

### La observación que lo cierra

Con la unidad corregida, reinstalada y reiniciada:

```
AmbientCapabilities=CAP_NET_RAW CAP_CHOWN
CapabilityBoundingSet=CAP_NET_RAW CAP_CHOWN
srw-rw---- root:vboxeruser /run/eje-latam/agente.sock
```

Cierra **PA-124** y, con él, **PA-120**: el socket nace en el directorio volátil,
con su grupo, y desaparece con él al parar el servicio.

Y cierra **PA-107**, que se mantuvo parcial un día de más a propósito: sus cinco
comprobaciones estaban hechas desde ayer, pero el artefacto que las pasaba dejaba
al cliente sin consola. Un punto cerrado por el enunciado habría dejado escrito
que el empaquetado estaba resuelto.

Al cerrarlo apareció un hilo suelto: `cargo xtask empaquetar` avisa en **cada
ejecución** de que el formato de distribución sigue sin decidirse (RPT-054 §9), y
eso no tenía fila en el tablero. Es **PA-126**.

### Y una observación que nadie buscó: sobrevive al apagado

La máquina se apagó por la noche y arrancó al día siguiente. Sin tocar nada:

```
PID actual: 1033
Aug 15 03:47:37 eje-prueba eje-agente[3240]: Escucha local : /run/eje-latam/agente.sock
Aug 16 02:54:13 eje-prueba eje-agente[1033]: Escucha local : /run/eje-latam/agente.sock
```

`/run` es `tmpfs`: se vació **entero** al apagar, y el socket volvió a nacer con
el servicio. El argumento de RPT-067 §2 deja de ser una propiedad de diseño y
pasa a ser una observación.

Vale la pena decir que esto no estaba en el protocolo. Salió de dejar la máquina
encendida de un día para otro, que es lo más parecido a un despliegue real que ha
tenido este proyecto.

## 4. PA-125 — el sensor se declaraba sano

Mientras ocurría todo lo anterior, las diez condiciones decían esto:

```
capturaNoDisponible   : false
accionAdministrativa  : true
salidaNoDisponible    : false
sinColector           : true
```

Todo correcto, y **ninguna dice que la escucha local esté caída**. Un sensor al
que la consola no puede conectarse se presenta como sano.

Peor: lo único que podría contarlo es la consola, que es justamente lo que no
puede conectar. Y la sala tampoco se entera, porque el latido lleva las
condiciones y esa no está entre ellas.

Es exactamente la forma de PA-109: la condición existe en el mundo y no en el
vocabulario. La diferencia es que aquella se descubrió razonando y ésta
ocurriendo.

## 5. Por qué PA-107 no se cierra hoy

Sus cinco comprobaciones de RPT-054 §8 están hechas. Cerrarlo dejaría escrito que
el empaquetado está resuelto, mientras el paquete que las pasa deja al cliente sin
consola.

Es la decisión de PA-64, repetida: *«un punto cerrado con el enunciado original
habría dejado escrito que el hueco está tapado»*.

## 6. Lo que la prueba **no** midió

- **`ReadWritePaths` no lo ejercitó el agente.** `/var/lib/eje-latam` quedó
  **vacío**: sin clave aprovisionada y sin alertas no hay nada que persistir. La
  observación 4.5 midió un servicio de juguete con las mismas directivas.
- **La captura fue sobre `lo`**, sin tráfico, a propósito.
- **El colector estuvo vacío**, también a propósito.

## 7. El montaje, para quien lo repita

Tres tropiezos, ninguno del producto, todos por supuestos míos sobre el entorno:

- **`systemctl is-active ssh` da `inactive` y está bien.** Desde Ubuntu 24.04
  OpenSSH va activado por socket: quien escucha es `ssh.socket`. Pregunté por el
  instrumento equivocado.
- **WSL2 no comparte `localhost` con Windows.** La redirección de VirtualBox vive
  en el loopback de Windows; el `scp` hay que lanzarlo desde PowerShell.
- **`scp` desde Windows pierde el bit de ejecución**, porque NTFS no tiene
  permisos Unix. No afecta a la instalación —`instalar.sh` hace `install -m
  0755`— pero sí a la comprobación de humo.

Y uno del propio VirtualBox: la regla de reenvío de puertos no se guarda hasta
aceptar **dos** diálogos. El síntoma es `Connection refused` desde el propio
anfitrión, y se diagnostica con `netstat -ano | findstr :2222`.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-117~~ | ✅ **Cerrado por observación** (§1) |
| ~~PA-124~~ | ✅ **Cerrado por observación** (§3) |
| ~~PA-120~~ | ✅ **Cerrado**: el socket nace en `/run` y muere con el directorio |
| ~~PA-107~~ | ✅ **Cerrado** (§3), un día después de tener sus cinco comprobaciones |
| PA-125 | Undécima condición: nadie declara que la escucha esté caída. Sin abordar |
| PA-122 | La línea de uso y el analizador son dos listas a mano |
| PA-123 | El informe completo cada vuelta, a `journald` |
| PA-126 | El formato de distribución del paquete, sin decidir |

---

*Reporte Nº 69 — La prueba de fuego · PremosCorp · 15 de agosto de 2026*
