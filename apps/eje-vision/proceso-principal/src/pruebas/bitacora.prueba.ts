/**
 * PA-98 — la bitácora acumula sin repetir, y vigila su propia continuidad.
 *
 * La prueba que más importa es la del salto: el cursor ahorra un megabyte por
 * refresco y, mal hecho, convierte un hueco visible en uno silencioso.
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  BITACORA_INICIAL,
  SUCESOS_EN_MEMORIA,
  esperaSugerida,
  incorporar,
} from "@eje/vision-base";
import type { RespuestaAlertas, SucesoAlerta } from "@eje/vision-base";

function suceso(asiento: number): SucesoAlerta {
  return {
    asiento,
    clase: "amenazaIncontenible",
    dispositivo: "00:11:22:33:44:55",
    detalle: `alerta ${asiento}`,
  };
}

function respuesta(
  primerDisponible: number,
  sucesos: readonly SucesoAlerta[],
  hayMas = false,
): RespuestaAlertas {
  return { primerDisponible, hayMas, sucesos };
}

describe("PA-98 — la bitácora lleva el cursor", () => {
  it("la primera consulta pide desde el principio", () => {
    assert.equal(BITACORA_INICIAL.desdeAsiento, 0);
  });

  it("la marca avanza al asiento más alto visto", () => {
    // Es lo que evita traerse el megabyte otra vez en el refresco siguiente.
    const tras = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(1), suceso(7)]));

    assert.equal(tras.desdeAsiento, 7);
  });

  it("una respuesta vacía no hace retroceder la marca", () => {
    // Retroceder pediría de nuevo lo que ya se tiene: el megabyte otra vez, y
    // los 40 ms de serialización robados al sensor (RPT-050).
    const primera = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(9)]));
    const segunda = incorporar(primera, respuesta(1, []));

    assert.equal(segunda.desdeAsiento, 9);
    assert.equal(segunda.sucesos.length, 1);
  });

  it("acumula entre consultas en lugar de sustituir", () => {
    const primera = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(1), suceso(2)]));
    const segunda = incorporar(primera, respuesta(1, [suceso(3)]));

    assert.deepEqual(
      segunda.sucesos.map((cada) => cada.asiento),
      [3, 2, 1],
    );
  });

  it("un suceso repetido por el agente no se duplica", () => {
    // Si un cliente pidiera desde una marca anterior por error, o el agente
    // reenviara de más, la lista no debe mostrar la misma alerta dos veces.
    const primera = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(5)]));
    const segunda = incorporar(primera, respuesta(1, [suceso(5), suceso(6)]));

    assert.deepEqual(
      segunda.sucesos.map((cada) => cada.asiento),
      [6, 5],
    );
  });

  it("una rotación que se salta asientos NO pasa en silencio", () => {
    // El peligro que introduce el cursor. Se tenía hasta el 100, el agente rotó
    // y ahora lo más antiguo es el 5000: los asientos 101 a 4999 no los verá
    // nunca este panel. Sin esta detección, el hueco es invisible.
    const previa = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(100)]));
    const tras = incorporar(previa, respuesta(5_000, [suceso(5_000)]));

    assert.ok(tras.huboSalto, "se archivaron asientos que este panel no vio");
    assert.equal(tras.saltoDesde, 101, "el hueco empieza justo tras lo último visto");
  });

  it("el salto es pegajoso: no se cierra porque la siguiente consulta vaya bien", () => {
    // Sigue habiendo alertas que este panel nunca mostró. Volver a `false`
    // sería afirmar que ya se vio todo.
    const previa = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(100)]));
    const salto = incorporar(previa, respuesta(5_000, [suceso(5_000)]));
    const despues = incorporar(salto, respuesta(5_000, [suceso(5_001)]));

    assert.ok(despues.huboSalto);
    assert.equal(despues.saltoDesde, 101, "el hueco conserva dónde empezó");
  });

  it("un panel recién abierto sobre histórico archivado NO es un salto", () => {
    // `primerDisponible > 1` en la primera consulta es histórico que nunca fue
    // suyo. Llamarlo salto acusaría de pérdida a quien acaba de abrir la ventana.
    const tras = incorporar(BITACORA_INICIAL, respuesta(5_000, [suceso(5_000)]));

    assert.ok(!tras.huboSalto);
    assert.ok(tras.hayHistoricoArchivado, "sí hay histórico, pero no es un salto");
  });

  it("la continuidad exacta no dispara el salto", () => {
    // Se tenía hasta el 100 y lo más antiguo pasa a ser el 101: no falta nada.
    // Un «mayor o igual» aquí daría un falso positivo en cada rotación limpia.
    const previa = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(100)]));
    const tras = incorporar(previa, respuesta(101, [suceso(101)]));

    assert.ok(!tras.huboSalto);
  });

  it("la memoria está acotada", () => {
    // Un panel abierto una semana no puede crecer sin límite: es el
    // agotamiento de RPT-018 §6 con otro nombre.
    const muchos = Array.from({ length: SUCESOS_EN_MEMORIA + 200 }, (_, indice) =>
      suceso(indice + 1),
    );
    const tras = incorporar(BITACORA_INICIAL, respuesta(1, muchos));

    assert.equal(tras.sucesos.length, SUCESOS_EN_MEMORIA);
    assert.equal(tras.sucesos[0]?.asiento, muchos.length, "se conserva lo reciente");
    assert.equal(tras.desdeAsiento, muchos.length, "la marca no se pierde al recortar");
  });

  it("con cola pendiente se vuelve a preguntar ya; si no, se espera", () => {
    // RPT-050: por debajo de 500 ms el agente no tiene nada nuevo que decir.
    // Pero si él mismo avisa de que cortó, esperar sólo alarga la cola.
    const conCola = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(1)], true));
    const alDia = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(1)], false));

    assert.equal(esperaSugerida(conCola), 0);
    assert.ok(esperaSugerida(alDia) >= 500, "no tiene sentido pedir más rápido que el ciclo");
  });

  it("incorporar no muta la bitácora anterior", () => {
    const previa = incorporar(BITACORA_INICIAL, respuesta(1, [suceso(1)]));
    incorporar(previa, respuesta(1, [suceso(2)]));

    assert.equal(previa.sucesos.length, 1);
    assert.equal(previa.desdeAsiento, 1);
  });
});
