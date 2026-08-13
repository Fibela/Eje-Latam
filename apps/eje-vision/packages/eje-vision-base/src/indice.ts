/**
 * # Eje-Visión — capa base
 *
 * Vistas y componentes bajo **Apache-2.0**: VIS-01, VIS-03 y VIS-04.
 *
 * ## Frontera de licencia
 *
 * Este paquete no puede importar de `@eje/vision-empresarial` ni de los módulos
 * internos de Node. Ambas reglas se verifican con dependency-cruiser en CI
 * (RPT-004 §7); no dependen de la disciplina de quien contribuye.
 */

export type {
  ClaseAlerta,
  Condiciones,
  EstadoAgente,
  EstadoBoveda,
  NodoInventario,
  PuenteEje,
  ResultadoConsulta,
  RespuestaAlertas,
  SucesoAlerta,
} from "./ipc/puente.js";
export {
  CAMPOS_CONDICIONES,
  CAMPOS_RESPUESTA_ALERTAS,
  CAMPOS_ESTADO_AGENTE,
  CAMPOS_ESTADO_BOVEDA,
  CAMPOS_NODO_INVENTARIO,
  CAMPOS_PETICION_ALERTAS,
  CAMPOS_PETICION_CONSULTA,
  CAMPOS_RESULTADO_CONSULTA,
  CAMPOS_SUCESO_ALERTA,
  requiereAlertaCapacidad,
  UMBRAL_ALERTA_BOVEDA,
} from "./ipc/puente.js";

export { BASE_DE_LA_CONSOLA } from "./vistas/vis-01-consola-almacen/indice.js";
export type { BaseAlcanzable } from "./vistas/vis-01-consola-almacen/indice.js";

export { configuracionPorDefecto } from "./vistas/vis-03-lanzador/indice.js";
export type {
  AlojamientoSenalizacion,
  ConfiguracionArranque,
  ModoEsquema,
} from "./vistas/vis-03-lanzador/indice.js";

export {
  alertasObligatorias,
  resumirPostura,
} from "./vistas/vis-04-panel-confianza-cero/indice.js";
export type {
  Alerta,
  ResumenPostura,
  Severidad,
} from "./vistas/vis-04-panel-confianza-cero/indice.js";

export {
  PREFIJO_RECHAZO,
  conDatos,
  desdeFallo,
  esObservacion,
  listaVacia,
} from "./vista/estado-panel.js";
export type { EstadoPanel } from "./vista/estado-panel.js";

export { leerSinAgente } from "./vista/sin-agente.js";
export type { LecturaSinAgente } from "./vista/sin-agente.js";

export { componerCabecera } from "./vista/cabecera.js";
export type { Cabecera, Urgencia } from "./vista/cabecera.js";

export { componerSucesos } from "./vista/sucesos.js";
export type { VistaSucesos } from "./vista/sucesos.js";

export {
  BITACORA_INICIAL,
  SUCESOS_EN_MEMORIA,
  esperaSugerida,
  incorporar,
} from "./vista/bitacora.js";
export type { Bitacora } from "./vista/bitacora.js";
