#!/usr/bin/env bash
#
# Arranca eje-agente para desarrollo y diagnostico. PA-89.
#
# # Por que existe
#
# El arranque manual costo CUATRO rondas de diagnostico en una sola sesion, y
# ninguna por un fallo del producto:
#
#   1. `> /tmp/eje/agente.log` la ejecuta la SHELL como el usuario, no `sudo`.
#      Con el directorio en manos de root, la redireccion falla y el agente ni
#      llega a arrancar. El mensaje no menciona ni permisos ni al agente.
#   2. El directorio desaparecio entre pruebas y el fallo volvio con otra cara.
#   3. `sudo` en segundo plano pide contrasena, no puede leer del terminal,
#      recibe SIGTTIN y el shell SUSPENDE el trabajo. Aparece como `Stopped`,
#      no como `Exit`, y no hay socket ni log que mirar.
#   4. Un socket huerfano de una ejecucion anterior hace que el cliente diga
#      ECONNREFUSED sobre un fichero que existe.
#
# Ninguna de las cuatro es un error de quien escribe los comandos: son cuatro
# formas de que el entorno mienta. Un guion no las evita por disciplina, las
# evita por construccion.
#
# Uso:
#   scripts/arrancar-agente.sh [interfaz]     # arranca (por omision: lo)
#   scripts/arrancar-agente.sh --parar        # detiene

set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ALMACEN="${EJE_ALMACEN:-/tmp/eje}"
SOCKET="$ALMACEN/agente.sock"
REGISTRO="$ALMACEN/agente.log"
BINARIO="$RAIZ/target/debug/eje-agente"

if [[ "${1:-}" == "--parar" ]]; then
  sudo pkill -x eje-agente 2>/dev/null && echo "agente detenido" || echo "no habia agente en marcha"
  sudo rm -f "$SOCKET"
  exit 0
fi

INTERFAZ="${1:-lo}"

if [[ ! -x "$BINARIO" ]]; then
  echo "No existe $BINARIO" >&2
  echo "Compilalo:  cd $RAIZ && cargo build --bin eje-agente" >&2
  exit 1
fi

# (1) y (2). El directorio lo crea el USUARIO, no root: la redireccion del log
# la hace esta shell y necesita poder escribir aqui.
mkdir -p "$ALMACEN"
if [[ ! -w "$ALMACEN" ]]; then
  echo "$ALMACEN existe y no puedes escribir en el." >&2
  echo "Probablemente lo creo root. Arreglalo:  sudo chown -R \$(id -u):\$(id -g) $ALMACEN" >&2
  exit 1
fi

# (4). Se retira solo si NO hay nadie escuchando. Si otro agente esta vivo sobre
# esa ruta, borrarlo lo dejaria sordo sin que se entere.
if [[ -S "$SOCKET" ]]; then
  if sudo pkill -0 -x eje-agente 2>/dev/null; then
    echo "Ya hay un eje-agente en marcha. Paralo primero:  $0 --parar" >&2
    exit 1
  fi
  echo "Retirando socket huerfano de una ejecucion anterior."
  sudo rm -f "$SOCKET"
fi

# (3). La contrasena se pide AQUI, en primer plano, donde se puede leer. Sin
# esto, el `sudo` de mas abajo recibe SIGTTIN y el trabajo queda suspendido.
echo "Se necesitan privilegios para capturar tramas en '$INTERFAZ'."
sudo -v

sudo "$BINARIO" \
  --interfaz "$INTERFAZ" \
  --almacen "$ALMACEN" \
  --ciclos 0 \
  --grupo-ipc "$(id -g)" \
  > "$REGISTRO" 2>&1 &

# El socket no aparece al instante. Se espera con cota en lugar de un `sleep`
# fijo: un `sleep 2` es correcto hasta el dia que la maquina va lenta.
for _ in $(seq 1 40); do
  [[ -S "$SOCKET" ]] && break
  sleep 0.25
done

if [[ ! -S "$SOCKET" ]]; then
  echo "El agente no abrio el socket. Ultimas lineas de $REGISTRO:" >&2
  tail -n 15 "$REGISTRO" >&2
  exit 1
fi

echo
ls -ln "$SOCKET"
echo "Tu grupo: $(id -g)   (debe coincidir con la cuarta columna, y el modo ser srw-rw----)"
echo
echo "Registro : tail -f $REGISTRO"
echo "Consola  : cd $RAIZ/apps/eje-vision && npm run diagnostico"
echo "Parar    : $0 --parar"
