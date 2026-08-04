/**
 * Guardián del puente IPC.
 *
 * Implementa RPT-004 §6.2 y §6.3. El proceso principal valida cada petición antes
 * de reenviarla a `eje-agente`.
 *
 * ## Transporte
 *
 * Socket de dominio Unix con ACL en Linux y macOS; named pipe con descriptor de
 * seguridad en Windows. **Sin puerto TCP local** (RPT-002 §9.3): un servicio en
 * `localhost` es alcanzable por cualquier proceso local y por cualquier página
 * que el usuario visite.
 */

/**
 * Canales permitidos, en correspondencia uno a uno con los métodos de `PuenteEje`.
 *
 * Esta lista es la autoridad. Un canal que no figure aquí se rechaza aunque el
 * preload lo invoque.
 */
export const CANALES_PERMITIDOS = [
  "obtener-estado-agente",
  "obtener-inventario",
  "obtener-estado-boveda",
  "consultar-sandbox",
] as const;

/** Canal admitido por el puente. */
export type CanalPermitido = (typeof CANALES_PERMITIDOS)[number];

/**
 * Indica si un canal está permitido.
 *
 * @param canal Nombre de canal recibido.
 * @returns `true` si figura en la lista de permitidos.
 */
export function esCanalPermitido(canal: string): canal is CanalPermitido {
  return (CANALES_PERMITIDOS as readonly string[]).includes(canal);
}

/**
 * Canales que jamás deben existir en el puente.
 *
 * No se trata de una lista de bloqueo —la autoridad es la lista de permitidos—
 * sino de una prueba de regresión: si alguien introduce uno de estos nombres, la
 * suite de pruebas lo detecta y explica por qué está prohibido.
 */
export const CANALES_PROHIBIDOS: readonly string[] = Object.freeze([
  // La contencion se decide en `guardian-cc`, en Rust, a partir de telemetria
  // real. Exponerla al renderer permitiria que la interfaz aislara nodos.
  "ordenar-contencion",
  // ALM-01 es de solo anexado. Ninguna ruta de la interfaz puede emitir DDL
  // contra el registro de evidencia (RPT-002 §5).
  "ejecutar-ddl-evidencia",
  // Un pasamanos generico traslada la autorizacion al renderer (RPT-004 §6.2).
  "invocar",
  "ejecutar-comando",
]);

/** Motivo de rechazo de una petición entrante. */
export type MotivoRechazoIpc = "canal-desconocido" | "carga-excesiva";

/** Resultado de validar una petición del renderer. */
export type ValidacionPeticion =
  | { readonly admitida: true; readonly canal: CanalPermitido }
  | { readonly admitida: false; readonly motivo: MotivoRechazoIpc };

/**
 * Tamaño máximo admitido para la carga útil de una petición.
 *
 * Acota el consumo del proceso principal ante un renderer comprometido.
 */
export const CARGA_MAXIMA_BYTES = 1_048_576;

/**
 * Valida una petición procedente del renderer.
 *
 * @param canal Canal solicitado.
 * @param tamanoCargaBytes Tamaño de la carga útil.
 * @returns Resultado de la validación.
 */
export function validarPeticion(
  canal: string,
  tamanoCargaBytes: number,
): ValidacionPeticion {
  if (!esCanalPermitido(canal)) {
    return { admitida: false, motivo: "canal-desconocido" };
  }

  if (tamanoCargaBytes > CARGA_MAXIMA_BYTES) {
    return { admitida: false, motivo: "carga-excesiva" };
  }

  return { admitida: true, canal };
}
