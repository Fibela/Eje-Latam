// Puesto de diagnóstico. NO es VIS-04.
//
// RPT-046 §11, PA-82. Recorre el camino entero: renderer -> preload ->
// proceso principal -> socket Unix -> eje-agente, y vuelta.
//
// Se escribe en JavaScript plano y no en TypeScript a propósito: no forma parte
// del producto, no entra en `tsc --build` y no debe dar la impresión de que sí.

const NOMBRES = {
  inventarioSuprimido: "inventario suprimido",
  inventarioNoVerifica: "inventario no verifica",
  observacionSaturada: "observación saturada",
  capturaConPerdida: "captura con pérdida",
  capturaNoDisponible: "CAPTURA NO DISPONIBLE",
  accionAdministrativa: "acción administrativa",
  salidaNoDisponible: "salida no disponible",
  registroSaturado: "registro saturado",
  evidenciaEnRiesgo: "evidencia en riesgo",
};

async function refrescar() {
  const tabla = document.getElementById("tabla");
  const sello = document.getElementById("sello");
  const error = document.getElementById("error");

  try {
    const condiciones = await window.eje.obtenerCondiciones();
    error.textContent = "";
    sello.textContent = new Date().toLocaleTimeString();

    tabla.replaceChildren();
    for (const [clave, etiqueta] of Object.entries(NOMBRES)) {
      const valor = condiciones[clave];
      const fila = tabla.insertRow();
      fila.insertCell().textContent = etiqueta;

      const celda = fila.insertCell();
      // Se distingue `undefined` de `false`: un campo que el agente no manda
      // NO es un campo en falso. Si el contrato se desincroniza, esto lo dice
      // en lugar de pintar «todo bien».
      celda.textContent =
        valor === undefined ? "AUSENTE EN LA RESPUESTA" : valor ? "sí" : "no";
      celda.className = valor === undefined ? "si" : valor ? "si" : "no";
    }
  } catch (fallo) {
    // El motivo entero, sin resumir. `enlace.ts` ya distingue las cuatro causas
    // y esa distinción sólo sirve si llega hasta aquí.
    sello.textContent = "sin respuesta";
    error.textContent = String(fallo && fallo.message ? fallo.message : fallo);
  }
}

void refrescar();
setInterval(() => void refrescar(), 2000);
