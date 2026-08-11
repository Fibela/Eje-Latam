/**
 * # Cliente del formato de cable de `eje-ipc`
 *
 * RPT-045. Este módulo es el **segundo salto**: el que va del proceso principal
 * al agente por socket de dominio Unix. No confundir con el IPC de Electron
 * (`contextBridge` / `ipcRenderer`), que comunica el renderer con este proceso.
 *
 * ## El formato
 *
 * ```text
 * marco     = longitud(u32 big-endian) ‖ carga
 * petición  = longitudNombre(u16 BE) ‖ nombre ‖ cargaÚtil
 * respuesta = código(1 byte) ‖ cuerpo
 * ```
 *
 * ## Por qué hay vectores
 *
 * Éste es el tercer sitio donde vive el contrato, y el primero a nivel de bytes.
 * Las barreras de PA-75 y PA-76 comprueban formas y esquemas; ninguna mira el
 * cable. Un prefijo escrito en little-endian pasaría las pruebas de los dos
 * lenguajes y fallaría la primera vez que los procesos se hablen.
 *
 * Por eso `vectores-ipc.json` lo genera Rust y esta implementación se contrasta
 * contra él byte a byte.
 */

/** Cotas del protocolo. Se contrastan contra `vectores-ipc.json` en las pruebas. */
export const LIMITES = {
  marcoMaximo: 1_048_576,
  prefijoLongitud: 4,
  prefijoNombre: 2,
  nombreMaximo: 64,
  codigoRespuesta: 0,
  codigoRechazo: 1,
} as const;

/** Fallos del cable, distintos de un rechazo del agente. */
export class ErrorCable extends Error {
  constructor(mensaje: string) {
    super(mensaje);
    this.name = "ErrorCable";
  }
}

/**
 * Antepone el prefijo de longitud a una carga.
 *
 * La cota se comprueba **antes** de reservar, igual que en Rust.
 */
export function enmarcar(carga: Uint8Array): Buffer {
  if (carga.length > LIMITES.marcoMaximo) {
    throw new ErrorCable(
      `carga de ${carga.length} bytes; el máximo del marco es ${LIMITES.marcoMaximo}`,
    );
  }

  const marco = Buffer.allocUnsafe(LIMITES.prefijoLongitud + carga.length);
  marco.writeUInt32BE(carga.length, 0);
  marco.set(carga, LIMITES.prefijoLongitud);
  return marco;
}

/**
 * Compone la carga de una petición: nombre de canal y carga útil.
 *
 * La longitud del nombre va en **bytes**, no en caracteres. Todos los
 * identificadores son ASCII hoy, pero contar con `String.prototype.length`
 * produciría otros bytes en cuanto alguno dejara de serlo, y ese error no lo
 * detecta ninguna prueba que use sólo nombres ASCII.
 */
export function componerPeticion(canal: string, carga: Uint8Array): Buffer {
  const nombre = Buffer.from(canal, "utf8");

  if (nombre.length > LIMITES.nombreMaximo) {
    throw new ErrorCable(
      `nombre de canal de ${nombre.length} bytes; el máximo es ${LIMITES.nombreMaximo}`,
    );
  }

  const salida = Buffer.allocUnsafe(
    LIMITES.prefijoNombre + nombre.length + carga.length,
  );
  salida.writeUInt16BE(nombre.length, 0);
  salida.set(nombre, LIMITES.prefijoNombre);
  salida.set(carga, LIMITES.prefijoNombre + nombre.length);
  return salida;
}

/** Lo que el agente contesta: o sirve, o rechaza **con motivo**. */
export type Respuesta =
  | { readonly clase: "respuesta"; readonly cuerpo: Buffer }
  | { readonly clase: "rechazo"; readonly motivo: string };

/**
 * Interpreta la carga de una respuesta ya desenmarcada.
 *
 * ## Un rechazo no es un error de transporte
 *
 * `CODIGO_RECHAZO` es una respuesta **válida** que lleva un motivo. Convertirlo
 * en una excepción genérica perdería ese motivo, que es justo lo que RPT-036 §6
 * puso ahí para que VIS-04 no confunda «no hay nada» con «esto no lo sirve
 * nadie».
 */
export function interpretarRespuesta(carga: Uint8Array): Respuesta {
  const codigo = carga.at(0);

  if (codigo === undefined) {
    throw new ErrorCable("respuesta vacía: falta el código");
  }

  const cuerpo = Buffer.from(carga.subarray(1));

  if (codigo === LIMITES.codigoRespuesta) {
    return { clase: "respuesta", cuerpo };
  }
  if (codigo === LIMITES.codigoRechazo) {
    return { clase: "rechazo", motivo: cuerpo.toString("utf8") };
  }

  throw new ErrorCable(`código de respuesta desconocido: ${codigo}`);
}

/**
 * Reensambla marcos a partir de los trozos que entrega el socket.
 *
 * ## Por qué esto no es opcional
 *
 * `socket.on("data")` **no entrega mensajes**: entrega trozos arbitrarios. Puede
 * llegar medio marco, dos marcos juntos, o incluso el prefijo de longitud
 * partido por la mitad.
 *
 * Es el fallo clásico de todo cliente de protocolo con longitud, y no lo detecta
 * ninguna prueba que mande mensajes pequeños por un socket local: ahí casi
 * siempre llega todo de una pieza. Aparece en producción, con carga real.
 */
export class Acumulador {
  #pendiente: Buffer = Buffer.alloc(0);

  /**
   * Añade un trozo y devuelve los marcos **completos** que ya se pueden leer.
   *
   * Devuelve una lista, no un marco: un solo trozo puede contener varios.
   */
  empujar(trozo: Uint8Array): Buffer[] {
    this.#pendiente = Buffer.concat([this.#pendiente, Buffer.from(trozo)]);

    const marcos: Buffer[] = [];
    let desde = 0;

    for (;;) {
      const disponible = this.#pendiente.length - desde;
      if (disponible < LIMITES.prefijoLongitud) {
        break;
      }

      const declarada = this.#pendiente.readUInt32BE(desde);

      // La cota se comprueba **antes** de esperar o reservar nada. El prefijo
      // llega del otro extremo: sin esto, un valor absurdo dejaría al cliente
      // acumulando hasta agotar la memoria a instancias de quien habla.
      if (declarada > LIMITES.marcoMaximo) {
        throw new ErrorCable(
          `marco de ${declarada} bytes declarado; el máximo es ${LIMITES.marcoMaximo}`,
        );
      }

      const total = LIMITES.prefijoLongitud + declarada;
      if (disponible < total) {
        break;
      }

      // Se copia en lugar de compartir memoria con el acumulador: quien reciba
      // el marco no debe depender de que nadie lo reutilice después.
      marcos.push(
        Buffer.from(
          this.#pendiente.subarray(desde + LIMITES.prefijoLongitud, desde + total),
        ),
      );
      desde += total;
    }

    this.#pendiente =
      desde === 0 ? this.#pendiente : Buffer.from(this.#pendiente.subarray(desde));

    return marcos;
  }

  /** Bytes que quedan sin formar un marco completo. */
  get pendientes(): number {
    return this.#pendiente.length;
  }
}
