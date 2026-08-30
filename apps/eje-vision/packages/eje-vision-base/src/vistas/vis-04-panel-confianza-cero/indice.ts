/**
 * VIS-04 — Panel de Confianza Cero e Inventario Vivo.
 *
 * Inventario de dispositivos IoT/OT, postura por nodo y alertas del sistema.
 */

import type { EstadoBoveda, NodoInventario } from "../../ipc/puente.js";
import { requiereAlertaCapacidad } from "../../ipc/puente.js";

/** Severidad de una alerta mostrada en el panel. */
export type Severidad = "informativa" | "advertencia" | "critica";

/** Alerta presentada al operador. */
export interface Alerta {
  /** Código estable de la alerta, para documentación y soporte. */
  readonly codigo: string;
  /** Severidad asignada. */
  readonly severidad: Severidad;
  /** Mensaje mostrado al operador. */
  readonly mensaje: string;
}

/**
 * Alertas obligatorias derivadas del estado del sistema.
 *
 * Estas alertas no son configurables ni silenciables: cada una corresponde a una
 * condición que un reporte canónico declara como no ignorable.
 *
 * @param estadoBoveda Ocupación actual de la Bóveda Aislada.
 * @param firmaModuloValida Resultado de la verificación de firma del paquete
 *   empresarial. `null` si no hay paquete empresarial instalado.
 * @returns Alertas que el panel debe mostrar.
 */
export function alertasObligatorias(
  estadoBoveda: EstadoBoveda,
  firmaModuloValida: boolean | null,
): readonly Alerta[] {
  const alertas: Alerta[] = [];

  if (requiereAlertaCapacidad(estadoBoveda)) {
    alertas.push({
      codigo: "BOV-CAPACIDAD",
      severidad: "critica",
      mensaje:
        "La Bóveda Aislada alcanzó su límite de capacidad. Los eventos más antiguos se están descartando.",
    });
  }

  if (firmaModuloValida === false) {
    alertas.push({
      codigo: "MOD-FIRMA-INVALIDA",
      severidad: "critica",
      mensaje:
        "La firma del paquete empresarial no es válida. El módulo no se cargó.",
    });
  }

  return alertas;
}

/**
 * Recuento del inventario por **calidad del respaldo**, para la cabecera.
 *
 * RPT-089, PA-139. Sustituye a `ResumenPostura`, que contaba por un campo que el
 * agente nunca produjo.
 *
 * Lo que el operador necesita de un vistazo no es cuántos nodos «están
 * conformes» —el agente no juzga eso—, sino **cuánto del inventario está
 * respaldado por una firma y cuánto no se sabe**. Las cuatro cifras suman
 * siempre el total.
 *
 * # Las cuatro ambigüedades se agregan aquí, y sólo aquí
 *
 * El cable las lleva separadas —marcado caducado, fuentes que se contradicen,
 * huella sin respaldo, segmento sin declarar— porque mandan a mirar sitios
 * distintos. Esta función las junta **para una cabecera**, que es un recuento y
 * no un diagnóstico.
 *
 * Que la agregación viva aquí y no en el agente es el punto entero de RPT-088:
 * si mañana la cabecera tiene que separarlas, se cambia esta capa y no hay que
 * recompilar el sensor ni volver a desplegar en planta. Cada nodo conserva su
 * motivo.
 */
export interface ResumenRespaldo {
  /** Marcado firmado y vigente por dispositivo, sea o no crítico. */
  readonly declarados: number;
  /** Sin marcado propio, pero en un segmento declarado libre de críticos. */
  readonly porSegmento: number;
  /** La evidencia falta o se contradice. Cada nodo dice cuál de los cuatro. */
  readonly ambiguos: number;
  /** Una fuente declarativa no se pudo consultar. Distinto de que no aporte. */
  readonly indeterminados: number;
}

/**
 * Agrega el inventario por calidad del respaldo.
 *
 * @param inventario Nodos observados.
 * @returns Recuento. Las cuatro cifras suman `inventario.length`.
 */
export function resumirRespaldo(
  inventario: readonly NodoInventario[],
): ResumenRespaldo {
  let declarados = 0;
  let porSegmento = 0;
  let ambiguos = 0;
  let indeterminados = 0;

  for (const nodo of inventario) {
    switch (nodo.clase) {
      case "declaradaSoporteVital":
      case "declaradaSeguridadFuncional":
      case "declaradaCaminoDeGestion":
      case "declaradaNoCritica":
        declarados += 1;
        break;
      case "segmentoDeclaradoSinCriticos":
        porSegmento += 1;
        break;
      case "ambiguaMarcadoCaducado":
      case "ambiguaConflictoEntreFuentes":
      case "ambiguaInferenciaSugiereCriticidad":
      case "ambiguaSegmentoPuedeAlojarCriticos":
        ambiguos += 1;
        break;
      case "indeterminada":
        indeterminados += 1;
        break;
    }
  }

  return { declarados, porSegmento, ambiguos, indeterminados };
}
