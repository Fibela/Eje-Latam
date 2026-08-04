#!/usr/bin/env bash
#
# verificar-inconclusos.sh
#
# RPT-003 §9.4. Bloquea el build de release ante implementaciones inconclusas y
# datos simulados fuera de `#[cfg(test)]`.
#
# Motivo: un `todo!()` que llega a produccion en un modulo de contencion no
# devuelve un error controlado — aborta el proceso que vigila la red de una
# fabrica. Y un mock que devuelve exito valida el mock, no la contencion.
#
# Uso:  ./scripts/verificar-inconclusos.sh [ruta]

set -euo pipefail

RAIZ="${1:-crates}"
FALLOS=0

rojo()  { printf '\033[0;31m%s\033[0m\n' "$1"; }
verde() { printf '\033[0;32m%s\033[0m\n' "$1"; }
gris()  { printf '\033[0;90m%s\033[0m\n' "$1"; }

# Lista de ficheros .rs excluyendo directorios de pruebas dedicados.
mapfile -t FUENTES < <(find "$RAIZ" -name '*.rs' -not -path '*/tests/*' -not -path '*/target/*' | sort)

if [ ${#FUENTES[@]} -eq 0 ]; then
    rojo "No se encontro ningun fichero .rs bajo '$RAIZ'."
    exit 1
fi

gris "Analizando ${#FUENTES[@]} ficheros bajo '$RAIZ'..."

# ---------------------------------------------------------------------------
# Elimina los bloques `#[cfg(test)] mod ... { ... }` antes de buscar.
# Los marcadores dentro de pruebas son legitimos; fuera de ellas, no.
# ---------------------------------------------------------------------------
sin_bloques_de_prueba() {
    awk '
        /#\[cfg\(test\)\]/ { en_prueba = 1 }
        en_prueba {
            n = gsub(/\{/, "{"); llaves += n
            n = gsub(/\}/, "}"); llaves -= n
            if (llaves <= 0 && /\}/) { en_prueba = 0; llaves = 0 }
            next
        }
        { print FILENAME ":" FNR ":" $0 }
    ' "$1"
}

buscar() {
    local etiqueta="$1" patron="$2" encontrados=""

    for fuente in "${FUENTES[@]}"; do
        local hallazgo
        hallazgo="$(sin_bloques_de_prueba "$fuente" | grep -E "$patron" || true)"
        if [ -n "$hallazgo" ]; then
            encontrados+="$hallazgo"$'\n'
        fi
    done

    if [ -n "$encontrados" ]; then
        rojo "FALLO — $etiqueta"
        printf '%s' "$encontrados" | sed 's/^/    /'
        FALLOS=$((FALLOS + 1))
    else
        verde "OK    — $etiqueta"
    fi
}

buscar "Implementaciones inconclusas (todo! / unimplemented!)" \
       '(^|[^[:alnum:]_])(todo|unimplemented)!'

buscar "Panicos con marcador pendiente" \
       'panic!\(\s*"(TODO|FIXME|PENDIENTE|pendiente)'

buscar "Marcadores de trabajo pendiente en ruta de produccion" \
       '//\s*(TODO|FIXME|XXX|HACK|PENDIENTE)'

buscar "Endpoints o rutas sin implementar" \
       '(NotImplemented|NoImplementado|501\s*Not\s*Implemented)'

buscar "Datos simulados fuera de pruebas" \
       '(^|[^[:alnum:]_])(mock|Mock|MOCK|dummy|Dummy|stub_|fake_|Fake)'

buscar "Credenciales o puntos finales de ejemplo" \
       '(localhost:[0-9]+|127\.0\.0\.1|example\.com|cambiame|changeme|contrasena123)'

echo
if [ "$FALLOS" -gt 0 ]; then
    rojo "$FALLOS verificacion(es) fallida(s). El build de release queda bloqueado."
    gris "Si un hallazgo es legitimo, muevelo a un bloque #[cfg(test)] o documenta"
    gris "la excepcion en un reporte antes de anadirla a este script."
    exit 1
fi

verde "Sin implementaciones inconclusas ni datos simulados en ruta de produccion."
