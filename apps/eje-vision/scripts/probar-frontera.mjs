/**
 * Prueba negativa del guardián de frontera.
 *
 * Introduce violaciones deliberadas, comprueba que dependency-cruiser las
 * detecta, y restaura los ficheros.
 *
 * ## Por qué existe
 *
 * Un guardián que nunca falla no sirve, y no hay forma de saber que está vivo sin
 * provocarlo. La primera versión de `.dependency-cruiser.cjs` excluía `dist` del
 * grafo; como los paquetes hermanos se resuelven a través de su `dist`, la regla
 * crítica quedó inerte y **pasaba en verde con la violación presente**. Se
 * descubrió solo por esta prueba.
 *
 * Ejecutar:  node scripts/probar-frontera.mjs
 */

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..");

const OBJETIVOS = [
  "packages/eje-vision-base/src",
  "packages/eje-vision-empresarial/src",
  "proceso-principal/src",
];

/**
 * Ejecuta dependency-cruiser y devuelve las reglas violadas.
 *
 * @returns {{ codigo: number, reglas: string[], salida: string }}
 */
function cruzar() {
  try {
    const salida = execFileSync(
      process.platform === "win32" ? "npx.cmd" : "npx",
      ["depcruise", ...OBJETIVOS, "--config", ".dependency-cruiser.cjs"],
      { cwd: RAIZ, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
    );
    return { codigo: 0, reglas: [], salida };
  } catch (error) {
    const salida = `${error.stdout ?? ""}${error.stderr ?? ""}`;
    const reglas = [...salida.matchAll(/error\s+([a-z0-9-]+):/gu)].map(
      (coincidencia) => coincidencia[1],
    );
    return { codigo: error.status ?? 1, reglas: [...new Set(reglas)], salida };
  }
}

const CASOS = [
  {
    nombre: "la capa base importa de la capa empresarial",
    reglaEsperada: "base-no-importa-empresarial",
    fichero: "packages/eje-vision-base/src/indice.ts",
    inyectar: (original) =>
      `${original}\nexport { impactoVacio } from "@eje/vision-empresarial";\n`,
  },
  {
    nombre: "una vista del renderer importa un builtin de Node",
    reglaEsperada: "renderer-no-usa-builtins-de-node",
    fichero: "packages/eje-vision-base/src/vistas/vis-01-consola-almacen/indice.ts",
    inyectar: (original) =>
      `import { readFileSync } from "node:fs";\nvoid readFileSync;\n${original}`,
  },
  {
    nombre: "la capa base importa del proceso principal",
    reglaEsperada: "base-no-importa-proceso-principal",
    fichero: "packages/eje-vision-base/src/indice.ts",
    inyectar: (original) =>
      `${original}\nexport { capacidades } from "@eje/vision-proceso-principal";\n`,
  },
];

let fallos = 0;

const verde = (t) => `[32m${t}[0m`;
const rojo = (t) => `[31m${t}[0m`;
const gris = (t) => `[90m${t}[0m`;

console.log(gris("Comprobando que el arbol limpio no tiene violaciones..."));
const limpio = cruzar();
if (limpio.codigo !== 0) {
  console.log(rojo("FALLA  el arbol limpio ya presenta violaciones:"));
  console.log(limpio.salida);
  process.exit(1);
}
console.log(`${verde("PASA")}   arbol limpio sin violaciones\n`);

for (const caso of CASOS) {
  const ruta = join(RAIZ, caso.fichero);
  const original = readFileSync(ruta, "utf8");

  try {
    writeFileSync(ruta, caso.inyectar(original), "utf8");
    const resultado = cruzar();

    if (resultado.codigo === 0) {
      console.log(`${rojo("FALLA")}  ${caso.nombre}`);
      console.log(
        gris(
          `       el guardian NO detecto la violacion; se esperaba '${caso.reglaEsperada}'`,
        ),
      );
      fallos += 1;
    } else if (!resultado.reglas.includes(caso.reglaEsperada)) {
      console.log(`${rojo("FALLA")}  ${caso.nombre}`);
      console.log(
        gris(
          `       se detecto algo, pero no la regla esperada '${caso.reglaEsperada}'.` +
            ` Reglas activadas: ${resultado.reglas.join(", ") || "(ninguna identificada)"}`,
        ),
      );
      fallos += 1;
    } else {
      console.log(`${verde("PASA")}   ${caso.nombre}`);
      console.log(gris(`       regla activada: ${caso.reglaEsperada}`));
    }
  } finally {
    writeFileSync(ruta, original, "utf8");
  }
}

console.log();
if (fallos > 0) {
  console.log(rojo(`${fallos} caso(s) sin detectar. El guardian de frontera no protege.`));
  process.exit(1);
}
console.log(verde("El guardian de frontera detecta todas las violaciones probadas."));
