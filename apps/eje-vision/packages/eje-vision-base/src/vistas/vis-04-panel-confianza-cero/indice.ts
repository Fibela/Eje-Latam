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
 * RPT-088, PA-139. Sustituye a `ResumenPostura`, que contaba por un campo que el
 * agente nunca produjo.
 *
 * Lo que el operador necesita saber de un vistazo no es cuántos nodos «están
 * conformes» —el agente no juzga eso—, sino **cuánto del inventario está
 * respaldado por una firma y cuánto es suposición**. Las cinco cifras suman
 * siempre el total: ninguna categoría absorbe a otra.
 */
export interface ResumenRespaldo {
  /** Clase declarada por marcado firmado y vigente. */
  readonly declarados: number;
  /** Sin marcado; la huella observada sugiere una clase. Es suposición. */
  readonly inferidos: number;
  /** El marcado dice una cosa y la huella otra. Exige mirar. */
  readonly enConflicto: number;
  /** Nada apunta a nada. **No** significa que no sean críticos. */
  readonly sinIndicio: number;
  /** La fuente no se pudo consultar. Distinto de que no aporte. */
  readonly indeterminados: number;
}

/**
 * Agrega el inventario por calidad del respaldo.
 *
 * @param inventario Nodos observados.
 * @returns Recuento por respaldo. Las cinco cifras suman `inventario.length`.
 */
export function resumirRespaldo(
  inventario: readonly NodoInventario[],
): ResumenRespaldo {
  let declarados = 0;
  let inferidos = 0;
  let enConflicto = 0;
  let sinIndicio = 0;
  let indeterminados = 0;

  for (const nodo of inventario) {
    switch (nodo.clase) {
      case "declaradaSoporteVital":
      case "declaradaSeguridadFuncional":
      case "declaradaCaminoDeGestion":
        declarados += 1;
        break;
      case "inferidaSoporteVital":
      case "inferidaSeguridadFuncional":
        inferidos += 1;
        break;
      case "enConflicto":
        enConflicto += 1;
        break;
      case "sinIndicio":
        sinIndicio += 1;
        break;
      case "indeterminada":
        indeterminados += 1;
        break;
    }
  }

  return { declarados, inferidos, enConflicto, sinIndicio, indeterminados };
}
