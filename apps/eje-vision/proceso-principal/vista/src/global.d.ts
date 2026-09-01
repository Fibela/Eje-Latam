/**
 * El puente que el preload expone, tipado. RPT-091, PA-142.
 *
 * # Esto es lo que paga el punto entero
 *
 * Hasta hoy `window.eje` era `any` en el renderer, porque el renderer no lo
 * compilaba nadie. Escribir `window.eje.obtenerInvenatrio()` no daba error en
 * ninguna parte: el fallo esperaba a que un operador abriera la ventana.
 *
 * `PuenteEje` esta atado a `contrato-ipc.toml` por la prueba de paridad, y esa
 * al struct de Rust por desestructuracion exhaustiva. Al declararlo aqui, la
 * cadena llega hasta el ultimo centimetro de la capa visual: **un campo que el
 * agente no manda deja de compilar la vista**.
 *
 * # Por que `.d.ts` y no un import
 *
 * `declare global` solo vale en un modulo, y el fichero de declaraciones no
 * emite JavaScript. Anadirlo a `vis04.ts` obligaria a que ese fichero fuera un
 * modulo aumentando el ambito global, que es mas ruido del que hace falta.
 */
import type { PuenteEje } from "../../../packages/eje-vision-base/dist/indice.js";

declare global {
  interface Window {
    /**
     * Expuesto por `preload.cts` con `contextBridge` (RPT-004 §6.1).
     *
     * `readonly` porque el renderer no puede sustituirlo: si pudiera, un script
     * inyectado reemplazaria el puente por uno propio.
     */
    readonly eje: PuenteEje;
  }
}
