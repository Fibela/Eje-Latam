/**
 * # Proceso principal de Eje-Visión
 *
 * Guardián del puente IPC y cargador firmado de módulos.
 *
 * Este paquete es **Apache-2.0**: es la capa que aplica las restricciones de
 * seguridad, y una capa de seguridad que el cliente no puede auditar no cumple su
 * función (RPT-003 §2.5).
 */

export {
  directorioAdmisible,
  resumirContenido,
  serializarManifiesto,
  verificarPaquete,
} from "./cargador-firmado.js";
export type {
  ManifiestoPaquete,
  MotivoRechazo,
  ResultadoVerificacion,
} from "./cargador-firmado.js";

export { capacidades, fueLicenciadoAlgunaVez } from "./estado-licencia.js";
export type { CapacidadesInterfaz, EstadoLicencia } from "./estado-licencia.js";

export {
  CANALES_PERMITIDOS,
  CANALES_PROHIBIDOS,
  CARGA_MAXIMA_BYTES,
  esCanalPermitido,
  validarPeticion,
} from "./puente-ipc.js";
export type {
  CanalPermitido,
  MotivoRechazoIpc,
  ValidacionPeticion,
} from "./puente-ipc.js";

export {
  CSP,
  DESTINOS_EXTERNOS_PERMITIDOS,
  destinoExternoPermitido,
  PREFERENCIAS_SEGURIDAD,
} from "./seguridad-ventana.js";
export type { PreferenciasSeguridad } from "./seguridad-ventana.js";
