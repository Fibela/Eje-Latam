/**
 * El bloque de sucesos. RPT-048 §2 y §4.
 *
 * # La pregunta que este módulo contesta
 *
 * «¿Puedo decirle al operador que en este segmento no ha pasado nada?»
 *
 * Casi nunca. Una lista de alertas vacía significa que **no hay alertas en lo
 * que este sensor conserva a mano**, y eso no es lo mismo que decir que no
 * hubo ninguna. Tras una rotación del registro (PA-59) el histórico anterior
 * está archivado en disco y no viaja en la respuesta.
 *
 * `primerDisponible` existe para eso (RPT-041, PA-74): dice desde qué asiento
 * empieza lo que se entrega. Si vale más de 1, hay asientos antes que esta
 * respuesta no incluye — y presentarla como exhaustiva es cómo un operador
 * concluye que un incidente no ocurrió.
 */

import type { RespuestaAlertas, SucesoAlerta } from "../ipc/puente.js";

/** Lo que el bloque de sucesos puede afirmar, y lo que no. */
export interface VistaSucesos {
  /** Lo entregado, lo más reciente primero. */
  readonly sucesos: readonly SucesoAlerta[];
  /** Hay asientos anteriores archivados que esta respuesta no trae. */
  readonly hayHistoricoArchivado: boolean;
  /**
   * La respuesta se cortó: hay asientos **posteriores** sin entregar.
   *
   * RPT-049, PA-97. El agente devuelve como mucho lo que cabe en un marco, y
   * eso puede ser 256 de dos mil. Presentarlos como el histórico entero es el
   * mismo error que `hayHistoricoArchivado` evita, en la otra dirección.
   */
  readonly hayMasRecientes: boolean;
  /**
   * Si la ausencia de alertas puede leerse como «no ha pasado nada».
   *
   * Sólo cuando la lista está vacía **y** no hay nada archivado antes. Es la
   * única combinación en la que el silencio es una observación y no un hueco.
   */
  readonly puedeAfirmarQueNoHubo: boolean;
  /** Frase para el operador. Vacía cuando no hay salvedad que hacer. */
  readonly salvedad: string;
}

/**
 * Compone el bloque de sucesos a partir de la respuesta del agente.
 *
 * @param respuesta Lo que devolvió `consultar-alertas`.
 */
export function componerSucesos(respuesta: RespuestaAlertas): VistaSucesos {
  const { primerDisponible, hayMas, sucesos } = respuesta;

  // `primerDisponible` es el asiento más antiguo que sobrevive en disco. Vale 1
  // cuando no se ha archivado nada; cualquier valor mayor significa que hubo
  // rotación y que lo anterior no está aquí.
  const hayHistoricoArchivado = primerDisponible > 1;

  // Ordenado de lo más reciente a lo más antiguo. El número de asiento es
  // monótono (RPT-039 §3), así que ordena mejor que cualquier marca de tiempo.
  const ordenados = [...sucesos].sort((uno, otro) => otro.asiento - uno.asiento);

  const vacia = ordenados.length === 0;

  // Sólo se puede afirmar que no hubo alertas cuando no falta nada por ningún
  // lado: ni archivado detrás, ni cortado delante.
  if (vacia && !hayHistoricoArchivado && !hayMas) {
    return {
      sucesos: ordenados,
      hayHistoricoArchivado,
      hayMasRecientes: hayMas,
      puedeAfirmarQueNoHubo: true,
      salvedad: "",
    };
  }

  // Los dos huecos pueden darse a la vez, y entonces se dicen los dos: el
  // operador está viendo una ventana con algo fuera por delante y por detrás.
  //
  // El texto cambia según haya algo que mostrar o no. «Se muestran las alertas
  // desde el asiento N» con la lista vacía sería una frase que se contradice
  // sola, y una frase que no se sostiene no la lee nadie.
  const avisos: string[] = [];
  if (hayHistoricoArchivado) {
    avisos.push(
      vacia
        ? `Sin alertas desde el asiento ${primerDisponible}. Las anteriores ` +
          "están archivadas en este equipo y no se muestran aquí."
        : `Se muestran las alertas desde el asiento ${primerDisponible}; ` +
          "hay histórico anterior archivado en este equipo.",
    );
  }
  if (hayMas) {
    avisos.push(
      "Hay alertas más recientes que no caben en esta consulta. " +
        "Sigue consultando para verlas.",
    );
  }

  return {
    sucesos: ordenados,
    hayHistoricoArchivado,
    hayMasRecientes: hayMas,
    puedeAfirmarQueNoHubo: false,
    salvedad: avisos.join(" "),
  };
}
