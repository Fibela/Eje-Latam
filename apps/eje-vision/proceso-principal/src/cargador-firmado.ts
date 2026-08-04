/**
 * Cargador de módulos con verificación de firma.
 *
 * Implementa RPT-004 §5.
 *
 * ## Por qué existe este fichero
 *
 * La carga dinámica desde disco es una **superficie de inyección de código en el
 * proceso principal de Electron**, que es el mismo que se comunica con
 * `eje-agente` y puede solicitar órdenes de contención sobre la red del cliente.
 * Quien pueda escribir en el directorio de módulos obtiene ejecución con ese
 * privilegio.
 *
 * Los *fuses* de integridad de `asar` protegen el archivo principal empaquetado;
 * **no cubren módulos externos cargados en tiempo de ejecución**.
 *
 * ## Fallo cerrado
 *
 * Firma ausente, ilegible o inválida ⇒ el módulo no se carga. El criterio es el
 * mismo que rige `SIMULATION_ONLY` en RPT-003 §8.1: ante marca ausente o
 * inválida, no se actúa.
 */

import { createHash, verify } from "node:crypto";

/** Motivo por el que un paquete fue rechazado. */
export type MotivoRechazo =
  | "firma-ausente"
  | "firma-invalida"
  | "resumen-no-coincide"
  | "manifiesto-malformado"
  | "sin-licencia-previa";

/** Resultado de la verificación de un paquete empresarial. */
export type ResultadoVerificacion =
  | { readonly admitido: true }
  | { readonly admitido: false; readonly motivo: MotivoRechazo };

/** Manifiesto que acompaña al paquete empresarial. */
export interface ManifiestoPaquete {
  /** Nombre del paquete. */
  readonly nombre: string;
  /** Versión del paquete. */
  readonly version: string;
  /** Resumen SHA-256 del contenido, en hexadecimal. */
  readonly resumenSha256: string;
}

/**
 * Serializa el manifiesto de forma canónica.
 *
 * El orden de campos es fijo y explícito: firmar `JSON.stringify` de un objeto
 * dejaría la firma a merced del orden de inserción de claves.
 *
 * @param manifiesto Manifiesto a serializar.
 * @returns Bytes canónicos sobre los que se calcula la firma.
 */
export function serializarManifiesto(manifiesto: ManifiestoPaquete): Buffer {
  const canonico = [
    `nombre=${manifiesto.nombre}`,
    `version=${manifiesto.version}`,
    `resumen=${manifiesto.resumenSha256}`,
  ].join("\n");
  return Buffer.from(canonico, "utf8");
}

/**
 * Calcula el resumen SHA-256 del contenido de un paquete.
 *
 * @param contenido Bytes del paquete.
 * @returns Resumen en hexadecimal.
 */
export function resumirContenido(contenido: Buffer): string {
  return createHash("sha256").update(contenido).digest("hex");
}

/**
 * Verifica un paquete empresarial antes de permitir su carga.
 *
 * Comprueba, en este orden: que el nodo fue licenciado alguna vez, que el
 * manifiesto está bien formado, que la firma Ed25519 es válida sobre el
 * manifiesto canónico, y que el resumen declarado coincide con el contenido real.
 *
 * @param manifiesto Manifiesto declarado por el paquete.
 * @param firma Firma Ed25519 del manifiesto canónico.
 * @param clavePublica Clave pública de PremosCorp en formato SPKI DER o PEM.
 * @param contenido Bytes reales del paquete.
 * @param fueLicenciado Si el nodo tuvo licencia en algún momento.
 * @returns Resultado de la verificación.
 */
export function verificarPaquete(
  manifiesto: ManifiestoPaquete,
  firma: Buffer | null,
  clavePublica: Buffer | string,
  contenido: Buffer,
  fueLicenciado: boolean,
): ResultadoVerificacion {
  if (!fueLicenciado) {
    return { admitido: false, motivo: "sin-licencia-previa" };
  }

  if (firma === null || firma.length === 0) {
    return { admitido: false, motivo: "firma-ausente" };
  }

  if (
    manifiesto.nombre.length === 0 ||
    manifiesto.version.length === 0 ||
    manifiesto.resumenSha256.length !== 64
  ) {
    return { admitido: false, motivo: "manifiesto-malformado" };
  }

  const canonico = serializarManifiesto(manifiesto);

  let firmaValida = false;
  try {
    firmaValida = verify(null, canonico, clavePublica, firma);
  } catch {
    // Clave malformada o firma de longitud incorrecta. Se trata como invalida:
    // ante cualquier duda, no se carga.
    return { admitido: false, motivo: "firma-invalida" };
  }

  if (!firmaValida) {
    return { admitido: false, motivo: "firma-invalida" };
  }

  if (resumirContenido(contenido) !== manifiesto.resumenSha256) {
    return { admitido: false, motivo: "resumen-no-coincide" };
  }

  return { admitido: true };
}

/**
 * Indica si el directorio de módulos es admisible como origen de carga.
 *
 * Debe residir en una ubicación no escribible por el usuario sin elevación. Un
 * directorio bajo el perfil del usuario permite que cualquier proceso con sus
 * privilegios inyecte código en el proceso principal.
 *
 * @param ruta Ruta absoluta del directorio de módulos.
 * @param rutasDeInstalacion Prefijos considerados de instalación en el sistema.
 * @returns `true` si la ruta está bajo un prefijo de instalación.
 */
export function directorioAdmisible(
  ruta: string,
  rutasDeInstalacion: readonly string[],
): boolean {
  const normalizada = ruta.replace(/\\/gu, "/").toLowerCase();
  return rutasDeInstalacion.some((prefijo) =>
    normalizada.startsWith(prefijo.replace(/\\/gu, "/").toLowerCase()),
  );
}
