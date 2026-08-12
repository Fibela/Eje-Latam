/**
 * PA-93 — lo que lee un operador cuando no hay sensor.
 *
 * La prueba que de verdad importa es la última: ata los códigos que `enlace.ts`
 * emite con los que la vista traduce. Sin ella, cambiar uno de los dos lados
 * degrada todos los mensajes al genérico **sin que nada falle**.
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { leerSinAgente } from "@eje/vision-base";

import { CAUSAS } from "../enlace.js";

describe("PA-93 — el fallo técnico se traduce sin perderse", () => {
  it("un sensor caído se lee como desconexión y se reintenta", () => {
    const lectura = leerSinAgente(
      "[sin-escucha] el socket existe y no hay nadie escuchando",
    );

    assert.equal(lectura.titulo, "Sensor desconectado");
    assert.ok(lectura.reintentable);
  });

  it("el detalle técnico se conserva entero", () => {
    // Es lo que hace admisible resumir en el título: el forense no se pierde,
    // se mueve a segundo plano.
    const original = "[sin-permiso] sin permiso sobre el socket: ... (PA-82)";
    assert.equal(leerSinAgente(original).detalleTecnico, original);
  });

  it("sin permiso NO se reintenta", () => {
    // Reintentar cada dos segundos no concede permisos, y «reintentando…»
    // escondería que hace falta que intervenga un administrador.
    const lectura = leerSinAgente("[sin-permiso] el agente lo creó para otro usuario");

    assert.ok(!lectura.reintentable);
    assert.match(lectura.sugerencia, /administrador/);
  });

  it("un sensor que no responde no se anuncia como desconectado", () => {
    // El caso más engañoso: el servicio está vivo. Decir «desconectado» mandaría
    // a alguien a arrancar algo que ya corre.
    const lectura = leerSinAgente("[sin-respuesta] el agente no respondió en 5000 ms");

    assert.notEqual(lectura.titulo, "Sensor desconectado");
    assert.match(lectura.sugerencia, /no es una desconexi/);
  });

  it("una causa desconocida se admite como desconocida, no se disfraza", () => {
    // Una traducción que adivina es peor que ninguna: el operador actúa sobre
    // ella.
    const lectura = leerSinAgente("mensaje sin código de causa");

    assert.equal(lectura.titulo, "No se pudo consultar al sensor");
    assert.equal(lectura.detalleTecnico, "mensaje sin código de causa");
  });

  it("toda causa que enlace.ts emite tiene traducción propia", () => {
    // La barrera. `transporte` es la única que cae a propósito en el genérico:
    // es el cajón de lo que no se sabe clasificar.
    const generico = leerSinAgente("sin codigo").titulo;

    for (const causa of Object.values(CAUSAS)) {
      const lectura = leerSinAgente(`[${causa}] detalle cualquiera`);

      if (causa === CAUSAS.transporte) {
        continue;
      }

      assert.notEqual(
        lectura.titulo,
        generico,
        `la causa '${causa}' no tiene traducción y cae en el mensaje genérico`,
      );
    }
  });
});
