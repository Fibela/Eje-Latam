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
    // vuelva el normal. Dejarla abajo entre trece filas es exactamente cómo se
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
    // El agente siempre devuelve las trece condiciones. Si llega otra cosa, el
    // contrato cambió, y decirlo es mejor que suponer.
    const cabecera = componerCabecera({ clase: "vacio" });

    assert.equal(cabecera.urgencia, "critica");
    assert.ok(cabecera.datosDeAntes);
  });

  it("mientras se consulta, nada de lo que hay abajo es vigente", () => {
    assert.ok(componerCabecera({ clase: "consultando" }).datosDeAntes);
  });
});
