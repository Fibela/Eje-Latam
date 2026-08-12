/**
 * Enlace con `eje-agente`: el segundo salto, ya sobre un conducto real.
 *
 * RPT-046. `cable.ts` sabe componer y leer bytes; este módulo los mueve.
 *
 * ## Una conexión por petición, y no es por comodidad
 *
 * El formato de marco **no lleva identificador de correlación**: un marco de
 * respuesta no dice a qué petición contesta. Sobre una conexión persistente con
 * dos peticiones en vuelo no hay forma de saber cuál de las dos respuestas es
 * cuál, y el emparejamiento erróneo no falla: devuelve el inventario de otro
 * momento, o las condiciones de otra consulta, en silencio.
 *
 * Mientras el contrato no tenga correlación, **una conexión por petición es la
 * única forma correcta**. Es más cara y da igual: son consultas de interfaz, no
 * un camino caliente.
 *
 * ## Por qué el conducto entra como parámetro
 *
 * Misma costura que `FabricaVentana`: sin ella, probar el reensamblado o el
 * vencimiento exigiría levantar un socket de verdad, y entonces no se prueban —
 * que es como se llega a producción sin haberlos ejercitado nunca.
 */

import {
  Acumulador,
  ErrorCable,
  type Respuesta,
  componerPeticion,
  enmarcar,
  interpretarRespuesta,
} from "./cable.js";

/** Lo que este módulo necesita de un socket. */
export interface Conducto {
  escribir(datos: Uint8Array): void;
  alRecibir(manejador: (trozo: Uint8Array) => void): void;
  alFallar(manejador: (error: Error) => void): void;
  alCerrar(manejador: () => void): void;
  cerrar(): void;
}

/** Abre un conducto nuevo hacia el agente. */
export type AbrirConducto = () => Conducto;

/**
 * Espera máxima por una respuesta.
 *
 * Sin ella, un agente que acepta la conexión y no contesta —vivo pero atascado—
 * deja la interfaz esperando para siempre, que es peor que un error: el operador
 * ve una pantalla que carga y no sabe que el sensor no está respondiendo.
 */
export const ESPERA_MAXIMA_MS = 5_000;

/** El agente no contestó a tiempo, o el conducto se cerró antes de hacerlo. */
export class ErrorEnlace extends Error {
  constructor(mensaje: string) {
    super(mensaje);
    this.name = "ErrorEnlace";
  }
}

/**
 * Causas de que una petición no obtenga respuesta.
 *
 * # Por qué hay un código además del texto
 *
 * PA-93. El texto de estos errores está escrito para un técnico —«el socket
 * existe y no hay nadie escuchando»— y es exactamente lo que hace falta en un
 * registro o en una terminal. En la pantalla de un operador de planta no sirve
 * de nada.
 *
 * La traducción a lenguaje de operador **no se hace aquí**: hacerla aquí
 * perdería el diagnóstico forense. Se hace en la capa base, que es donde vive la
 * presentación, y necesita algo estable con lo que decidir. Ese algo es este
 * código; el texto sigue viajando detrás, intacto.
 *
 * Un solo sitio decide la causa. Dos capas la leen a su manera.
 */
export const CAUSAS = {
  /** El fichero del socket existe y no hay nadie detrás. */
  sinEscucha: "sin-escucha",
  /** El socket no existe en esa ruta. */
  sinSocket: "sin-socket",
  /** El socket existe y esta consola no puede abrirlo. */
  sinPermiso: "sin-permiso",
  /** El agente aceptó la conexión y no contestó a tiempo. */
  sinRespuesta: "sin-respuesta",
  /** El agente estaba y cerró sin responder. */
  colgado: "colgado",
  /** El conducto no se pudo ni abrir. */
  noAbre: "no-abre",
  /** Cualquier otro fallo del transporte. */
  transporte: "transporte",
} as const;

/** Antepone la causa al texto, en un formato que la capa base sabe leer. */
function conCausa(causa: string, texto: string): string {
  return `[${causa}] ${texto}`;
}

/**
 * Traduce un fallo del conducto a algo que un técnico pueda accionar.
 *
 * ## Por qué esto no es cosmética
 *
 * Sobre un socket de dominio Unix, `ECONNREFUSED` significa algo muy concreto:
 * **el fichero del socket existe y no hay nadie escuchando detrás**. Es el
 * rastro de un agente que murió sin limpiar, y es el caso más frecuente en
 * campo.
 *
 * Comprobar que el fichero existe no lo detecta —existe—, y «el conducto falló»
 * no le dice al técnico que arranque el demonio. Es el mismo colapso de causas
 * distintas que las tres formas de no obtener respuesta (RPT-046 §5), con una
 * cuarta que se descubrió al usarlo.
 */
function diagnosticar(error: Error): string {
  const codigo = (error as { code?: unknown }).code;

  if (codigo === "ECONNREFUSED") {
    return conCausa(
      CAUSAS.sinEscucha,
      "el socket existe y no hay nadie escuchando: el agente no está en " +
        "marcha y dejó su fichero atrás. Arranca 'eje-agente'",
    );
  }
  if (codigo === "ENOENT") {
    return conCausa(
      CAUSAS.sinSocket,
      "no existe el socket: el agente nunca llegó a abrirlo en esta ruta",
    );
  }
  if (codigo === "EACCES") {
    // RPT-046, PA-82: el socket se crea en 0600. Si el agente corre como
    // servicio y la consola en la sesión del operador, esto es lo que pasa.
    return conCausa(
      CAUSAS.sinPermiso,
      "sin permiso sobre el socket: el agente lo creó para su propio usuario " +
        "y esta consola corre como otro (PA-82)",
    );
  }

  return conCausa(CAUSAS.transporte, `el conducto falló: ${error.message}`);
}

/**
 * Hace una petición y espera **un** marco de respuesta.
 *
 * @param abrir Fábrica de conductos. En producción envuelve a `net.connect`.
 * @param canal Identificador del canal, ya validado por el guardián.
 * @param carga Carga útil, ya serializada.
 * @param esperaMs Espera máxima antes de rendirse.
 */
export function pedir(
  abrir: AbrirConducto,
  canal: string,
  carga: Uint8Array,
  esperaMs: number = ESPERA_MAXIMA_MS,
): Promise<Respuesta> {
  return new Promise<Respuesta>((cumplir, fallar) => {
    const acumulador = new Acumulador();
    let resuelto = false;
    let conducto: Conducto;

    const cerrar = (): void => {
      resuelto = true;
      clearTimeout(vencimiento);
      conducto.cerrar();
    };

    const vencimiento = setTimeout(() => {
      if (resuelto) {
        return;
      }
      cerrar();
      fallar(
        new ErrorEnlace(
          conCausa(
            CAUSAS.sinRespuesta,
            `el agente no respondió a «${canal}» en ${esperaMs} ms`,
          ),
        ),
      );
    }, esperaMs);

    // ## Por qué NO se llama a `unref()` sobre el vencimiento
    //
    // Parece prudente —«que un temporizador no mantenga vivo el proceso»— y es
    // un error. Mientras una petición está en vuelo, el proceso **debe** seguir
    // vivo hasta que se resuelva; eso es precisamente lo que se está esperando.
    //
    // Con `unref()`, si no queda ningún otro asa viva, el bucle de eventos se
    // vacía y la promesa nunca se resuelve ni se rechaza: queda colgada. En
    // Node eso aparece como «Promise resolution is still pending but the event
    // loop has already resolved», y **depende de si hay otro trabajo asíncrono
    // por casualidad** — pasó en verde tres veces antes de fallar.
    //
    // No hace falta: todos los caminos de salida llaman a `clearTimeout`, así
    // que el temporizador nunca sobrevive a la petición.

    try {
      conducto = abrir();
    } catch (error) {
      clearTimeout(vencimiento);
      fallar(
        new ErrorEnlace(
          conCausa(CAUSAS.noAbre, `no se pudo abrir el conducto: ${String(error)}`),
        ),
      );
      return;
    }

    conducto.alRecibir((trozo) => {
      if (resuelto) {
        return;
      }
      try {
        const marcos = acumulador.empujar(trozo);
        const primero = marcos[0];
        if (primero === undefined) {
          return;
        }
        // Se toma el primero y se cierra. Un segundo marco sobre la misma
        // conexión no tiene lectura posible sin correlación: descartarlo es lo
        // honesto, atribuirlo a esta petición sería inventárselo.
        cerrar();
        cumplir(interpretarRespuesta(primero));
      } catch (error) {
        cerrar();
        fallar(error instanceof ErrorCable ? error : new ErrorEnlace(String(error)));
      }
    });

    conducto.alFallar((error) => {
      if (resuelto) {
        return;
      }
      cerrar();
      fallar(new ErrorEnlace(diagnosticar(error)));
    });

    conducto.alCerrar(() => {
      if (resuelto) {
        return;
      }
      clearTimeout(vencimiento);
      resuelto = true;
      // Cierre limpio sin respuesta. No es lo mismo que un vencimiento —el
      // agente estaba y colgó— y el mensaje debe permitir distinguirlo.
      fallar(
        new ErrorEnlace(
          conCausa(CAUSAS.colgado, `el agente cerró la conexión sin responder a «${canal}»`) +
            (acumulador.pendientes > 0
              ? `; quedaron ${acumulador.pendientes} byte(s) de un marco incompleto`
              : ""),
        ),
      );
    });

    try {
      conducto.escribir(enmarcar(componerPeticion(canal, carga)));
    } catch (error) {
      cerrar();
      fallar(error instanceof ErrorCable ? error : new ErrorEnlace(String(error)));
    }
  });
}
