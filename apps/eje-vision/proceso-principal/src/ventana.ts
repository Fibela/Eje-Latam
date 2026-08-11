/**
 * Creación de la ventana principal, con la fábrica inyectada.
 *
 * RPT-004 §6.1, RPT-046.
 *
 * ## Por qué el constructor entra como parámetro
 *
 * `BrowserWindow` sólo existe dentro de Electron y sólo funciona con una sesión
 * gráfica. Si esta lógica lo invocara directamente, no habría forma de comprobar
 * **qué opciones se le pasan** sin arrancar un escritorio — y las opciones son
 * justamente lo que importa.
 *
 * Es la misma costura que `Despacho` (RPT-032) y `Atiende` (RPT-035): el efecto
 * detrás de un parámetro, para que la decisión se pueda observar sin ejecutarlo.
 *
 * ## Y por qué eso no es ceremonia
 *
 * `PREFERENCIAS_SEGURIDAD` está declarada y congelada desde RPT-004, con pruebas
 * que la verifican. Pero Electron **ignora en silencio** las claves que no
 * conoce y acepta cualquier subconjunto: pasar la mitad del objeto crearía una
 * ventana insegura sin que ninguna de esas pruebas se inmutara.
 *
 * Lo que hace falta comprobar no es la constante: es que la ventana se crea
 * **con ella**.
 */

import {
  CSP,
  PREFERENCIAS_SEGURIDAD,
  type PreferenciasSeguridad,
  destinoExternoPermitido,
} from "./seguridad-ventana.js";

/** Lo que se decide al abrir una ventana emergente. */
export type Apertura = "permitir" | "denegar";

/** Opciones con las que se crea la ventana. Subconjunto de las de Electron. */
export interface OpcionesVentana {
  readonly width: number;
  readonly height: number;
  readonly show: boolean;
  readonly webPreferences: PreferenciasSeguridad & { readonly preload: string };
}

/** La ventana ya creada, en lo que este módulo necesita de ella. */
export interface VentanaAbierta {
  cargarFichero(ruta: string): Promise<void>;
  alAbrirVentana(manejador: (url: string) => Apertura): void;
  alResponderCabeceras(manejador: (cabeceras: Cabeceras) => Cabeceras): void;
}

/** Cabeceras de una respuesta, tal como Electron las entrega. */
export type Cabeceras = Record<string, readonly string[]>;

/** Crea la ventana. En producción envuelve a `new BrowserWindow(...)`. */
export type FabricaVentana = (opciones: OpcionesVentana) => VentanaAbierta;

/** Medidas iniciales. No son seguridad: son comodidad y se pueden cambiar. */
export const ANCHO_INICIAL = 1280;
export const ALTO_INICIAL = 800;

/**
 * Opciones de la ventana principal.
 *
 * Las preferencias salen **enteras** de la constante congelada; aquí sólo se le
 * añade la ruta del preload, que es lo único que este módulo aporta.
 */
export function opcionesDeVentana(rutaPreload: string): OpcionesVentana {
  return {
    width: ANCHO_INICIAL,
    height: ALTO_INICIAL,
    // Se muestra al terminar de cargar, no antes: una ventana en blanco durante
    // el arranque parece un fallo del producto.
    show: false,
    webPreferences: { ...PREFERENCIAS_SEGURIDAD, preload: rutaPreload },
  };
}

/**
 * Aplica la política de contenido a unas cabeceras de respuesta.
 *
 * Se impone desde el proceso principal y no con una etiqueta `<meta>`: una
 * etiqueta vive en el documento, y quien pueda alterar el documento puede
 * quitarla.
 */
export function conPolitica(cabeceras: Cabeceras): Cabeceras {
  return { ...cabeceras, "Content-Security-Policy": [CSP] };
}

/**
 * Decide si una ventana emergente puede abrirse.
 *
 * Siempre `denegar` para la ventana: el destino permitido se abre en el
 * navegador del sistema, no dentro de la aplicación. Una ventana de Electron
 * apuntando a la web es una superficie que no hace falta.
 */
export function decidirApertura(url: string): Apertura {
  return destinoExternoPermitido(url) ? "permitir" : "denegar";
}

/**
 * Monta la ventana principal.
 *
 * @param fabrica Constructor inyectado. En producción, `new BrowserWindow(...)`.
 * @param rutaPreload Ruta absoluta del preload compilado.
 * @param rutaIndice Ruta absoluta del HTML de la interfaz.
 */
export async function montarVentanaPrincipal(
  fabrica: FabricaVentana,
  rutaPreload: string,
  rutaIndice: string,
): Promise<VentanaAbierta> {
  const ventana = fabrica(opcionesDeVentana(rutaPreload));

  ventana.alResponderCabeceras(conPolitica);
  ventana.alAbrirVentana(decidirApertura);

  await ventana.cargarFichero(rutaIndice);
  return ventana;
}
