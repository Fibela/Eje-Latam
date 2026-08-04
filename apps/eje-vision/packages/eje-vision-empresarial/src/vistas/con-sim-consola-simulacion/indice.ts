/**
 * CON-SIM — Consola de Simulación.
 *
 * Ordena y observa simulacros ejecutados por `SIM-01`.
 *
 * ## Esta consola NO es el simulador
 *
 * `SIM-01` reside en `eje-agente`, en Rust (RPT-002 §4, RPT-004 §2.2).
 *
 * La distinción no es formal. RPT-003 §8.1 exige que `SIM-01` y la ruta de
 * contención residan en dominios de capacidad separados: el simulador **no posee**
 * la capacidad de invocar contención — no es que la invoque y sea rechazada.
 *
 * Si el motor de simulación se reimplementara aquí, esa garantía arquitectónica
 * quedaría reducida a una comprobación de interfaz, que es exactamente el modo de
 * fallo capaz de aislar equipamiento médico durante un simulacro.
 */

/**
 * Orden de simulacro remitida al agente.
 *
 * Nótese que no existe campo alguno para solicitar contención: esta consola no
 * puede expresar esa petición.
 */
export interface OrdenSimulacro {
  /** Escenario a inyectar. */
  readonly escenario:
    | "ransomware-en-linea-de-produccion"
    | "caida-de-pasarela-de-pagos"
    | "secuestro-de-base-de-clientes"
    | "apagon-digital";
  /** Duración del simulacro en minutos. */
  readonly duracionMinutos: number;
  /** Participantes convocados al ejercicio. */
  readonly participantes: readonly string[];
}

/**
 * Invariante documentada: esta consola no ejecuta el motor de simulación.
 *
 * Se exporta como constante para que quede visible en el índice del paquete y
 * cualquier intento futuro de añadir un motor aquí choque con una declaración
 * explícita en lugar de con una convención tácita.
 */
export const CONSOLA_NO_EJECUTA_SIMULACION = true as const;
