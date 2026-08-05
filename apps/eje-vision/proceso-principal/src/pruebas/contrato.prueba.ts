/**
 * Paridad del contrato IPC con `contrato-ipc.toml`.
 *
 * RPT-006, PA-20.
 *
 * # Por qué esta prueba es obligatoria
 *
 * Rust y TypeScript no pueden compartir tipos. `crates/eje-ipc` valida su
 * `enum Canal` contra el manifiesto; **sin esta prueba, solo la mitad del
 * mecanismo existiría** y el lado TypeScript podría volver a divergir en
 * silencio, que es exactamente cómo llegamos aquí.
 *
 * Es el mismo patrón que `probar-frontera.mjs`: una comprobación ejecutable en
 * lugar de una convención.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  CANALES_PERMITIDOS,
  CANALES_PROHIBIDOS,
  CARGA_MAXIMA_BYTES,
} from "../puente-ipc.js";

/** Raíz del repositorio, cuatro niveles por encima de este fichero compilado. */
function rutaManifiesto(): string {
  const aqui = dirname(fileURLToPath(import.meta.url));
  return join(aqui, "..", "..", "..", "..", "..", "contrato-ipc.toml");
}

function manifiesto(): string {
  const ruta = rutaManifiesto();
  try {
    return readFileSync(ruta, "utf8");
  } catch (error) {
    throw new Error(
      `no se pudo leer el manifiesto ${ruta}: ${String(error)}\n` +
        "contrato-ipc.toml es la fuente de verdad del puente y debe estar versionado.",
    );
  }
}

/**
 * Extrae los valores de `nombre = "..."` que siguen a una cabecera de tabla.
 *
 * Analizador deliberadamente simple: el formato lo controla este proyecto y
 * añadir una dependencia TOML solo para esta prueba no se justifica. Es el mismo
 * criterio que en `xtask/src/vectores.rs`.
 */
function nombresBajo(contenido: string, cabecera: string): string[] {
  const nombres: string[] = [];
  let dentro = false;

  for (const linea of contenido.split("\n")) {
    const limpia = linea.trim();

    if (limpia.startsWith("[")) {
      dentro = limpia === cabecera;
      continue;
    }
    if (limpia.startsWith("#") || !dentro) {
      continue;
    }

    const coincidencia = /^nombre\s*=\s*"([^"]+)"/u.exec(limpia);
    if (coincidencia?.[1] !== undefined) {
      nombres.push(coincidencia[1]);
    }
  }

  return nombres;
}

describe("PA-20 — paridad con contrato-ipc.toml", () => {
  it("los canales permitidos coinciden con el manifiesto", () => {
    const declarados = nombresBajo(manifiesto(), "[[canal]]");
    const implementados = [...CANALES_PERMITIDOS];

    assert.deepEqual(
      implementados,
      declarados,
      `CANALES_PERMITIDOS y contrato-ipc.toml divergen.\n` +
        `  manifiesto: ${JSON.stringify(declarados)}\n` +
        `  codigo    : ${JSON.stringify(implementados)}\n` +
        `  Anadir un canal exige tocar el manifiesto, crates/eje-ipc y este puente. ` +
        `Esa friccion es deliberada: un canal amplia la superficie de ataque del ` +
        `proceso privilegiado.`,
    );
  });

  it("el orden de los canales coincide con el manifiesto", () => {
    // El orden importa: si un lado reordena, un diff futuro parecerá inocuo
    // cuando en realidad cambió la correspondencia con el enum de Rust.
    const declarados = nombresBajo(manifiesto(), "[[canal]]");
    assert.equal(CANALES_PERMITIDOS.length, declarados.length);
    declarados.forEach((nombre, indice) => {
      assert.equal(CANALES_PERMITIDOS[indice], nombre);
    });
  });

  it("todos los canales prohibidos del manifiesto están recogidos", () => {
    const prohibidos = nombresBajo(manifiesto(), "[[prohibido]]");

    assert.ok(
      prohibidos.length > 0,
      "el manifiesto debe declarar canales prohibidos como prueba de regresion",
    );

    for (const nombre of prohibidos) {
      assert.ok(
        CANALES_PROHIBIDOS.includes(nombre),
        `'${nombre}' esta declarado como prohibido en el manifiesto pero no en CANALES_PROHIBIDOS`,
      );
      assert.ok(
        !(CANALES_PERMITIDOS as readonly string[]).includes(nombre),
        `'${nombre}' esta prohibido y aparece entre los permitidos`,
      );
    }
  });

  it("el límite de carga coincide con el manifiesto", () => {
    // Un límite distinto en cada extremo permite que un lado acepte lo que el
    // otro rechaza, y esa asimetría es explotable.
    const contenido = manifiesto();
    assert.ok(
      contenido.includes(`longitud_maxima = ${CARGA_MAXIMA_BYTES}`),
      `el manifiesto debe declarar 'longitud_maxima = ${CARGA_MAXIMA_BYTES}'`,
    );
  });

  it("el manifiesto documenta el motivo de cada prohibición", () => {
    // Una lista de prohibidos sin motivos se erosiona: alguien la revisa dentro
    // de un ano, no encuentra la razon y la borra.
    const contenido = manifiesto();
    const prohibidos = nombresBajo(contenido, "[[prohibido]]");
    const motivos = (contenido.match(/^motivo\s*=/gmu) ?? []).length;

    assert.equal(
      motivos,
      prohibidos.length,
      `hay ${prohibidos.length} canales prohibidos y ${motivos} motivos declarados`,
    );
  });
});
