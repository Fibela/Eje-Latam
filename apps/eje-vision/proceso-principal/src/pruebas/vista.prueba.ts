/**
 * PA-102 y PA-106 — la vista es el sexto sitio donde vive el contrato.
 *
 * `diagnostico.js` es un fichero del renderer: no se compila con `tsc`, no lo
 * cruza `dependency-cruiser` y ninguna prueba lo ejecuta. Escribe a mano los
 * nombres de las condiciones, igual que `preload.cts` escribe a mano los
 * canales.
 *
 * `vis04` ya no está en ese grupo: desde RPT-091 (PA-142) vive en
 * `vista/src/vis04.ts`, lo compila `tsc` y su tabla de condiciones lleva un
 * `satisfies keyof Condiciones`. **La comprobación de las trece la hace ahora el
 * compilador**, y la prueba textual se queda como red de seguridad del orden y
 * de los identificadores del HTML, que el tipo no ve.
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
 *
 * # Lee la fuente, no el artefacto. RPT-091, PA-142
 *
 * Desde que `vis04` se compila, hay dos ficheros: `src/vis04.ts` —lo que
 * alguien escribió y lo que se revisa— y `dist/vis04.js` —lo que se emite—. Lo
 * que estas barreras vigilan es la **intención**, así que miran el `.ts`.
 *
 * Que los dos coincidan no lo comprueba una prueba: lo garantiza `tsc`, y el
 * `.js` ni siquiera está versionado.
 */
function fuenteDeVista(relativa: string): string {
  const aqui = dirname(fileURLToPath(import.meta.url));
  const ruta = join(aqui, "..", "..", "vista", ...relativa.split("/"));

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
  it("vis04.ts pinta exactamente las trece, sin faltar ni sobrar", () => {
    const tabla = bloque(fuenteDeVista("src/vis04.ts"), "const CONDICIONES = [", "] as const satisfies");
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

  it("todo identificador que vis04.ts busca existe en su HTML", () => {
    // PA-102. `getElementById` devuelve `null` y el fallo aparece más tarde, en
    // otra línea y como otra cosa. Un identificador renombrado en el HTML deja el
    // tablero en blanco sin decir por qué.
    const guion = fuenteDeVista("src/vis04.ts");
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

/**
 * PA-97 — el sondeo de alertas consume lo que el agente le contesta.
 *
 * # Por que este bloque existe, que es lo interesante
 *
 * PA-97 decia «`componerSucesos` no lee `hayMas` todavia» y llevaba semanas
 * siendo falso: RPT-050 cableo el bucle y la fila del tablero no se actualizo.
 * Un punto abierto que ya esta hecho no es inofensivo — dirige el trabajo hacia
 * donde no hace falta, y este bloque nace de haber estado a punto de reescribir
 * algo que funcionaba.
 *
 * La causa de que se quedara caduco es la de siempre: **nada lo comprobaba**.
 * `vis04.js` no lo compila `tsc` ni lo cruza `dependency-cruiser`, asi que el
 * bucle podia desaparecer sin que ninguna suite se enterase.
 *
 * # Lo que se sujeta
 *
 * No que las funciones existan —eso ya lo prueban `bitacora.prueba.ts` y
 * `sucesos.prueba.ts`— sino que la vista **las llame y en el orden correcto**.
 * Es la clase de defecto dominante del proyecto: piezas correctas que nadie
 * invoca.
 */
describe("PA-97 — la vista pagina de verdad", () => {
  const FUENTE = fuenteDeVista("src/vis04.ts");

  it("pide desde la marca de la bitacora y no desde el principio", () => {
    assert.match(
      FUENTE,
      /consultarAlertas\(\s*bitacora\.desdeAsiento\s*\)/,
      "vis04.js tiene que pedir desde donde se quedo; con un literal volveria a " +
        "traer la primera pagina en cada vuelta y no avanzaria nunca",
    );
  });

  it("actualiza la bitacora ANTES de decidir cuando volver a preguntar", () => {
    const incorpora = FUENTE.indexOf("bitacora = incorporar(");
    const espera = FUENTE.indexOf("esperaSugerida(bitacora)");

    assert.notEqual(incorpora, -1, "no se encontro la reasignacion de la bitacora");
    assert.notEqual(espera, -1, "la cadencia ya no sale de 'esperaSugerida'");

    // El orden es el mecanismo entero. Al reves, 'esperaSugerida' leeria el
    // 'hayMas' de la vuelta ANTERIOR: con cola pendiente esperaria 2 s en lugar
    // de volver a preguntar, y la cola se vaciaria a un asiento cada dos
    // segundos. No falla ruidosamente: solo va lento, que es peor.
    assert.ok(
      incorpora < espera,
      "la bitacora se actualiza despues de programar el refresco: la cadencia " +
        "se decidiria con el 'hayMas' de la vuelta anterior",
    );
  });
});
