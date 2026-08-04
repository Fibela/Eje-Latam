/**
 * Frontera open-core de Eje-Vision, verificada automaticamente.
 *
 * RPT-004 §7. La frontera no puede depender de la disciplina de quien
 * contribuye: "limites fisicos de archivos y package.json" no impide que alguien
 * anada una dependencia prohibida. Este fichero es el equivalente npm de lo que
 * `cargo deny` hace en el lado Rust.
 *
 * Ejecutar:  npm run verificar:frontera
 *
 * ---------------------------------------------------------------------------
 * ADVERTENCIA PARA QUIEN MODIFIQUE ESTE FICHERO
 * ---------------------------------------------------------------------------
 *
 * `options.exclude` elimina el modulo del grafo POR COMPLETO. Un modulo excluido
 * no puede ser detectado por ninguna regla, ni siquiera como destino prohibido.
 *
 * Una version anterior excluia `dist`, y como los paquetes hermanos se resuelven
 * a traves de su `dist`, la regla critica `base-no-importa-empresarial` quedo
 * inerte: pasaba en verde con la violacion presente. Se detecto unicamente
 * porque la prueba negativa es obligatoria.
 *
 * Para acotar el recorrido usese `doNotFollow`, que deja el modulo visible como
 * destino pero no lo atraviesa. Y tras cualquier cambio aqui, EJECUTAR LA PRUEBA
 * NEGATIVA:  node scripts/probar-frontera.mjs
 */

/** Capa base, Apache-2.0. */
const BASE = "^packages/eje-vision-base";

/**
 * Capa empresarial, propietaria.
 *
 * Se contemplan las dos formas en que puede resolverse un paquete del workspace:
 * la ruta real (npm y pnpm con enlaces simbolicos) y la ruta bajo `node_modules`
 * (instalaciones sin enlace). Depender de una sola dejaria la regla a merced del
 * gestor de paquetes, y PA-12 sigue abierto.
 */
const EMPRESARIAL =
  "(^packages/eje-vision-empresarial|node_modules/@eje/vision-empresarial)";

/** Proceso principal de Electron, con capacidades privilegiadas. */
const PROCESO_PRINCIPAL =
  "(^proceso-principal|node_modules/@eje/vision-proceso-principal)";

module.exports = {
  forbidden: [
    {
      name: "base-no-importa-empresarial",
      comment:
        "CRITICO. La capa base es Apache-2.0. Importar codigo propietario aqui " +
        "contamina el paquete abierto y rompe la frontera ratificada en RPT-003 §2.7.",
      severity: "error",
      from: { path: BASE },
      to: { path: EMPRESARIAL },
    },
    {
      name: "base-no-importa-proceso-principal",
      comment:
        "La capa base se ejecuta en el renderer. Importar del proceso principal " +
        "arrastraria capacidades privilegiadas al contexto de la interfaz.",
      severity: "error",
      from: { path: BASE },
      to: { path: PROCESO_PRINCIPAL },
    },
    {
      name: "empresarial-no-importa-proceso-principal",
      comment: "Misma razon que la regla anterior, en la capa empresarial.",
      severity: "error",
      from: { path: EMPRESARIAL },
      to: { path: PROCESO_PRINCIPAL },
    },
    {
      name: "renderer-no-usa-builtins-de-node",
      comment:
        "CRITICO. RPT-004 §6.1: nodeIntegration esta desactivado y el renderer " +
        "corre en sandbox. Un import de 'node:fs' o similar en una vista indica " +
        "que alguien asume acceso privilegiado que no existe, o que pretende " +
        "obtenerlo.",
      severity: "error",
      from: { path: [BASE, EMPRESARIAL] },
      to: { dependencyTypes: ["core"] },
    },
    {
      name: "sin-dependencias-circulares",
      comment:
        "Un ciclo entre modulos hace imposible razonar sobre el orden de carga " +
        "y sobre que codigo acaba en cada artefacto.",
      severity: "error",
      from: {},
      to: { circular: true },
    },
    {
      name: "sin-devdependencies-en-produccion",
      comment:
        "Una devDependency importada desde codigo de produccion falla en el " +
        "instalador del cliente, no en CI.",
      severity: "error",
      from: {
        path: [BASE, EMPRESARIAL, PROCESO_PRINCIPAL],
        pathNot: "(^|/)pruebas/",
      },
      to: { dependencyTypes: ["npm-dev"] },
    },
    {
      name: "sin-modulos-no-resueltos",
      comment:
        "Un import que no resuelve es un fallo en tiempo de ejecucion esperando " +
        "a ocurrir.",
      severity: "error",
      from: {},
      to: { couldNotResolve: true },
    },
  ],

  options: {
    // `doNotFollow` deja el modulo VISIBLE como destino sin atravesarlo.
    // No usar `exclude` aqui: vease la advertencia de cabecera.
    doNotFollow: { path: "node_modules" },
    tsPreCompilationDeps: true,
    tsConfig: { fileName: "tsconfig.base.json" },
    enhancedResolveOptions: {
      exportsFields: ["exports"],
      conditionNames: ["import", "require", "node", "default", "types"],
      extensions: [".ts", ".js"],
    },
    reporterOptions: {
      text: { highlightFocused: true },
    },
  },
};
