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

/** Recuento de nodos por postura, para la cabecera del panel. */
export interface ResumenPostura {
  /** Nodos conformes. */
  readonly conformes: number;
  /** Nodos con comportamiento anómalo. */
  readonly anomalos: number;
  /** Nodos actualmente contenidos. */
  readonly contenidos: number;
}

/**
 * Agrega el inventario por postura.
 *
 * @param inventario Nodos descubiertos.
 * @returns Recuento por postura.
 */
export function resumirPostura(
  inventario: readonly NodoInventario[],
): ResumenPostura {
  let conformes = 0;
  let anomalos = 0;
  let contenidos = 0;

  for (const nodo of inventario) {
    switch (nodo.postura) {
      case "conforme":
        conformes += 1;
        break;
      case "anomalo":
        anomalos += 1;
        break;
      case "contenido":
        contenidos += 1;
        break;
    }
  }

  return { conformes, anomalos, contenidos };
}
