/**
 * Diagnóstico de PA-78: hacer que los dos procesos se hablen, por primera vez.
 *
 * Uso:
 *   node scripts/hablar-con-agente.mjs <ruta-del-socket> [canal]
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

import { componerPeticion, enmarcar } from "../proceso-principal/dist/cable.js";
import { pedir } from "../proceso-principal/dist/enlace.js";

const [ruta, canal = "obtener-condiciones"] = process.argv.slice(2);

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

function abrirConducto() {
  const zocalo = connect(ruta);
  zocalo.setNoDelay(true);

  return {
    escribir: (datos) => {
      process.stdout.write(`→ ${datos.length} bytes: ${hex(datos)}\n`);
      zocalo.write(datos);
    },
    alRecibir: (manejador) =>
      zocalo.on("data", (trozo) => {
        process.stdout.write(`← ${trozo.length} bytes: ${hex(trozo)}\n`);
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

const carga =
  canal === "consultar-alertas"
    ? Buffer.from(JSON.stringify({ desdeAsiento: 0 }), "utf8")
    : Buffer.alloc(0);

process.stdout.write(`Socket : ${ruta}\nCanal  : ${canal}\n\n`);
process.stdout.write(
  `Petición esperada: ${hex(enmarcar(componerPeticion(canal, carga)))}\n\n`,
);

try {
  const respuesta = await pedir(abrirConducto, canal, carga, 5_000);

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
  } else {
    process.stdout.write(`Cuerpo : ${respuesta.cuerpo.toString("utf8")}\n`);
    process.stdout.write("\nConversación completa.\n");
  }
} catch (error) {
  process.stderr.write(`\nNo hubo conversación: ${String(error)}\n`);
  process.exit(1);
}
