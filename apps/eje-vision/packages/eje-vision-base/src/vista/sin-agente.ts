/**
 * Qué lee un operador cuando no hay sensor. RPT-048, PA-93.
 *
 * # El problema
 *
 * `enlace.ts` distingue con precisión por qué no hubo respuesta, y lo dice con
 * frases como «el socket existe y no hay nadie escuchando». Eso es exactamente
 * lo que un técnico necesita en un registro, y no significa nada para quien
 * vigila una planta o una planta de hospitalización.
 *
 * # Y por qué no se resuelve escribiendo dos mensajes
 *
 * Traducirlo en `enlace.ts` perdería el diagnóstico forense. Traducirlo en cada
 * panel haría que cada panel lo reinventara.
 *
 * Así que **una sola fuente decide la causa** —el código que viaja en el
 * mensaje— y aquí se traduce a primer plano, conservando el detalle técnico
 * detrás. El operador lee una frase; el técnico despliega y encuentra la línea
 * entera, sin recortar.
 */

/** Lo que se enseña cuando no se pudo hablar con el agente. */
export interface LecturaSinAgente {
  /** Una frase, en presente, sin jerga. Lo único que se lee de lejos. */
  readonly titulo: string;
  /** Qué puede hacer quien está delante. Vacío si no puede hacer nada. */
  readonly sugerencia: string;
  /** El mensaje original, entero. Para el registro y para desplegar. */
  readonly detalleTecnico: string;
  /**
   * Si reintentar por sí solo tiene sentido.
   *
   * `false` cuando hace falta que alguien intervenga: sin permiso sobre el
   * socket, reintentar cada dos segundos durante horas no lo va a arreglar y
   * además esconde el problema detrás de un mensaje de «reintentando».
   */
  readonly reintentable: boolean;
}

/** Extrae el código de causa que `enlace.ts` antepone entre corchetes. */
function causaDe(detalle: string): string {
  const coincidencia = /^\[([a-z-]+)\]/.exec(detalle);
  return coincidencia?.[1] ?? "";
}

/**
 * Traduce un fallo de enlace a algo legible.
 *
 * Nunca inventa: si la causa no se reconoce, lo dice — no la disfraza de
 * problema conocido. Una traducción que adivina es peor que ninguna, porque el
 * operador actúa sobre ella.
 */
export function leerSinAgente(detalle: string): LecturaSinAgente {
  const base = { detalleTecnico: detalle } as const;

  switch (causaDe(detalle)) {
    case "sin-escucha":
    case "sin-socket":
    case "no-abre":
      return {
        ...base,
        titulo: "Sensor desconectado",
        sugerencia:
          "El servicio de vigilancia no está en marcha en este equipo. " +
          "Avisa a soporte si no vuelve por sí solo.",
        reintentable: true,
      };

    case "sin-permiso":
      // No es reintentable: reintentar cada dos segundos no concede permisos, y
      // «reintentando…» escondería que hace falta una intervención.
      return {
        ...base,
        titulo: "Esta consola no tiene acceso al sensor",
        sugerencia:
          "El sensor está en marcha y no autoriza a este usuario. " +
          "Hace falta que un administrador lo revise.",
        reintentable: false,
      };

    case "sin-respuesta":
      // El caso más engañoso: el sensor está vivo. Decir «desconectado» seria
      // mandar a alguien a arrancar algo que ya corre.
      return {
        ...base,
        titulo: "El sensor no responde",
        sugerencia:
          "El servicio está en marcha pero no contesta. " +
          "Si persiste, avisa a soporte: no es una desconexión.",
        reintentable: true,
      };

    case "colgado":
      return {
        ...base,
        titulo: "Se perdió la conexión con el sensor",
        sugerencia: "Reconectando.",
        reintentable: true,
      };

    default:
      return {
        ...base,
        titulo: "No se pudo consultar al sensor",
        sugerencia: "Consulta el detalle técnico o avisa a soporte.",
        reintentable: true,
      };
  }
}
