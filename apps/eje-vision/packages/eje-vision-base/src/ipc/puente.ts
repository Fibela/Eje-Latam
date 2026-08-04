/**
 * Superficie del puente IPC expuesta al proceso de renderizado.
 *
 * RPT-004 §6.2. Queda **prohibido** exponer un pasamanos genérico del tipo
 * `invoke(canal, argumentos)`: eso traslada la decisión de autorización al
 * renderer, que es justo la capa que no debe tenerla.
 *
 * Cada operación permitida es un método con nombre y tipo propios. Añadir una
 * operación exige tocar este fichero, el `puente-ipc.ts` del proceso principal y
 * la lista de canales permitidos — tres puntos, todos revisables.
 */

/** Estado resumido del demonio `eje-agente`. */
export interface EstadoAgente {
  /** Versión del agente en ejecución. */
  readonly version: string;
  /** Perfil del segmento vigilado. */
  readonly perfil: "corporativo" | "ot";
  /** Si la respuesta automática está habilitada según vigencia de reglas. */
  readonly respuestaAutomatica: boolean;
}

/** Nodo IoT/OT descubierto, tal como se muestra en VIS-04. */
export interface NodoInventario {
  /** Identificador estable del nodo. */
  readonly identificador: string;
  /** Dirección de capa de enlace observada. */
  readonly direccionEnlace: string;
  /** Clasificación del dispositivo. */
  readonly clase: "plc" | "camara" | "medico" | "estacion" | "desconocido";
  /** Postura de confianza cero evaluada. */
  readonly postura: "conforme" | "anomalo" | "contenido";
}

/**
 * Estado de ocupación de la Bóveda Aislada.
 *
 * VIS-04 debe emitir alerta obligatoria al alcanzar el límite: un disco lleno en
 * un nodo hospitalario es una interrupción (RPT-002 §5, AGT-04).
 */
export interface EstadoBoveda {
  /** Bytes ocupados por la cola de eventos pendientes. */
  readonly usadoBytes: number;
  /** Límite configurado en bytes. */
  readonly limiteBytes: number;
  /** Eventos pendientes de reconciliación. */
  readonly eventosPendientes: number;
}

/** Resultado de una consulta al sandbox del analista (ALM-02). */
export interface ResultadoConsulta {
  /** Nombres de columna devueltos. */
  readonly columnas: readonly string[];
  /** Filas devueltas, en el orden de `columnas`. */
  readonly filas: readonly (readonly string[])[];
}

/**
 * API cerrada expuesta por el preload.
 *
 * Nótese que no existe ningún método que acepte un nombre de canal como dato.
 */
export interface PuenteEje {
  /** VIS-04 — estado del demonio local. */
  obtenerEstadoAgente(): Promise<EstadoAgente>;

  /** VIS-04 — inventario vivo de dispositivos IoT/OT. */
  obtenerInventario(): Promise<readonly NodoInventario[]>;

  /** VIS-04 — ocupación de la Bóveda, para la alerta de capacidad. */
  obtenerEstadoBoveda(): Promise<EstadoBoveda>;

  /**
   * VIS-01 — consulta contra el sandbox del analista.
   *
   * Opera **solo contra ALM-02**. El registro de evidencia ALM-01 no es
   * alcanzable desde la interfaz (RPT-002 §5).
   */
  consultarSandbox(sentencia: string): Promise<ResultadoConsulta>;
}

/**
 * Umbral a partir del cual VIS-04 debe emitir alerta de capacidad.
 *
 * Se alerta antes de agotar el espacio, no al agotarlo: una vez lleno ya se están
 * descartando eventos.
 */
export const UMBRAL_ALERTA_BOVEDA = 0.85;

/**
 * Indica si el estado de la Bóveda exige alerta en VIS-04.
 *
 * @param estado Ocupación actual de la Bóveda.
 * @returns `true` si se superó el umbral de alerta.
 */
export function requiereAlertaCapacidad(estado: EstadoBoveda): boolean {
  if (estado.limiteBytes <= 0) {
    return true;
  }
  return estado.usadoBytes / estado.limiteBytes >= UMBRAL_ALERTA_BOVEDA;
}
