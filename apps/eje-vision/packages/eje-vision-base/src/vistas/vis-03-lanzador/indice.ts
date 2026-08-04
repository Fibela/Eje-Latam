/**
 * VIS-03 — Lanzador GUI.
 *
 * Aceptación de términos, selección de esquema de base y modo de red.
 */

/** Modo de esquema de `eje-almacen` seleccionable al arranque. */
export type ModoEsquema = "estandar" | "iot-ligero" | "personalizado";

/**
 * Alojamiento del servidor de señalización para la Capa B.
 *
 * RPT-003 §7: la opción propia debe ser **prominente y sin coste adicional**. Un
 * servidor operado por PremosCorp observa las direcciones IP públicas de todos
 * los clientes, metadato sensible en un producto que vende soberanía del dato.
 */
export type AlojamientoSenalizacion =
  | { readonly tipo: "oficial" }
  | { readonly tipo: "propio"; readonly puntoFinal: string };

/** Configuración recogida por el lanzador antes de arrancar la interfaz. */
export interface ConfiguracionArranque {
  /** Términos y licencia aceptados por el operador. */
  readonly terminosAceptados: boolean;
  /** Esquema de base seleccionado. */
  readonly modoEsquema: ModoEsquema;
  /** Perfil del segmento donde opera el nodo. */
  readonly perfilSegmento: "corporativo" | "ot";
  /** Alojamiento del servidor de señalización. */
  readonly senalizacion: AlojamientoSenalizacion;
  /** Autorización deliberada de la Capa B. */
  readonly capaBAutorizada: boolean;
}

/**
 * Configuración por defecto para un perfil dado.
 *
 * En perfil OT la Capa B arranca deshabilitada: una conexión saliente a internet
 * desde un segmento industrial puede vulnerar la segmentación en zonas y
 * conductos que exige IEC 62443 (RPT-003 §7).
 *
 * @param perfilSegmento Perfil del segmento vigilado.
 * @returns Configuración inicial coherente con el perfil.
 */
export function configuracionPorDefecto(
  perfilSegmento: "corporativo" | "ot",
): ConfiguracionArranque {
  return {
    terminosAceptados: false,
    modoEsquema: "estandar",
    perfilSegmento,
    senalizacion: { tipo: "oficial" },
    capaBAutorizada: false,
  };
}
