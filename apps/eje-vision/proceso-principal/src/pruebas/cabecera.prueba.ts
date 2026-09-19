/**
 * RPT-048 §2 — la cabecera del tablero.
 *
 * Lo que se comprueba aquí es sobre todo **el orden**: qué gana cuando dos cosas
 * son verdad a la vez. Un tablero que elija mal manda al operador a arreglar lo
 * segundo mientras lo primero sigue pasando.
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { componerCabecera } from "@eje/vision-base";
import type { Condiciones } from "@eje/vision-base";

/** Todo en calma. Base para encender una sola condición por prueba. */
const CALMA: Condiciones = {
  inventarioSuprimido: false,
  inventarioNoVerifica: false,
  observacionSaturada: false,
  capturaConPerdida: false,
  capturaNoDisponible: false,
  accionAdministrativa: false,
  salidaNoDisponible: false,
  // Calma incluye tener colector: un sensor que no informa a nadie no está en
  // calma, está sordo. Ver la prueba de PA-109 al final.
  sinColector: false,
  // Y tener escucha. RPT-070, PA-125: un sensor al que nadie puede preguntar no
  // está en calma, está incomunicado.
  escuchaNoDisponible: false,
  // Y con configuracion firmada: un sensor cuyos parametros los decide
  // quien controle el arranque no esta en calma (RPT-074, PA-79).
  configuracionSinFirmar: false,
  configuracionNoVerifica: false,
  // Y con clave de recuperación anclada. PA-150: un sensor que no se puede
  // revocar el día que su identidad se comprometa no está en calma, está sin
  // salida — y que hoy no le pase nada es justo lo que hace que nadie se
  // acuerde. Las otras dos son manipulación y por eso en calma van apagadas.
  recuperacionNoAprovisionada: false,
  recuperacionSuprimida: false,
  recuperacionNoVerifica: false,
  // PA-146b fase 3. Y con identidad anclada: un sensor cuya clave operativa
  // nadie declaró no está en calma, está indefenso ante una sustitución que no
  // se notaría. `operativaRotada` en `false` porque en calma la ceremonia está
  // cerrada, no a medias.
  operativaNoAprovisionada: false,
  operativaSuprimida: false,
  operativaNoVerifica: false,
  operativaRotada: false,
  // PA-156. En calma la clave anclada no está revocada.
  operativaRevocada: false,
  registroSaturado: false,
  evidenciaEnRiesgo: false,
};

function cabeceraDe(condiciones: Condiciones) {
  return componerCabecera({ clase: "datos", valor: condiciones });
}

describe("RPT-048 §2 — la cabecera decide cómo se lee todo lo demás", () => {
  it("en calma no hay cabecera y los datos son del ahora", () => {
    const cabecera = cabeceraDe(CALMA);

    assert.equal(cabecera.titulo, "");
    assert.equal(cabecera.urgencia, "normal");
    assert.equal(cabecera.datosDeAntes, false);
  });

  it("sin captura, lo de debajo es de antes", () => {
    // La consecuencia que da sentido a toda la cabecera. Sin esta marca, un
    // tablero con el sensor ciego se lee igual que uno con la red tranquila.
    const cabecera = cabeceraDe({ ...CALMA, capturaNoDisponible: true });

    assert.equal(cabecera.urgencia, "critica");
    assert.ok(cabecera.datosDeAntes, "el resto del tablero no es vigente");
    assert.match(cabecera.titulo, /no está vigilando/);
  });

  it("sin captura gana sobre cualquier otra condición", () => {
    // Si se anunciara la manipulación mientras el sensor está ciego, el operador
    // leeria el resto del tablero como actual. Lo primero es si se puede creer
    // lo que hay en pantalla.
    const cabecera = cabeceraDe({
      ...CALMA,
      capturaNoDisponible: true,
      inventarioSuprimido: true,
      registroSaturado: true,
    });

    assert.match(cabecera.titulo, /no está vigilando/);
    assert.ok(cabecera.datosDeAntes);
  });

  it("la manipulación se anuncia como incidente, no como aviso", () => {
    const cabecera = cabeceraDe({ ...CALMA, inventarioSuprimido: true });

    assert.equal(cabecera.urgencia, "critica");
    assert.match(cabecera.detalle, /incidente/);
    assert.equal(cabecera.datosDeAntes, false, "el sensor sigue observando");
  });

  it("una clave de recuperación sustituida ocupa la cabecera", () => {
    // PA-150 / PA-149. Esto lo descubrió una captura de pantalla, no una
    // prueba: en `eje-prueba`, con la clave sustituida y el agente gritando
    // «trate este equipo como comprometido», la cabecera titulaba sobre el
    // colector que falta. La fila decía la verdad y nadie miraba la fila.
    const cabecera = cabeceraDe({
      ...CALMA,
      sinColector: true,
      recuperacionNoVerifica: true,
    });

    assert.equal(cabecera.urgencia, "critica");
    assert.match(cabecera.titulo, /clave de recuperación/i);
    assert.doesNotMatch(
      cabecera.titulo,
      /colector/i,
      "una condición de atención no puede tapar una de manipulación",
    );
  });

  it("una clave de recuperación borrada también, y con otro texto", () => {
    // Borrar y sustituir se separaron en el contrato porque los remedios no se
    // parecen. Si la cabecera los contara igual, esa separación no llegaría al
    // único sitio donde alguien la lee.
    const borrada = cabeceraDe({ ...CALMA, recuperacionSuprimida: true });
    const sustituida = cabeceraDe({ ...CALMA, recuperacionNoVerifica: true });

    assert.equal(borrada.urgencia, "critica");
    assert.match(borrada.titulo, /borró|borro/i);
    assert.notEqual(
      borrada.titulo,
      sustituida.titulo,
      "sabotaje y secuestro no pueden titular igual",
    );
  });

  // PA-154, encontrado perturbando (RPT-101 §4). Las tres de abajo son el
  // espejo de las tres de arriba para la clave OPERATIVA, y hasta hoy no
  // existían: la fase 3 añadió los cuatro campos al literal `CALMA` —lo justo
  // para compilar— y se apoyó en que la barrera del contrato cubría el resto.
  //
  // No lo cubría. Esa barrera exige `urgencia === "critica"` y título no vacío;
  // acepta que dos condiciones distintas titulen exactamente igual. Es
  // deliberadamente laxa, porque es genérica y no puede saber qué prosa
  // corresponde a cada condición. Lo que no puede hacer una barrera derivada
  // hay que escribirlo a mano, y esto es lo que hay que escribir a mano.
  it("una identidad operativa sustituida ocupa la cabecera", () => {
    // El caso grave del par: alguien puso otra clave y ningún certificado la
    // autoriza. Lo que este sensor obedece cada día lo firma otro.
    const cabecera = cabeceraDe({
      ...CALMA,
      sinColector: true,
      operativaNoVerifica: true,
    });

    assert.equal(cabecera.urgencia, "critica");
    assert.match(cabecera.titulo, /identidad/i);
    assert.doesNotMatch(
      cabecera.titulo,
      /colector/i,
      "una condición de atención no puede tapar una de manipulación",
    );
  });

  it("la identidad operativa va antes que la de recuperación", () => {
    // El orden de la cascada es la prioridad, y aquí no es arbitrario: una
    // identidad sustituida dice que lo que el sensor obedece AHORA no es de
    // quien el operador cree; una clave de recuperación sustituida dice que
    // falta un remedio para mañana. Con las dos encendidas manda la primera.
    //
    // Sin esta prueba el orden de las ramas es un accidente del fichero, y el
    // día que alguien reordene la cascada el diff parecerá cosmético.
    const ambas = cabeceraDe({
      ...CALMA,
      operativaNoVerifica: true,
      recuperacionNoVerifica: true,
    });
    const soloOperativa = cabeceraDe({ ...CALMA, operativaNoVerifica: true });

    assert.equal(ambas.titulo, soloOperativa.titulo);
    assert.doesNotMatch(
      ambas.titulo,
      /recuperación/i,
      "con la identidad del sensor secuestrada, la cabecera no puede hablar " +
        "del remedio que falta para mañana",
    );
  });

  it("una identidad operativa borrada también, y con otro texto", () => {
    // Mismo motivo que en el par de recuperación: borrar y sustituir se
    // separaron en el contrato porque los remedios no se parecen, y si la
    // cabecera los contara igual esa separación no llegaría a quien la lee.
    const borrada = cabeceraDe({ ...CALMA, operativaSuprimida: true });
    const sustituida = cabeceraDe({ ...CALMA, operativaNoVerifica: true });

    assert.equal(borrada.urgencia, "critica");
    assert.match(borrada.titulo, /borró|borro/i);
    assert.notEqual(
      borrada.titulo,
      sustituida.titulo,
      "sabotaje y secuestro no pueden titular igual",
    );
  });

  it("la rotación legítima y la ceremonia pendiente NO ocupan la cabecera", () => {
    // Las otras dos del grupo operativo, y ninguna es manipulación.
    //
    // `operativaRotada` es el caso interesante: hay una clave distinta de la
    // anclada y un certificado verificado la nombra sucesora. Es exactamente lo
    // que se espera ver DESPUÉS de una rotación hecha bien. Ponerla en crítica
    // enseñaría al operador a ignorar la crítica justo después de cada
    // ceremonia legítima, que es la fatiga de alertas de PA-45 con el peor
    // disparador posible.
    //
    // Que sea condición y no estado transitorio ya se decidió: el agente no
    // reescribe su propia ancla, así que el sensor se queda ahí hasta que un
    // administrador vuelva a anclar. La fila lo dice; la cabecera calla.
    for (const condicion of ["operativaRotada", "operativaNoAprovisionada"] as const) {
      const cabecera = cabeceraDe({ ...CALMA, [condicion]: true });

      assert.equal(
        cabecera.urgencia,
        "normal",
        `'${condicion}' no es manipulación y no puede ocupar la cabecera`,
      );
      assert.equal(cabecera.titulo, "", `'${condicion}' no puede titular`);
    }
  });

  // PA-156. Escritas a mano y no delegadas a la barrera del contrato, que es la
  // lección de PA-154 aplicada el mismo día que se aprendió: aquella barrera
  // sólo recorre las condiciones que el contrato marca como manipulación, y ésta
  // declara `manipulacion = false`. **Para esta condición la barrera genérica no
  // comprueba absolutamente nada.**
  it("una identidad revocada ocupa la cabecera aunque no sea manipulación", () => {
    // El precedente es `registroSaturado`: crítica sin que nadie haya tocado el
    // almacén. Aquí el sensor obedece a una clave que dos custodios declararon
    // muerta, y por debajo del corte el agente sigue aceptando lo que firme.
    const cabecera = cabeceraDe({ ...CALMA, operativaRevocada: true });

    assert.equal(cabecera.urgencia, "critica");
    assert.match(cabecera.titulo, /revocada/i);

    // Se comprueba contra las frases que USAN las ramas de manipulación, y no
    // contra las palabras sueltas «ataque» o «comprometido».
    //
    // La primera versión de esta prueba hacía lo segundo y se puso roja con la
    // cascada correcta: el detalle dice «**No** es un ataque a esta máquina», y
    // una expresión regular no distingue una afirmación de su negación. Buscar
    // la palabra prohibía explicar por qué no aplica, que es justo lo que este
    // detalle tiene que hacer.
    for (const frase of [
      /responder como incidente/i,
      /tratar el equipo como comprometido/i,
    ]) {
      assert.doesNotMatch(
        cabecera.detalle,
        frase,
        "mandar a respuesta a incidentes por una ceremonia sin terminar es " +
          "llamar al equipo equivocado",
      );
    }

    // Y lo que sí tiene que decir: dónde está el remedio.
    assert.match(cabecera.detalle, /rotación/i);
  });

  it("una manipulación en curso manda sobre la identidad revocada", () => {
    // Los dos hechos son independientes y pueden coincidir. Si alguien está
    // dentro del almacén de este equipo, eso se lee antes que una rotación
    // pendiente: hay un ataque en curso y lo otro es una ceremonia a medias.
    const ambas = cabeceraDe({
      ...CALMA,
      operativaRevocada: true,
      inventarioNoVerifica: true,
    });

    assert.equal(ambas.urgencia, "critica");
    assert.match(ambas.titulo, /evidencia/i);
    assert.doesNotMatch(
      ambas.titulo,
      /revocada/i,
      "con alguien dentro del almacén, la cabecera no puede hablar de la ceremonia",
    );
  });

  it("la ceremonia pendiente NO ocupa la cabecera", () => {
    // `recuperacionNoAprovisionada` es la tercera del grupo y no es
    // manipulación: nadie tocó nada, falta una ceremonia. Subirla a la cabecera
    // pondría en crítica a todos los sensores recién instalados, y en un mes
    // nadie miraría la cabecera.
    const cabecera = cabeceraDe({
      ...CALMA,
      recuperacionNoAprovisionada: true,
    });

    assert.equal(cabecera.urgencia, "normal");
    assert.equal(cabecera.titulo, "");
  });

  it("el registro lleno es crítico y NO se presenta como manipulación", () => {
    // Tan grave como que alguien tocara el almacén, sin serlo. Mezclarlos manda
    // a alguien a buscar un atacante que no existe.
    const cabecera = cabeceraDe({ ...CALMA, registroSaturado: true });

    assert.equal(cabecera.urgencia, "critica");
    assert.doesNotMatch(cabecera.detalle, /alter|incidente/);
  });

  it("la salida caída sube a la cabecera aunque no sea la más grave", () => {
    // Es una de las dos condiciones que no viajan por syslog: si nadie mira
    // esta pantalla, nadie se entera. Por eso no puede quedarse en la lista.
    const cabecera = cabeceraDe({ ...CALMA, salidaNoDisponible: true });

    assert.equal(cabecera.urgencia, "atencion");
    assert.match(cabecera.detalle, /lo único que lo sabe/);
  });

  it("sin colector, el técnico en sitio se entera aquí o no se entera", () => {
    // PA-109. La otra condición que no puede viajar por syslog, y por una
    // imposibilidad: el aviso iría por el canal que no existe. Esta pantalla es
    // el único sitio donde un sensor sordo puede decir que lo está.
    const cabecera = cabeceraDe({ ...CALMA, sinColector: true });

    assert.equal(cabecera.urgencia, "atencion");
    assert.match(cabecera.detalle, /nadie fuera notará/);
    assert.equal(
      cabecera.datosDeAntes,
      false,
      "el sensor sigue observando: lo que no hace es contarlo fuera",
    );
  });

  it("una avería en curso gana sobre una instalación incompleta", () => {
    // Si las dos se anunciaran, «el colector no responde» sería lo que se lee, y
    // sin colector configurado no hay colector que responda. El orden importa
    // porque las dos frases mandan al técnico a sitios distintos.
    const cabecera = cabeceraDe({
      ...CALMA,
      salidaNoDisponible: true,
      sinColector: true,
    });

    assert.match(cabecera.titulo, /no están saliendo/);
  });

  it("sin escucha local se dice, aunque casi nunca pueda leerse aquí", () => {
    // RPT-070, PA-125. La rama más rara del fichero: para leer esta pantalla hay
    // que estar conectado, y si la condición está encendida no se puede estar.
    //
    // Tiene rama y prueba de todos modos por los dos casos en que sí llega: una
    // consulta que alcanzó al agente justo antes de caer, y un segundo agente en
    // la misma máquina cuyo socket responde. Sin esto, el técnico vería un panel
    // que no explica por qué el otro sensor no aparece.
    const cabecera = cabeceraDe({ ...CALMA, escuchaNoDisponible: true });

    assert.equal(cabecera.urgencia, "atencion");
    assert.match(cabecera.detalle, /ninguna consola puede preguntarle/);
    assert.equal(
      cabecera.datosDeAntes,
      false,
      "el sensor sigue observando y registrando: lo que no admite son preguntas",
    );
  });

  it("la sala se entera de la escucha caída aunque la consola no", () => {
    // La afirmación entera de PA-125, vista desde este lado. `sinColector` y
    // `salidaNoDisponible` describen el canal de syslog y por eso sólo llegan
    // aquí; `escuchaNoDisponible` describe el otro, y por eso sale por syslog.
    //
    // Es la única de las tres que este panel podría no ver nunca y que aun así
    // no se pierde: quien se entera es la sala. Lo sujeta
    // `un_sensor_incomunicado_lo_dice_por_el_canal_que_le_queda` en Rust; aquí se
    // deja escrito para que nadie "arregle" la asimetría igualándolas.
    const cabecera = cabeceraDe({ ...CALMA, escuchaNoDisponible: true });

    assert.notEqual(cabecera.urgencia, "normal");
  });

  it("una configuración que no verifica se dice sin acusar a nadie", () => {
    // RPT-074, PA-79. La firma rota apunta a manipulación, pero una máquina
    // ajena o una clave rotada dan la misma condición y no son un ataque.
    // Presentarla como incidente mandaría a respuesta a incidentes por un error
    // de despliegue, que es la fatiga de alertas de PA-45.
    const cabecera = cabeceraDe({ ...CALMA, configuracionNoVerifica: true });

    assert.equal(cabecera.urgencia, "atencion");
    assert.doesNotMatch(cabecera.detalle, /incidente|alter/);
    assert.match(cabecera.detalle, /diario del sensor/);
  });

  it("correr sin configuración firmada ocupa cabecera, no una fila", () => {
    // El riesgo de esta condición no es técnico: es que el estado degradado se
    // vuelva el normal. Dejarla abajo entre veintiuna filas es exactamente cómo se
    // aprende a ignorarla (RPT-074 §8).
    const cabecera = cabeceraDe({ ...CALMA, configuracionSinFirmar: true });

    assert.equal(cabecera.urgencia, "atencion");
    assert.match(cabecera.detalle, /ventana de silencio/);
    assert.equal(
      cabecera.datosDeAntes,
      false,
      "el sensor observa y registra: lo que no está firmado es qué se le pidió",
    );
  });

  it("una configuración rota gana sobre una que sólo falta", () => {
    // Las dos a la vez no pueden ocurrir —se derivan de un solo estado— pero si
    // la vista las tratara como independientes, «no verifica» es la que manda a
    // mirar el diario. El orden queda sujeto por si alguien las separa.
    const cabecera = cabeceraDe({
      ...CALMA,
      configuracionSinFirmar: true,
      configuracionNoVerifica: true,
    });

    assert.match(cabecera.titulo, /no verifica/);
  });

  it("una condición menor no ocupa la cabecera", () => {
    // `accionAdministrativa` es real y no cambia cómo se lee el tablero.
    // Subirla aquí es como se enseña a un operador a ignorar la cabecera.
    const cabecera = cabeceraDe({ ...CALMA, accionAdministrativa: true });

    assert.equal(cabecera.titulo, "");
    assert.equal(cabecera.urgencia, "normal");
  });

  it("sin agente la cabecera habla en lenguaje de operador", () => {
    const cabecera = componerCabecera({
      clase: "sinAgente",
      detalle: "[sin-escucha] el socket existe y no hay nadie escuchando",
    });

    assert.equal(cabecera.titulo, "Sensor desconectado");
    assert.ok(cabecera.datosDeAntes);
  });

  it("una respuesta inesperada se declara en lugar de elegir una rama", () => {
    // El agente siempre devuelve las veintiuna condiciones. Si llega otra cosa, el
    // contrato cambió, y decirlo es mejor que suponer.
    const cabecera = componerCabecera({ clase: "vacio" });

    assert.equal(cabecera.urgencia, "critica");
    assert.ok(cabecera.datosDeAntes);
  });

  it("mientras se consulta, nada de lo que hay abajo es vigente", () => {
    assert.ok(componerCabecera({ clase: "consultando" }).datosDeAntes);
  });
});
