/**
 * Arranque de Electron: el punto donde todo lo verificado se enchufa.
 *
 * RPT-046. Este fichero es deliberadamente **fino**. Todo lo que se puede
 * decidir mal —qué preferencias lleva la ventana, qué destinos se abren, cómo se
 * reensambla un marco— vive en módulos que se prueban sin Electron. Aquí sólo
 * queda la fontanería que no se puede probar sin un escritorio.
 *
 * Si este fichero crece, algo se ha escrito en el sitio equivocado.
 */

import { connect } from "node:net";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { app, BrowserWindow, ipcMain, session, shell } from "electron";

import { type Conducto, pedir } from "./enlace.js";
import { CANALES_PERMITIDOS, type CanalPermitido } from "./puente-ipc.js";
import {
  type Cabeceras,
  type OpcionesVentana,
  type VentanaAbierta,
  montarVentanaPrincipal,
} from "./ventana.js";

/** Directorio de este módulo compilado. */
const AQUI = dirname(fileURLToPath(import.meta.url));

/**
 * Ruta del socket del agente.
 *
 * Sin puerto TCP local (RPT-002 §9.3). En Windows sería un named pipe; PA-79
 * queda anotado porque esta ruta está fijada y debería salir de configuración.
 */
export const RUTA_SOCKET = process.env["EJE_SOCKET"] ?? "/run/eje/agente.sock";

/** Envuelve un socket real en el `Conducto` que `enlace.ts` sabe manejar. */
function abrirConducto(): Conducto {
  const zocalo = connect(RUTA_SOCKET);
  zocalo.setNoDelay(true);

  return {
    escribir: (datos) => void zocalo.write(datos),
    alRecibir: (manejador) => void zocalo.on("data", manejador),
    alFallar: (manejador) => void zocalo.on("error", manejador),
    alCerrar: (manejador) => void zocalo.on("close", manejador),
    cerrar: () => zocalo.destroy(),
  };
}

/**
 * Convierte los argumentos de un canal en la carga útil que espera el agente.
 *
 * Los dos canales con argumento son los únicos que llevan cuerpo; los demás
 * mandan carga vacía, que el formato admite y los vectores cubren.
 */
function cargaDe(canal: CanalPermitido, argumento: unknown): Uint8Array {
  if (canal === "consultar-sandbox") {
    return Buffer.from(JSON.stringify({ sentencia: argumento }), "utf8");
  }
  if (canal === "consultar-alertas") {
    return Buffer.from(JSON.stringify({ desdeAsiento: argumento }), "utf8");
  }
  return Buffer.alloc(0);
}

/**
 * Registra un manejador por canal.
 *
 * Se recorre `CANALES_PERMITIDOS`, que es la autoridad: así **no puede existir
 * un canal permitido sin manejador ni un manejador fuera de la lista**. Escribir
 * seis `ipcMain.handle` a mano permitiría las dos cosas.
 */
export function registrarCanales(
  registrar: (canal: string, manejador: (...args: unknown[]) => unknown) => void,
  abrir: () => Conducto,
): void {
  for (const canal of CANALES_PERMITIDOS) {
    registrar(canal, async (_suceso: unknown, argumento: unknown) => {
      const respuesta = await pedir(abrir, canal, cargaDe(canal, argumento));

      if (respuesta.clase === "rechazo") {
        // El motivo se conserva. RPT-036 §6: «no hay nada» y «esto no lo sirve
        // nadie» no son lo mismo, y `Error` genérico las colapsa.
        throw new Error(`el agente rechazó «${canal}»: ${respuesta.motivo}`);
      }

      return JSON.parse(respuesta.cuerpo.toString("utf8")) as unknown;
    });
  }
}

/** Adapta `BrowserWindow` a la `FabricaVentana` que `ventana.ts` espera. */
function fabricar(opciones: OpcionesVentana): VentanaAbierta {
  const ventana = new BrowserWindow(opciones);

  return {
    cargarFichero: (ruta) => ventana.loadFile(ruta),
    alAbrirVentana: (manejador) => {
      ventana.webContents.setWindowOpenHandler(({ url }) => {
        if (manejador(url) === "permitir") {
          // Al navegador del sistema, nunca a una ventana de Electron.
          void shell.openExternal(url);
        }
        return { action: "deny" };
      });
    },
    alResponderCabeceras: (manejador) => {
      session.defaultSession.webRequest.onHeadersReceived((detalles, responder) => {
        responder({
          responseHeaders: manejador(
            (detalles.responseHeaders ?? {}) as Cabeceras,
          ) as Record<string, string[]>,
        });
      });
    },
  };
}

/**
 * Arranca la aplicación.
 *
 * PA-77 sigue abierto: si este binario acaba en el sensor y el sensor es
 * headless, `app.whenReady()` falla. El fallo debe **decir eso**, no morir con
 * un error de X11 que nadie relaciona con una decisión de despliegue.
 */
export async function arrancar(): Promise<void> {
  try {
    await app.whenReady();
  } catch (error) {
    throw new Error(
      "Electron no pudo inicializarse. Si este equipo no tiene sesión gráfica, " +
        "Eje-Visión no es el componente que debe correr aquí: el sensor es " +
        `'eje-agente', sin interfaz. Causa: ${String(error)}`,
    );
  }

  registrarCanales(
    (canal, manejador) => void ipcMain.handle(canal, manejador),
    abrirConducto,
  );

  // VIS-04 por omision; el puesto de diagnostico solo si se pide a proposito.
  // Al reves seria un producto que arranca en modo desarrollo por descuido.
  const vista = process.env["EJE_VISTA"] === "diagnostico" ? "indice.html" : "vis04.html";

  const ventana = await montarVentanaPrincipal(
    fabricar,
    join(AQUI, "preload.cjs"),
    join(AQUI, "..", "vista", vista),
  );

  // La ventana nace con `show: false`; se enseña cuando hay algo que enseñar.
  void ventana;
  for (const abierta of BrowserWindow.getAllWindows()) {
    abierta.show();
  }

  // En Linux y Windows cerrar la última ventana termina la aplicación. No se
  // imita el comportamiento de macOS: un panel de seguridad que sigue vivo sin
  // ventana es un proceso que el operador cree haber cerrado.
  app.on("window-all-closed", () => app.quit());
}
