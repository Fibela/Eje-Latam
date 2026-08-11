/**
 * Preload: el primer salto, del renderer al proceso principal.
 *
 * RPT-004 §6.2, RPT-046.
 *
 * ## Por qué este fichero es `.cts` y no `.ts`
 *
 * `PREFERENCIAS_SEGURIDAD` fija `sandbox: true`, y **Electron no admite preloads
 * en módulos ES bajo sandbox**. El paquete es `"type": "module"`, así que la
 * extensión `.cts` es lo que hace que `tsc` emita `preload.cjs`.
 *
 * No es una preferencia de estilo: es el sandbox el que manda, y el sandbox está
 * ratificado.
 *
 * ## Por qué los canales están escritos aquí y no importados
 *
 * Un preload sandboxeado no puede cargar `@eje/vision-base`, que es ESM. La lista
 * de canales no puede llegar aquí en tiempo de ejecución.
 *
 * Eso convierte este fichero en **un sitio más donde vive el contrato**, que es
 * exactamente lo que PA-20 existe para impedir. La atadura es una prueba que lee
 * este fuente y lo compara con el manifiesto — el mismo mecanismo que PA-75 usa
 * con `puente.ts`. Si alguien añade un método aquí sin declararlo en
 * `contrato-ipc.toml`, la suite lo dice.
 *
 * ## Y por qué no hay un `invocar(canal, args)` genérico
 *
 * Porque trasladaría la decisión de autorización al renderer, que es justo la
 * capa que no debe tenerla (RPT-004 §6.2). Cada operación es un método con
 * nombre propio; añadir una exige tocar cuatro sitios revisables.
 */

// `import ... = require(...)` y no `import { } from`: con `verbatimModuleSyntax`
// activo, un fichero `.cts` no admite sintaxis de módulos ES. Es la misma
// decisión del sandbox, vista desde el compilador.
import electron = require("electron");

const { contextBridge, ipcRenderer } = electron;

/**
 * Superficie expuesta. Corresponde uno a uno con `PuenteEje`.
 *
 * Los tipos no se importan de `@eje/vision-base` **a propósito**: importarlos
 * ataría este fichero a un paquete ESM que el sandbox no puede cargar, y el tipo
 * se borra al compilar de todas formas. La forma la comprueba la prueba de
 * paridad, no el compilador.
 */
const puente = {
  obtenerEstadoAgente: async () => ipcRenderer.invoke("obtener-estado-agente"),
  obtenerInventario: async () => ipcRenderer.invoke("obtener-inventario"),
  obtenerEstadoBoveda: async () => ipcRenderer.invoke("obtener-estado-boveda"),
  consultarSandbox: async (sentencia: string) =>
    ipcRenderer.invoke("consultar-sandbox", sentencia),
  consultarAlertas: async (desdeAsiento: number) =>
    ipcRenderer.invoke("consultar-alertas", desdeAsiento),
  obtenerCondiciones: async () => ipcRenderer.invoke("obtener-condiciones"),
};

// `exposeInMainWorld` y no `window.eje = ...`: con `contextIsolation: true` el
// preload y la página tienen mundos distintos, y una asignación directa no
// llegaría al renderer. Que no llegue es lo correcto — el puente cruza por aquí
// o no cruza.
contextBridge.exposeInMainWorld("eje", puente);
