// VIS-04 — tablero de vigilancia. RPT-048, y tipado desde RPT-091 (PA-142).
//
// # Este fichero no decide nada
//
// Toda la lógica vive en `@eje/vision-base` y está probada sin ventana:
// `componerCabecera` decide qué se destaca, `componerSucesos` decide qué se
// puede afirmar, `incorporar` lleva el cursor, `leerSinAgente` traduce fallos.
//
// Aquí sólo se pinta. Si en este fichero aparece un `if` sobre una condición del
// agente, está en el sitio equivocado: significa que hay lógica sin probar.
//
// # Sin empaquetador, a propósito
//
// Se importa el módulo compilado directamente desde disco. Un empaquetador es
// código que transforma código: auditar el resultado dejaría de ser auditar lo
// que escribimos, y este producto se vende por ser auditable. `tsc` no lo es:
// emite un módulo por fichero y la correspondencia sigue siendo uno a uno.
//
// # Por qué la fuente vive en `src/` y la salida en `dist/`
//
// A la misma profundidad, para que el import relativo a la capa base valga
// igual escrito y emitido. Con la salida un nivel más abajo, la ruta del
// artefacto apuntaría a un directorio que no existe — y sólo se vería al abrir
// la ventana.

import {
  BITACORA_INICIAL,
  componerCabecera,
  componerSucesos,
  esperaSugerida,
  incorporar,
  resumirRespaldo,
  type Bitacora,
  type Cabecera,
  type Condiciones,
  type EstadoAgente,
  type EstadoPanel,
  type NodoInventario,
  type VistaSucesos,
} from "../../../packages/eje-vision-base/dist/indice.js";

/**
 * Nombres legibles de las trece condiciones, en el orden del contrato.
 *
 * El `satisfies` sobre `keyof Condiciones` es nuevo con PA-142 y **sustituye a
 * media barrera**: hasta hoy que estuvieran las trece y ninguna sobrara lo
 * comprobaba una prueba leyendo este fichero como texto. Ahora una clave mal
 * escrita no compila.
 */
const CONDICIONES = [
  ["capturaNoDisponible", "El sensor no está vigilando la red"],
  ["inventarioSuprimido", "Inventario suprimido"],
  ["inventarioNoVerifica", "El inventario no verifica"],
  ["registroSaturado", "El registro de evidencia está lleno"],
  ["evidenciaEnRiesgo", "Hay alertas sin guardar en disco"],
  ["salidaNoDisponible", "Las alertas no salen de este equipo"],
  ["sinColector", "Sin colector: este sensor no informa a ninguna sala"],
  // RPT-070, PA-125. Rara de ver aquí —para leer esta pantalla hay que estar
  // conectado— y aun así tiene fila: llega cuando la consulta alcanzó al agente
  // justo antes de caer, y cuando otro agente de la misma máquina sí responde.
  ["escuchaNoDisponible", "Sin escucha local: ninguna consola puede preguntarle"],
  ["configuracionSinFirmar", "Configuracion sin firmar: los parametros salen de la linea de ordenes"],
  ["configuracionNoVerifica", "Configuracion NO verifica: hay un fichero y el agente no lo acepta"],
  ["observacionSaturada", "Observación saturada"],
  ["capturaConPerdida", "Se están perdiendo tramas"],
  ["accionAdministrativa", "Requiere acción del administrador"],
] as const satisfies readonly (readonly [keyof Condiciones, string])[];

let bitacora: Bitacora = BITACORA_INICIAL;

/**
 * Elemento por identificador, o el fallo dicho en voz alta.
 *
 * `getElementById` devuelve `null` y hasta hoy nadie lo miraba: un identificador
 * mal escrito daba `Cannot read properties of null` **en la consola del
 * navegador**, que en una ventana de Electron sin devtools no la ve nadie. Aquí
 * revienta con el nombre del identificador, que al menos se puede buscar.
 */
function elemento(id: string): HTMLElement {
  const encontrado = document.getElementById(id);

  if (encontrado === null) {
    throw new Error(`vis04.html no tiene ningún elemento con id '${id}'`);
  }

  return encontrado;
}

/** Igual, para los que se usan como tabla y no valen si no lo son. */
function tabla(id: string): HTMLTableElement {
  const encontrado = elemento(id);

  if (!(encontrado instanceof HTMLTableElement)) {
    throw new Error(`el elemento '${id}' de vis04.html no es una tabla`);
  }

  return encontrado;
}

function pintarCabecera(cabecera: Cabecera): void {
  const zona = elemento("cabecera");
  zona.className = `cabecera ${cabecera.urgencia}`;
  zona.hidden = cabecera.titulo === "";
  elemento("cabecera-titulo").textContent = cabecera.titulo;
  elemento("cabecera-detalle").textContent = cabecera.detalle;

  // `datosDeAntes` es la marca que impide leer el resto como el ahora. Sin
  // esto, un tablero con el sensor ciego se lee igual que uno en calma.
  document.body.classList.toggle("de-antes", cabecera.datosDeAntes);
  elemento("marca-de-antes").hidden = !cabecera.datosDeAntes;
}

function pintarCondiciones(estado: EstadoPanel<Condiciones>): void {
  const destino = tabla("condiciones");
  destino.replaceChildren();

  if (estado.clase !== "datos") {
    const fila = destino.insertRow();
    const celda = fila.insertCell();
    celda.colSpan = 2;
    celda.className = "sin-dato";
    celda.textContent =
      estado.clase === "noServido"
        ? `No servido: ${estado.motivo}`
        : "Sin datos del sensor.";
    return;
  }

  for (const [clave, etiqueta] of CONDICIONES) {
    // El tipo dice `boolean`; el cable puede no mandar nada. `PuenteEje` es una
    // promesa sobre lo que el agente DEBERÍA enviar, no una validación de lo
    // que envió: entre `JSON.parse` y este punto no hay nadie comprobando.
    //
    // Por eso la aserción es a `boolean | undefined` y no se quita «porque el
    // tipo ya lo garantiza». Ausente NO es falso: un panel que pinte un campo
    // que falta como «no» diría que todo va bien exactamente igual.
    const valor: boolean | undefined = estado.valor[clave];

    const fila = destino.insertRow();
    fila.insertCell().textContent = etiqueta;

    const celda = fila.insertCell();

    if (valor === undefined) {
      celda.textContent = "AUSENTE EN LA RESPUESTA";
      celda.className = "roto";
    } else {
      celda.textContent = valor ? "sí" : "no";
      celda.className = valor ? "activa" : "inactiva";
    }
  }
}

/**
 * Lo que el operador necesita saber del sensor mismo. RPT-091, PA-78 mitad B.
 *
 * `respuestaAutomatica` es el campo con consecuencias: dice si este sensor puede
 * contener sin que un humano lo apruebe. Se pinta con las palabras del operador
 * —«contiene sin aprobación» / «todo pasa por un humano»— y no con un booleano,
 * porque «respuestaAutomatica: false» no le dice nada a quien está de guardia.
 */
function pintarEstadoAgente(estado: EstadoPanel<EstadoAgente>): void {
  const zona = elemento("estado-agente");

  if (estado.clase !== "datos") {
    zona.className = "sin-dato";
    zona.textContent =
      estado.clase === "noServido"
        ? `No servido: ${estado.motivo}`
        : "Sin datos del sensor.";
    return;
  }

  const { version, perfil, respuestaAutomatica } = estado.valor;

  zona.className = respuestaAutomatica ? "activa" : "inactiva";
  zona.textContent =
    `Eje-Agente ${version} · perfil ${perfil} · ` +
    (respuestaAutomatica
      ? "contiene sin aprobación"
      : "toda contención pasa por un humano");
}

/**
 * El recuento por calidad del respaldo, en una línea.
 *
 * La agregación de las cinco ambigüedades la hace `resumirRespaldo` en la capa
 * base, probada sin ventana (RPT-089 §3). Aquí sólo se escribe.
 */
function pintarRespaldo(nodos: readonly NodoInventario[]): void {
  const zona = elemento("respaldo");
  const resumen = resumirRespaldo(nodos);

  zona.className = resumen.ambiguos > 0 || resumen.indeterminados > 0 ? "activa" : "inactiva";
  zona.textContent =
    `${nodos.length} equipos · ${resumen.declarados} con marcado firmado · ` +
    `${resumen.porSegmento} por declaración de segmento · ` +
    `${resumen.ambiguos} sin respaldo suficiente · ` +
    `${resumen.indeterminados} sin veredicto`;
}

/**
 * Un equipo por fila, con su clase tal como llegó.
 *
 * **El valor del cable se muestra literal.** Traducirlo aquí a algo más corto
 * —«ambiguo», «crítico»— borraría la procedencia, que es lo único que distingue
 * «lo firma un humano» de «lo supongo por el tráfico» (RPT-088 §4). Si mañana
 * hace falta una etiqueta legible, se añade al contrato, no aquí.
 */
function pintarInventario(estado: EstadoPanel<readonly NodoInventario[]>): void {
  const destino = tabla("inventario");
  destino.replaceChildren();

  if (estado.clase !== "datos") {
    const celda = destino.insertRow().insertCell();
    celda.colSpan = 3;
    celda.className = "sin-dato";
    celda.textContent =
      estado.clase === "noServido"
        ? `No servido: ${estado.motivo}`
        : "Sin datos del sensor.";

    elemento("respaldo").className = "sin-dato";
    elemento("respaldo").textContent =
      estado.clase === "noServido"
        ? `No servido: ${estado.motivo}`
        : "Sin datos del sensor.";
    return;
  }

  pintarRespaldo(estado.valor);

  if (estado.valor.length === 0) {
    const celda = destino.insertRow().insertCell();
    celda.colSpan = 3;
    celda.className = "sin-dato";
    // El agente rechaza con motivo mientras no ha mirado, así que una lista
    // vacía que llega aquí SÍ significa que no vio ningún equipo.
    celda.textContent = "El sensor no ha observado ningún equipo.";
    return;
  }

  for (const nodo of estado.valor) {
    const fila = destino.insertRow();
    fila.insertCell().textContent = nodo.direccionEnlace;

    const clase = fila.insertCell();
    clase.textContent = nodo.clase;
    // Sólo lo declarado por una firma se presenta como asentado. Todo lo demás
    // —inferido, ambiguo, sin veredicto— pide que alguien mire.
    clase.className = nodo.clase.startsWith("declarada") ? "inactiva" : "activa";

    const contexto = fila.insertCell();
    contexto.className = "detalle";
    contexto.textContent =
      `segmento ${nodo.declaracionSegmento}` +
      (nodo.vistoEnSegmentoCritico ? " · visto en segmento crítico" : "") +
      (nodo.protocolosObservados.length > 0
        ? ` · ${nodo.protocolosObservados.join(", ")}`
        : "");
  }
}

function pintarSucesos(vista: VistaSucesos): void {
  const salvedad = elemento("salvedad");
  salvedad.textContent = vista.salvedad;
  salvedad.hidden = vista.salvedad === "";

  const aviso = elemento("salto");
  aviso.hidden = !bitacora.huboSalto;
  aviso.textContent = bitacora.huboSalto
    ? `Se archivaron alertas desde el asiento ${bitacora.saltoDesde} que esta ` +
      "consola no llegó a mostrar."
    : "";

  const lista = elemento("sucesos");
  lista.replaceChildren();

  if (vista.sucesos.length === 0) {
    const vacio = document.createElement("li");
    vacio.className = "sin-dato";
    // La única frase que puede afirmar ausencia, y sólo cuando la lógica lo
    // autoriza (RPT-048 §4).
    vacio.textContent = vista.puedeAfirmarQueNoHubo
      ? "Sin alertas registradas en este sensor."
      : "No se muestran alertas aquí.";
    lista.append(vacio);
    return;
  }

  for (const suceso of vista.sucesos) {
    const punto = document.createElement("li");
    const cabeza = document.createElement("b");
    cabeza.textContent = `#${suceso.asiento} · ${suceso.dispositivo}`;
    const cuerpo = document.createElement("div");
    cuerpo.className = "detalle";
    cuerpo.textContent = suceso.detalle;
    punto.append(cabeza, cuerpo);
    lista.append(punto);
  }
}

/**
 * El detalle de un fallo, sin perder nada por el camino.
 *
 * En TypeScript lo que se captura es `unknown`, y eso es correcto: se puede
 * lanzar cualquier cosa. El `fallo?.message` anterior era una suposición sobre
 * la forma de algo que no se había mirado.
 */
function detalleDe(fallo: unknown): string {
  return fallo instanceof Error ? fallo.message : String(fallo);
}

async function refrescar(): Promise<void> {
  let estadoCondiciones: EstadoPanel<Condiciones>;

  try {
    estadoCondiciones = {
      clase: "datos",
      valor: await window.eje.obtenerCondiciones(),
    };
  } catch (fallo) {
    estadoCondiciones = { clase: "sinAgente", detalle: detalleDe(fallo) };
  }

  pintarCabecera(componerCabecera(estadoCondiciones));
  pintarCondiciones(estadoCondiciones);

  // Los dos canales que el sensor servía y esta pantalla no consumía. Cada uno
  // con su propio `try`: que el inventario falle no debe borrar el estado del
  // agente, ni al revés. Son cuatro consultas independientes al mismo socket.
  try {
    pintarEstadoAgente({
      clase: "datos",
      valor: await window.eje.obtenerEstadoAgente(),
    });
  } catch (fallo) {
    pintarEstadoAgente({ clase: "sinAgente", detalle: detalleDe(fallo) });
  }

  try {
    pintarInventario({
      clase: "datos",
      valor: await window.eje.obtenerInventario(),
    });
  } catch (fallo) {
    pintarInventario({ clase: "sinAgente", detalle: detalleDe(fallo) });
  }

  try {
    const respuesta = await window.eje.consultarAlertas(bitacora.desdeAsiento);
    bitacora = incorporar(bitacora, respuesta);
    pintarSucesos(
      componerSucesos({
        primerDisponible: respuesta.primerDisponible,
        hayMas: respuesta.hayMas,
        sucesos: bitacora.sucesos,
      }),
    );
  } catch {
    // El fallo de alertas no borra lo ya mostrado: la bitácora conserva lo que
    // se sabe, y la cabecera ya avisa de que no hay sensor.
  }

  elemento("sello").textContent = new Date().toLocaleTimeString();

  // La cadencia sale de la medida, no de una intuición (RPT-050 §6). Con cola
  // pendiente se vuelve a preguntar ya; en régimen, cada dos segundos.
  setTimeout(() => void refrescar(), esperaSugerida(bitacora));
}

void refrescar();
