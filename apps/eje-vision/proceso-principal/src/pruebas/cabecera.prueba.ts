/**
 * RPT-048 §2 — la cabecera del tablero.
 *
 * Lo que se comprueba aquí es sobre todo **el orden**: qué gana cuando dos cosas
 * son verdad a la vez. Un tablero que elija mal manda al operador a arreglar lo
 * segundo mientras lo primero sigue pasando.
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { componerCabecera } from "@eje/vision-base";
import type { Condiciones } from "@eje/vision-base";

/** Todo en calma. Base para encender una sola condición por prueba. */
const CALMA: Condiciones = {
  inventarioSuprimido: false,
  inventarioNoVerifica: false,
  observacionSaturada: false,
  capturaConPerdida: false,
  capturaNoDisponible: false,
  accionAdministrativa: false,
  salidaNoDisponible: false,
  registroSaturado: false,
  evidenciaEnRiesgo: false,
};

function cabeceraDe(condiciones: Condiciones) {
  return componerCabecera({ clase: "datos", valor: condiciones });
}

describe("RPT-048 §2 — la cabecera decide cómo se lee todo lo demás", () => {
  it("en calma no hay cabecera y los datos son del ahora", () => {
    const cabecera = cabeceraDe(CALMA);

    assert.equal(cabecera.titulo, "");
    assert.equal(cabecera.urgencia, "normal");
    assert.equal(cabecera.datosDeAntes, false);
  });

  it("sin captura, lo de debajo es de antes", () => {
    // La consecuencia que da sentido a toda la cabecera. Sin esta marca, un
    // tablero con el sensor ciego se lee igual que uno con la red tranquila.
    const cabecera = cabeceraDe({ ...CALMA, capturaNoDisponible: true });

    assert.equal(cabecera.urgencia, "critica");
    assert.ok(cabecera.datosDeAntes, "el resto del tablero no es vigente");
    assert.match(cabecera.titulo, /no está vigilando/);
  });

  it("sin captura gana sobre cualquier otra condición", () => {
    // Si se anunciara la manipulación mientras el sensor está ciego, el operador
    // leeria el resto del tablero como actual. Lo primero es si se puede creer
    // lo que hay en pantalla.
    const cabecera = cabeceraDe({
      ...CALMA,
      capturaNoDisponible: true,
      inventarioSuprimido: true,
      registroSaturado: true,
    });

    assert.match(cabecera.titulo, /no está vigilando/);
    assert.ok(cabecera.datosDeAntes);
  });

  it("la manipulación se anuncia como incidente, no como aviso", () => {
    const cabecera = cabeceraDe({ ...CALMA, inventarioSuprimido: true });

    assert.equal(cabecera.urgencia, "critica");
    assert.match(cabecera.detalle, /incidente/);
    assert.equal(cabecera.datosDeAntes, false, "el sensor sigue observando");
  });

  it("el registro lleno es crítico y NO se presenta como manipulación", () => {
    // Tan grave como que alguien tocara el almacén, sin serlo. Mezclarlos manda
    // a alguien a buscar un atacante que no existe.
    const cabecera = cabeceraDe({ ...CALMA, registroSaturado: true });

    assert.equal(cabecera.urgencia, "critica");
    assert.doesNotMatch(cabecera.detalle, /alter|incidente/);
  });

  it("la salida caída sube a la cabecera aunque no sea la más grave", () => {
    // Es la única condición que no viaja por syslog: si nadie mira esta
    // pantalla, nadie se entera. Por eso no puede quedarse en la lista.
    const cabecera = cabeceraDe({ ...CALMA, salidaNoDisponible: true });

    assert.equal(cabecera.urgencia, "atencion");
    assert.match(cabecera.detalle, /lo único que lo sabe/);
  });

  it("una condición menor no ocupa la cabecera", () => {
    // `accionAdministrativa` es real y no cambia cómo se lee el tablero.
    // Subirla aquí es como se enseña a un operador a ignorar la cabecera.
    const cabecera = cabeceraDe({ ...CALMA, accionAdministrativa: true });

    assert.equal(cabecera.titulo, "");
    assert.equal(cabecera.urgencia, "normal");
  });

  it("sin agente la cabecera habla en lenguaje de operador", () => {
    const cabecera = componerCabecera({
      clase: "sinAgente",
      detalle: "[sin-escucha] el socket existe y no hay nadie escuchando",
    });

    assert.equal(cabecera.titulo, "Sensor desconectado");
    assert.ok(cabecera.datosDeAntes);
  });

  it("una respuesta inesperada se declara en lugar de elegir una rama", () => {
    // El agente siempre devuelve las nueve condiciones. Si llega otra cosa, el
    // contrato cambió, y decirlo es mejor que suponer.
    const cabecera = componerCabecera({ clase: "vacio" });

    assert.equal(cabecera.urgencia, "critica");
    assert.ok(cabecera.datosDeAntes);
  });

  it("mientras se consulta, nada de lo que hay abajo es vigente", () => {
    assert.ok(componerCabecera({ clase: "consultando" }).datosDeAntes);
  });
});
