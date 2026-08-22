# Puesta en marcha local — agente y consola

Cómo levantar `eje-agente` y el puesto de diagnóstico de `eje-vision` en una
máquina de desarrollo (Linux o WSL). PA-89.

No describe el despliegue de un cliente: eso depende de PA-77 y PA-79, que
siguen abiertos.

---

## 1. Lo mínimo

```bash
cd /mnt/c/Eje-latam
cargo build --bin eje-agente
scripts/arrancar-agente.sh
```

El guion pide la contraseña, arranca el agente en segundo plano y termina
mostrando los permisos del socket. Debe verse así:

```text
srw-rw---- 1 0 1000  /tmp/eje/agente.sock
```

`root` como propietario, **tu grupo** en la cuarta columna, y `srw-rw----`. Si
falta el segundo `rw`, la consola no podrá conectar sin `sudo`.

> **En desarrollo el socket está en `/tmp/eje`; en producción, en
> `/run/eje-latam`.** No es un descuido: `/run` lo crea `systemd` con
> `RuntimeDirectory=` y hace falta root, lo que no tiene sentido para levantar
> una consola de diagnóstico. El guion pasa `--directorio-socket "$ALMACEN"`
> justamente por eso (RPT-067, PA-120). Lo persistente sigue en `--almacen`.

Después, la consola:

```bash
cd apps/eje-vision && npm run diagnostico
```

Se llama `diagnostico` y no `start` a propósito: **no es VIS-04**, es un puesto
de observación deliberadamente feo. Si alguien lo confunde con el producto, el
problema es que se parece demasiado.

Para parar: `scripts/arrancar-agente.sh --parar`.

---

## 2. Por qué hay un guion y no tres comandos

El arranque manual costó cuatro rondas de diagnóstico en una sola sesión, y
ninguna por un fallo del producto. Vale la pena conocerlas, porque reaparecerán
en cualquier despliegue que se haga a mano:

**La redirección la ejecuta tu shell, no `sudo`.** `sudo agente > /tmp/eje/log`
crea el fichero como **tu usuario**. Si el directorio es de root, falla antes de
que `sudo` entre en juego, y el mensaje —`Permission denied` sobre el log— no
menciona ni permisos del agente ni la captura.

**`sudo` en segundo plano puede quedar suspendido.** Si la contraseña no está
cacheada, `sudo` intenta leer del terminal, recibe `SIGTTIN` y el shell suspende
el trabajo. Aparece como `Stopped`, no como `Exit`: no hay socket, no hay log, y
nada dice por qué. El guion hace `sudo -v` **antes**, en primer plano.

**Un socket huérfano sobrevive al proceso.** Sin captura de señales (RPT-034 §1),
un agente que muere deja el fichero. El cliente da `ECONNREFUSED` sobre algo que
existe: comprobar que el fichero está no dice que el agente esté. El guion lo
retira, pero **sólo si nadie escucha** — borrarlo con otro agente vivo lo dejaría
sordo sin que se entere.

Esto **sigue siendo cierto aquí y ya no en producción**: con el socket en `/run`,
que es tmpfs, y con `systemd` retirando el directorio al parar el servicio, el
socket huérfano dejó de ser posible por construcción (RPT-067). El guion lo sigue
tratando porque `/tmp/eje` no se vacía solo.

**Esperar con `sleep 2` es correcto hasta el día que la máquina va lenta.** El
guion espera al socket con cota, no a ciegas.

---

## 3. Electron en Linux mínimo

Un Linux de servidor no trae las bibliotecas que Electron necesita. En Ubuntu:

```bash
sudo apt-get install -y libnss3 libnspr4 libatk1.0-0t64 libatk-bridge2.0-0t64 \
  libcups2t64 libdrm2 libxkbcommon0 libxcomposite1 libxdamage1 libxfixes3 \
  libxrandr2 libgbm1 libasound2t64 libpango-1.0-0 libcairo2
```

(En versiones anteriores a 24.04, sin los sufijos `t64`.)

**Que haga falta esto es parte del argumento de PA-77.** Son NSS, CUPS, ALSA y
GBM —varios cientos de megas— instalados en el equipo que vigila la red. En una
máquina de desarrollo es razonable; en un sensor hospitalario es superficie que
alguien tendrá que justificar.

---

## 4. Qué se ve cuando funciona

Las nueve condiciones, refrescadas cada dos segundos. Con un almacén nuevo,
`acción administrativa` sale en `sí`: no hay clave aprovisionada y nada se puede
verificar todavía.

Si alguna fila dice **`AUSENTE EN LA RESPUESTA`**, el contrato se ha
desincronizado: el agente no está mandando un campo que la interfaz espera. El
puesto lo distingue de `no` a propósito — un panel que pintara los campos
ausentes como «no» diría que todo va bien exactamente igual.

---

## 5. Puntos abiertos que afectan a esto

| ID | Qué cambiaría |
|---|---|
| PA-77 | Si el sensor es headless, la consola no va ahí y esta guía se parte en dos |
| PA-79 | La ruta del socket saldría de configuración firmada, no de `--almacen` |
| PA-81 | Hoy el agente muere si no puede capturar; entonces se degradaría y seguiría sirviendo |
| PA-84 | `--grupo-ipc` aceptaría un nombre de grupo y no un número |

---

*PremosCorp · Puesta en marcha local*
