/**
 * Los tres estados de un panel. RPT-048 §1.
 *
 * # Por qué tres y no dos
 *
 * Cuatro de los seis canales rechazan hoy con motivo: están declarados y no
 * tienen manejador en el agente. Un panel con sólo «datos» y «vacío» pinta ese
 * rechazo como un segmento sin dispositivos, y le dice al operador que ahí no
 * hay nada.
 *
 * Es la mentira de RPT-036 §6: «no hay nada» y «esto no lo sirve nadie todavía»
 * no son lo mismo. La primera es una observación; la segunda es una parte del
 * producto que aún no existe.
 *
 * Se construye ahora y no cuando lleguen los manejadores, porque después habría
 * que rehacer cada panel — y porque el estado que se añade tarde es el que nadie
 * comprueba.
 *
 * # Esta capa no toca Electron
 *
 * Vive en `eje-vision-base` a propósito: es lógica pura y se prueba sin ventana,
 * sin agente y sin escritorio.
 */

/** Qué se está mostrando, y por qué. */
export type EstadoPanel<T> =
  /** Aún no se ha recibido la primera respuesta. */
  | { readonly clase: "consultando" }
  /** Hay datos y no están vacíos. */
  | { readonly clase: "datos"; readonly valor: T }
  /** El agente respondió, y lo que hay es nada. Es una observación. */
  | { readonly clase: "vacio" }
  /** El agente rechazó: este canal no lo sirve nadie todavía. */
  | { readonly clase: "noServido"; readonly motivo: string }
  /** No se pudo hablar con el agente. */
  | { readonly clase: "sinAgente"; readonly detalle: string };

/**
 * Prefijo con el que el proceso principal envuelve un rechazo del agente.
 *
 * ## Deuda declarada
 *
 * Reconocer un rechazo por el texto de un `Error` es frágil, y no es la forma
 * correcta: el primer salto (`ipcRenderer.invoke`) convierte cualquier fallo en
 * una excepción y **pierde la forma** del rechazo, que en el cable sí era un
 * dato con su código y su motivo.
 *
 * Lo correcto sería que el manejador devolviera la unión discriminada en lugar
 * de lanzar. Cambia el contrato de los seis canales, así que se anota como
 * **PA-94** y no se hace de paso.
 *
 * Mientras tanto esta constante está en un solo sitio, y una prueba comprueba
 * que coincide con lo que compone el proceso principal.
 */
export const PREFIJO_RECHAZO = "el agente rechazó";

/**
 * Clasifica una respuesta que llegó bien.
 *
 * @param valor Lo que devolvió el puente.
 * @param estaVacio Qué significa «vacío» para este panel. No se adivina: una
 *   lista vacía y un objeto con ceros no se parecen en nada.
 */
export function conDatos<T>(
  valor: T,
  estaVacio: (valor: T) => boolean,
): EstadoPanel<T> {
  return estaVacio(valor) ? { clase: "vacio" } : { clase: "datos", valor };
}

/**
 * Clasifica un fallo.
 *
 * Distingue el rechazo —el agente está y contestó que no sirve ese canal— de no
 * poder hablar con él. Los dos acaban en pantalla y se arreglan distinto: uno
 * espera a que exista el módulo, el otro a que arranque el sensor.
 */
export function desdeFallo<T>(error: unknown): EstadoPanel<T> {
  const texto =
    error instanceof Error ? error.message : String(error ?? "fallo sin motivo");

  if (texto.startsWith(PREFIJO_RECHAZO)) {
    // Se conserva el mensaje entero, con el motivo del agente dentro. Recortarlo
    // para que «quede bonito» es cómo se pierde el dato que RPT-036 §6 puso ahí.
    return { clase: "noServido", motivo: texto };
  }

  return { clase: "sinAgente", detalle: texto };
}

/** Una lista sin elementos está vacía. El caso más común. */
export function listaVacia<T>(valor: readonly T[]): boolean {
  return valor.length === 0;
}

/**
 * Indica si el panel puede afirmar algo sobre el mundo.
 *
 * `false` en los tres estados que no son una observación. Sirve para que la
 * interfaz no escriba «0 dispositivos» donde lo cierto es «no se sabe».
 */
export function esObservacion<T>(estado: EstadoPanel<T>): boolean {
  return estado.clase === "datos" || estado.clase === "vacio";
}
