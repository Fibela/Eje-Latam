/**
 * Paridad del contrato IPC con `contrato-ipc.toml`.
 *
 * RPT-006, PA-20.
 *
 * # Por qué esta prueba es obligatoria
 *
 * Rust y TypeScript no pueden compartir tipos. `crates/eje-ipc` valida su
 * `enum Canal` contra el manifiesto; **sin esta prueba, solo la mitad del
 * mecanismo existiría** y el lado TypeScript podría volver a divergir en
 * silencio, que es exactamente cómo llegamos aquí.
 *
 * Es el mismo patrón que `probar-frontera.mjs`: una comprobación ejecutable en
 * lugar de una convención.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  CAMPOS_CONDICIONES,
  CAMPOS_RESPUESTA_ALERTAS,
  CAMPOS_ESTADO_AGENTE,
  CAMPOS_ESTADO_BOVEDA,
  CAMPOS_NODO_INVENTARIO,
  CAMPOS_PETICION_ALERTAS,
  CAMPOS_PETICION_CONSULTA,
  CAMPOS_RESULTADO_CONSULTA,
  CAMPOS_SUCESO_ALERTA,
} from "@eje/vision-base";
import {
  CANALES_PERMITIDOS,
  CANALES_PROHIBIDOS,
  CARGA_MAXIMA_BYTES,
} from "../puente-ipc.js";
import {
  DIRECTORIO_SOCKET,
  NOMBRE_SOCKET,
  RUTA_SOCKET_POR_OMISION,
  rutaSocket,
} from "../punto-de-encuentro.js";

/** Raíz del repositorio, cuatro niveles por encima de este fichero compilado. */
function rutaManifiesto(): string {
  const aqui = dirname(fileURLToPath(import.meta.url));
  return join(aqui, "..", "..", "..", "..", "..", "contrato-ipc.toml");
}

function manifiesto(): string {
  const ruta = rutaManifiesto();
  try {
    return readFileSync(ruta, "utf8");
  } catch (error) {
    throw new Error(
      `no se pudo leer el manifiesto ${ruta}: ${String(error)}\n` +
        "contrato-ipc.toml es la fuente de verdad del puente y debe estar versionado.",
    );
  }
}

/**
 * Fuente de `puente.ts`, que es donde vive el contrato de la interfaz.
 *
 * Tres niveles: `pruebas` → `dist` → `proceso-principal` → `eje-vision`. El
 * manifiesto necesita cinco porque vive en la raíz del repositorio.
 *
 * Se ancla en `import.meta.url` y **no** en `process.cwd()`: el directorio de
 * trabajo depende de desde dónde se invoque npm, y esa fragilidad ya nos costó
 * una verificación de TypeScript que no llegó a ejecutarse.
 */
function fuenteDelPuente(): string {
  const aqui = dirname(fileURLToPath(import.meta.url));
  const ruta = join(
    aqui,
    "..",
    "..",
    "..",
    "packages",
    "eje-vision-base",
    "src",
    "ipc",
    "puente.ts",
  );
  try {
    return readFileSync(ruta, "utf8");
  } catch (error) {
    throw new Error(`no se pudo leer ${ruta}: ${String(error)}`);
  }
}

/**
 * Respuesta declarada de cada canal, leída del manifiesto.
 *
 * Sólo se recogen los bloques `direccion = "respuesta"`: la petición viaja por
 * los argumentos del método y se comprueba aparte.
 */
function respuestasDeclaradas(): Map<string, string> {
  const formas = new Map<string, string>();
  for (const bloque of manifiesto().split("[[mensaje]]").slice(1)) {
    const canal = /canal\s*=\s*"([^"]+)"/.exec(bloque)?.[1];
    const direccion = /direccion\s*=\s*"([^"]+)"/.exec(bloque)?.[1];
    const forma = /forma\s*=\s*"([^"]+)"/.exec(bloque)?.[1];
    if (canal && forma && direccion === "respuesta") {
      formas.set(canal, forma);
    }
  }
  return formas;
}

/** `consultar-alertas` → `consultarAlertas`. */
function metodoDe(canal: string): string {
  return canal.replace(/-([a-z])/g, (_, letra: string) => letra.toUpperCase());
}

/** `lista<X>` → `readonly X[]`; `X` → `X`. */
function tipoDe(forma: string): string {
  const interior = /^lista<(.+)>$/.exec(forma)?.[1];
  return interior === undefined ? forma : `readonly ${interior}[]`;
}

/** Tipo devuelto por cada método de `PuenteEje`, leído del fuente. */
function retornosDelPuente(): Map<string, string> {
  const fuente = fuenteDelPuente();
  const cuerpo = /export interface PuenteEje \{([\s\S]*?)\n\}/.exec(fuente)?.[1];
  assert.ok(cuerpo, "no se encontro la interfaz PuenteEje en puente.ts");

  const retornos = new Map<string, string>();
  for (const linea of cuerpo.split("\n")) {
    // Los grupos de captura son `string | undefined` con
    // `noUncheckedIndexedAccess`, y la lente tiene razón: una línea que no sea
    // una firma no debe colarse como método sin nombre.
    const [, metodo, retorno] = /^\s*(\w+)\s*\([^)]*\)\s*:\s*Promise<(.+)>;\s*$/.exec(
      linea,
    ) ?? [];
    if (metodo !== undefined && retorno !== undefined) {
      retornos.set(metodo, retorno.trim());
    }
  }
  return retornos;
}

/**
 * Extrae los valores de `nombre = "..."` que siguen a una cabecera de tabla.
 *
 * Analizador deliberadamente simple: el formato lo controla este proyecto y
 * añadir una dependencia TOML solo para esta prueba no se justifica. Es el mismo
 * criterio que en `xtask/src/vectores.rs`.
 */
/**
 * Extrae los bloques `[[campo]]` de un registro, en orden de aparición.
 *
 * Devuelve pares `[nombre, tipo]` comparables con las constantes del código.
 */
function camposDe(contenido: string, registro: string): [string, string][] {
  const campos: [string, string][] = [];
  let dentro = false;
  let actual: { registro?: string; nombre?: string; tipo?: string } | null = null;

  const cerrar = (): void => {
    if (
      actual?.registro === registro &&
      actual.nombre !== undefined &&
      actual.tipo !== undefined
    ) {
      campos.push([actual.nombre, actual.tipo]);
    }
    actual = null;
  };

  for (const linea of contenido.split("\n")) {
    const limpia = linea.trim();

    if (limpia.startsWith("[")) {
      cerrar();
      dentro = limpia === "[[campo]]";
      if (dentro) {
        actual = {};
      }
      continue;
    }
    if (limpia.startsWith("#") || actual === null) {
      continue;
    }

    const coincidencia = /^(registro|nombre|tipo)\s*=\s*"([^"]+)"/u.exec(limpia);
    if (coincidencia?.[1] !== undefined && coincidencia[2] !== undefined) {
      actual[coincidencia[1] as "registro" | "nombre" | "tipo"] = coincidencia[2];
    }
  }
  cerrar();

  return campos;
}

function nombresBajo(contenido: string, cabecera: string): string[] {
  const nombres: string[] = [];
  let dentro = false;

  for (const linea of contenido.split("\n")) {
    const limpia = linea.trim();

    if (limpia.startsWith("[")) {
      dentro = limpia === cabecera;
      continue;
    }
    if (limpia.startsWith("#") || !dentro) {
      continue;
    }

    const coincidencia = /^nombre\s*=\s*"([^"]+)"/u.exec(limpia);
    if (coincidencia?.[1] !== undefined) {
      nombres.push(coincidencia[1]);
    }
  }

  return nombres;
}

describe("PA-20 — paridad con contrato-ipc.toml", () => {
  it("los canales permitidos coinciden con el manifiesto", () => {
    const declarados = nombresBajo(manifiesto(), "[[canal]]");
    const implementados = [...CANALES_PERMITIDOS];

    assert.deepEqual(
      implementados,
      declarados,
      `CANALES_PERMITIDOS y contrato-ipc.toml divergen.\n` +
        `  manifiesto: ${JSON.stringify(declarados)}\n` +
        `  codigo    : ${JSON.stringify(implementados)}\n` +
        `  Anadir un canal exige tocar el manifiesto, crates/eje-ipc y este puente. ` +
        `Esa friccion es deliberada: un canal amplia la superficie de ataque del ` +
        `proceso privilegiado.`,
    );
  });

  it("el orden de los canales coincide con el manifiesto", () => {
    // El orden importa: si un lado reordena, un diff futuro parecerá inocuo
    // cuando en realidad cambió la correspondencia con el enum de Rust.
    const declarados = nombresBajo(manifiesto(), "[[canal]]");
    assert.equal(CANALES_PERMITIDOS.length, declarados.length);
    declarados.forEach((nombre, indice) => {
      assert.equal(CANALES_PERMITIDOS[indice], nombre);
    });
  });

  it("todos los canales prohibidos del manifiesto están recogidos", () => {
    const prohibidos = nombresBajo(manifiesto(), "[[prohibido]]");

    assert.ok(
      prohibidos.length > 0,
      "el manifiesto debe declarar canales prohibidos como prueba de regresion",
    );

    for (const nombre of prohibidos) {
      assert.ok(
        CANALES_PROHIBIDOS.includes(nombre),
        `'${nombre}' esta declarado como prohibido en el manifiesto pero no en CANALES_PROHIBIDOS`,
      );
      assert.ok(
        !(CANALES_PERMITIDOS as readonly string[]).includes(nombre),
        `'${nombre}' esta prohibido y aparece entre los permitidos`,
      );
    }
  });

  it("las cargas útiles coinciden con el manifiesto", () => {
    // PA-21. El manifiesto blinda qué canales existen; esto blinda qué forma
    // tienen sus mensajes, que es la siguiente capa donde ambos extremos pueden
    // divergir sin que nadie se entere hasta que algo llega `undefined`.
    const contenido = manifiesto();

    const registros: [string, readonly (readonly [string, string])[]][] = [
      ["EstadoAgente", CAMPOS_ESTADO_AGENTE],
      ["NodoInventario", CAMPOS_NODO_INVENTARIO],
      ["EstadoBoveda", CAMPOS_ESTADO_BOVEDA],
      ["PeticionConsulta", CAMPOS_PETICION_CONSULTA],
      ["ResultadoConsulta", CAMPOS_RESULTADO_CONSULTA],
      ["PeticionAlertas", CAMPOS_PETICION_ALERTAS],
      ["SucesoAlerta", CAMPOS_SUCESO_ALERTA],
      ["Condiciones", CAMPOS_CONDICIONES],
      ["RespuestaAlertas", CAMPOS_RESPUESTA_ALERTAS],
    ];

    for (const [nombre, implementados] of registros) {
      const declarados = camposDe(contenido, nombre);
      assert.deepEqual(
        declarados,
        implementados.map(([campo, tipo]) => [campo, tipo]),
        `el registro '${nombre}' diverge entre contrato-ipc.toml y el codigo.\n` +
          `  manifiesto: ${JSON.stringify(declarados)}\n` +
          `  codigo    : ${JSON.stringify(implementados)}`,
      );
    }
  });

  it("el límite de carga coincide con el manifiesto", () => {
    // Un límite distinto en cada extremo permite que un lado acepte lo que el
    // otro rechaza, y esa asimetría es explotable.
    const contenido = manifiesto();
    assert.ok(
      contenido.includes(`longitud_maxima = ${CARGA_MAXIMA_BYTES}`),
      `el manifiesto debe declarar 'longitud_maxima = ${CARGA_MAXIMA_BYTES}'`,
    );
  });

  it("el manifiesto declara la forma de cada canal", () => {
    const contenido = manifiesto();
    for (const canal of CANALES_PERMITIDOS) {
      assert.ok(
        contenido.includes(`canal = "${canal}"`),
        `el canal '${canal}' no declara ningun mensaje en el manifiesto`,
      );
    }
  });

  // PA-75. Las pruebas de arriba comprueban que los ESQUEMAS coinciden; ninguna
  // comprobaba que el contrato del puente USE el registro declarado.
  //
  // El caso real: `RespuestaAlertas` se declaró en el manifiesto y en `puente.ts`
  // con sus campos, las dos pruebas de paridad pasaron, y la firma seguía
  // diciendo `Promise<readonly SucesoAlerta[]>`. El tipo existía y no lo usaba
  // nadie — el patrón que más veces ha aparecido en este proyecto.
  it("cada método del puente devuelve la forma que el manifiesto declara", () => {
    const declaradas = respuestasDeclaradas();
    const retornos = retornosDelPuente();

    assert.ok(retornos.size > 0, "no se leyó ninguna firma de PuenteEje");

    for (const canal of CANALES_PERMITIDOS) {
      const forma = declaradas.get(canal);
      assert.ok(forma, `el canal '${canal}' no declara respuesta en el manifiesto`);

      const metodo = metodoDe(canal);
      const devuelve = retornos.get(metodo);
      assert.ok(
        devuelve,
        `PuenteEje no declara el método '${metodo}' para el canal '${canal}'`,
      );

      assert.equal(
        devuelve,
        tipoDe(forma),
        `el canal '${canal}' declara responder '${forma}' y ` +
          `PuenteEje.${metodo} devuelve 'Promise<${devuelve}>'.\n` +
          "Declarar un registro no lo cablea: la firma tiene que usarlo.",
      );
    }
  });

  // La barrera de arriba pasa. Que pase no demuestra que sirva: la divergencia
  // real (`lista<SucesoAlerta>` en el manifiesto contra `RespuestaAlertas` en la
  // firma) ya estaba corregida cuando se escribió, así que **nunca la vio**.
  //
  // Es la misma disciplina de `probar-frontera.mjs`: un guardián que jamás se ha
  // puesto en rojo es un guardián sin probar. Aquí se ejercita la traducción y
  // la comparación con datos fabricados, incluida la divergencia concreta que se
  // nos escapó hoy.
  it("la traducción de canal y forma detecta la divergencia que se nos escapó", () => {
    assert.equal(metodoDe("consultar-alertas"), "consultarAlertas");
    assert.equal(metodoDe("obtener-estado-boveda"), "obtenerEstadoBoveda");
    assert.equal(metodoDe("obtenerCondiciones"), "obtenerCondiciones");

    assert.equal(tipoDe("RespuestaAlertas"), "RespuestaAlertas");
    assert.equal(tipoDe("lista<SucesoAlerta>"), "readonly SucesoAlerta[]");
    assert.equal(tipoDe("lista<NodoInventario>"), "readonly NodoInventario[]");

    // El caso de hoy: el manifiesto decía lista y la firma devolvía el objeto.
    assert.notEqual(
      tipoDe("lista<SucesoAlerta>"),
      "RespuestaAlertas",
      "la comparación debe distinguir un array de su envoltorio",
    );
  });

  // RPT-079 §2.1, PA-132. La mitad TypeScript de la barrera del punto de
  // encuentro; la otra vive en `xtask`, atando el manifiesto a la constante del
  // agente. Media barrera no es una barrera: es exactamente así como la ruta
  // acabó viviendo en tres sitios con dos mal.
  //
  // Lo que este defecto tuvo de particular es que **no se quedó corto, apuntó a
  // otro sitio**. Un índice corto se nota porque falta algo; una ruta que apunta
  // a otro sitio produce un sensor sano, una consola sana y `ECONNREFUSED`.
  it("la consola busca al agente donde el contrato dice que está", () => {
    const contenido = manifiesto();
    const seccion = contenido.split("[socket]")[1];
    assert.ok(seccion, "el contrato debe declarar el punto de encuentro");

    // Se lee bajo `[socket]` y no la primera coincidencia del fichero: el
    // manifiesto tiene `nombre =` en cada canal y en cada campo, y un `match`
    // ingenuo se llevaría el de `obtener-estado-agente`.
    const valorDe = (clave: string): string => {
      for (const linea of seccion.split("\n")) {
        if (linea.trimStart().startsWith("[")) break;
        const encaje = new RegExp(`^${clave}\\s*=\\s*"([^"]*)"`, "u").exec(
          linea.trim(),
        );
        if (encaje?.[1] !== undefined) return encaje[1];
      }
      throw new Error(`[socket] no declara ${clave}`);
    };

    assert.equal(
      DIRECTORIO_SOCKET,
      valorDe("directorio"),
      "el contrato manda a la consola a un directorio y ésta abre otro",
    );
    assert.equal(NOMBRE_SOCKET, valorDe("nombre"));
    assert.equal(
      RUTA_SOCKET_POR_OMISION,
      `${valorDe("directorio")}/${valorDe("nombre")}`,
      "el contrato y la consola no componen la misma ruta",
    );
  });

  // Y la variable de entorno no puede reintroducir el agujero por otra puerta.
  //
  // `EJE_SOCKET=` entrega una variable definida y vacía —systemd y los guiones
  // de shell lo hacen sin querer todo el tiempo— y `?? valor` NO la sustituye,
  // porque `??` sólo mira `undefined` y `null`. La consola intentaría abrir la
  // cadena vacía y presentaría el fallo como «el agente no responde», que es la
  // mentira de PA-118 en otro sitio.
  it("una variable vacía no es un punto de encuentro", () => {
    assert.equal(rutaSocket({}), RUTA_SOCKET_POR_OMISION);
    assert.equal(rutaSocket({ EJE_SOCKET: "" }), RUTA_SOCKET_POR_OMISION);
    assert.equal(rutaSocket({ EJE_SOCKET: "   " }), RUTA_SOCKET_POR_OMISION);
    assert.equal(
      rutaSocket({ EJE_SOCKET: "/tmp/eje/agente.sock" }),
      "/tmp/eje/agente.sock",
      "un destino declarado de verdad tiene que seguir mandando",
    );
  });

  // RPT-081, PA-135. El contrato distingue declarado de cableado, y esta
  // comprobación existe para que esa distinción no se quede sólo en el lado
  // Rust — que es exactamente cómo el punto de encuentro acabó divergiendo.
  //
  // Aquí no se puede llamar al manejador: vive en el agente. Lo que sí se puede
  // afirmar es que **todo canal declara si está servido y por qué no**, que es
  // lo que permite a VIS-04 presentar un panel como «no servido» sin gastar una
  // consulta de medio segundo en descubrirlo (RPT-079 §11.2).
  it("todo canal declara si está servido, y los que no dicen por qué", () => {
    const contenido = manifiesto();
    const bloques = contenido.split("[[canal]]").slice(1);

    assert.equal(
      bloques.length,
      CANALES_PERMITIDOS.length,
      "el manifiesto y la lista de permitidos cuentan distinto",
    );

    for (const bloque of bloques) {
      const cabecera = bloque.split(/^\[/mu)[0] ?? "";
      const nombre = /^nombre\s*=\s*"([^"]+)"/mu.exec(cabecera)?.[1];
      const servido = /^servido\s*=\s*(true|false)/mu.exec(cabecera)?.[1];

      assert.ok(nombre, "un canal sin nombre en el manifiesto");
      assert.ok(
        servido,
        `'${nombre}' no declara 'servido'. Estar declarado y estar cableado son ` +
          "cosas distintas, y el contrato tiene que decir cuál es cuál",
      );

      if (servido === "false") {
        // Un hueco sin motivo se erosiona: alguien lo revisa dentro de un año,
        // no encuentra la razón, y lo cablea a medias o lo borra. Es la misma
        // disciplina que los canales prohibidos.
        assert.match(
          cabecera,
          /^motivo_no_servido\s*=\s*"[^"]+"/mu,
          `'${nombre}' se declara sin servir y no dice por qué`,
        );
      }
    }
  });

  it("el manifiesto documenta el motivo de cada prohibición", () => {
    // Una lista de prohibidos sin motivos se erosiona: alguien la revisa dentro
    // de un ano, no encuentra la razon y la borra.
    const contenido = manifiesto();
    const prohibidos = nombresBajo(contenido, "[[prohibido]]");
    const motivos = (contenido.match(/^motivo\s*=/gmu) ?? []).length;

    assert.equal(
      motivos,
      prohibidos.length,
      `hay ${prohibidos.length} canales prohibidos y ${motivos} motivos declarados`,
    );
  });
});
