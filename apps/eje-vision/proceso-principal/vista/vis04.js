// VIS-04 — tablero de vigilancia. RPT-048.
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
// que escribimos, y este producto se vende por ser auditable.

import {
  BITACORA_INICIAL,
  componerCabecera,
  componerSucesos,
  esperaSugerida,
  incorporar,
} from "../../packages/eje-vision-base/dist/indice.js";

/** Nombres legibles de las nueve condiciones, en el orden del contrato. */
const CONDICIONES = [
  ["capturaNoDisponible", "El sensor no está vigilando la red"],
  ["inventarioSuprimido", "Inventario suprimido"],
  ["inventarioNoVerifica", "El inventario no verifica"],
  ["registroSaturado", "El registro de evidencia está lleno"],
  ["evidenciaEnRiesgo", "Hay alertas sin guardar en disco"],
  ["salidaNoDisponible", "Las alertas no salen de este equipo"],
  ["observacionSaturada", "Observación saturada"],
  ["capturaConPerdida", "Se están perdiendo tramas"],
  ["accionAdministrativa", "Requiere acción del administrador"],
];

let bitacora = BITACORA_INICIAL;

const elemento = (id) => document.getElementById(id);

function pintarCabecera(cabecera) {
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

function pintarCondiciones(estado) {
  const tabla = elemento("condiciones");
  tabla.replaceChildren();

  if (estado.clase !== "datos") {
    const fila = tabla.insertRow();
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
    const valor = estado.valor[clave];
    const fila = tabla.insertRow();
    fila.insertCell().textContent = etiqueta;

    const celda = fila.insertCell();
    // Ausente NO es falso. Si el agente deja de mandar un campo, un panel que
    // lo pinte como «no» diría que todo va bien exactamente igual.
    if (valor === undefined) {
      celda.textContent = "AUSENTE EN LA RESPUESTA";
      celda.className = "roto";
    } else {
      celda.textContent = valor ? "sí" : "no";
      celda.className = valor ? "activa" : "inactiva";
    }
  }
}

function pintarSucesos(vista) {
  elemento("salvedad").textContent = vista.salvedad;
  elemento("salvedad").hidden = vista.salvedad === "";

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
    punto.innerHTML = "";
    const cabeza = document.createElement("b");
    cabeza.textContent = `#${suceso.asiento} · ${suceso.dispositivo}`;
    const cuerpo = document.createElement("div");
    cuerpo.className = "detalle";
    cuerpo.textContent = suceso.detalle;
    punto.append(cabeza, cuerpo);
    lista.append(punto);
  }
}

async function refrescar() {
  let estadoCondiciones;
  try {
    estadoCondiciones = { clase: "datos", valor: await window.eje.obtenerCondiciones() };
  } catch (fallo) {
    estadoCondiciones = {
      clase: "sinAgente",
      detalle: fallo?.message ?? String(fallo),
    };
  }

  pintarCabecera(componerCabecera(estadoCondiciones));
  pintarCondiciones(estadoCondiciones);

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
