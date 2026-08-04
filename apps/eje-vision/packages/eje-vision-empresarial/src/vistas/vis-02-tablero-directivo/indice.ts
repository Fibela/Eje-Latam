/**
 * VIS-02 — Tablero Directivo.
 *
 * Traduce el incidente a impacto operativo, financiero y reputacional, con
 * acciones estratégicas. Diferenciador frente a los SIEM tradicionales, que
 * saturan al directivo con registros técnicos.
 *
 * ## Disponibilidad
 *
 * Esta vista opera durante un incidente activo **aunque la licencia esté
 * vencida** (RPT-003 §3.4). La condición de carga es haber sido licenciado
 * alguna vez, no estarlo ahora.
 */

/** Impacto estimado de un incidente, en las tres dimensiones del tablero. */
export interface ImpactoIncidente {
  /** Procesos o líneas de producción afectados. */
  readonly operativo: number;
  /** Estimación de coste, en la moneda configurada. */
  readonly financiero: number;
  /** Escala reputacional de 0 a 100. */
  readonly reputacional: number;
}

/**
 * Acción estratégica ofrecida al comité de crisis.
 *
 * La ejecución la realiza `eje-agente`; esta vista solo la solicita.
 */
export type AccionEstrategica =
  | "aislar-red-industrial"
  | "notificar-regulador"
  | "activar-redundancia-fuera-de-linea"
  | "convocar-comite";

/**
 * Impacto neutro, usado como estado inicial antes de recibir telemetría.
 *
 * @returns Impacto con todas las dimensiones en cero.
 */
export function impactoVacio(): ImpactoIncidente {
  return { operativo: 0, financiero: 0, reputacional: 0 };
}
