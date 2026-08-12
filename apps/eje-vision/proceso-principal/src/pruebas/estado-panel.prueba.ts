/**
 * La máquina de estados de un panel. RPT-048 §1.
 *
 * Se prueba sin Electron, sin agente y sin escritorio: esa es la razón de que
 * viva en la capa base.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  PREFIJO_RECHAZO,
  conDatos,
  desdeFallo,
  esObservacion,
  listaVacia,
} from "@eje/vision-base";

describe("RPT-048 §1 — un panel tiene tres estados, no dos", () => {
  it("una lista con elementos son datos", () => {
    const estado = conDatos([1, 2], listaVacia);
    assert.equal(estado.clase, "datos");
  });

  it("una lista sin elementos está vacía, que es una observación", () => {
    // «No hay dispositivos» es algo que se sabe. Distinto de no saberlo.
    const estado = conDatos([], listaVacia);
    assert.equal(estado.clase, "vacio");
    assert.ok(esObservacion(estado));
  });

  it("un rechazo del agente NO se confunde con estar vacío", () => {
    // El fallo que este módulo existe para impedir: pintar «no hay nada» donde
    // lo cierto es «esto no lo sirve nadie todavía» (RPT-036 §6).
    const estado = desdeFallo(
      new Error(
        `${PREFIJO_RECHAZO} «obtener-inventario»: el canal 'obtener-inventario' ` +
          "esta declarado y aun no tiene manejador en el agente",
      ),
    );

    assert.equal(estado.clase, "noServido");
    assert.ok(!esObservacion(estado), "un rechazo no afirma nada sobre la red");
  });

  it("el motivo del agente llega entero, sin recortar", () => {
    // Recortarlo «para que quede bonito» pierde el dato que distingue tres
    // llamadas de soporte distintas.
    const motivo = "el canal no tiene manejador: revisión pendiente";
    const estado = desdeFallo(new Error(`${PREFIJO_RECHAZO} «x»: ${motivo}`));

    assert.ok(
      estado.clase === "noServido" && estado.motivo.includes(motivo),
      "el motivo se perdió por el camino",
    );
  });

  it("no poder hablar con el agente no es un rechazo", () => {
    // Uno espera a que exista el módulo; el otro, a que arranque el sensor.
    const estado = desdeFallo(new Error("el socket existe y no hay nadie escuchando"));

    assert.equal(estado.clase, "sinAgente");
    assert.ok(!esObservacion(estado));
  });

  it("un fallo que no es un Error tampoco descarrila", () => {
    assert.equal(desdeFallo("algo raro").clase, "sinAgente");
    assert.equal(desdeFallo(undefined).clase, "sinAgente");
  });

  it("sólo los dos estados con respuesta afirman algo sobre el mundo", () => {
    // La cuenta importa: si mañana se añade un cuarto estado y se olvida aquí,
    // la interfaz escribirá «0 dispositivos» donde lo cierto es «no se sabe».
    assert.ok(!esObservacion({ clase: "consultando" }));
    assert.ok(!esObservacion({ clase: "sinAgente", detalle: "x" }));
    assert.ok(!esObservacion({ clase: "noServido", motivo: "x" }));
    assert.ok(esObservacion({ clase: "vacio" }));
    assert.ok(esObservacion({ clase: "datos", valor: 1 }));
  });
});

describe("PA-94 — el prefijo del rechazo vive en dos sitios", () => {
  it("el proceso principal compone el rechazo con el prefijo que la vista espera", () => {
    // Deuda declarada: el primer salto convierte el rechazo en una excepción y
    // pierde su forma. Mientras eso no se arregle, lo único que impide que los
    // dos lados divergan en silencio es esta comparación.
    const aqui = dirname(fileURLToPath(import.meta.url));
    const fuente = readFileSync(join(aqui, "..", "..", "src", "arranque.ts"), "utf8");

    assert.ok(
      fuente.includes(PREFIJO_RECHAZO),
      `'arranque.ts' ya no compone el rechazo con «${PREFIJO_RECHAZO}»: ` +
        "todo rechazo se leería como que el agente no está",
    );
  });
});
