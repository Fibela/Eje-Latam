/**
 * Punto de entrada de Electron.
 *
 * RPT-046. Sólo arranca y falla ruidosamente. Toda la decisión está en
 * `arranque.ts`, y toda la lógica verificable, en los módulos que éste usa.
 */

import { arrancar } from "./arranque.js";

arrancar().catch((error: unknown) => {
  // A `stderr` y con código distinto de cero: un arranque fallido que termina en
  // silencio con código 0 es un servicio que el supervisor cree haber levantado.
  process.stderr.write(`Eje-Visión no pudo arrancar.\n${String(error)}\n`);
  process.exitCode = 1;
});
