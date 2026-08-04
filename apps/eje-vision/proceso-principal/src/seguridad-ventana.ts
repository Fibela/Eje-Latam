/**
 * Configuración de seguridad de la ventana de Electron.
 *
 * Implementa RPT-004 §6.1. Se declara como constante congelada para que la
 * configuración sea un dato inspeccionable y verificable en pruebas, en lugar de
 * un objeto literal disperso en la creación de ventanas.
 */

/**
 * Subconjunto de `WebPreferences` de Electron cuyo valor es obligatorio.
 *
 * Se declara localmente en lugar de importar los tipos de Electron para que este
 * módulo sea verificable sin la dependencia completa.
 */
export interface PreferenciasSeguridad {
  /** Aísla el contexto del preload del de la página. */
  readonly contextIsolation: true;
  /** El renderer no accede a Node. */
  readonly nodeIntegration: false;
  /** El renderer corre en sandbox del sistema operativo. */
  readonly sandbox: true;
  /** No se desactiva bajo ninguna circunstancia. */
  readonly webSecurity: true;
  /** Sin integración de Node en workers. */
  readonly nodeIntegrationInWorker: false;
  /** Sin integración de Node en subframes. */
  readonly nodeIntegrationInSubFrames: false;
}

/**
 * Preferencias obligatorias de toda ventana de Eje-Visión.
 *
 * Los tipos son literales, no `boolean`: escribir `contextIsolation: false` es un
 * error de compilación, no una decisión que llegue a revisión de código.
 */
export const PREFERENCIAS_SEGURIDAD: PreferenciasSeguridad = Object.freeze({
  contextIsolation: true,
  nodeIntegration: false,
  sandbox: true,
  webSecurity: true,
  nodeIntegrationInWorker: false,
  nodeIntegrationInSubFrames: false,
});

/**
 * Política de seguridad de contenido aplicada a la interfaz.
 *
 * Sin `unsafe-inline` ni `unsafe-eval`. Todo el contenido se sirve desde disco
 * local: la interfaz no carga recursos remotos (RPT-004 §6.1).
 */
export const CSP = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self'",
  "img-src 'self' data:",
  "font-src 'self'",
  "connect-src 'none'",
  "object-src 'none'",
  "frame-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join("; ");

/**
 * Destinos externos que `shell.openExternal` puede abrir.
 *
 * Un enlace recibido por telemetría o mostrado en un evento no se abre sin
 * validación de destino.
 */
export const DESTINOS_EXTERNOS_PERMITIDOS: readonly string[] = Object.freeze([
  "https://premoscorp.com/",
  "https://docs.premoscorp.com/",
]);

/**
 * Indica si una URL puede abrirse en el navegador del sistema.
 *
 * @param url URL solicitada.
 * @returns `true` solo si el destino está explícitamente permitido.
 */
export function destinoExternoPermitido(url: string): boolean {
  let analizada: URL;
  try {
    analizada = new URL(url);
  } catch {
    return false;
  }

  if (analizada.protocol !== "https:") {
    return false;
  }

  return DESTINOS_EXTERNOS_PERMITIDOS.some((permitido) =>
    url.startsWith(permitido),
  );
}
