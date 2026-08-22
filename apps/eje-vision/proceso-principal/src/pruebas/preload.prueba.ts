/**
 * El preload es un cuarto sitio donde vive el contrato. Ésta es su atadura.
 *
 * RPT-046, PA-20. Un preload sandboxeado no puede cargar `@eje/vision-base`
 * —es ESM— así que la lista de canales está **escrita a mano** en `preload.cts`.
 * No hay forma de importarla; la única defensa es leer el fuente y compararlo.
 *
 * Mismo mecanismo que PA-75 usa con `puente.ts`, y por el mismo motivo: lo que
 * no se compara, diverge.
 */

import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import { CANALES_PERMITIDOS, CANALES_PROHIBIDOS } from "../puente-ipc.js";

import { sinComentarios } from "./lexico.js";

/**
 * Fuente del preload.
 *
 * Tres niveles desde `dist/pruebas` hasta la raíz del paquete, y de ahí a `src`.
 * Anclado en `import.meta.url` y no en `process.cwd()`: el directorio de trabajo
 * depende de desde dónde se invoque npm, y esa fragilidad ya costó una suite
 * entera sin ejecutar.
 */
function fuenteDelPreload(): string {
  const aqui = dirname(fileURLToPath(import.meta.url));
  const ruta = join(aqui, "..", "..", "src", "preload.cts");

  try {
    return readFileSync(ruta, "utf8");
  } catch (error) {
    throw new Error(`no se pudo leer ${ruta}: ${String(error)}`);
  }
}

/** Canales que el preload invoca de verdad. Sólo código. */
function canalesInvocados(fuente: string): string[] {
  return [...sinComentarios(fuente).matchAll(/ipcRenderer\.invoke\(\s*"([^"]+)"/g)].map(
    (coincidencia) => coincidencia[1] ?? "",
  );
}

describe("RPT-046 — el preload no puede divergir del guardián", () => {
  it("invoca exactamente los canales permitidos, sin faltar ni sobrar", () => {
    const invocados = canalesInvocados(fuenteDelPreload());

    assert.deepEqual(
      [...invocados].sort(),
      [...CANALES_PERMITIDOS].sort(),
      "el preload y CANALES_PERMITIDOS ya no dicen lo mismo",
    );
  });

  it("cada canal se invoca una sola vez", () => {
    // Dos métodos apuntando al mismo canal significa que uno está mal escrito y
    // devuelve datos de otra cosa — un fallo que en pantalla parece «datos raros».
    const invocados = canalesInvocados(fuenteDelPreload());

    assert.equal(new Set(invocados).size, invocados.length);
  });

  it("no aparece ningún canal prohibido", () => {
    const fuente = fuenteDelPreload();

    for (const prohibido of CANALES_PROHIBIDOS) {
      assert.ok(
        !canalesInvocados(fuente).includes(prohibido),
        `el preload invoca el canal prohibido '${prohibido}'`,
      );
    }
  });

  it("no existe un pasamanos genérico", () => {
    // RPT-004 §6.2: un `invoke(canal, args)` con el canal como dato traslada la
    // autorización al renderer. Aquí se comprueba que el argumento de cada
    // invocación es un literal y no una variable.
    const fuente = sinComentarios(fuenteDelPreload());
    const invocaciones = [...fuente.matchAll(/ipcRenderer\.invoke\(\s*([^,)]+)/g)];

    assert.ok(invocaciones.length > 0, "no se encontró ninguna invocación");

    for (const invocacion of invocaciones) {
      const primerArgumento = invocacion[1] ?? "";
      assert.ok(
        primerArgumento.trim().startsWith('"'),
        `el canal llega como dato y no como literal: ${primerArgumento}`,
      );
    }
  });

  it("se expone por contextBridge y no asignando a window", () => {
    // Con `contextIsolation: true` una asignación directa no llegaría al
    // renderer. Que no llegue es lo correcto; lo peligroso sería desactivar el
    // aislamiento para que funcionara.
    const fuente = sinComentarios(fuenteDelPreload());

    assert.match(fuente, /contextBridge\.exposeInMainWorld\(/);
    assert.doesNotMatch(fuente, /\bwindow\.\w+\s*=/);
  });

  it("el preload es CommonJS y no existe una versión en módulos ES", () => {
    // Si alguien lo renombra a `.ts`, Electron falla al cargar el preload bajo
    // `sandbox: true`, y eso no lo detecta ninguna prueba que no arranque un
    // escritorio. Se comprueba el directorio, no una constante: una prueba que
    // afirme que ".cts".endsWith(".cts") es cierta y no verifica nada.
    const src = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "src");
    const ficheros = readdirSync(src);

    assert.ok(ficheros.includes("preload.cts"), "falta preload.cts");
    assert.ok(
      !ficheros.includes("preload.ts"),
      "existe un preload.ts: bajo sandbox, Electron no puede cargarlo",
    );
  });
});

describe("RPT-046 — el lector de fuentes ignora los comentarios", () => {
  // Estas pruebas existen porque la primera versión de la suite se leyó a sí
  // misma. El fallo fue en la dirección inofensiva; el simétrico —un ejemplo
  // comentado que hace pasar una paridad rota— no habría fallado nunca.

  it("un canal citado en un comentario no cuenta como invocado", () => {
    const fingido =
      '// ejemplo: ipcRenderer.invoke("ordenar-contencion")\n' +
      '/* tampoco ipcRenderer.invoke("invocar") */\n' +
      'ipcRenderer.invoke("obtener-condiciones");';

    const codigo = sinComentarios(fingido);

    assert.ok(codigo.includes("obtener-condiciones"));
    assert.ok(!codigo.includes("ordenar-contencion"), "comentario de línea");
    assert.ok(!codigo.includes("invocar"), "comentario de bloque");
  });

  it("una barra doble dentro de una cadena no abre un comentario", () => {
    // Si el lexer no distingue cadenas, se comería el resto de la línea y
    // dejaría de ver código real que viene después.
    const codigo = sinComentarios('const u = "https://ejemplo"; const v = 1;');

    assert.ok(codigo.includes("https://ejemplo"));
    assert.ok(codigo.includes("const v = 1"), "se perdió el código posterior");
  });

  it("el propio preload sigue citando el antipatrón en un comentario", () => {
    // Guardia sobre la corrección: el comentario que causó el fallo se
    // conserva a propósito, porque documenta la decisión. Si alguien lo borra
    // «para que pase la prueba», esta prueba dice que ese no era el arreglo.
    const fuente = fuenteDelPreload();

    assert.match(fuente, /window\.\w+\s*=/, "el ejemplo comentado desapareció");
    assert.doesNotMatch(sinComentarios(fuente), /window\.\w+\s*=/);
  });
});
