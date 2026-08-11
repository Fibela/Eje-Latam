/**
 * El enlace con el agente, sobre un conducto de mentira.
 *
 * RPT-046. Lo que se comprueba aquí no es el socket —eso es PA-78, y hace falta
 * el agente vivo— sino las tres formas en que una petición **no** obtiene
 * respuesta: silencio, cierre limpio y fallo del conducto. Las tres acaban hoy
 * en la interfaz, y si se colapsan en un solo error el operador no puede
 * distinguir «el agente no está» de «el agente está y no contesta».
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { enmarcar } from "../cable.js";
import { type Conducto, ErrorEnlace, pedir } from "../enlace.js";

/** Conducto controlado a mano: cada prueba decide qué llega y cuándo. */
function conductoFalso(): {
  conducto: Conducto;
  escrito: Uint8Array[];
  recibir: (datos: Uint8Array) => void;
  fallar: (error: Error) => void;
  colgar: () => void;
  cerrado: () => boolean;
} {
  const escrito: Uint8Array[] = [];
  let alRecibir: (trozo: Uint8Array) => void = () => {};
  let alFallar: (error: Error) => void = () => {};
  let alCerrar: () => void = () => {};
  let cerrado = false;

  return {
    escrito,
    recibir: (datos) => alRecibir(datos),
    fallar: (error) => alFallar(error),
    colgar: () => alCerrar(),
    cerrado: () => cerrado,
    conducto: {
      escribir: (datos) => void escrito.push(datos),
      alRecibir: (manejador) => void (alRecibir = manejador),
      alFallar: (manejador) => void (alFallar = manejador),
      alCerrar: (manejador) => void (alCerrar = manejador),
      cerrar: () => void (cerrado = true),
    },
  };
}

describe("RPT-046 — el enlace con el agente", () => {
  it("escribe la petición enmarcada y devuelve la respuesta", async () => {
    const falso = conductoFalso();
    const promesa = pedir(() => falso.conducto, "obtener-condiciones", Buffer.alloc(0));

    // Un solo marco de escritura: no se manda la petición en dos trozos.
    assert.equal(falso.escrito.length, 1);

    falso.recibir(enmarcar(Buffer.concat([Buffer.of(0), Buffer.from("{}")])));

    const respuesta = await promesa;
    assert.equal(respuesta.clase, "respuesta");
    assert.ok(falso.cerrado(), "la conexión se cierra al recibir la respuesta");
  });

  it("un rechazo llega como rechazo, con su motivo intacto", async () => {
    // No como excepción: el motivo es información que RPT-036 §6 puso ahí.
    const falso = conductoFalso();
    const promesa = pedir(() => falso.conducto, "obtener-inventario", Buffer.alloc(0));
    const motivo = "el inventario no verifica: revisión pendiente";

    falso.recibir(
      enmarcar(Buffer.concat([Buffer.of(1), Buffer.from(motivo, "utf8")])),
    );

    const respuesta = await promesa;
    assert.equal(respuesta.clase, "rechazo");
    assert.equal(
      respuesta.clase === "rechazo" ? respuesta.motivo : "",
      motivo,
      "el acento no debe perderse por el camino",
    );
  });

  it("una respuesta partida en trozos se reensambla", async () => {
    const falso = conductoFalso();
    const promesa = pedir(() => falso.conducto, "obtener-estado-agente", Buffer.alloc(0));
    const marco = enmarcar(Buffer.concat([Buffer.of(0), Buffer.from('{"a":1}')]));

    for (const byte of marco) {
      falso.recibir(Uint8Array.of(byte));
    }

    assert.equal((await promesa).clase, "respuesta");
  });

  it("el silencio vence y lo dice", async () => {
    // Un agente vivo y atascado deja la interfaz cargando para siempre. Eso es
    // peor que un error: el operador no sabe que el sensor no responde.
    const falso = conductoFalso();

    await assert.rejects(
      pedir(() => falso.conducto, "obtener-condiciones", Buffer.alloc(0), 10),
      (error: unknown) =>
        error instanceof ErrorEnlace && /no respondió/.test(error.message),
    );
    assert.ok(falso.cerrado(), "el vencimiento cierra el conducto");
  });

  it("colgar sin responder no se confunde con vencer", async () => {
    // El agente estaba y colgó. Distinto de que no conteste: uno es un servicio
    // que rechaza, el otro un servicio atascado, y se arreglan distinto.
    const falso = conductoFalso();
    const promesa = pedir(() => falso.conducto, "obtener-inventario", Buffer.alloc(0));

    falso.colgar();

    await assert.rejects(promesa, (error: unknown) =>
      error instanceof ErrorEnlace && /cerró la conexión/.test(error.message),
    );
  });

  it("colgar a media respuesta menciona los bytes que faltaban", async () => {
    // El diagnóstico útil: distingue «no mandó nada» de «se cortó a la mitad».
    const falso = conductoFalso();
    const promesa = pedir(() => falso.conducto, "obtener-inventario", Buffer.alloc(0));

    falso.recibir(enmarcar(Buffer.from("cuerpo largo")).subarray(0, 6));
    falso.colgar();

    await assert.rejects(promesa, (error: unknown) =>
      error instanceof ErrorEnlace && /marco incompleto/.test(error.message),
    );
  });

  it("un conducto que no se puede abrir falla al abrir, no al esperar", async () => {
    // Si el agente no está corriendo, el socket no existe. Ese error debe llegar
    // ya, no dentro de cinco segundos.
    await assert.rejects(
      pedir(
        () => {
          throw new Error("ENOENT");
        },
        "obtener-condiciones",
        Buffer.alloc(0),
      ),
      (error: unknown) =>
        error instanceof ErrorEnlace && /no se pudo abrir/.test(error.message),
    );
  });

  it("distingue las cuatro causas de fallo del conducto", async () => {
    // La cuarta —ECONNREFUSED— apareció al usarlo de verdad: sobre un socket
    // Unix significa que el fichero está y el agente no. Comprobar que el
    // fichero existe no lo detecta, y es el caso más frecuente en campo.
    const casos: readonly (readonly [string, RegExp])[] = [
      ["ECONNREFUSED", /no hay nadie escuchando/],
      ["ENOENT", /nunca llegó a abrirlo/],
      ["EACCES", /sin permiso/],
      ["EPIPE", /el conducto falló/],
    ];

    for (const [codigo, esperado] of casos) {
      const falso = conductoFalso();
      const promesa = pedir(() => falso.conducto, "obtener-inventario", Buffer.alloc(0));

      const error = new Error("fallo del sistema");
      Object.assign(error, { code: codigo });
      falso.fallar(error);

      await assert.rejects(promesa, (motivo: unknown) => {
        assert.ok(motivo instanceof ErrorEnlace);
        assert.match(motivo.message, esperado, `diagnóstico de ${codigo}`);
        return true;
      });
    }
  });

  it("un fallo del conducto no queda como promesa colgada", async () => {
    const falso = conductoFalso();
    const promesa = pedir(() => falso.conducto, "obtener-condiciones", Buffer.alloc(0));

    falso.fallar(new Error("ECONNRESET"));

    await assert.rejects(promesa, (error: unknown) =>
      error instanceof ErrorEnlace && /ECONNRESET/.test(error.message),
    );
  });
});
