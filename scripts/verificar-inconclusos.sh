#!/usr/bin/env bash
#
# RETIRADO — sustituido por `cargo xtask verificar` (RPT-003 §9.5, PA-11).
#
# Este script se conserva unicamente como redirector, para que no falle en
# silencio quien tenga la invocacion antigua en un gancho de git o en su
# memoria muscular. Puede eliminarse con `git rm scripts/verificar-inconclusos.sh`.
#
# ¿Por que se retiro?
#
# Mantener el guardian como script obligaba a dos implementaciones divergentes
# —bash para CI y PowerShell para desarrollo en Windows— y la version PowerShell
# resulto tener un falso negativo silencioso: `break` al primer `#[cfg(test)]`
# abandonaba el fichero completo, de modo que una violacion posterior al modulo
# de pruebas quedaba sin revisar.
#
# Sobre todo: un script suelto no se puede probar. El guardian vive ahora en el
# crate `xtask`, con trece pruebas que incluyen ese mismo caso.

set -euo pipefail

echo "scripts/verificar-inconclusos.sh esta retirado. Use: cargo xtask verificar" >&2
echo "Redirigiendo..." >&2
echo >&2

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RAIZ"
exec cargo xtask verificar "${1:-crates}"
