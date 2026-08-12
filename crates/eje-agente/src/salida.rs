//! Salida de alertas fuera del equipo, por syslog.
//!
//! RPT-032, PA-42.
//!
//! # El formato se separa del envio
//!
//! `linea_de_suceso` y `linea_de_transicion` son funciones puras: reciben datos
//! y devuelven texto. El socket vive detras de [`Despacho`].
//!
//! No es una preferencia de comprobabilidad. Es que **la parte que se puede
//! equivocar en silencio es el formato** —un campo mal escapado, un marco mal
//! contado— y esa parte tiene que ser probable sin red. El envio falla de forma
//! ruidosa y no necesita la misma disciplina.
//!
//! # El marcado de longitud, y el ataque que cierra
//!
//! RFC 6587 admite dos formas de delimitar mensajes sobre TCP: contar octetos, o
//! terminar en salto de linea. Aqui se cuenta.
//!
//! Con delimitacion por salto de linea, **un salto dentro del mensaje inyecta
//! una linea de syslog completa**. Quien controle cualquier texto que acabe en
//! el mensaje —hoy solo nuestro codigo, manana quiza el nombre de un
//! dispositivo— podria escribir entradas falsas en el SIEM del cliente,
//! atribuidas al agente.
//!
//! Contar octetos elimina la clase entera. Y aun asi se saneen los caracteres de
//! control, porque una defensa que depende de que nadie cambie el marco es una
//! defensa que caduca.

use std::fmt::Write as _;

use eje_ipc::mensajes::{Condiciones, SucesoAlerta};

/// Nombre de la aplicacion en el campo `APP-NAME` de RFC 5424.
pub const APLICACION: &str = "eje-agente";

/// Facilidad de syslog: `13` es *log audit*.
///
/// No `local0`..`local7`: esos los reparte el cliente entre sus propios usos y
/// ocupar uno obligaria a negociarlo en cada despliegue. *Log audit* describe lo
/// que esto es.
pub const FACILIDAD: u8 = 13;

/// Gravedad de RFC 5424 que corresponde a cada cosa que emitimos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravedad {
    /// `1` — hay que actuar de inmediato.
    ///
    /// Se reserva a la amenaza incontenible: no existe accion automatica posible
    /// y la unica respuesta es humana.
    Alerta,
    /// `3` — condicion de error. Alguien toco el almacen.
    Error,
    /// `4` — aviso. Degradacion que exige accion administrativa.
    Aviso,
    /// `6` — informativo. Una condicion degradada que se resolvio.
    Informativo,
}

impl Gravedad {
    /// Codigo numerico de RFC 5424.
    #[must_use]
    pub const fn codigo(self) -> u8 {
        match self {
            Self::Alerta => 1,
            Self::Error => 3,
            Self::Aviso => 4,
            Self::Informativo => 6,
        }
    }

    /// Prioridad completa: facilidad por ocho mas gravedad.
    #[must_use]
    pub const fn prioridad(self) -> u16 {
        FACILIDAD as u16 * 8 + self.codigo() as u16
    }
}

/// Fallos del envio.
#[derive(Debug, thiserror::Error)]
pub enum ErrorSalida {
    /// El colector no acepta la conexion o la corto.
    #[error("el colector de syslog no esta disponible: {detalle}")]
    NoDisponible {
        /// Motivo.
        detalle: String,
    },
}

/// Por donde sale la alerta.
///
/// Es un rasgo y no una funcion concreta para que las pruebas puedan observar
/// **exactamente que bytes** se emitirian sin abrir un socket. Un formato de
/// cable que solo se prueba contra un colector real es un formato que nadie
/// prueba.
pub trait Despacho {
    /// Emite una linea ya marcada en longitud.
    ///
    /// # Errores
    ///
    /// [`ErrorSalida::NoDisponible`] si el colector no responde.
    fn enviar(&mut self, marco: &[u8]) -> Result<(), ErrorSalida>;
}

/// Sanea un texto para que quepa en un mensaje de syslog.
///
/// Sustituye cualquier caracter de control por un espacio. Con marcado de
/// longitud, un salto de linea ya no inyecta una linea nueva; esto es la segunda
/// linea de defensa, para el dia que alguien cambie el marco.
#[must_use]
pub fn sanear(texto: &str) -> String {
    texto
        .chars()
        .map(
            |caracter| {
                if caracter.is_control() { ' ' } else { caracter }
            },
        )
        .collect()
}

/// Convierte milisegundos desde la epoca a la marca de tiempo de RFC 5424.
///
/// # Por que se calcula a mano
///
/// Traer una dependencia de fechas por una funcion de veinte lineas anadiria
/// superficie a un binario que corre con privilegios de captura. El algoritmo es
/// el civil-from-days de Howard Hinnant, que es publico, exacto para todo el
/// rango de `i64` util y **probable contra fechas conocidas**, que es lo que las
/// pruebas hacen.
///
/// Un instante anterior a la epoca produce una fecha anterior a 1970, que es
/// correcta y absurda: si aparece, el reloj del sensor esta mal y eso es en si
/// mismo una noticia.
#[must_use]
pub fn marca_de_tiempo(milisegundos: i64) -> String {
    let (dias, resto) = (
        milisegundos.div_euclid(86_400_000),
        milisegundos.rem_euclid(86_400_000),
    );

    let (anio, mes, dia) = civil_desde_dias(dias);

    let hora = resto / 3_600_000;
    let minuto = (resto % 3_600_000) / 60_000;
    let segundo = (resto % 60_000) / 1_000;
    let milis = resto % 1_000;

    format!("{anio:04}-{mes:02}-{dia:02}T{hora:02}:{minuto:02}:{segundo:02}.{milis:03}Z")
}

/// Fecha civil a partir de dias desde la epoca.
const fn civil_desde_dias(dias: i64) -> (i64, u32, u32) {
    let z = dias + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };

    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// Compone una linea de RFC 5424 ya marcada en longitud (RFC 6587).
fn componer(
    gravedad: Gravedad,
    instante_utc: i64,
    maquina: &str,
    identificador: &str,
    mensaje: &str,
) -> Vec<u8> {
    let mut linea = String::new();

    // `write!` sobre un `String` no puede fallar; el resultado se ignora a
    // proposito en lugar de desenvolverlo, que las lindes prohiben.
    let _ = write!(
        linea,
        "<{}>1 {} {} {APLICACION} - {} - {}",
        gravedad.prioridad(),
        marca_de_tiempo(instante_utc),
        sanear(maquina),
        sanear(identificador),
        sanear(mensaje)
    );

    let mut marco = format!("{} ", linea.len()).into_bytes();
    marco.extend_from_slice(linea.as_bytes());
    marco
}

/// Linea correspondiente a un suceso de alerta.
///
/// El numero de asiento viaja en el mensaje como **referencia cruzada** a
/// ALM-01: el SIEM sabe que ocurrio y donde esta la evidencia completa, sin que
/// se dupliquen las dos.
#[must_use]
pub fn linea_de_suceso(suceso: &SucesoAlerta, instante_utc: i64, maquina: &str) -> Vec<u8> {
    componer(
        Gravedad::Alerta,
        instante_utc,
        maquina,
        "amenaza-incontenible",
        &format!(
            "asiento={} dispositivo={} {}",
            suceso.asiento, suceso.dispositivo, suceso.detalle
        ),
    )
}

/// Marco de un sello: el extremo del registro, hacia el testigo externo.
///
/// RPT-038, PA-64.
///
/// # Que es esto y que no
///
/// **No es una alerta.** Es una constancia: dice «a estas alturas mi registro
/// terminaba asi». Va con gravedad informativa porque no le pide nada al
/// operador; le pide algo al colector, que es quien correlaciona.
///
/// # Por que existe
///
/// El ancla de RPT-033 detecta el recorte del registro, pero es un fichero mas
/// en el mismo disco: quien recorta puede recalcularla y escribirla, y el cotejo
/// dice `Conforme`. Una firma local no lo arregla, porque la clave viviria donde
/// el atacante escribe (RPT-038 §2).
///
/// Lo que si lo arregla es que el extremo haya salido de la maquina. El colector
/// guarda la serie; si el registro local retrocede o su extremo deja de coincidir
/// con el que se anoto para ese asiento, la discrepancia se ve **fuera** del
/// equipo comprometido, que es el unico sitio donde puede verse.
#[must_use]
pub fn linea_de_sello(numero: u64, sello: &str, instante_utc: i64, maquina: &str) -> Vec<u8> {
    componer(
        Gravedad::Informativo,
        instante_utc,
        maquina,
        "sello-de-evidencia",
        &format!("sello={sello} asiento={numero}"),
    )
}

/// Condicion que cambio de valor entre dos ciclos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transicion {
    /// Identificador estable de la condicion.
    pub condicion: &'static str,
    /// Valor nuevo.
    pub activa: bool,
    /// Si la condicion, cuando esta activa, indica manipulacion.
    pub manipulacion: bool,
}

impl Transicion {
    /// Gravedad con la que se emite.
    ///
    /// Una condicion que **se resuelve** es informativa: el operador quiere
    /// saberlo y no le exige nada.
    #[must_use]
    pub const fn gravedad(&self) -> Gravedad {
        if !self.activa {
            return Gravedad::Informativo;
        }
        if self.manipulacion {
            return Gravedad::Error;
        }
        Gravedad::Aviso
    }
}

/// Condiciones que se emiten, con su identificador y si acusan a alguien.
///
/// **`salidaNoDisponible` no figura**, y no por olvido: es la condicion que dice
/// que no se puede emitir. Emitirla exigiria el canal que acaba de fallar.
/// Viaja solo por IPC, que es donde VIS-04 la consulta (RPT-032 §4).
const EMISIBLES: [(&str, bool); 8] = [
    ("inventarioSuprimido", true),
    ("inventarioNoVerifica", true),
    ("observacionSaturada", false),
    ("capturaConPerdida", false),
    // RPT-047 §2, PA-81. La mas grave de las ocho, y la que justifica que el
    // agente siga vivo cuando no puede capturar.
    //
    // Un proceso muerto lo reinicia el supervisor y alguien se entera. Un agente
    // vivo que no observa puede pasar por sano durante meses: el proceso existe,
    // el socket responde, el panel pinta. Cambiar «muere ruidosamente» por «vive
    // en silencio» solo es admisible si esto sale por syslog con la gravedad de
    // una amenaza incontenible.
    //
    // Con `false` aqui, las herramientas del cliente leerian una red muy
    // tranquila mientras el sensor esta ciego. Ese es exactamente el fallo que
    // este producto existe para no tener.
    ("capturaNoDisponible", true),
    ("accionAdministrativa", false),
    // `registroSaturado` viaja con la gravedad de la manipulacion sin serlo. El
    // segundo campo no dice "esto es un ataque" sino "esto no puede esperar al
    // lunes", y un sensor que dejo de registrar amenazas no puede.
    //
    // Que no sea manipulacion se conserva donde importa: `hay_manipulacion()` no
    // lo incluye, asi que VIS-04 no lo presentara como que alguien toco nada.
    ("registroSaturado", true),
    // Perder durabilidad de la evidencia no es que alguien la tocara, pero
    // tampoco puede esperar: mientras dure, un corte de luz se lleva alertas.
    ("evidenciaEnRiesgo", true),
];

/// Valor de una condicion por su identificador.
///
/// Devuelve `None` para un identificador que no corresponde a ningun campo. Es
/// deliberado: un `_ => false` haria que una condicion mal escrita en
/// [`EMISIBLES`] pareciera apagada **para siempre**, y una condicion que nunca
/// se activa no la echa de menos nadie.
fn valor_de(condiciones: &Condiciones, identificador: &str) -> Option<bool> {
    match identificador {
        "inventarioSuprimido" => Some(condiciones.inventario_suprimido),
        "inventarioNoVerifica" => Some(condiciones.inventario_no_verifica),
        "observacionSaturada" => Some(condiciones.observacion_saturada),
        "capturaConPerdida" => Some(condiciones.captura_con_perdida),
        "capturaNoDisponible" => Some(condiciones.captura_no_disponible),
        "accionAdministrativa" => Some(condiciones.accion_administrativa),
        "registroSaturado" => Some(condiciones.registro_saturado),
        "evidenciaEnRiesgo" => Some(condiciones.evidencia_en_riesgo),
        _ => None,
    }
}

/// Condiciones que cambiaron entre dos ciclos.
///
/// # Por que la transicion y no la condicion
///
/// Las condiciones son verdaderas hasta que alguien interviene (RPT-019 §2).
/// Emitirlas en cada ciclo inundaria el SIEM con la misma noticia, que es el
/// defecto que aquel reporte evito al no anexarlas a ALM-01.
///
/// `anterior` en `None` es el primer ciclo: se emite lo que este **activo**, y
/// nada de lo que este apagado. Emitir «todo apagado» al arrancar seria ruido
/// puro.
#[must_use]
pub fn transiciones(anterior: Option<&Condiciones>, actual: &Condiciones) -> Vec<Transicion> {
    EMISIBLES
        .into_iter()
        .filter_map(|(condicion, manipulacion)| {
            let ahora = valor_de(actual, condicion)?;
            let antes = anterior.is_some_and(|previo| valor_de(previo, condicion) == Some(true));

            if ahora == antes {
                return None;
            }

            Some(Transicion {
                condicion,
                activa: ahora,
                manipulacion,
            })
        })
        .collect()
}

/// Linea correspondiente a una transicion de condicion.
#[must_use]
pub fn linea_de_transicion(transicion: &Transicion, instante_utc: i64, maquina: &str) -> Vec<u8> {
    componer(
        transicion.gravedad(),
        instante_utc,
        maquina,
        "condicion",
        &format!(
            "condicion={} estado={}",
            transicion.condicion,
            if transicion.activa {
                "activa"
            } else {
                "resuelta"
            }
        ),
    )
}

/// Despacho por TCP hacia un colector de syslog.
///
/// # Solo emite
///
/// No hay lectura del socket, ni siquiera para descartar. RPT-031 §2 exige que
/// la interfaz de gestion sea de emision pura, y aqui eso es literal: **no
/// existe metodo que lea**, del mismo modo que en `eje-captura` no existe metodo
/// que envie.
///
/// La conexion se abre en cada envio y se cierra al soltarse. Mantenerla abierta
/// seria mas eficiente y dejaria un descriptor vivo hacia la red de gestion
/// entre alerta y alerta, que son horas o dias. Para un volumen de alertas raras
/// y graves, el coste de reconectar es irrelevante y la ventana cerrada vale
/// mas.
pub struct DespachoTcp {
    destino: String,
    plazo: std::time::Duration,
}

impl DespachoTcp {
    /// Despacho hacia `destino`, con el plazo dado para conectar y escribir.
    #[must_use]
    pub fn nuevo(destino: &str, plazo: std::time::Duration) -> Self {
        Self {
            destino: destino.to_owned(),
            plazo,
        }
    }
}

impl Despacho for DespachoTcp {
    fn enviar(&mut self, marco: &[u8]) -> Result<(), ErrorSalida> {
        use std::io::Write as _;
        use std::net::{TcpStream, ToSocketAddrs as _};

        let fallo = |detalle: String| ErrorSalida::NoDisponible { detalle };

        let direccion = self
            .destino
            .to_socket_addrs()
            .map_err(|error| fallo(error.to_string()))?
            .next()
            .ok_or_else(|| fallo(format!("'{}' no resuelve", self.destino)))?;

        let mut flujo = TcpStream::connect_timeout(&direccion, self.plazo)
            .map_err(|error| fallo(error.to_string()))?;

        // El plazo de escritura es tan necesario como el de conexion: un colector
        // que acepta y no lee dejaria al agente bloqueado indefinidamente, y con
        // el la observacion detenida.
        flujo
            .set_write_timeout(Some(self.plazo))
            .map_err(|error| fallo(error.to_string()))?;

        flujo
            .write_all(marco)
            .map_err(|error| fallo(error.to_string()))?;

        flujo.flush().map_err(|error| fallo(error.to_string()))
    }
}

/// Emisor con memoria del ciclo anterior.
pub struct Emisor<D> {
    despacho: D,
    anteriores: Option<Condiciones>,
    /// Ultimo sello **efectivamente entregado** al colector.
    ultimo_sello: Option<(u64, String)>,
    maquina: String,
}

impl<D: Despacho> Emisor<D> {
    /// Emisor sobre el despacho dado.
    pub fn nuevo(despacho: D, maquina: &str) -> Self {
        Self {
            despacho,
            anteriores: None,
            ultimo_sello: None,
            maquina: maquina.to_owned(),
        }
    }

    /// Emite el extremo del registro hacia el testigo externo.
    ///
    /// RPT-038, PA-64. Devuelve `false` si el envio fallo.
    ///
    /// # Solo si cambio
    ///
    /// El extremo cambia exactamente cuando cambia el registro, asi que emitir en
    /// cada cambio **es** emitir solo lo que cambia (RPT-032 §3). No es un latido
    /// periodico: eso si inundaria.
    ///
    /// # Y aqui el fallo si se reintenta, al reves que en [`Self::emitir`]
    ///
    /// `anteriores` se actualiza aunque el envio falle, porque reemitir una
    /// transicion pasada le mostraria al operador un incidente que no ocurrio.
    ///
    /// Con el sello la asimetria es la contraria: un sello que no llego es un
    /// **hueco en la cadena del testigo**, y reenviar el extremo vigente cuando
    /// el colector vuelve no cuenta nada falso — cuenta lo que sigue siendo
    /// cierto. Por eso `ultimo_sello` se actualiza **solo tras un envio
    /// correcto**.
    pub fn sellar(&mut self, numero: u64, sello: &str, instante_utc: i64) -> bool {
        if self
            .ultimo_sello
            .as_ref()
            .is_some_and(|(anterior, valor)| *anterior == numero && valor == sello)
        {
            return true;
        }

        if self
            .despacho
            .enviar(&linea_de_sello(numero, sello, instante_utc, &self.maquina))
            .is_err()
        {
            return false;
        }

        self.ultimo_sello = Some((numero, sello.to_owned()));
        true
    }

    /// Emite los sucesos nuevos y las transiciones de condicion.
    ///
    /// # El fallo no se traga y no se reintenta sin cota
    ///
    /// Devuelve `false` si algo no se pudo enviar. Quien llama lo convierte en la
    /// condicion `salidaNoDisponible`, que viaja por IPC.
    ///
    /// **No hay cola.** Una cola de alertas no enviadas que crece sin limite es
    /// el agotamiento de memoria de RPT-018 §6 con otro nombre. Lo que no sale
    /// sigue en ALM-01, que es para lo que existe.
    ///
    /// El estado anterior se actualiza **aunque el envio falle**: si no, al
    /// recuperarse el colector se reemitirian transiciones ya pasadas como si
    /// fueran nuevas, y el operador veria un incidente que no ocurrio.
    pub fn emitir(
        &mut self,
        sucesos: &[SucesoAlerta],
        condiciones: &Condiciones,
        instante_utc: i64,
    ) -> bool {
        let mut todo_bien = true;

        for suceso in sucesos {
            if self
                .despacho
                .enviar(&linea_de_suceso(suceso, instante_utc, &self.maquina))
                .is_err()
            {
                todo_bien = false;
            }
        }

        for transicion in transiciones(self.anteriores.as_ref(), condiciones) {
            if self
                .despacho
                .enviar(&linea_de_transicion(
                    &transicion,
                    instante_utc,
                    &self.maquina,
                ))
                .is_err()
            {
                todo_bien = false;
            }
        }

        self.anteriores = Some(*condiciones);
        todo_bien
    }
}

#[cfg(test)]
mod pruebas_emisibles {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::{Condiciones, EMISIBLES, valor_de};

    /// Toda condicion sale al SIEM salvo la que no puede emitirse.
    ///
    /// # Por que existe esta prueba
    ///
    /// PA-91. `Condiciones` crecio a nueve campos y `EMISIBLES` se quedo en
    /// siete durante varios turnos **sin que nada protestara**. El defecto no es
    /// ruidoso: una condicion que no figura aqui se calcula, se sirve por el
    /// puente, se pinta en VIS-04 — y no sale nunca hacia el SIEM del cliente.
    /// Quien vigile por syslog vera una red tranquila.
    ///
    /// # Como obliga
    ///
    /// La desestructuracion es exhaustiva y **sin `..`**. Anadir un decimo campo
    /// a `Condiciones` deja de compilar aqui, y quien lo anada tiene que decidir
    /// a proposito si va al SIEM o si es otra excepcion. No se puede olvidar,
    /// que es la unica forma de que no se olvide.
    #[test]
    fn toda_condicion_sale_al_siem_salvo_la_que_no_puede() {
        let condiciones = Condiciones {
            inventario_suprimido: true,
            inventario_no_verifica: true,
            observacion_saturada: true,
            captura_con_perdida: true,
            captura_no_disponible: true,
            accion_administrativa: true,
            salida_no_disponible: true,
            registro_saturado: true,
            evidencia_en_riesgo: true,
        };

        let Condiciones {
            inventario_suprimido,
            inventario_no_verifica,
            observacion_saturada,
            captura_con_perdida,
            captura_no_disponible,
            accion_administrativa,
            salida_no_disponible,
            registro_saturado,
            evidencia_en_riesgo,
        } = condiciones;

        let todas = [
            ("inventarioSuprimido", inventario_suprimido),
            ("inventarioNoVerifica", inventario_no_verifica),
            ("observacionSaturada", observacion_saturada),
            ("capturaConPerdida", captura_con_perdida),
            ("capturaNoDisponible", captura_no_disponible),
            ("accionAdministrativa", accion_administrativa),
            ("salidaNoDisponible", salida_no_disponible),
            ("registroSaturado", registro_saturado),
            ("evidenciaEnRiesgo", evidencia_en_riesgo),
        ];

        for (identificador, valor) in todas {
            let emitida = EMISIBLES.iter().any(|(nombre, _)| *nombre == identificador);

            // `valor_de` debe mapear EXACTAMENTE lo emisible: ni menos —una
            // condicion que `EMISIBLES` nombra y `valor_de` no conoce se queda
            // apagada para siempre, que es el aviso de su propia documentacion—
            // ni mas, porque un mapeo que sobra sugiere que algo se emite y no
            // se emite.
            assert_eq!(
                valor_de(&condiciones, identificador),
                if emitida { Some(valor) } else { None },
                "'{identificador}': `valor_de` y `EMISIBLES` no dicen lo mismo"
            );

            if identificador == "salidaNoDisponible" {
                // La unica excepcion, y no es una decision de estilo: emitirla
                // exigiria el canal que acaba de fallar. Llega solo por el
                // puente, que es donde VIS-04 la consulta.
                assert!(
                    !emitida,
                    "'salidaNoDisponible' no puede salir por el canal que fallo"
                );
            } else {
                assert!(
                    emitida,
                    "'{identificador}' no figura en EMISIBLES: se calcula, se sirve \
                     y NO llega al SIEM del cliente"
                );
            }
        }

        assert_eq!(
            EMISIBLES.len(),
            todas.len() - 1,
            "EMISIBLES debe cubrir todas las condiciones menos salidaNoDisponible"
        );
    }
}
