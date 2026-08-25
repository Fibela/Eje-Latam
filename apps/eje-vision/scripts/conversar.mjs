/**
 * La conversación con el agente, sin ventana. RPT-079 §5, PA-78.
 *
 * # Qué es esto y qué no es
 *
 * **No es un simulacro.** El enmarcado, el vencimiento, la interpretación de la
 * respuesta y la lista de canales son los módulos que se despliegan:
 * `enlace.js`, `cable.js` y `puente-ipc.js`, importados de `dist/`. Lo único que
 * falta es la ventana.
 *
 * Lo que **sí** se recrea aquí, y conviene decirlo en voz alta, son dos piezas
 * de fontanería que viven dentro de `arranque.ts` y no se pueden importar
 * porque ese fichero arrastra Electron:
 *
 *   - `abrirConducto`, ocho líneas de envoltorio sobre `net.connect`;
 *   - `cargaDe`, la traducción de argumento a carga útil.
 *
 * Duplicar código es lo que este proyecto persigue todo el tiempo, así que la
 * duplicación se acota a propósito: **el protocolo no se reimplementa**, sólo el
 * pegamento. Y si `cargaDe` estuviera mal, el agente rechazaría la petición con
 * su motivo, que es un fallo ruidoso y no uno silencioso.
 *
 * # Por qué recorre la lista en vez de pedir unos cuantos
 *
 * `CANALES_PERMITIDOS` es la autoridad. Escribir seis peticiones a mano sería el
 * séptimo índice escrito a mano de la serie, y se quedaría corto el día que se
 * añada un canal — justo el día en que más falta haría preguntarlo.
 *
 * # Uso
 *
 *   node scripts/conversar.mjs
 *
 * Sin `EJE_SOCKET`: el valor de fábrica es el que se prueba (PA-132).
 */

import { connect } from "node:net";

import { pedir } from "../proceso-principal/dist/enlace.js";
import { CANALES_PERMITIDOS } from "../proceso-principal/dist/puente-ipc.js";
import { rutaSocket } from "../proceso-principal/dist/punto-de-encuentro.js";

const RUTA = rutaSocket();

/** Envoltorio sobre un socket real. Copia de `arranque.ts`; ver la cabecera. */
function abrirConducto() {
  const zocalo = connect(RUTA);
  zocalo.setNoDelay(true);

  return {
    escribir: (datos) => void zocalo.write(datos),
    alRecibir: (manejador) => void zocalo.on("data", manejador),
    alFallar: (manejador) => void zocalo.on("error", manejador),
    alCerrar: (manejador) => void zocalo.on("close", manejador),
    cerrar: () => zocalo.destroy(),
  };
}

/** Carga útil por canal. Copia de `cargaDe` en `arranque.ts`. */
function cargaDe(canal) {
  if (canal === "consultar-sandbox") {
    return Buffer.from(JSON.stringify({ sentencia: "SELECT 1" }), "utf8");
  }
  if (canal === "consultar-alertas") {
    return Buffer.from(JSON.stringify({ desdeAsiento: 0 }), "utf8");
  }
  return Buffer.alloc(0);
}

console.log(`Punto de encuentro : ${RUTA}`);
console.log(`Canales a preguntar: ${CANALES_PERMITIDOS.length}`);
console.log("");

let fallos = 0;

for (const canal of CANALES_PERMITIDOS) {
  const inicio = Date.now();

  try {
    const respuesta = await pedir(abrirConducto, canal, cargaDe(canal));
    const tardo = Date.now() - inicio;

    if (respuesta.clase === "rechazo") {
      // Un rechazo NO es un fallo de transporte: es una respuesta válida con
      // motivo (RPT-036 §6). Se cuenta aparte para no confundir «el agente dijo
      // que no» con «el agente no dijo nada».
      console.log(`RECHAZO  ${canal}  (${tardo} ms)`);
      console.log(`         motivo: ${respuesta.motivo}`);
      continue;
    }

    const texto = respuesta.cuerpo.toString("utf8");
    console.log(`OK       ${canal}  (${tardo} ms, ${respuesta.cuerpo.length} bytes)`);
    console.log(`         ${texto.length > 600 ? `${texto.slice(0, 600)}…` : texto}`);
  } catch (error) {
    fallos += 1;
    console.log(`FALLO    ${canal}  (${Date.now() - inicio} ms)`);
    console.log(`         ${error instanceof Error ? error.message : String(error)}`);
  }

  console.log("");
}

console.log(
  fallos === 0
    ? "Los dos procesos se hablan."
    : `${fallos} canal(es) no llegaron a responder.`,
);

process.exit(fallos === 0 ? 0 : 1);
