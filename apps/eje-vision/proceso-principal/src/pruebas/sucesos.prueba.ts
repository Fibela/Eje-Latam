/**
 * RPT-048 §4 — cuándo se puede decir «no ha pasado nada».
 *
 * Casi nunca, y ésa es toda la prueba. Una lista vacía con histórico archivado
 * detrás no es una observación: es un hueco.
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { componerSucesos } from "@eje/vision-base";
import type { SucesoAlerta } from "@eje/vision-base";

function suceso(asiento: number): SucesoAlerta {
  return {
    asiento,
    clase: "amenazaIncontenible",
    dispositivo: "00:11:22:33:44:55",
    detalle: `alerta ${asiento}`,
  };
}

describe("RPT-048 §4 — el histórico archivado no es ausencia", () => {
  it("sin alertas y sin nada archivado, el silencio SÍ es una observación", () => {
    // El único caso en que se puede afirmar que no hubo alertas.
    const vista = componerSucesos({ primerDisponible: 1, hayMas: false, sucesos: [] });

    assert.ok(vista.puedeAfirmarQueNoHubo);
    assert.equal(vista.salvedad, "");
  });

  it("sin alertas pero con histórico archivado, NO se puede afirmar nada", () => {
    // El caso que da sentido a PA-74. Un panel que aquí escriba «sin
    // incidentes» está mintiendo sobre todo lo que se archivó antes.
    const vista = componerSucesos({ primerDisponible: 480, hayMas: false, sucesos: [] });

    assert.ok(!vista.puedeAfirmarQueNoHubo);
    assert.ok(vista.hayHistoricoArchivado);
    assert.match(vista.salvedad, /archivadas/);
    assert.doesNotMatch(vista.salvedad, /no ha pasado|sin incidentes/i);
  });

  it("la salvedad nombra el asiento desde el que se sabe", () => {
    // Un aviso que dice «puede faltar histórico» sin decir desde dónde no le
    // sirve a nadie para buscarlo.
    const vista = componerSucesos({ primerDisponible: 480, hayMas: false, sucesos: [suceso(500)] });

    assert.match(vista.salvedad, /480/);
  });

  it("con alertas y sin rotación no hay salvedad que hacer", () => {
    const vista = componerSucesos({ primerDisponible: 1, hayMas: false, sucesos: [suceso(3)] });

    assert.equal(vista.salvedad, "");
    assert.ok(!vista.hayHistoricoArchivado);
  });

  it("lo más reciente va primero", () => {
    // Por número de asiento y no por marca de tiempo: el asiento es monótono
    // (RPT-039 §3) y un reloj que se ajusta no lo es.
    const vista = componerSucesos({
      primerDisponible: 1,
      hayMas: false,
      sucesos: [suceso(2), suceso(7), suceso(5)],
    });

    assert.deepEqual(
      vista.sucesos.map((cada) => cada.asiento),
      [7, 5, 2],
    );
  });

  it("no se altera la respuesta que llegó", () => {
    // Ordenar en sitio mutaría lo que devolvió el puente, y quien lo consulte
    // después vería otra cosa.
    const original = [suceso(2), suceso(9)];
    componerSucesos({ primerDisponible: 1, hayMas: false, sucesos: original });

    assert.deepEqual(
      original.map((cada) => cada.asiento),
      [2, 9],
    );
  });

  it("una respuesta cortada NO se presenta como el histórico entero", () => {
    // PA-97. El agente devuelve lo que cabe en un marco, y eso pueden ser 256
    // de dos mil. Es el mismo error que `primerDisponible` evita, en la otra
    // dirección: mostrar una ventana como si fuera todo.
    const vista = componerSucesos({
      primerDisponible: 1,
      hayMas: true,
      sucesos: [suceso(1), suceso(2)],
    });

    assert.ok(vista.hayMasRecientes);
    assert.ok(!vista.puedeAfirmarQueNoHubo);
    assert.match(vista.salvedad, /más recientes/);
  });

  it("con huecos por delante y por detrás se dicen los dos", () => {
    const vista = componerSucesos({
      primerDisponible: 480,
      hayMas: true,
      sucesos: [suceso(500)],
    });

    assert.match(vista.salvedad, /480/);
    assert.match(vista.salvedad, /más recientes/);
  });

  it("primerDisponible en 1 con alertas no inventa histórico", () => {
    const vista = componerSucesos({ primerDisponible: 1, hayMas: false, sucesos: [suceso(1)] });

    assert.ok(!vista.hayHistoricoArchivado);
    assert.ok(!vista.puedeAfirmarQueNoHubo, "hay alertas: no aplica");
  });
});
