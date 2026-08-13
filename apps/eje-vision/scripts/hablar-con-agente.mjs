/**
 * Diagnóstico de PA-78: hacer que los dos procesos se hablen, por primera vez.
 *
 * Uso:
 *   node scripts/hablar-con-agente.mjs <ruta-del-socket> [canal] [repeticiones]
 *
 * Ejemplo:
 *   node scripts/hablar-con-agente.mjs /tmp/eje/agente.sock obtener-condiciones
 *
 * ## Por qué usa el código compilado y no un cliente propio
 *
 * Un cliente escrito aquí a mano comprobaría que *algo* puede hablar con el
 * agente, que no es lo que hace falta saber. Lo que hace falta saber es si
 * **`enlace.ts` y `cable.ts` —los que van en el producto—** hablan con él.
 *
 * Por eso importa de `dist/`. Si esto funciona, funciona lo que enviamos.
 *
 * ## Qué imprime, y por qué tanto
 *
 * Los bytes crudos en hexadecimal, en los dos sentidos. Es la primera vez que
 * este cable se usa de verdad: si algo no cuadra, el diagnóstico útil no es
 * «falló», es ver el prefijo y contar.
 */

import { connect } from "node:net";
import { existsSync, statSync } from "node:fs";

import { BITACORA_INICIAL, incorporar } from "@eje/vision-base";

import { componerPeticion, enmarcar } from "../proceso-principal/dist/cable.js";
import { pedir } from "../proceso-principal/dist/enlace.js";

const [ruta, canal = "obtener-condiciones", vueltasTexto, modo] = process.argv.slice(2);
const repeticiones = Number.parseInt(vueltasTexto ?? "1", 10) || 1;

/**
 * Con `cursor`, cada consulta avanza `desdeAsiento` con la bitacora real de la
 * capa base (PA-98). Sin el, se pide siempre desde cero — que es lo que hacia la
 * consola antes y lo que RPT-050 §4 midio en 40 ms de sensor por refresco.
 */
const conCursor = modo === "cursor";

if (ruta === undefined) {
  process.stderr.write(
    "uso: node scripts/hablar-con-agente.mjs <ruta-del-socket> [canal]\n",
  );
  process.exit(2);
}

// Se comprueba antes de conectar para que el fallo diga qué pasa. `ENOENT` a
// secas no distingue «el agente no está» de «el agente está en otra ruta».
if (!existsSync(ruta)) {
  process.stderr.write(
    `No existe ${ruta}.\n` +
      "El agente imprime su ruta al arrancar, en la línea 'Escucha local'.\n" +
      "Si dice 'NO disponible', el socket no llegó a abrirse y el motivo está ahí.\n",
  );
  process.exit(1);
}

if (!statSync(ruta).isSocket()) {
  process.stderr.write(`${ruta} existe y no es un socket.\n`);
  process.exit(1);
}

// Estas dos comprobaciones NO dicen que el agente esté vivo. Un agente que muere
// sin limpiar deja el fichero en su sitio y las dos pasan. Se dice aquí porque
// la primera versión de este script daba luz verde y fallaba después.

/**
 * Cronómetro de una petición. PA-83.
 *
 * Separa **espera** de **transferencia** a propósito. El agente atiende al final
 * de cada vuelta (~500 ms), así que hay un suelo de latencia que no depende del
 * tamaño de la respuesta. Medir sólo el total mezclaría las dos cosas y llevaría
 * a optimizar la que no importa.
 */
const reloj = { escrito: 0, primerTrozo: 0, trozos: 0, bytes: 0 };

function abrirConducto() {
  const zocalo = connect(ruta);
  zocalo.setNoDelay(true);

  return {
    escribir: (datos) => {
      if (detallado) {
        process.stdout.write(`→ ${datos.length} bytes: ${hex(datos)}\n`);
      }
      reloj.escrito = performance.now();
      zocalo.write(datos);
    },
    alRecibir: (manejador) =>
      zocalo.on("data", (trozo) => {
        if (reloj.trozos === 0) {
          reloj.primerTrozo = performance.now();
        }
        reloj.trozos += 1;
        reloj.bytes += trozo.length;
        if (detallado) {
          process.stdout.write(`← ${trozo.length} bytes: ${hex(trozo)}\n`);
        }
        manejador(trozo);
      }),
    alFallar: (manejador) => zocalo.on("error", manejador),
    alCerrar: (manejador) => zocalo.on("close", manejador),
    cerrar: () => zocalo.destroy(),
  };
}

function hex(bytes) {
  const texto = Buffer.from(bytes).toString("hex");
  // Un volcado de 256 bytes no aporta más que los primeros; el prefijo y el
  // arranque del cuerpo es donde están los errores de este cable.
  return texto.length > 160 ? `${texto.slice(0, 160)}… (${bytes.length} B)` : texto;
}

// Con repeticiones se mide; con una sola vuelta se inspecciona. Volcar mil
// líneas de hexadecimal mientras se cronometra falsearía la medida.
const detallado = repeticiones === 1;

let bitacora = BITACORA_INICIAL;

function cargaDe() {
  if (canal !== "consultar-alertas") {
    return Buffer.alloc(0);
  }
  const desde = conCursor ? bitacora.desdeAsiento : 0;
  return Buffer.from(JSON.stringify({ desdeAsiento: desde }), "utf8");
}

process.stdout.write(`Socket : ${ruta}\nCanal  : ${canal}\n`);

if (detallado) {
  process.stdout.write(
    `\nPetición esperada: ${hex(enmarcar(componerPeticion(canal, cargaDe())))}\n\n`,
  );
} else {
  process.stdout.write(
    `Vueltas: ${repeticiones}${conCursor ? "  (con cursor, PA-98)" : "  (sin cursor)"}\n\n`,
  );
  process.stdout.write("  #   espera    transfer.   total    trozos     bytes\n");
}

const medidas = [];

try {
  let respuesta;
  for (let vuelta = 1; vuelta <= repeticiones; vuelta += 1) {
    reloj.trozos = 0;
    reloj.bytes = 0;
    const arranque = performance.now();

    respuesta = await pedir(abrirConducto, canal, cargaDe(), 10_000);

    const fin = performance.now();
    // Espera: desde que se escribe hasta el primer byte de vuelta. Es el agente
    // terminando su vuelta, no el cable.
    const espera = reloj.primerTrozo - reloj.escrito;
    // Transferencia: del primer trozo al último, incluido el reensamblado.
    const transferencia = fin - reloj.primerTrozo;
    medidas.push({ espera, transferencia, total: fin - arranque, trozos: reloj.trozos, bytes: reloj.bytes });

    // La bitacora se alimenta SIEMPRE, se use o no el cursor para pedir: asi la
    // comparacion entre los dos modos usa exactamente el mismo codigo.
    if (canal === "consultar-alertas" && respuesta.clase === "respuesta") {
      bitacora = incorporar(bitacora, JSON.parse(respuesta.cuerpo.toString("utf8")));
    }

    if (!detallado) {
      process.stdout.write(
        `${String(vuelta).padStart(3)}  ${espera.toFixed(1).padStart(8)}ms ` +
          `${transferencia.toFixed(1).padStart(8)}ms ` +
          `${(fin - arranque).toFixed(1).padStart(8)}ms ` +
          `${String(reloj.trozos).padStart(6)} ${String(reloj.bytes).padStart(9)}\n`,
      );
    }
  }

  if (!detallado) {
    const resumen = (clave) => {
      const valores = medidas.map((cada) => cada[clave]).sort((uno, otro) => uno - otro);
      const mediana = valores[Math.floor(valores.length / 2)];
      return {
        min: valores[0],
        mediana,
        max: valores[valores.length - 1],
      };
    };

    const espera = resumen("espera");
    const transferencia = resumen("transferencia");

    process.stdout.write("\n");
    process.stdout.write(
      `Espera del agente : min ${espera.min.toFixed(1)}ms  ` +
        `mediana ${espera.mediana.toFixed(1)}ms  max ${espera.max.toFixed(1)}ms\n`,
    );
    process.stdout.write(
      `Transferencia     : min ${transferencia.min.toFixed(1)}ms  ` +
        `mediana ${transferencia.mediana.toFixed(1)}ms  max ${transferencia.max.toFixed(1)}ms\n`,
    );
    const bytes = medidas.map((cada) => cada.bytes);
    process.stdout.write(
      `Bytes por vuelta  : primera ${bytes[0]}  ultima ${bytes[bytes.length - 1]}\n`,
    );

    if (canal === "consultar-alertas") {
      process.stdout.write(
        `Bitacora          : ${bitacora.sucesos.length} sucesos, marca en ` +
          `${bitacora.desdeAsiento}, salto=${bitacora.huboSalto}\n`,
      );
    }

    process.stdout.write(
      "\nSi la espera domina, la cadencia (PA-92) depende del ciclo del agente\n" +
        "y no del tamano de la respuesta.\n",
    );
  }

  process.stdout.write(`\nClase  : ${respuesta.clase}\n`);

  if (respuesta.clase === "rechazo") {
    // Un rechazo NO es un fallo de esta prueba. Cuatro de los seis canales no
    // tienen manejador todavía y el agente lo dice con motivo: eso es
    // exactamente lo que RPT-036 §6 quería, y significa que el cable funciona.
    process.stdout.write(`Motivo : ${respuesta.motivo}\n`);
    process.stdout.write(
      "\nEl agente rechazó con motivo. El cable funciona: la conversación " +
        "cruzó entera y el texto llegó legible.\n",
    );
  } else if (detallado) {
    process.stdout.write(`Cuerpo : ${respuesta.cuerpo.toString("utf8")}\n`);
    process.stdout.write("\nConversación completa.\n");
  } else {
    // Midiendo no se vuelca el cuerpo: un megabyte de JSON por consola no
    // aporta nada y ademas falsea el cronometro de la ultima vuelta.
    process.stdout.write(`Cuerpo : ${respuesta.cuerpo.length} bytes (no se vuelca)\n`);
  }
} catch (error) {
  process.stderr.write(`\nNo hubo conversación: ${String(error)}\n`);
  process.exit(1);
}
