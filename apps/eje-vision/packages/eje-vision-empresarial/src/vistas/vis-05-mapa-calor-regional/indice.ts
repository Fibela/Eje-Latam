/**
 * VIS-05 — Mapa de Calor Regional.
 *
 * Visualización de vectores de ataque activos y comparación de postura.
 *
 * ## Dependencia declarada
 *
 * La comparativa contra el promedio sectorial de Latinoamérica **solo existe con
 * agregación multiinquilino**, es decir `NUC-01`, que es Fase 2. En Fase 1 esta
 * vista se limita a los datos del propio despliegue, y así debe comunicarse en el
 * material comercial (RPT-002 §9.6).
 */

/** Alcance de los datos que el mapa puede representar. */
export type AlcanceMapa = "despliegue-propio" | "sectorial-regional";

/**
 * Indica si la comparativa sectorial puede ofrecerse.
 *
 * @param nucleoRegionalDisponible Si `NUC-01` está operativo y el cliente
 *   participa en la agregación.
 * @returns `true` solo cuando existe agregación multiinquilino real.
 */
export function comparativaSectorialDisponible(
  nucleoRegionalDisponible: boolean,
): boolean {
  return nucleoRegionalDisponible;
}
