/**
 * Prueba negativa del guardián de frontera.
 *
 * RPT-006. Verifica que `dependency-cruiser` detecta las violaciones que debe
 * detectar, y que el propio script distingue los tres estados posibles.
 *
 * # Tri-estática de verificación
 *
 * Todo verificador debe modelar **tres** estados y no permitir que colapsen
 * entre sí:
 *
 * | Estado | Significado |
 * |---|---|
 * | `Conforme` | Se ejecutó el análisis y no hay violaciones |
 * | `ViolacionDetectada` | Se ejecutó el análisis y hay violaciones |
 * | `ComprobacionImposible` | **No se comprobó nada** |
 *
 * El tercero es el que se olvida, y es el más peligroso: colapsado en el
 * primero produce falsos verdes; colapsado en el segundo manda a buscar
 * problemas inexistentes. Este guardián sufrió ambos colapsos antes de que la
 * distinción fuera explícita:
 *
 * 1. `exclude` de `dist` dejaba la regla crítica inerte — verde con violación.
 * 2. `npx.cmd` no arrancaba en Windows — se reportó como violación de frontera.
 * 3. Una expresión regular no reconocía la salida — se declaró "no protege"
 *    cuando protegía.
 * 4. Con salida JSON el código de salida es 0 aun con violaciones — se declaró
 *    "no detectó" cuando sí detectaba.
 *
 * Los cuatro comparten raíz: el guardián no podía decir con precisión qué había
 * visto. Por eso el estado es ahora un dato explícito, no algo que el llamante
 * infiere de un código de salida.
 *
 * Ejecutar:  node scripts/probar-frontera.mjs
 */

import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..");

const OBJETIVOS = [
  "packages/eje-vision-base/src",
  "packages/eje-vision-empresarial/src",
  "proceso-principal/src",
];

/**
 * Punto de entrada de dependency-cruiser dentro de node_modules.
 *
 * Se invoca con `process.execPath` en lugar de con `npx`. Desde la corrección de
 * CVE-2024-27980, Node se niega a lanzar ficheros `.cmd` o `.bat` mediante
 * `execFile` sin `shell: true`, de modo que `npx.cmd` lanzaba excepción en
 * Windows antes siquiera de ejecutar la herramienta.
 */
const DEPCRUISE = join(
  RAIZ,
  "node_modules",
  "dependency-cruiser",
  "bin",
  "dependency-cruise.mjs",
);

/** Los tres estados posibles de una comprobación. */
const ESTADO = Object.freeze({
  CONFORME: "Conforme",
  VIOLACION: "ViolacionDetectada",
  IMPOSIBLE: "ComprobacionImposible",
});

/**
 * Extrae las reglas violadas del informe JSON de dependency-cruiser.
 *
 * Se consume salida estructurada y no el informe legible: el formato para
 * humanos puede cambiar entre versiones, plataformas o ajustes de color, y no
 * constituye un contrato.
 *
 * @param {string} textoJson
 * @returns {{ reglas: string[], detalle: string[] }}
 */
function interpretar(textoJson) {
  const informe = JSON.parse(textoJson);

  if (!Array.isArray(informe?.summary?.violations)) {
    throw new Error("el informe no contiene 'summary.violations'");
  }

  const violaciones = informe.summary.violations;
  const reglas = violaciones.map((v) => v?.rule?.name ?? "(sin nombre)");
  const detalle = violaciones.map(
    (v) => `${v?.rule?.name ?? "?"}: ${v?.from ?? "?"} -> ${v?.to ?? "?"}`,
  );

  return { reglas: [...new Set(reglas)], detalle };
}

/**
 * Ejecuta dependency-cruiser y clasifica el resultado en uno de los tres estados.
 *
 * @param {string} binario Ruta del ejecutable, parametrizable para poder probar
 *   el estado `ComprobacionImposible`.
 * @returns {{ estado: string, reglas: string[], salida: string, codigo: number }}
 */
function cruzar(binario = DEPCRUISE) {
  if (!existsSync(binario)) {
    return {
      estado: ESTADO.IMPOSIBLE,
      reglas: [],
      salida: `no se encontro dependency-cruiser en ${binario}\nEjecute: npm ci`,
      codigo: -1,
    };
  }

  const argumentos = [
    binario,
    ...OBJETIVOS,
    "--config",
    ".dependency-cruiser.cjs",
    "--output-type",
    "json",
  ];
  const opciones = {
    cwd: RAIZ,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 32 * 1024 * 1024,
  };

  let bruto = "";
  let codigo = 0;

  try {
    bruto = execFileSync(process.execPath, argumentos, opciones);
  } catch (error) {
    // `status` numerico significa que el proceso corrio y termino. Su ausencia
    // significa que ni siquiera se pudo lanzar.
    if (typeof error.status !== "number") {
      return {
        estado: ESTADO.IMPOSIBLE,
        reglas: [],
        salida: `no se pudo lanzar dependency-cruiser: ${error.message}`,
        codigo: -1,
      };
    }
    bruto = `${error.stdout ?? ""}`;
    codigo = error.status;
  }

  try {
    const { reglas, detalle } = interpretar(bruto);
    return {
      estado: reglas.length > 0 ? ESTADO.VIOLACION : ESTADO.CONFORME,
      reglas,
      salida: detalle.length > 0 ? detalle.join("\n") : "(sin violaciones)",
      codigo,
    };
  } catch (error) {
    // La herramienta corrio pero su salida no es interpretable. NO se puede
    // afirmar que no haya violaciones ni que las haya: no se comprobo nada.
    return {
      estado: ESTADO.IMPOSIBLE,
      reglas: [],
      salida:
        `salida no interpretable: ${error.message}\n` +
        `--- recibido (2000 primeros caracteres) ---\n${bruto.slice(0, 2000)}`,
      codigo,
    };
  }
}

// ---------------------------------------------------------------------------
// Casos que deben producir ViolacionDetectada
// ---------------------------------------------------------------------------

const CASOS_VIOLACION = [
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

// ---------------------------------------------------------------------------
// Casos que deben producir ComprobacionImposible
// ---------------------------------------------------------------------------
//
// Sin estos casos, la rama de imposibilidad seria fe y no garantia: existiria en
// el codigo sin que nada comprobase que sigue existiendo. Cubren las dos formas
// en que aparece — la herramienta no arranca, y la herramienta arranca pero
// responde algo que no se puede interpretar. La segunda es la traicionera: el
// proceso termina con exito aparente.

const CASOS_IMPOSIBLE = [
  {
    nombre: "la herramienta no esta instalada",
    preparar: () => ({
      binario: join(RAIZ, "node_modules", "no-existe", "inexistente.mjs"),
      limpiar: () => {},
    }),
  },
  {
    nombre: "la herramienta responde algo que no es JSON",
    preparar: () => {
      const carpeta = mkdtempSync(join(tmpdir(), "eje-frontera-"));
      const binario = join(carpeta, "impostor.mjs");
      // Termina con exito y escribe basura: el caso mas enganoso, porque un
      // guardian descuidado lo leeria como "sin violaciones".
      writeFileSync(binario, 'console.log("no soy json");\n', "utf8");
      return {
        binario,
        limpiar: () => rmSync(carpeta, { recursive: true, force: true }),
      };
    },
  },
];

// ---------------------------------------------------------------------------

let fallos = 0;

const verde = (t) => `[32m${t}[0m`;
const rojo = (t) => `[31m${t}[0m`;
const gris = (t) => `[90m${t}[0m`;

const sangrar = (texto) =>
  texto
    .trim()
    .split("\n")
    .map((linea) => `       ${linea}`)
    .join("\n");

console.log(gris("Comprobando que el arbol limpio no tiene violaciones..."));
const limpio = cruzar();

if (limpio.estado === ESTADO.IMPOSIBLE) {
  console.log(rojo("ERROR  la comprobacion no pudo realizarse."));
  console.log(gris("       Esto NO significa que haya violaciones de frontera,"));
  console.log(gris("       ni que no las haya: significa que no se comprobo nada."));
  console.log(sangrar(limpio.salida));
  process.exit(1);
}

if (limpio.estado === ESTADO.VIOLACION) {
  console.log(rojo("FALLA  el arbol limpio ya presenta violaciones:"));
  console.log(sangrar(limpio.salida));
  process.exit(1);
}

console.log(`${verde("PASA")}   arbol limpio ${gris("(Conforme)")}\n`);

for (const caso of CASOS_VIOLACION) {
  const ruta = join(RAIZ, caso.fichero);
  const original = readFileSync(ruta, "utf8");

  try {
    writeFileSync(ruta, caso.inyectar(original), "utf8");
    const resultado = cruzar();

    if (resultado.estado === ESTADO.IMPOSIBLE) {
      console.log(`${rojo("ERROR")}  ${caso.nombre}`);
      console.log(gris("       no se comprobo nada; no se afirma ni se niega la violacion"));
      console.log(sangrar(resultado.salida));
      fallos += 1;
    } else if (resultado.estado === ESTADO.CONFORME) {
      console.log(`${rojo("FALLA")}  ${caso.nombre}`);
      console.log(
        gris(`       el guardian NO detecto la violacion; se esperaba '${caso.reglaEsperada}'`),
      );
      fallos += 1;
    } else if (!resultado.reglas.includes(caso.reglaEsperada)) {
      console.log(`${rojo("FALLA")}  ${caso.nombre}`);
      console.log(
        gris(
          `       se detecto algo, pero no la regla esperada '${caso.reglaEsperada}'.` +
            ` Reglas activadas: ${resultado.reglas.join(", ")}`,
        ),
      );
      console.log(sangrar(resultado.salida));
      fallos += 1;
    } else {
      console.log(`${verde("PASA")}   ${caso.nombre}`);
      console.log(gris(`       ViolacionDetectada: ${caso.reglaEsperada}`));
    }
  } finally {
    writeFileSync(ruta, original, "utf8");
  }
}

console.log();

for (const caso of CASOS_IMPOSIBLE) {
  const { binario, limpiar } = caso.preparar();

  try {
    const resultado = cruzar(binario);

    if (resultado.estado === ESTADO.IMPOSIBLE) {
      console.log(`${verde("PASA")}   ${caso.nombre}`);
      console.log(gris("       ComprobacionImposible, como debe ser"));
    } else {
      console.log(`${rojo("FALLA")}  ${caso.nombre}`);
      console.log(
        gris(
          `       se reporto '${resultado.estado}' cuando no se comprobo nada.\n` +
            `       Un estado de imposibilidad colapsado en Conforme produce falsos verdes;\n` +
            `       colapsado en ViolacionDetectada manda a buscar problemas inexistentes.`,
        ),
      );
      fallos += 1;
    }
  } finally {
    limpiar();
  }
}

console.log();
if (fallos > 0) {
  console.log(rojo(`${fallos} caso(s) sin detectar. El guardian de frontera no protege.`));
  process.exit(1);
}
console.log(
  verde("El guardian distingue los tres estados y detecta todas las violaciones probadas."),
);
