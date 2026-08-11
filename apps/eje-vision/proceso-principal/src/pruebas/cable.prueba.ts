/**
 * Paridad de bytes del formato de cable con `vectores-ipc.json`.
 *
 * RPT-045 §3. Los vectores los genera **Rust**, que es el codificador que manda.
 * Aquí sólo se comprueba que esta implementación produce exactamente esos bytes.
 *
 * Si esta suite falla y el cambio en Rust era deliberado, el arreglo es
 * `cargo xtask vectores-ipc` y revisar el diff — no tocar los números a mano.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  Acumulador,
  ErrorCable,
  LIMITES,
  componerPeticion,
  enmarcar,
  interpretarRespuesta,
} from "../cable.js";

interface CasoPeticion {
  readonly nombre: string;
  readonly canal: string;
  readonly cargaHex: string;
  readonly cuerpoHex: string;
  readonly marcoHex: string;
}

interface CasoRespuesta {
  readonly nombre: string;
  readonly clase: "respuesta" | "rechazo";
  readonly cuerpoHex: string;
  readonly cargaHex: string;
  readonly marcoHex: string;
}

interface Vectores {
  readonly limites: Record<string, number>;
  readonly peticiones: readonly CasoPeticion[];
  readonly respuestas: readonly CasoRespuesta[];
}

/**
 * Vectores anclados, en la raíz del repositorio.
 *
 * Cinco niveles desde `dist/pruebas`, igual que el manifiesto. Anclado en
 * `import.meta.url` y no en `process.cwd()`: el directorio de trabajo depende de
 * desde dónde se invoque npm.
 */
function vectores(): Vectores {
  const aqui = dirname(fileURLToPath(import.meta.url));
  const ruta = join(aqui, "..", "..", "..", "..", "..", "vectores-ipc.json");

  try {
    return JSON.parse(readFileSync(ruta, "utf8")) as Vectores;
  } catch (error) {
    throw new Error(
      `no se pudo leer ${ruta}: ${String(error)}\n` +
        "Genera los vectores con 'cargo xtask vectores-ipc'.",
    );
  }
}

/** Hexadecimal en minúsculas, como lo escribe Rust. */
function hex(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString("hex");
}

function desdeHex(texto: string): Buffer {
  return Buffer.from(texto, "hex");
}

describe("RPT-045 — paridad de bytes con vectores-ipc.json", () => {
  it("las cotas del cliente coinciden con las que declara Rust", () => {
    // Sin esto, las constantes podrían derivar en silencio y sólo se notaría al
    // rozar un límite — es decir, con carga real y nunca en pruebas.
    const { limites } = vectores();

    assert.equal(LIMITES.marcoMaximo, limites["marcoMaximo"]);
    assert.equal(LIMITES.prefijoLongitud, limites["prefijoLongitud"]);
    assert.equal(LIMITES.prefijoNombre, limites["prefijoNombre"]);
    assert.equal(LIMITES.nombreMaximo, limites["nombreMaximo"]);
    assert.equal(LIMITES.codigoRespuesta, limites["codigoRespuesta"]);
    assert.equal(LIMITES.codigoRechazo, limites["codigoRechazo"]);
  });

  it("cada petición produce exactamente los bytes que produjo Rust", () => {
    const casos = vectores().peticiones;
    assert.ok(casos.length > 0, "los vectores no traen peticiones");

    for (const caso of casos) {
      const carga = desdeHex(caso.cargaHex);

      assert.equal(
        hex(componerPeticion(caso.canal, carga)),
        caso.cuerpoHex,
        `cuerpo distinto en «${caso.nombre}»`,
      );
      assert.equal(
        hex(enmarcar(componerPeticion(caso.canal, carga))),
        caso.marcoHex,
        `marco distinto en «${caso.nombre}»`,
      );
    }
  });

  it("cada respuesta se interpreta como la clase y el cuerpo que Rust compuso", () => {
    const casos = vectores().respuestas;
    assert.ok(casos.length > 0, "los vectores no traen respuestas");

    for (const caso of casos) {
      const leida = interpretarRespuesta(desdeHex(caso.cargaHex));

      assert.equal(leida.clase, caso.clase, `clase distinta en «${caso.nombre}»`);

      if (leida.clase === "respuesta") {
        assert.equal(hex(leida.cuerpo), caso.cuerpoHex, caso.nombre);
      } else {
        // El motivo vuelve a bytes para comparar: es la única forma de detectar
        // que se perdió un carácter multibyte por el camino.
        assert.equal(
          hex(Buffer.from(leida.motivo, "utf8")),
          caso.cuerpoHex,
          `motivo distinto en «${caso.nombre}»`,
        );
      }

      assert.equal(
        hex(enmarcar(desdeHex(caso.cargaHex))),
        caso.marcoHex,
        `marco distinto en «${caso.nombre}»`,
      );
    }
  });

  it("los vectores incluyen un motivo con carácter multibyte", () => {
    // El caso está puesto con intención: Rust recorta los motivos por BYTES y
    // `String.prototype.slice` recorta por unidades UTF-16. Si alguien «limpia»
    // el acento del vector, la trampa deja de existir sin que nada falle.
    const acentuados = vectores().respuestas.filter((caso) => {
      const bytes = desdeHex(caso.cuerpoHex);
      return bytes.length > bytes.toString("utf8").length;
    });

    assert.ok(
      acentuados.length > 0,
      "ningún vector de rechazo lleva un carácter multibyte: la trampa se perdió",
    );
  });
});

describe("RPT-045 §4 — el acumulador de marcos", () => {
  const marcoUno = enmarcar(Buffer.from("uno"));
  const marcoDos = enmarcar(Buffer.from("dos y algo mas"));

  it("un marco entero de una pieza se entrega tal cual", () => {
    const acumulador = new Acumulador();
    const marcos = acumulador.empujar(marcoUno);

    assert.equal(marcos.length, 1);
    assert.equal(marcos[0]?.toString(), "uno");
    assert.equal(acumulador.pendientes, 0);
  });

  it("dos marcos en un solo trozo se entregan los dos", () => {
    // Lo que pasa cuando el agente responde rápido y el sistema junta escrituras.
    const acumulador = new Acumulador();
    const marcos = acumulador.empujar(Buffer.concat([marcoUno, marcoDos]));

    assert.equal(marcos.length, 2);
    assert.equal(marcos[0]?.toString(), "uno");
    assert.equal(marcos[1]?.toString(), "dos y algo mas");
    assert.equal(acumulador.pendientes, 0);
  });

  it("un marco partido byte a byte se entrega entero y una sola vez", () => {
    // El caso que ninguna prueba con mensajes pequeños en local reproduce.
    const acumulador = new Acumulador();
    const entregados: Buffer[] = [];

    for (const byte of marcoDos) {
      entregados.push(...acumulador.empujar(Uint8Array.of(byte)));
    }

    assert.equal(entregados.length, 1);
    assert.equal(entregados[0]?.toString(), "dos y algo mas");
    assert.equal(acumulador.pendientes, 0);
  });

  it("el prefijo de longitud partido por la mitad no descarrila", () => {
    // El peor corte posible: llegan dos bytes del prefijo y nada más. Una
    // implementación que lea el prefijo sin comprobar que están los cuatro
    // bytes leería basura y esperaría un marco de longitud absurda.
    const acumulador = new Acumulador();

    assert.equal(acumulador.empujar(marcoDos.subarray(0, 2)).length, 0);
    assert.equal(acumulador.empujar(marcoDos.subarray(2, 3)).length, 0);

    const marcos = acumulador.empujar(marcoDos.subarray(3));
    assert.equal(marcos.length, 1);
    assert.equal(marcos[0]?.toString(), "dos y algo mas");
  });

  it("una longitud absurda se rechaza antes de acumular nada", () => {
    // El prefijo llega del otro extremo. Sin esta cota, quien hable puede hacer
    // que el cliente espere —y acumule— hasta agotar la memoria.
    const acumulador = new Acumulador();
    const absurdo = Buffer.alloc(LIMITES.prefijoLongitud);
    absurdo.writeUInt32BE(LIMITES.marcoMaximo + 1, 0);

    assert.throws(() => acumulador.empujar(absurdo), ErrorCable);
  });

  it("un marco vacío es válido y se distingue de no haber recibido nada", () => {
    const acumulador = new Acumulador();
    const marcos = acumulador.empujar(enmarcar(Buffer.alloc(0)));

    assert.equal(marcos.length, 1, "un marco de carga cero sigue siendo un marco");
    assert.equal(marcos[0]?.length, 0);
  });
});
