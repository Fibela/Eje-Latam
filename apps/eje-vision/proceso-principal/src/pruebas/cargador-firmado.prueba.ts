/**
 * Pruebas del cargador firmado.
 *
 * Se firma con claves Ed25519 generadas en la propia prueba: la verificación es
 * real, no simulada. RPT-003 §9.2 prohíbe mocks en rutas críticas, y la carga de
 * módulos en el proceso principal lo es.
 */

import assert from "node:assert/strict";
import { generateKeyPairSync, sign } from "node:crypto";
import { describe, it } from "node:test";

import {
  directorioAdmisible,
  resumirContenido,
  serializarManifiesto,
  verificarPaquete,
  type ManifiestoPaquete,
} from "../cargador-firmado.js";

const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const { privateKey: privadaAjena } = generateKeyPairSync("ed25519");

const CONTENIDO = Buffer.from("contenido del paquete empresarial", "utf8");

function manifiestoValido(): ManifiestoPaquete {
  return {
    nombre: "@eje/vision-empresarial",
    version: "0.1.0",
    resumenSha256: resumirContenido(CONTENIDO),
  };
}

function firmar(manifiesto: ManifiestoPaquete, clave = privateKey): Buffer {
  return sign(null, serializarManifiesto(manifiesto), clave);
}

describe("verificarPaquete", () => {
  it("admite un paquete integro firmado por PremosCorp", () => {
    const manifiesto = manifiestoValido();
    const resultado = verificarPaquete(
      manifiesto,
      firmar(manifiesto),
      publicKey.export({ type: "spki", format: "pem" }).toString(),
      CONTENIDO,
      true,
    );
    assert.deepEqual(resultado, { admitido: true });
  });

  it("rechaza el paquete si falta la firma", () => {
    const manifiesto = manifiestoValido();
    const resultado = verificarPaquete(
      manifiesto,
      null,
      publicKey.export({ type: "spki", format: "pem" }).toString(),
      CONTENIDO,
      true,
    );
    assert.deepEqual(resultado, { admitido: false, motivo: "firma-ausente" });
  });

  it("rechaza una firma emitida con otra clave", () => {
    const manifiesto = manifiestoValido();
    const resultado = verificarPaquete(
      manifiesto,
      firmar(manifiesto, privadaAjena),
      publicKey.export({ type: "spki", format: "pem" }).toString(),
      CONTENIDO,
      true,
    );
    assert.deepEqual(resultado, { admitido: false, motivo: "firma-invalida" });
  });

  it("rechaza el paquete si el manifiesto fue alterado tras la firma", () => {
    const manifiesto = manifiestoValido();
    const firma = firmar(manifiesto);
    const alterado: ManifiestoPaquete = { ...manifiesto, version: "9.9.9" };

    const resultado = verificarPaquete(
      alterado,
      firma,
      publicKey.export({ type: "spki", format: "pem" }).toString(),
      CONTENIDO,
      true,
    );
    assert.deepEqual(resultado, { admitido: false, motivo: "firma-invalida" });
  });

  it("rechaza el paquete si el contenido no coincide con el resumen firmado", () => {
    const manifiesto = manifiestoValido();
    const resultado = verificarPaquete(
      manifiesto,
      firmar(manifiesto),
      publicKey.export({ type: "spki", format: "pem" }).toString(),
      Buffer.from("contenido sustituido por un atacante", "utf8"),
      true,
    );
    assert.deepEqual(resultado, {
      admitido: false,
      motivo: "resumen-no-coincide",
    });
  });

  it("rechaza un manifiesto malformado", () => {
    const manifiesto: ManifiestoPaquete = {
      nombre: "",
      version: "0.1.0",
      resumenSha256: "corto",
    };
    const resultado = verificarPaquete(
      manifiesto,
      firmar(manifiesto),
      publicKey.export({ type: "spki", format: "pem" }).toString(),
      CONTENIDO,
      true,
    );
    assert.deepEqual(resultado, {
      admitido: false,
      motivo: "manifiesto-malformado",
    });
  });

  it("no carga en un nodo que nunca tuvo licencia", () => {
    const manifiesto = manifiestoValido();
    const resultado = verificarPaquete(
      manifiesto,
      firmar(manifiesto),
      publicKey.export({ type: "spki", format: "pem" }).toString(),
      CONTENIDO,
      false,
    );
    assert.deepEqual(resultado, {
      admitido: false,
      motivo: "sin-licencia-previa",
    });
  });

  it("trata una clave publica malformada como fallo cerrado", () => {
    const manifiesto = manifiestoValido();
    const resultado = verificarPaquete(
      manifiesto,
      firmar(manifiesto),
      "esto no es una clave",
      CONTENIDO,
      true,
    );
    assert.deepEqual(resultado, { admitido: false, motivo: "firma-invalida" });
  });
});

describe("directorioAdmisible", () => {
  const instalacion = ["C:/Program Files/Eje-Latam", "/opt/eje-latam"];

  it("admite rutas bajo los prefijos de instalacion", () => {
    assert.ok(
      directorioAdmisible("C:/Program Files/Eje-Latam/modulos", instalacion),
    );
    assert.ok(directorioAdmisible("/opt/eje-latam/modulos", instalacion));
  });

  it("rechaza el perfil del usuario", () => {
    assert.ok(
      !directorioAdmisible("C:/Users/alexx/AppData/Local/modulos", instalacion),
    );
    assert.ok(!directorioAdmisible("/home/alexx/.eje/modulos", instalacion));
  });

  it("normaliza separadores de Windows", () => {
    assert.ok(
      directorioAdmisible(
        "C:\\Program Files\\Eje-Latam\\modulos",
        instalacion,
      ),
    );
  });
});
