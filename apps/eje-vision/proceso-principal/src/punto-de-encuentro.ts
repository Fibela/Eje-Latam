/**
 * Dónde encuentra la consola al agente.
 *
 * RPT-079 §2.1, PA-132.
 *
 * # Por qué esto es un módulo y no una línea en `arranque.ts`
 *
 * Estaba ahí, y ahí es donde se rompió. `arranque.ts` importa Electron, así que
 * ninguna prueba podía leer esa constante sin levantar un escritorio — y una
 * constante que ninguna prueba puede mirar es una constante que se queda atrás
 * sin que nadie se entere.
 *
 * Es lo que pasó: el agente movió su socket a `/run/eje-latam` en RPT-067
 * (PA-120) y esto se quedó en `/run/eje`. **Con los valores de fábrica, un
 * sensor sano y una consola sana no se encontraban.** Nadie lo vio porque los
 * guiones de desarrollo pasan `EJE_SOCKET` a mano y tapaban el agujero: el
 * defecto sólo aparecía en un despliegue de verdad, que es donde no hay nadie
 * mirando.
 *
 * Aquí no se importa nada. Ni Electron, ni `node:fs`. Eso es lo que permite que
 * `contrato.prueba.ts` compare este valor con `contrato-ipc.toml`.
 *
 * # Y por qué el valor no se lee del manifiesto en tiempo de ejecución
 *
 * `contrato-ipc.toml` vive en el repositorio, no en la máquina del cliente. Una
 * consola instalada que fuera a buscarlo no lo encontraría, y caer a un valor
 * por omisión al no encontrarlo devolvería exactamente el problema que esto
 * cierra. El manifiesto es la fuente de verdad **en tiempo de prueba**; en
 * tiempo de ejecución la fuente es esta constante, y la prueba garantiza que
 * digan lo mismo.
 */

/**
 * Directorio volátil donde el agente abre su socket, por omisión.
 *
 * `systemd` lo crea con `RuntimeDirectory=` y lo destruye al parar. `/run` es
 * `tmpfs`: se vacía en cada arranque, con lo que el socket huérfano —el fichero
 * que sobrevive al proceso y hace que el cliente reciba `ECONNREFUSED` sobre
 * algo que existe— deja de ser posible por construcción (RPT-067, PA-120).
 */
export const DIRECTORIO_SOCKET = "/run/eje-latam";

/**
 * Nombre del fichero. **No** es configurable en el agente, a propósito.
 *
 * Se puede mover el directorio, nunca el fichero. Si la ruta completa fuera
 * configurable, nada impediría apuntarla de vuelta al directorio de evidencia y
 * deshacer la separación de RPT-067 sin que ninguna comprobación se enterase.
 */
export const NOMBRE_SOCKET = "agente.sock";

/** El punto de encuentro de fábrica, compuesto. */
export const RUTA_SOCKET_POR_OMISION = `${DIRECTORIO_SOCKET}/${NOMBRE_SOCKET}`;

/**
 * Dónde buscar al agente en esta ejecución.
 *
 * `EJE_SOCKET` existe para el desarrollo sin privilegios: crear `/run/eje-latam`
 * exige root, y obligar a `sudo` para levantar la consola de diagnóstico haría
 * que nadie la levantara.
 *
 * **Una cadena vacía no es una ruta.** `EJE_SOCKET=` entrega una variable
 * definida y vacía, y tomarla por un destino haría que la consola intentara
 * abrir `""` y presentara el fallo como «el agente no responde». Es el mismo
 * argumento de `colector_declarado` en el agente (RPT-064, PA-118), un piso más
 * abajo: un destino vacío no es un destino.
 */
export function rutaSocket(
  entorno: NodeJS.ProcessEnv = process.env,
): string {
  const declarada = entorno["EJE_SOCKET"]?.trim();
  return declarada === undefined || declarada === ""
    ? RUTA_SOCKET_POR_OMISION
    : declarada;
}
