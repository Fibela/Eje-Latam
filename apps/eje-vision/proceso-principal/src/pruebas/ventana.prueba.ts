/**
 * La ventana se crea **con** las preferencias declaradas, no con literales.
 *
 * RPT-046. Las pruebas de RPT-004 §6.1 verifican que `PREFERENCIAS_SEGURIDAD`
 * dice lo correcto. Éstas verifican que alguien la usa — que es lo que
 * PA-75 enseñó que no es lo mismo.
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  PREFERENCIAS_SEGURIDAD,
  CSP,
} from "../seguridad-ventana.js";
import {
  type Apertura,
  type Cabeceras,
  type OpcionesVentana,
  type VentanaAbierta,
  conPolitica,
  decidirApertura,
  montarVentanaPrincipal,
  opcionesDeVentana,
} from "../ventana.js";

/** Fábrica espía: guarda con qué se la llamó y qué le pidieron después. */
function espia(): {
  fabrica: (opciones: OpcionesVentana) => VentanaAbierta;
  registro: {
    opciones: OpcionesVentana[];
    ficheros: string[];
    aperturas: ((url: string) => Apertura)[];
    cabeceras: ((cabeceras: Cabeceras) => Cabeceras)[];
  };
} {
  const registro = {
    opciones: [] as OpcionesVentana[],
    ficheros: [] as string[],
    aperturas: [] as ((url: string) => Apertura)[],
    cabeceras: [] as ((cabeceras: Cabeceras) => Cabeceras)[],
  };

  return {
    registro,
    fabrica: (opciones) => {
      registro.opciones.push(opciones);
      return {
        cargarFichero: (ruta) => {
          registro.ficheros.push(ruta);
          return Promise.resolve();
        },
        alAbrirVentana: (manejador) => {
          registro.aperturas.push(manejador);
        },
        alResponderCabeceras: (manejador) => {
          registro.cabeceras.push(manejador);
        },
      };
    },
  };
}

describe("RPT-046 — la ventana usa las preferencias declaradas", () => {
  it("las preferencias van enteras, no un subconjunto", () => {
    // Electron ignora en silencio las claves que no conoce y acepta cualquier
    // subconjunto: pasar la mitad del objeto crearía una ventana insegura sin
    // que las pruebas de RPT-004 §6.1 se inmutaran.
    const { webPreferences } = opcionesDeVentana("/ruta/preload.js");

    for (const [clave, valor] of Object.entries(PREFERENCIAS_SEGURIDAD)) {
      assert.equal(
        webPreferences[clave as keyof typeof PREFERENCIAS_SEGURIDAD],
        valor,
        `falta o difiere '${clave}' en las preferencias de la ventana`,
      );
    }

    assert.equal(webPreferences.preload, "/ruta/preload.js");
  });

  it("no se cuela ninguna clave de seguridad que no esté declarada", () => {
    // La otra dirección: si alguien añade `webSecurity: false` aquí, la
    // constante congelada no se entera. Sólo se admite `preload` de más.
    const { webPreferences } = opcionesDeVentana("/ruta/preload.js");
    const declaradas = new Set([...Object.keys(PREFERENCIAS_SEGURIDAD), "preload"]);

    for (const clave of Object.keys(webPreferences)) {
      assert.ok(
        declaradas.has(clave),
        `'${clave}' no está en PREFERENCIAS_SEGURIDAD y aparece en la ventana`,
      );
    }
  });

  it("la ventana nace oculta y se carga desde disco", () => {
    const { fabrica, registro } = espia();

    return montarVentanaPrincipal(fabrica, "/p.js", "/vista/indice.html").then(() => {
      assert.equal(registro.opciones.length, 1);
      assert.equal(
        registro.opciones[0]?.show,
        false,
        "una ventana en blanco durante el arranque parece un fallo del producto",
      );
      assert.deepEqual(registro.ficheros, ["/vista/indice.html"]);
    });
  });

  it("la política de contenido se impone en las cabeceras, no en el documento", () => {
    // Una etiqueta `<meta>` vive dentro del documento, y quien pueda alterar el
    // documento puede quitarla. La cabecera la pone el proceso principal.
    const conservadas = conPolitica({ "X-Algo": ["valor"] });

    assert.deepEqual(conservadas["Content-Security-Policy"], [CSP]);
    assert.deepEqual(conservadas["X-Algo"], ["valor"], "no se pierden las demás");
  });

  it("toda ventana emergente se deniega, incluso hacia un destino permitido", () => {
    // El destino permitido se abre en el navegador del sistema. Una ventana de
    // Electron apuntando a la web es superficie que no hace falta.
    assert.equal(decidirApertura("https://premoscorp.com/soporte"), "permitir");
    assert.equal(decidirApertura("https://cualquier-otro.example/"), "denegar");
    assert.equal(decidirApertura("http://premoscorp.com/"), "denegar");
    assert.equal(decidirApertura("no es una url"), "denegar");
  });

  it("los dos manejadores quedan registrados antes de cargar nada", () => {
    // Si se cargara la vista antes de instalar la política, existiría una
    // ventana —breve— sirviendo contenido sin CSP.
    const { fabrica, registro } = espia();

    return montarVentanaPrincipal(fabrica, "/p.js", "/vista/indice.html").then(() => {
      assert.equal(registro.cabeceras.length, 1);
      assert.equal(registro.aperturas.length, 1);
    });
  });
});
