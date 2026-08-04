/**
 * VIS-01 — Consola Eje-Almacén.
 *
 * Cliente SQL, visor de esquemas e importación/exportación.
 *
 * ## Restricción
 *
 * Opera **exclusivamente contra ALM-02**, el sandbox del analista. El registro de
 * evidencia ALM-01 es de solo anexado con encadenamiento Merkle y no admite DDL
 * desde la interfaz — la autorización se aplica además en `eje-almacen`, en Rust
 * (RPT-002 §5).
 */

/** Base de datos alcanzable desde la consola. */
export type BaseAlcanzable = "alm-02-sandbox";

/**
 * Base contra la que opera VIS-01.
 *
 * El tipo tiene un único valor a propósito: no existe forma de expresar
 * "consultar ALM-01" desde esta vista.
 */
export const BASE_DE_LA_CONSOLA: BaseAlcanzable = "alm-02-sandbox";
