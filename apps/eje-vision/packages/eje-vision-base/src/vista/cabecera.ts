/**
 * La cabecera del tablero. RPT-048 §2.
 *
 * # Por qué hay una cabecera y no sólo una lista
 *
 * Un panel de vigilancia se mira dos segundos. Lo que esté arriba es lo único
 * que se lee de verdad, y hay dos estados que **cambian cómo hay que leer todo
 * lo demás**:
 *
 * - **Sin captura**: el sensor no está observando. Todo lo que muestre la
 *   pantalla es de antes. Como fila número cinco entre diez, invita a leer el
 *   resto como si fuera actual — que es el zombi de RPT-047 §2 reapareciendo en
 *   la capa visual.
 * - **Sin agente**: no hay con quién hablar. Lo de la pantalla es de la última
 *   consulta que funcionó, si es que hubo alguna.
 *
 * Los dos merecen ancho completo, y los dos tienen la misma consecuencia:
 * `datosDeAntes`.
 *
 * # Lógica pura
 *
 * Este módulo decide **qué** se dice y con qué urgencia. No decide colores ni
 * tamaños: eso es la vista, y esto se prueba sin ella.
 */

import type { Condiciones } from "../ipc/puente.js";

import type { EstadoPanel } from "./estado-panel.js";
import { leerSinAgente } from "./sin-agente.js";

/**
 * Urgencia de la cabecera.
 *
 * Tres niveles y no cinco: un operador no distingue cinco, y el cuarto acaba
 * usándose para cosas que no lo merecen (PA-45 Fase 1).
 */
export type Urgencia = "normal" | "atencion" | "critica";

/** Lo que ocupa el ancho superior del tablero. */
export interface Cabecera {
  readonly urgencia: Urgencia;
  /** Frase única, en presente. Vacía cuando no hay nada que destacar. */
  readonly titulo: string;
  /** Contexto o acción. Vacío si no aporta. */
  readonly detalle: string;
  /**
   * Si lo que hay debajo puede leerse como el estado actual del segmento.
   *
   * `false` obliga a la vista a marcar los paneles como no vigentes. Sin esto,
   * un tablero con el sensor ciego se lee igual que uno con la red tranquila, y
   * esa confusión es el motivo entero de que exista `capturaNoDisponible`.
   */
  readonly datosDeAntes: boolean;
}

/**
 * Decide la cabecera a partir del estado de las condiciones.
 *
 * El orden de las ramas **es** la prioridad, y no es negociable: lo primero que
 * se comprueba es si se puede creer lo que hay en pantalla.
 */
export function componerCabecera(estado: EstadoPanel<Condiciones>): Cabecera {
  if (estado.clase === "consultando") {
    return {
      urgencia: "normal",
      titulo: "Consultando al sensor",
      detalle: "",
      // Todavía no hay nada abajo, pero tampoco hay nada vigente que mostrar.
      datosDeAntes: true,
    };
  }

  if (estado.clase === "sinAgente") {
    const lectura = leerSinAgente(estado.detalle);
    return {
      urgencia: "critica",
      titulo: lectura.titulo,
      detalle: lectura.sugerencia,
      datosDeAntes: true,
    };
  }

  if (estado.clase !== "datos") {
    // `vacio` y `noServido` no tienen sentido para las condiciones: el agente
    // siempre devuelve las trece. Si llega uno de estos, algo cambió en el
    // contrato y decirlo es mejor que elegir una rama al azar.
    return {
      urgencia: "critica",
      titulo: "El sensor no está informando de su estado",
      detalle: "Respuesta inesperada en el canal de condiciones.",
      datosDeAntes: true,
    };
  }

  const condiciones = estado.valor;

  // Lo primero, siempre. Mientras el sensor no observe, nada de lo que hay
  // debajo describe el ahora.
  if (condiciones.capturaNoDisponible) {
    return {
      urgencia: "critica",
      titulo: "El sensor no está vigilando la red",
      detalle:
        "No hay captura de tráfico. Lo que se muestra debajo es de antes de " +
        "que dejara de observar.",
      datosDeAntes: true,
    };
  }

  // Manipulación antes que el resto: alguien tocó el almacén, y eso cambia a
  // quién hay que avisar. `hay_manipulacion` en Rust separa estas dos de las
  // demás; aquí se respeta esa separación en lugar de reinventarla.
  if (condiciones.inventarioSuprimido || condiciones.inventarioNoVerifica) {
    return {
      urgencia: "critica",
      titulo: "La evidencia de este equipo no verifica",
      detalle: "Responder como incidente: alguien alteró el almacén del sensor.",
      datosDeAntes: false,
    };
  }

  // El registro lleno es tan grave como la manipulación **sin serlo**: el sensor
  // dejó de anotar amenazas. No se presenta como que alguien tocó nada.
  if (condiciones.registroSaturado) {
    return {
      urgencia: "critica",
      titulo: "El sensor ha dejado de registrar amenazas",
      detalle: "El registro de evidencia está lleno. Requiere intervención ya.",
      datosDeAntes: false,
    };
  }

  // Una de las dos que no viajan por syslog: si nadie mira esta pantalla, nadie
  // lo sabe. Por eso sube a la cabecera aunque no sea la más grave.
  if (condiciones.salidaNoDisponible) {
    return {
      urgencia: "atencion",
      titulo: "Las alertas no están saliendo de este equipo",
      detalle: "El colector no responde. Esta pantalla es lo único que lo sabe.",
      datosDeAntes: false,
    };
  }

  // La otra, y va **después**: una avería en curso es más urgente que una
  // instalación incompleta. Si las dos estuvieran activas, «el colector no
  // responde» sería engañoso —no hay colector al que responder—, pero eso no
  // puede pasar: sin colector configurado no hay envío que falle.
  //
  // RPT-054 §4, PA-109. Este es el sitio donde el técnico que fue a la planta se
  // entera, y el único: por definición no hay forma de contarlo hacia fuera.
  if (condiciones.sinColector) {
    return {
      urgencia: "atencion",
      titulo: "Este sensor no informa a ninguna sala",
      detalle:
        "No tiene colector configurado: vigila el segmento, pero nada sale de " +
        "este equipo y nadie fuera notará si se apaga.",
      datosDeAntes: false,
    };
  }

  // RPT-070, PA-125. Va la última de las tres del canal, y no por menos grave:
  // por lo raro que es verla. Para leer esta pantalla hay que estar conectado, y
  // si esta condición está encendida no se puede estar.
  //
  // Llega en dos casos: una consulta que alcanzó al agente justo antes de que la
  // escucha cayera, y un segundo agente en la misma máquina cuyo socket sí
  // responde. Presentarla es barato y callarla dejaría al técnico mirando un
  // panel que no explica por qué el otro sensor no aparece.
  if (condiciones.escuchaNoDisponible) {
    return {
      urgencia: "atencion",
      titulo: "Este sensor no admite consultas",
      detalle:
        "Su escucha local no está abierta: sigue vigilando y registrando, pero " +
        "ninguna consola puede preguntarle nada. La sala sí se entera.",
      datosDeAntes: false,
    };
  }

  // RPT-074, PA-79. Después de las tres del canal, porque aquéllas dicen que el
  // sensor no puede contar lo que ve y ésta dice que puede no estar viendo lo que
  // alguien creyó haberle mandado mirar. Las primeras son mudez; ésta es duda.
  //
  // No se anuncia como incidente aunque la firma rota apunte a manipulación: una
  // máquina ajena o una clave rotada dan lo mismo, y mandar a respuesta a
  // incidentes por un error de despliegue es la fatiga de alertas de PA-45.
  if (condiciones.configuracionNoVerifica) {
    return {
      urgencia: "atencion",
      titulo: "La configuración de este sensor no verifica",
      detalle:
        "Hay un fichero firmado y el agente no lo acepta. El motivo está en el " +
        "diario del sensor: firma, máquina o clave.",
      datosDeAntes: false,
    };
  }

  // La última de las tres, y la más leve de las tres: el sensor hace lo que se le
  // pidió, sólo que quien se lo pidió no está firmado.
  //
  // Ocupa cabecera y no fila porque es exactamente la condición que se aprende a
  // ignorar si se deja abajo, y su riesgo es que el estado degradado se vuelva el
  // normal (RPT-074 §8).
  if (condiciones.configuracionSinFirmar) {
    return {
      urgencia: "atencion",
      titulo: "Este sensor corre sin configuración firmada",
      detalle:
        "Sus parámetros salen de la línea de órdenes: quien controle el arranque " +
        "puede alargar la ventana de silencio que la sala vigila.",
      datosDeAntes: false,
    };
  }

  return { urgencia: "normal", titulo: "", detalle: "", datosDeAntes: false };
}
