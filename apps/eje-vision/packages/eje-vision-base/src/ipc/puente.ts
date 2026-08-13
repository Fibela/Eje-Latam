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
 * Clase de suceso de alerta.
 *
 * Una sola variante a propósito: de los tres centinelas de RPT-019 §1, sólo uno
 * es un suceso. Los otros dos son condiciones y viajan en {@link Condiciones}.
 */
export type ClaseAlerta = "amenazaIncontenible";

/**
 * Suceso de alerta ya anexado a ALM-01.
 *
 * Se llama `SucesoAlerta` y no `Alerta` porque VIS-04 ya tiene su propio tipo
 * `Alerta`, y no son lo mismo: aquel es **algo que mostrar**, con severidad;
 * este es **el registro de un hecho**. La colision de nombres fue el sintoma de
 * que «alerta» era ambiguo entre las dos capas.
 */
export interface SucesoAlerta {
  /** Asiento de ALM-01 que la contiene. */
  readonly asiento: number;
  /** Qué ocurrió. */
  readonly clase: ClaseAlerta;
  /** Dispositivo implicado. */
  readonly dispositivo: string;
  /** Contexto para el operador. */
  readonly detalle: string;
}

/**
 * Estados degradados vigentes.
 *
 * Son verdaderos hasta que alguien interviene, así que no se anexan al registro:
 * se consultan. Anotarlos repetidamente inundaría ALM-01 con la misma noticia
 * (RPT-019 §2).
 */
export interface Condiciones {
  /** Había inventario y ya no está (RPT-017 §2). */
  readonly inventarioSuprimido: boolean;
  /** El inventario está presente y no supera la verificación. */
  readonly inventarioNoVerifica: boolean;
  /** La mitad pegajosa del almacén de observación se llenó (RPT-018 §6). */
  readonly observacionSaturada: boolean;
  /** La captura perdió tramas y la vista de la red está incompleta. */
  readonly capturaConPerdida: boolean;
  /**
   * **No hay captura en absoluto**: este sensor no está observando.
   *
   * RPT-047, PA-81. Distinta de `capturaConPerdida`: aquélla dice que la vista
   * es incompleta, ésta dice que no hay vista. VIS-04 no debe presentarlas
   * juntas — un operador que las confunda concluirá que en ese segmento no pasó
   * nada, cuando lo cierto es que nadie estaba mirando.
   *
   * Es la condición que debe verse antes que ninguna otra: mientras esté
   * activa, todo lo demás que muestre el panel es de antes.
   *
   * El motivo —privilegios, interfaz inexistente, interfaz desaparecida— no
   * viaja aquí: está en el asiento de ALM-01 que anota la transición.
   */
  readonly capturaNoDisponible: boolean;
  /**
   * El almacén exige una acción del administrador, sin indicio de ataque.
   *
   * Cubre `FormatoObsoleto` (RPT-022) y `SinClaveAprovisionada` (RPT-024): dos
   * estados que **exigen alerta y no son manipulación**. Su remedio es reemitir
   * o aprovisionar, no responder a un incidente.
   *
   * VIS-04 debe presentarlo distinto de `inventarioSuprimido` y
   * `inventarioNoVerifica`. Mezclarlos produce la fatiga de alertas que la
   * Fase 1 de PA-45 existía para evitar.
   */
  readonly accionAdministrativa: boolean;
  /**
   * El colector de syslog no responde y la alerta no sale del equipo.
   *
   * **Es la única condición que no viaja también por syslog**: emitirla exigiría
   * el canal que acaba de fallar. Llega sólo por este puente.
   *
   * VIS-04 debe mostrarla de forma prominente: mientras esté activa, el SIEM del
   * cliente **no se está enterando de nada**, y esta interfaz es lo único que lo
   * sabe.
   */
  readonly salidaNoDisponible: boolean;
  /**
   * El registro de evidencia está lleno y **ya no admite alertas**.
   *
   * RPT-039, PA-72. Es la más grave de las siete: las otras seis dicen que algo
   * se ve peor, ésta dice que este sensor ha dejado de registrar amenazas.
   *
   * VIS-04 no debe presentarlo como manipulación —nadie tocó nada— pero tampoco
   * como un aviso que pueda esperar. La libreta se acabó.
   */
  readonly registroSaturado: boolean;
  /**
   * Hay alertas anexadas que **sólo viven en memoria**.
   *
   * RPT-044, PA-69. La escritura a disco falló: las alertas existen y están
   * completas, lo que no está es su durabilidad. Un corte de luz se las lleva.
   *
   * VIS-04 debe distinguirlo de `registroSaturado`: allí el sensor dejó de
   * anotar; aquí anota y no consigue guardar.
   */
  readonly evidenciaEnRiesgo: boolean;
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

  /**
   * VIS-04 — sucesos de alerta desde un asiento, exclusivo.
   *
   * Es una **consulta**, no una suscripción. RPT-019 §4 descarta el empuje: el
   * agente no inicia comunicación hacia el renderer, porque eso sería una
   * capacidad nueva y no un mensaje nuevo.
   */
  consultarAlertas(desdeAsiento: number): Promise<RespuestaAlertas>;

  /** VIS-04 — estados degradados vigentes. */
  obtenerCondiciones(): Promise<Condiciones>;
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

/** Campos de la petición de `consultar-alertas`. */
export const CAMPOS_PETICION_ALERTAS = [["desdeAsiento", "entero"]] as const;

/** Campos de [`SucesoAlerta`]. */
export const CAMPOS_SUCESO_ALERTA = [
  ["asiento", "entero"],
  ["clase", "enumerado"],
  ["dispositivo", "texto"],
  ["detalle", "texto"],
] as const satisfies readonly (readonly [keyof SucesoAlerta, string])[];

/** Campos de [`Condiciones`]. */
/**
 * Respuesta de `consultar-alertas`.
 *
 * RPT-041, PA-74. Antes era un array desnudo. Tras la segmentación de PA-59 eso
 * se convirtió en una vista parcial con apariencia de exhaustividad: quien pedía
 * `desdeAsiento: 0` recibía el segmento activo sin forma de saber que había
 * asientos archivados antes.
 *
 * VIS-04 debe presentar `primerDisponible > 1` como «el histórico anterior está
 * archivado», nunca como «no hay más». Son cosas distintas y confundirlas es
 * como un operador concluye que un incidente no ocurrió.
 */
export interface RespuestaAlertas {
  /** Asiento más antiguo que sobrevive en disco. */
  readonly primerDisponible: number;
  /**
   * **La respuesta se cortó**: hay asientos posteriores sin entregar.
   *
   * RPT-049, PA-96. `primerDisponible` dice qué falta por detrás; esto dice qué
   * falta por delante. VIS-04 no puede presentar la lista como el histórico
   * entero mientras esto sea `true`.
   */
  readonly hayMas: boolean;
  /** Alertas de esta respuesta, desde `desdeAsiento` exclusive. */
  readonly sucesos: readonly SucesoAlerta[];
}

export const CAMPOS_RESPUESTA_ALERTAS = [
  ["primerDisponible", "entero"],
  ["hayMas", "booleano"],
  ["sucesos", "lista"],
] as const satisfies readonly (readonly [keyof RespuestaAlertas, string])[];

exigirCompleto<Faltantes<RespuestaAlertas, typeof CAMPOS_RESPUESTA_ALERTAS>>();

export const CAMPOS_CONDICIONES = [
  ["inventarioSuprimido", "booleano"],
  ["inventarioNoVerifica", "booleano"],
  ["observacionSaturada", "booleano"],
  ["capturaConPerdida", "booleano"],
  ["capturaNoDisponible", "booleano"],
  ["accionAdministrativa", "booleano"],
  ["salidaNoDisponible", "booleano"],
  ["registroSaturado", "booleano"],
  ["evidenciaEnRiesgo", "booleano"],
] as const satisfies readonly (readonly [keyof Condiciones, string])[];

// Comprobación de exhaustividad. Estas llamadas no producen código: si una
// interfaz gana un campo y su tabla no lo declara, `tsc` falla aquí.
// `PeticionConsulta` y `PeticionAlertas` no tienen interfaz propia —son
// argumentos de sus métodos— y por eso no figuran.
exigirCompleto<Faltantes<EstadoAgente, typeof CAMPOS_ESTADO_AGENTE>>();
exigirCompleto<Faltantes<NodoInventario, typeof CAMPOS_NODO_INVENTARIO>>();
exigirCompleto<Faltantes<EstadoBoveda, typeof CAMPOS_ESTADO_BOVEDA>>();
exigirCompleto<Faltantes<ResultadoConsulta, typeof CAMPOS_RESULTADO_CONSULTA>>();
exigirCompleto<Faltantes<SucesoAlerta, typeof CAMPOS_SUCESO_ALERTA>>();
exigirCompleto<Faltantes<Condiciones, typeof CAMPOS_CONDICIONES>>();

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
