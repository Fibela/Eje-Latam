/**
 * # Eje-Visión — capa empresarial
 *
 * VIS-02, VIS-05 y CON-SIM. Licencia propietaria PremosCorp.
 *
 * ## Advertencia sobre la protección de este código
 *
 * Este paquete es JavaScript dentro de una aplicación Electron. El formato
 * `asar` no es cifrado: `npx asar extract` recupera el árbol completo. Cualquier
 * cliente con una licencia puede leer este código íntegro.
 *
 * **Ningún secreto puede residir aquí** — ni credenciales, ni umbrales de
 * detección, ni algoritmos cuya divulgación resulte perjudicial. La protección de
 * este paquete es jurídica, no técnica (RPT-004 §4).
 *
 * El activo comercial protegido es el **contenido y la operación**: la
 * Suscripción de Inteligencia Regional (RPT-003 §2.5).
 */

export { CONSOLA_NO_EJECUTA_SIMULACION } from "./vistas/con-sim-consola-simulacion/indice.js";
export type { OrdenSimulacro } from "./vistas/con-sim-consola-simulacion/indice.js";

export { impactoVacio } from "./vistas/vis-02-tablero-directivo/indice.js";
export type {
  AccionEstrategica,
  ImpactoIncidente,
} from "./vistas/vis-02-tablero-directivo/indice.js";

export { comparativaSectorialDisponible } from "./vistas/vis-05-mapa-calor-regional/indice.js";
export type { AlcanceMapa } from "./vistas/vis-05-mapa-calor-regional/indice.js";
