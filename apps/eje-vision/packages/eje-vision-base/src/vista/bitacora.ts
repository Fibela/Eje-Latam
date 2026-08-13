/**
 * Bitácora de alertas: acumula entre consultas en vez de repetirlas. PA-98.
 *
 * # Por qué existe
 *
 * `consultar-alertas` acepta `desdeAsiento` **exclusivo** desde RPT-019, y hasta
 * ahora nadie lo usaba: la consola pedía siempre desde cero y se traía el
 * histórico entero en cada refresco.
 *
 * Medido (RPT-050): con 256 alertas de detalle largo, la respuesta ocupa 1 MB y
 * el agente tarda **40 ms en serializarla** antes de mandar el primer byte. El
 * agente tiene un solo hilo: esos 40 ms no salen de la nada, salen del tiempo
 * que debería estar observando la red.
 *
 * Un panel que refresca cada dos segundos le estaría robando al sensor un 2% de
 * su vigilancia para reenviarle lo que ya tiene. Con cursor, la respuesta en
 * régimen son 55 bytes.
 *
 * **No es una optimización de red.** El cable movía el megabyte en 1,8 ms sin
 * inmutarse. Es una salvaguarda de la observación.
 *
 * # El peligro que introduce el cursor
 *
 * Si la marca está en el asiento 100, el agente rota, y `primerDisponible` pasa
 * a 5000, pedir «desde 100» devuelve desde el 5000 — y los asientos 101 a 4999
 * **desaparecen sin que nada lo diga**.
 *
 * El cursor convierte un hueco visible en un hueco silencioso: exactamente el
 * error de PA-74, creado por la propia optimización que lo evitaba. Por eso esta
 * bitácora comprueba su continuidad en cada incorporación.
 */

import type { RespuestaAlertas, SucesoAlerta } from "../ipc/puente.js";

/**
 * Alertas que se conservan en memoria.
 *
 * El registro en disco es la fuente; esto es una ventana para mostrar. Sin cota,
 * un panel abierto una semana acumularía sin límite — el agotamiento de memoria
 * de RPT-018 §6 con otro nombre.
 */
export const SUCESOS_EN_MEMORIA = 500;

/** Estado acumulado entre consultas. */
export interface Bitacora {
  /** Lo conocido, lo más reciente primero, acotado a {@link SUCESOS_EN_MEMORIA}. */
  readonly sucesos: readonly SucesoAlerta[];
  /** Qué pedir la próxima vez. Exclusivo: cero significa «desde el principio». */
  readonly desdeAsiento: number;
  /**
   * Entre dos consultas se archivaron asientos que esta consola no llegó a ver.
   *
   * **Pegajoso**: una vez cierto, no vuelve a falso. El hueco no se cierra
   * porque la siguiente consulta vaya bien; sigue habiendo alertas que este
   * panel nunca mostró, y decir lo contrario sería peor que no avisar.
   */
  readonly huboSalto: boolean;
  /** El primer asiento que este panel se saltó, si hubo salto. */
  readonly saltoDesde: number;
  /** El agente cortó la respuesta: conviene volver a preguntar ya. */
  readonly hayMasRecientes: boolean;
  /** Hay histórico archivado anterior a lo que este panel ha visto. */
  readonly hayHistoricoArchivado: boolean;
}

/** Bitácora inicial: no se sabe nada y se pide desde el principio. */
export const BITACORA_INICIAL: Bitacora = Object.freeze({
  sucesos: [],
  desdeAsiento: 0,
  huboSalto: false,
  saltoDesde: 0,
  hayMasRecientes: false,
  hayHistoricoArchivado: false,
});

/**
 * Incorpora una respuesta a la bitácora.
 *
 * Pura: devuelve una bitácora nueva y no toca la anterior. Eso permite probar
 * secuencias largas y hace imposible que un refresco a medias deje el estado en
 * un punto intermedio.
 */
export function incorporar(
  previa: Bitacora,
  respuesta: RespuestaAlertas,
): Bitacora {
  const { primerDisponible, hayMas, sucesos } = respuesta;

  // La detección del salto se hace ANTES de mezclar nada. Después de mezclar, la
  // marca ya avanzó y el hueco es indistinguible de una consulta normal.
  //
  // Sólo aplica si esta consola ya había visto algo: en la primera consulta,
  // `primerDisponible > 1` es histórico archivado —que nunca fue suyo— y no un
  // salto. Confundirlos acusaría de pérdida a un panel recién abierto.
  const yaHabiaVisto = previa.desdeAsiento > 0;
  const saltoAhora = yaHabiaVisto && primerDisponible > previa.desdeAsiento + 1;

  const nuevos = sucesos.filter((cada) => cada.asiento > previa.desdeAsiento);

  const combinados = [...nuevos, ...previa.sucesos]
    .sort((uno, otro) => otro.asiento - uno.asiento)
    .slice(0, SUCESOS_EN_MEMORIA);

  // La marca avanza al asiento más alto visto, nunca retrocede. Si la respuesta
  // llega vacía, se conserva: pedir desde más atrás traería lo mismo otra vez.
  const masAlto = nuevos.reduce(
    (mayor, cada) => Math.max(mayor, cada.asiento),
    previa.desdeAsiento,
  );

  return {
    sucesos: combinados,
    desdeAsiento: masAlto,
    huboSalto: previa.huboSalto || saltoAhora,
    saltoDesde: previa.huboSalto
      ? previa.saltoDesde
      : saltoAhora
        ? previa.desdeAsiento + 1
        : 0,
    hayMasRecientes: hayMas,
    hayHistoricoArchivado: primerDisponible > 1,
  };
}

/**
 * Cuánto esperar antes de volver a consultar, en milisegundos.
 *
 * # Los números salen de una medida, no de una intuición
 *
 * RPT-050: el agente atiende **al final de cada vuelta**, y la vuelta dura unos
 * 500 ms. Por debajo de eso no hay nada nuevo que pedir — el agente contestaría
 * lo mismo. Peor: el sondeo secuencial aterriza siempre al principio de una
 * vuelta nueva y paga la vuelta **entera**, nunca la media.
 *
 * Cuando el agente dice `hayMas`, es lo contrario: hay cola pendiente y esperar
 * sólo la alarga. Se vuelve a preguntar en cuanto se pueda.
 */
export function esperaSugerida(bitacora: Bitacora): number {
  return bitacora.hayMasRecientes ? 0 : 2_000;
}
