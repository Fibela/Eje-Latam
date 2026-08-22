/**
 * PA-102 y PA-106 — la vista es el sexto sitio donde vive el contrato.
 *
 * `vis04.js` y `diagnostico.js` son ficheros del renderer: no se compilan con
 * `tsc`, no los cruza `dependency-cruiser` y ninguna prueba los ejecuta. Escriben
 * a mano los nombres de las condiciones, igual que `preload.cts` escribe a mano
 * los canales, y por la misma razón: no pueden importar del paquete base con la
 * ventana en modo estricto.
 *
 * Lo que no se compara, diverge. `sinColector` acaba de entrar en el contrato; si
 * alguien la añade al agente y no aquí, el tablero mostrará nueve filas y quien
 * lo mire concluirá que se comprobaron diez.
 *
 * # Por qué esto es paridad y no igualdad
 *
 * PA-106 se enunció como «que el IPC y syslog lleven la misma información». **No
 * la llevan, y no deben**: `salidaNoDisponible` y `sinColector` no pueden viajar
 * por syslog porque emitirlas exigiría el canal que falta (RPT-055 §4).
 *
 * La paridad que se puede exigir es otra: que **la vista nombre las trece** y que
 * el lado de syslog declare por escrito cuáles excluye. Lo segundo lo sujeta
 * `pruebas_emisibles` en Rust; lo primero es este fichero.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import { CAMPOS_CONDICIONES } from "@eje/vision-base";

import { sinComentarios } from "./lexico.js";

/** Nombres de las condiciones según el contrato. La autoridad. */
const DEL_CONTRATO: readonly string[] = CAMPOS_CONDICIONES.map(([nombre]) => nombre);

/**
 * Fuente de un fichero de la vista.
 *
 * Anclado en `import.meta.url` y no en `process.cwd()`: el directorio de trabajo
 * depende de desde dónde se invoque npm, y esa fragilidad ya costó una suite
 * entera sin ejecutar.
 */
function fuenteDeVista(nombre: string): string {
  const aqui = dirname(fileURLToPath(import.meta.url));
  const ruta = join(aqui, "..", "..", "vista", nombre);

  try {
    return sinComentarios(readFileSync(ruta, "utf8"));
  } catch (error) {
    throw new Error(`no se pudo leer ${ruta}: ${String(error)}`);
  }
}

/** Trozo entre dos marcas, para no barrer el fichero entero. */
function bloque(fuente: string, desde: string, hasta: string): string {
  const inicio = fuente.indexOf(desde);
  assert.notEqual(inicio, -1, `no se encontró '${desde}': ¿se renombró la tabla?`);

  const fin = fuente.indexOf(hasta, inicio + desde.length);
  assert.notEqual(fin, -1, `no se encontró el cierre de '${desde}'`);

  return fuente.slice(inicio + desde.length, fin);
}

describe("PA-102 / PA-106 — la vista nombra las condiciones del contrato", () => {
  it("vis04.js pinta exactamente las trece, sin faltar ni sobrar", () => {
    const tabla = bloque(fuenteDeVista("vis04.js"), "const CONDICIONES = [", "];");
    const nombradas = [...tabla.matchAll(/"([A-Za-z]+)"\s*,/g)].map((c) => c[1] ?? "");

    // Faltar es lo grave: una condición activa que el tablero no tiene fila donde
    // mostrar desaparece sin que nada avise.
    for (const condicion of DEL_CONTRATO) {
      assert.ok(
        nombradas.includes(condicion),
        `'${condicion}' está en el contrato y VIS-04 no la pinta`,
      );
    }

    // Sobrar también importa: un nombre que el agente ya no manda se pintaría
    // como AUSENTE EN LA RESPUESTA para siempre, y esa alarma permanente es cómo
    // se enseña a ignorar la única señal que distingue ausente de falso.
    for (const nombrada of nombradas) {
      assert.ok(
        DEL_CONTRATO.includes(nombrada),
        `VIS-04 pinta '${nombrada}', que no está en el contrato`,
      );
    }

    assert.equal(nombradas.length, DEL_CONTRATO.length);
  });

  it("el panel de diagnóstico traduce las trece", () => {
    const tabla = bloque(fuenteDeVista("diagnostico.js"), "const NOMBRES = {", "};");
    const nombradas = [...tabla.matchAll(/([A-Za-z]+)\s*:/g)].map((c) => c[1] ?? "");

    assert.deepEqual([...nombradas].sort(), [...DEL_CONTRATO].sort());
  });

  it("todo identificador que vis04.js busca existe en su HTML", () => {
    // PA-102. `getElementById` devuelve `null` y el fallo aparece más tarde, en
    // otra línea y como otra cosa. Un identificador renombrado en el HTML deja el
    // tablero en blanco sin decir por qué.
    const guion = fuenteDeVista("vis04.js");
    const html = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), "..", "..", "vista", "vis04.html"),
      "utf8",
    );

    const buscados = [...guion.matchAll(/elemento\(\s*"([^"]+)"\s*\)/g)].map(
      (c) => c[1] ?? "",
    );

    assert.ok(buscados.length > 0, "el lector de identificadores no encontró ninguno");

    for (const identificador of new Set(buscados)) {
      assert.ok(
        html.includes(`id="${identificador}"`),
        `vis04.js busca '${identificador}' y vis04.html no lo tiene`,
      );
    }
  });

  it("el texto inicial del sello declara el estado roto, no uno neutro", () => {
    // PA-101. Si el módulo no arranca —ruta mal, CSP, error de sintaxis— la
    // ventana pinta el HTML y nada más. Un texto neutro ahí sería indistinguible
    // de un tablero sin datos, que es RPT-006 §4 aplicado a la herramienta.
    const html = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), "..", "..", "vista", "vis04.html"),
      "utf8",
    );

    assert.match(html, /id="sello"[^>]*>\s*EL MÓDULO DE LA VISTA NO ARRANCÓ/);
  });
});
