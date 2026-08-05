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

// ---------------------------------------------------------------------------
// Descripción declarativa de las cargas útiles — PA-21
// ---------------------------------------------------------------------------
//
// TypeScript borra sus tipos al compilar: no hay forma de comparar una interfaz
// contra el manifiesto en tiempo de ejecución. Estas constantes son el puente,
// y `contrato.prueba.ts` comprueba que coincidan con `contrato-ipc.toml`.
//
// El lado Rust ata su equivalente a los structs mediante desestructuración
// exhaustiva: añadir un campo rompe la compilación. Aquí hacen falta **dos**
// mecanismos para lograr lo mismo, porque `satisfies` solo cubre la mitad:
//
//   1. `satisfies readonly (readonly [keyof T, string])[]` rechaza un nombre de
//      campo que no exista en la interfaz — cubre el campo *sobrante*.
//   2. `exigirCompleto<Faltantes<...>>()` rechaza que quede alguna clave de la
//      interfaz sin declarar — cubre el campo *ausente*.
//
// Sin (2) esta constante sería una lista optativa: alguien añadiría un campo a
// la interfaz, no lo declararía aquí, y nada protestaría.

/** Claves que una tabla de campos declara. */
type ClavesDeclaradas<T extends readonly (readonly [string, string])[]> =
  T[number][0];

/** Claves de `I` que la tabla `T` **no** declara. `never` si están todas. */
type Faltantes<
  I,
  T extends readonly (readonly [string, string])[],
> = Exclude<keyof I, ClavesDeclaradas<T>>;

/**
 * Falla la compilación si el parámetro de tipo no es `never`.
 *
 * Equivalente en TypeScript a la desestructuración exhaustiva de Rust. El error
 * nombra la clave que falta, así que el diagnóstico es directo.
 */
function exigirCompleto<_Faltantes extends never>(): void {
  // Sin cuerpo: toda la comprobación ocurre en el sistema de tipos.
}

/** Campos de [`EstadoAgente`], en el orden del manifiesto. */
export const CAMPOS_ESTADO_AGENTE = [
  ["version", "texto"],
  ["perfil", "enumerado"],
  ["respuestaAutomatica", "booleano"],
] as const satisfies readonly (readonly [keyof EstadoAgente, string])[];

/** Campos de [`NodoInventario`]. */
export const CAMPOS_NODO_INVENTARIO = [
  ["identificador", "texto"],
  ["direccionEnlace", "texto"],
  ["clase", "enumerado"],
  ["postura", "enumerado"],
] as const satisfies readonly (readonly [keyof NodoInventario, string])[];

/** Campos de [`EstadoBoveda`]. */
export const CAMPOS_ESTADO_BOVEDA = [
  ["usadoBytes", "entero"],
  ["limiteBytes", "entero"],
  ["eventosPendientes", "entero"],
] as const satisfies readonly (readonly [keyof EstadoBoveda, string])[];

/** Campos de la petición de `consultar-sandbox`. */
export const CAMPOS_PETICION_CONSULTA = [["sentencia", "texto"]] as const;

/** Campos de [`ResultadoConsulta`]. */
export const CAMPOS_RESULTADO_CONSULTA = [
  ["columnas", "lista<texto>"],
  ["filas", "lista<lista<texto>>"],
] as const satisfies readonly (readonly [keyof ResultadoConsulta, string])[];

// Comprobación de exhaustividad. Estas cuatro llamadas no producen código: si
// una interfaz gana un campo y su tabla no lo declara, `tsc` falla aquí.
// `PeticionConsulta` no tiene interfaz propia — es el argumento de
// `consultarSandbox`— y por eso no figura.
exigirCompleto<Faltantes<EstadoAgente, typeof CAMPOS_ESTADO_AGENTE>>();
exigirCompleto<Faltantes<NodoInventario, typeof CAMPOS_NODO_INVENTARIO>>();
exigirCompleto<Faltantes<EstadoBoveda, typeof CAMPOS_ESTADO_BOVEDA>>();
exigirCompleto<Faltantes<ResultadoConsulta, typeof CAMPOS_RESULTADO_CONSULTA>>();

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
