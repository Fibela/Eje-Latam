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
/// # Y por que declara la interfaz
///
/// RPT-061, PA-115. La serie que guarda el colector se indexa por quien la
/// emite, y hasta hoy eso era solo el `HOSTNAME`. Dos agentes en un mismo
/// servidor perimetral —un sensor por segmento— entrelazaban sus series: sus
/// registros son independientes y de longitudes distintas, asi que el cotejo
/// leia un registro que encoge y **acusaba de recorte a dos sensores intactos**.
///
/// Se observo en ejecucion real antes de corregirlo (RPT-061 §3).
#[must_use]
pub fn linea_de_sello(datos: &DatosSello<'_>) -> Vec<u8> {
    let DatosSello {
        numero,
        sello,
        instante_utc,
        maquina,
        interfaz,
    } = *datos;

    componer(
        Gravedad::Informativo,
        instante_utc,
        maquina,
        "sello-de-evidencia",
        &format!(
            "sello={sello} asiento={numero} interfaz={}",
            sanear(interfaz)
        ),
    )
}

/// Todo lo que una linea de sello necesita saber.
///
/// Con nombres por el mismo motivo que [`DatosLatido`]: `maquina` e `interfaz`
/// son los dos `&str`, e invertirlos compilaria sin una queja y volveria a
/// colapsar la identidad que PA-115 acaba de separar.
#[derive(Debug, Clone, Copy)]
pub struct DatosSello<'a> {
    /// Ultimo asiento del registro.
    pub numero: u64,
    /// Extremo de la cadena de resumenes, en hexadecimal.
    pub sello: &'a str,
    /// Instante para la marca de tiempo de RFC 5424.
    pub instante_utc: i64,
    /// Maquina, para el campo `HOSTNAME`.
    pub maquina: &'a str,
    /// Interfaz que vigila este agente.
    pub interfaz: &'a str,
}

/// Intervalo por omision entre latidos, en milisegundos.
///
/// RPT-052 §5, PA-104. **Es una hipotesis declarada, no una medida.**
///
/// Pocos intervalos levantan a la sala de madrugada por un corte de diez
/// segundos; muchos dejan un sensor muerto sin detectar justo ese tiempo. No hay
/// cifra correcta sin medir la red del cliente, que es PA-41 con otro nombre.
///
/// Por eso el valor **viaja dentro del propio latido**: el colector no tiene que
/// suponerlo, y cuando pase a configuracion firmada (PA-79) el cambio no rompe
/// nada al otro lado.
pub const INTERVALO_LATIDO_MS: i64 = 60_000;

/// Que ocurrio con el latido en una vuelta. RPT-052, PA-104.
///
/// # Por que cuatro estados y no un booleano
///
/// Desde fuera del proceso, «no tocaba», «no hay colector» y «tocaba y no pude»
/// producen exactamente lo mismo: **ninguna linea**. Por dentro son tres cosas
/// distintas, y una de ellas es un sensor mudo.
///
/// Colapsarlas en un `bool` haria que el resumen por pantalla dijera «sin
/// latido» en los tres casos y que el operador no pudiera distinguir el
/// funcionamiento normal de la averia. Es RPT-006 §4 aplicado al latido: no se
/// sabe no es no hay, y no toca no es no puedo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Latido {
    /// Salio hacia el colector en esta vuelta.
    Emitido,
    /// Aun no habia pasado el intervalo. Es el caso normal y no es un fallo.
    NoTocaba,
    /// Este agente no tiene colector configurado, asi que no late nunca.
    ///
    /// No es una averia del canal: es una decision de despliegue. Un agente sin
    /// colector **no esta cubierto por PA-105**, y decirlo aqui es lo que
    /// impide que alguien lo instale creyendo que la sala lo vigila.
    SinColector,
    /// Tocaba latir y el despacho fallo.
    ///
    /// El instante no se marca, asi que la vuelta siguiente lo reintenta.
    NoSePudo,
}

/// Interpreta el valor de `--syslog`: una cadena vacia **no es un destino**.
///
/// RPT-064, PA-118.
///
/// # Por que existe esta funcion de tres lineas
///
/// `systemd` no tiene condicionales en `ExecStart`, y `${VARIABLE}` se sustituye
/// como **un argumento, vacio incluido** —a diferencia de `$VARIABLE`, que se
/// parte en palabras—. Asi que una unidad con `--syslog ${EJE_COLECTOR}` y la
/// variable vacia entregaba literalmente `--syslog ""`.
///
/// Esa unidad ya no existe: desde RPT-077 el colector sale de la configuracion
/// firmada y `ExecStart` no lo pasa. La funcion **sigue haciendo falta**, y por
/// una razon distinta: el campo firmado tambien puede venir vacio, que es un
/// despliegue legitimo (RPT-054 §1), y las dos vias tienen que significar lo
/// mismo. Borrarla habria devuelto la mentira de abajo por la puerta nueva.
///
/// Sin esto, el agente lo tomaba por un colector configurado que no responde:
///
/// ```text
/// Salida de alertas  :
/// salidaNoDisponible : true     <- averia
/// sinColector        : false    <- MENTIRA
/// ```
///
/// Y esas dos cosas mandan al tecnico a sitios distintos (RPT-055 §3): una a
/// llamar a quien mantiene el SIEM, otra a terminar la instalacion. La decima
/// condicion, anulada por un fichero de configuracion.
///
/// # No es una correccion de conveniencia
///
/// `Some("")` **nunca** es un estado legitimo: `"".to_socket_addrs()` no puede
/// resolver jamas. Es un estado imposible que solo puede existir por el camino
/// que lo trajo, y esto lo elimina en la frontera en lugar de dejarlo entrar.
///
/// Quien llama lo declara por pantalla: no se sustituye en silencio.
#[must_use]
pub fn colector_declarado(valor: &str) -> Option<&str> {
    let limpio = valor.trim();
    if limpio.is_empty() {
        None
    } else {
        Some(limpio)
    }
}

/// Todo lo que una linea de latido necesita saber.
///
/// Los campos van con nombre precisamente porque varios tienen el mismo tipo:
/// `maquina` e `interfaz` son los dos `&str`, y son el par que PA-113 acaba de
/// separar. Ver [`linea_de_latido`].
#[derive(Debug, Clone, Copy)]
pub struct DatosLatido<'a> {
    /// Ultimo asiento del registro de evidencia.
    pub numero: u64,
    /// Extremo de la cadena de resumenes, en hexadecimal.
    pub sello: &'a str,
    /// Condiciones vigentes. Solo viajan las emisibles.
    pub condiciones: &'a Condiciones,
    /// Cada cuanto promete latir este sensor.
    pub intervalo_ms: i64,
    /// Numero de este latido en la serie del sensor.
    pub contador: u64,
    /// Instante para la marca de tiempo de RFC 5424.
    pub instante_utc: i64,
    /// Maquina, para el campo `HOSTNAME`.
    pub maquina: &'a str,
    /// Interfaz que vigila este agente.
    pub interfaz: &'a str,
}

/// Compone la linea de latido: prueba de vida **con estado**.
///
/// # Por que no basta «estoy vivo»
///
/// RPT-052 §3. Un latido que solo dice que existe obliga a la sala a preguntar
/// lo demas por un camino que no tiene. Un sensor vivo y **ciego**
/// (`capturaNoDisponible`) debe verse desde la sala, y hoy no se ve.
///
/// # El par (asiento, sello) NO prueba vida, y esto corrige a RPT-052 §4
///
/// Aquel reporte dijo que el par repetido es sospechoso porque el asiento es
/// monotono y el extremo cambia con el. **Eso es falso para un sensor en calma**,
/// que es justo el caso para el que existe el latido: sin alertas nuevas el
/// registro no crece, el extremo no cambia, y dos latidos legitimos separados por
/// horas llevan exactamente el mismo par.
///
/// Con la regla de RPT-052 §4 tal cual, todo sensor tranquilo quedaria marcado
/// como sospechoso. Y al reves, lo grave: un atacante que capture **un** latido y
/// lo reproduzca mantiene la sala en verde para siempre, porque el par sigue
/// siendo el correcto.
///
/// # Por eso lleva un contador propio
///
/// `latido=N` es monotono **en calma tambien**: cuenta latidos, no asientos. Un
/// mensaje repetido tal cual trae un `N` ya visto y se reconoce.
///
/// Lo que esto **no** compra: no detiene a quien reproduzca incrementando el
/// contador. Eso exigiria firmar cada latido con una clave que el atacante no
/// tenga, y RPT-038 §2 explica por que una clave local no sirve. La barrera que
/// pone el contador es que el atacante tenga que **seguir emitiendo**, no que le
/// baste con grabar un paquete. Queda anotado como PA-112.
///
/// # Por que declara su propio intervalo
///
/// Para que la ausencia sea calculable sin acuerdos implicitos. El colector no
/// deduce cuanto esperar: se lo dicen.
///
/// # Por que los datos van en un registro y no en ocho parametros
///
/// Clippy lo pidio con `too_many_arguments`, y el aviso vale doble aqui: ocho
/// argumentos seguidos son ocho oportunidades de invertir dos, y el par que mas
/// duele es justo `(maquina, interfaz)`. Cambiarlos de sitio **compila sin una
/// queja** y vuelve a colapsar la identidad —esta vez al reves— sin que ninguna
/// comprobacion de tipos lo note. Los nombres del registro lo impiden.
///
/// # Y por que declara su interfaz
///
/// RPT-059, PA-113. La identidad del sensor en la sala era solo el `HOSTNAME`, y
/// eso es correcto para una maquina con un sensor. Un servidor perimetral con un
/// agente por segmento —el despliegue normal en una planta grande— tiene varios,
/// y todos dirian llamarse igual: **el latido de uno taparia la muerte del
/// otro**.
///
/// Es el mismo error que RPT-058 §2, un escalon mas arriba: alli se tomo una
/// parte (la interfaz) por el todo, aqui se tomaba el todo (la maquina) por la
/// parte. La identidad es el par.
#[must_use]
pub fn linea_de_latido(datos: &DatosLatido<'_>) -> Vec<u8> {
    let DatosLatido {
        numero,
        sello,
        condiciones,
        intervalo_ms,
        contador,
        instante_utc,
        maquina,
        interfaz,
    } = *datos;

    // Las condiciones activas, por nombre. Enumerar tambien las inactivas
    // engordaria cada latido sin decir nada nuevo; lo que importa es que la
    // lista este, aunque venga vacia, porque «vacia» es una afirmacion.
    let activas: Vec<&str> = EMISIBLES
        .iter()
        .filter(|(nombre, _)| valor_de(condiciones, nombre) == Some(true))
        .map(|(nombre, _)| *nombre)
        .collect();

    componer(
        // Informativo: un latido normal no es una alerta. Lo que alerta es su
        // AUSENCIA, y eso lo decide el colector (PA-105).
        Gravedad::Informativo,
        instante_utc,
        maquina,
        "latido-de-sensor",
        &format!(
            "latido={contador} interfaz={} sello={sello} asiento={numero} \
             intervaloMs={intervalo_ms} condiciones={}",
            sanear(interfaz),
            if activas.is_empty() {
                "ninguna".to_owned()
            } else {
                activas.join(",")
            }
        ),
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
/// **Faltan dos, y ninguna por olvido.**
///
/// `salidaNoDisponible` es la condicion que dice que no se puede emitir:
/// emitirla exigiria el canal que acaba de fallar (RPT-032 §4).
///
/// `sinColector` dice que no hay canal en absoluto (RPT-054 §4, PA-109). Un
/// agente sin colector no puede avisar de que no tiene colector.
///
/// Las dos viajan solo por IPC, que es donde VIS-04 las consulta. La segunda,
/// ademas, la declaran el instalador y `journald`, que son los unicos sitios
/// donde un sensor mudo puede decir algo.
///
/// # Y por que `escuchaNoDisponible` SI esta
///
/// RPT-070, PA-125. Las dos ausentes describen **el canal de syslog mismo**, y
/// por eso no pueden viajar por el. Aquella describe **el otro canal**: cuando la
/// escucha local cae, syslog es justo lo que sigue funcionando.
///
/// Es ademas el unico camino posible. Lo que podria contar que la consola no
/// conecta es la consola, que es lo que no conecta. Sin esta linea, un sensor
/// vivo e inalcanzable seria invisible tambien para la sala.
const EMISIBLES: [(&str, bool); 11] = [
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
    // RPT-070, PA-125. Con gravedad alta y sin acusar a nadie: nadie toco nada,
    // pero el tecnico no puede preguntarle al sensor y la sala solo se entera por
    // aqui. Un sensor vivo e inalcanzable pasa por sano mientras dure, que es el
    // mismo argumento de `capturaNoDisponible` aplicado al puente.
    ("escuchaNoDisponible", true),
    // RPT-074, PA-79. Sin acusar: un sensor sin configuracion firmada esta en un
    // estado legitimo de desarrollo, y la sala tiene que poder distinguir cuantos
    // de su flota lo estan. Es un aviso, no un incidente.
    ("configuracionSinFirmar", false),
    // Esta si con gravedad alta, y tampoco acusando. Mismo criterio que
    // `registroSaturado`: la firma rota apunta a manipulacion, pero una maquina
    // ajena o una clave rotada dan la misma condicion y no son un ataque. El
    // motivo viaja en el diario, donde se diagnostica.
    ("configuracionNoVerifica", true),
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
/// deliberado: un `false` de consuelo haria que una condicion mal escrita en
/// [`EMISIBLES`] pareciera apagada **para siempre**, y una condicion que nunca
/// se activa no la echa de menos nadie.
///
/// Devuelve `Some` para **las trece**, incluidas las dos que no se emiten: esto es
/// un accesor y no una politica. Quien decide que sale es [`EMISIBLES`], en un
/// solo sitio y con el motivo escrito.
fn valor_de(condiciones: &Condiciones, identificador: &str) -> Option<bool> {
    // Se busca en `Condiciones::enumerar` en lugar de repetir la lista aqui.
    // Esta funcion tenia su propio `match`, y ese `match` se quedo dos
    // condiciones por detras del contrato durante varios turnos (PA-91).
    //
    // Las dos no emisibles se excluyen despues, en `EMISIBLES`, y no callandolas
    // aqui: si `valor_de` mintiera sobre su existencia, la barrera de PA-91 no
    // podria comprobar que la exclusion es deliberada.
    condiciones
        .enumerar()
        .into_iter()
        .find(|(nombre, _)| *nombre == identificador)
        .map(|(_, valor)| valor)
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
    /// Instante del ultimo latido enviado con exito. RPT-052, PA-104.
    ///
    /// `None` hasta el primero: un agente recien arrancado late en su primera
    /// vuelta, sin esperar un intervalo. Lo contrario dejaria una ventana de
    /// silencio justo al arrancar, que es cuando mas probable es que algo este
    /// mal configurado.
    ultimo_latido: Option<i64>,
    /// Latidos **entregados** hasta ahora. RPT-057, PA-105.
    ///
    /// Se incrementa solo tras un envio correcto, asi que la serie que ve el
    /// colector es contigua. Un hueco en ella significa que se perdio una linea
    /// en transito, que es informacion y no ruido.
    ///
    /// Vuelve a cero al reiniciar el proceso. El colector no puede distinguir eso
    /// de una repeticion, y lo declara como tal en lugar de elegir: ver
    /// `eje-vigia`.
    latidos: u64,
    maquina: String,
    /// Interfaz que este agente vigila. RPT-059, PA-113.
    ///
    /// Viaja en el latido porque la identidad del sensor en la sala es el par
    /// (maquina, interfaz): varios agentes en un mismo servidor perimetral
    /// comparten `HOSTNAME` y no son el mismo sensor.
    interfaz: String,
}

impl<D: Despacho> Emisor<D> {
    /// Emisor sobre el despacho dado.
    pub fn nuevo(despacho: D, maquina: &str, interfaz: &str) -> Self {
        Self {
            despacho,
            anteriores: None,
            ultimo_sello: None,
            ultimo_latido: None,
            latidos: 0,
            maquina: maquina.to_owned(),
            interfaz: interfaz.to_owned(),
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
            .enviar(&linea_de_sello(&DatosSello {
                numero,
                sello,
                instante_utc,
                maquina: &self.maquina,
                interfaz: &self.interfaz,
            }))
            .is_err()
        {
            return false;
        }

        self.ultimo_sello = Some((numero, sello.to_owned()));
        true
    }

    /// Emite el latido si toca, segun el intervalo. RPT-052, PA-104.
    ///
    /// # Por que no reutiliza `sellar`
    ///
    /// `sellar` emite **solo si el extremo cambio**, y esta escrito asi a
    /// proposito (RPT-032 §3). En un sensor tranquilo el extremo no cambia
    /// nunca, asi que el sello **no sale** — que es exactamente el caso
    /// indistinguible que PA-104 existe para cubrir.
    ///
    /// Las dos conductas son correctas en su contexto: no repetir transiciones,
    /// y latir siempre. Por eso se separan en lugar de cambiar una por otra.
    ///
    /// # El fallo no se traga, pero tampoco se acumula
    ///
    /// Si el envio falla, el instante **no** se marca: el proximo ciclo lo
    /// reintenta. Un latido perdido no se reenvia despues —seria mentir sobre
    /// cuando estaba vivo— pero tampoco hace que el agente deje de intentarlo.
    ///
    /// Distingue los tres desenlaces que un `bool` confundiria: ver [`Latido`].
    pub fn latir(
        &mut self,
        numero: u64,
        sello: &str,
        condiciones: &Condiciones,
        intervalo_ms: i64,
        ahora_ms: i64,
        instante_utc: i64,
    ) -> Latido {
        // `is_some_and` y no un `let` encadenado: eso exige Rust 1.88 y el
        // proyecto fija 1.85 como minimo (MSRV).
        //
        // El transcurrido tiene que ser positivo ADEMAS de corto. El reloj que
        // llega aqui es de pared, no monotono: un ajuste horario o un `ntpd` que
        // corrige hacia atras da un transcurrido negativo, y un simple
        // `< intervalo_ms` lo leeria como «acabo de latir». El agente se quedaria
        // callado justo lo que dure el salto —horas, si el salto es de horas—
        // mientras la sala lo da por muerto. Ante la duda se late de mas.
        if self
            .ultimo_latido
            .is_some_and(|ultimo| (0..intervalo_ms).contains(&ahora_ms.saturating_sub(ultimo)))
        {
            return Latido::NoTocaba;
        }

        // El contador se reserva y **solo se consume si el envio funciona**: asi
        // la serie que llega al colector es contigua, y un hueco en ella significa
        // que alguien perdio una linea por el camino. Quemar el numero en un
        // intento fallido produciria huecos que no significan nada, y un hueco que
        // a veces es normal deja de poder mirarse.
        let contador = self.latidos.saturating_add(1);

        if self
            .despacho
            .enviar(&linea_de_latido(&DatosLatido {
                numero,
                sello,
                condiciones,
                intervalo_ms,
                contador,
                instante_utc,
                maquina: &self.maquina,
                interfaz: &self.interfaz,
            }))
            .is_err()
        {
            return Latido::NoSePudo;
        }

        self.latidos = contador;
        self.ultimo_latido = Some(ahora_ms);
        Latido::Emitido
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

    use super::{
        Condiciones, DatosLatido, EMISIBLES, INTERVALO_LATIDO_MS, linea_de_latido, valor_de,
    };

    /// Las trece condiciones a cierto, para ejercitar todas las salidas a la vez.
    fn todas_encendidas() -> Condiciones {
        Condiciones {
            inventario_suprimido: true,
            inventario_no_verifica: true,
            observacion_saturada: true,
            captura_con_perdida: true,
            captura_no_disponible: true,
            accion_administrativa: true,
            salida_no_disponible: true,
            sin_colector: true,
            escucha_no_disponible: true,
            configuracion_sin_firmar: true,
            configuracion_no_verifica: true,
            registro_saturado: true,
            evidencia_en_riesgo: true,
        }
    }

    /// El latido nombra lo emisible, ni una mas ni una menos.
    ///
    /// # Por que no basta con la barrera de PA-91
    ///
    /// Aquella ata `EMISIBLES` a `Condiciones`. Esta ata **la linea que sale al
    /// cable** a `EMISIBLES`, que es otra superficie: el latido lleva su propia
    /// lista de nombres, y hasta ahora nada comprobaba que fuera la misma.
    ///
    /// PA-106. La sala solo ve esta linea. Si aqui faltara una condicion, el
    /// tecnico en sitio la veria por IPC y el operador de sala no, y los dos
    /// creerian estar mirando el mismo sensor.
    #[test]
    fn el_latido_nombra_lo_emisible_y_calla_lo_que_no_puede_salir() {
        let linea = String::from_utf8_lossy(&linea_de_latido(&DatosLatido {
            numero: 7,
            sello: "abc",
            condiciones: &todas_encendidas(),
            intervalo_ms: INTERVALO_LATIDO_MS,
            contador: 1,
            instante_utc: 0,
            maquina: "sensor-1",
            interfaz: "eth0",
        }))
        .into_owned();

        let lista = linea
            .split("condiciones=")
            .nth(1)
            .expect("el latido declara las condiciones vigentes")
            .trim()
            .to_owned();
        let nombradas: Vec<&str> = lista.split(',').collect();

        for (nombre, _) in EMISIBLES {
            assert!(
                nombradas.contains(&nombre),
                "'{nombre}' es emisible y no viaja en el latido: la sala no lo vera nunca"
            );
        }
        assert_eq!(
            nombradas.len(),
            EMISIBLES.len(),
            "el latido nombra algo que no esta en EMISIBLES: {nombradas:?}"
        );

        // Y las dos que no pueden salir no salen tampoco por aqui. Se comprueba
        // sobre la linea entera y no sobre la lista: un latido que las llevara en
        // cualquier otro campo seria el mismo fallo.
        assert!(
            !linea.contains("salidaNoDisponible") && !linea.contains("sinColector"),
            "una condicion no emisible viajo en el latido: {linea}"
        );
    }

    /// Un sensor al que nadie puede preguntar **si** llega a la sala.
    ///
    /// RPT-070, PA-125. Es la afirmacion entera del punto, y por eso tiene prueba
    /// propia en lugar de confiarse a la de arriba: aquella comprueba que todo lo
    /// emisible viaja, y pasaria igual si `escuchaNoDisponible` estuviera del
    /// otro lado de la lista.
    ///
    /// La situacion que se ejercita es exactamente la observada en RPT-069 §3: el
    /// agente vive, observa, registra y emite; lo unico que no tiene es escucha
    /// local. Si esta linea no saliera, **nadie en ningun sitio** lo sabria — la
    /// consola no puede contarlo porque es lo que no conecta.
    #[test]
    fn un_sensor_incomunicado_lo_dice_por_el_canal_que_le_queda() {
        let solo_sin_escucha = Condiciones {
            escucha_no_disponible: true,
            configuracion_sin_firmar: false,
            configuracion_no_verifica: false,
            inventario_suprimido: false,
            inventario_no_verifica: false,
            observacion_saturada: false,
            captura_con_perdida: false,
            captura_no_disponible: false,
            accion_administrativa: false,
            salida_no_disponible: false,
            sin_colector: false,
            registro_saturado: false,
            evidencia_en_riesgo: false,
        };

        let linea = String::from_utf8_lossy(&linea_de_latido(&DatosLatido {
            numero: 3,
            sello: "abc",
            condiciones: &solo_sin_escucha,
            intervalo_ms: INTERVALO_LATIDO_MS,
            contador: 5,
            instante_utc: 0,
            maquina: "sensor-1",
            interfaz: "eth0",
        }))
        .into_owned();

        assert!(
            linea.contains("condiciones=escuchaNoDisponible"),
            "un sensor sin escucha local seria invisible tambien para la sala: {linea}"
        );
    }

    /// La sala se entera de que un sensor corre sin configuracion firmada.
    ///
    /// RPT-074, PA-79. Es lo que convierte esta condicion en algo mas que una
    /// nota local: sin ella, saber cuantos sensores de una flota estan
    /// aprovisionados exigiria visitarlos uno a uno.
    ///
    /// Las dos se emiten a proposito. `configuracionSinFirmar` describe un estado
    /// legitimo de despliegue y `configuracionNoVerifica` uno que exige mirar el
    /// diario, y **ninguna de las dos describe el canal de syslog**, que es el
    /// unico motivo por el que una condicion no puede viajar.
    #[test]
    fn la_configuracion_sin_firmar_llega_a_la_sala() {
        let base = Condiciones {
            inventario_suprimido: false,
            inventario_no_verifica: false,
            observacion_saturada: false,
            captura_con_perdida: false,
            captura_no_disponible: false,
            accion_administrativa: false,
            salida_no_disponible: false,
            sin_colector: false,
            escucha_no_disponible: false,
            configuracion_sin_firmar: true,
            configuracion_no_verifica: false,
            registro_saturado: false,
            evidencia_en_riesgo: false,
        };

        let linea = String::from_utf8_lossy(&linea_de_latido(&DatosLatido {
            numero: 1,
            sello: "abc",
            condiciones: &base,
            intervalo_ms: INTERVALO_LATIDO_MS,
            contador: 1,
            instante_utc: 0,
            maquina: "sensor-1",
            interfaz: "eth0",
        }))
        .into_owned();

        assert!(
            linea.contains("condiciones=configuracionSinFirmar"),
            "la sala no puede saber que sensores corren sin firmar: {linea}"
        );

        // Y la otra, por el mismo camino y con la otra semantica.
        let rota = Condiciones {
            configuracion_sin_firmar: false,
            configuracion_no_verifica: true,
            ..base
        };

        let linea = String::from_utf8_lossy(&linea_de_latido(&DatosLatido {
            numero: 1,
            sello: "abc",
            condiciones: &rota,
            intervalo_ms: INTERVALO_LATIDO_MS,
            contador: 1,
            instante_utc: 0,
            maquina: "sensor-1",
            interfaz: "eth0",
        }))
        .into_owned();

        assert!(
            linea.contains("condiciones=configuracionNoVerifica"),
            "{linea}"
        );
    }

    /// Un colector vacio es ausencia de colector, no un colector roto.
    ///
    /// RPT-064, PA-118. Observado en `systemd` real: `--syslog ${VARIABLE}` con
    /// la variable vacia entrega `--syslog ""`, y el agente lo tomaba por un
    /// colector configurado que no responde.
    #[test]
    fn un_colector_vacio_no_es_un_colector() {
        use super::colector_declarado;

        assert_eq!(colector_declarado(""), None);
        assert_eq!(colector_declarado("   "), None, "ni con espacios");
        assert_eq!(colector_declarado("\t\n"), None, "ni con blancos raros");

        assert_eq!(colector_declarado("127.0.0.1:5514"), Some("127.0.0.1:5514"));
        assert_eq!(
            colector_declarado("  siem.hospital:514  "),
            Some("siem.hospital:514"),
            "y el que hay se limpia de los blancos que trae el fichero"
        );
    }

    /// En calma la lista viene vacia, y lo dice.
    ///
    /// «ninguna» es una afirmacion: dice que se miraron las ocho y no habia
    /// ninguna activa. Un campo ausente diria que no se miro.
    #[test]
    fn en_calma_el_latido_afirma_que_no_hay_ninguna() {
        let calma = Condiciones {
            inventario_suprimido: false,
            inventario_no_verifica: false,
            observacion_saturada: false,
            captura_con_perdida: false,
            captura_no_disponible: false,
            accion_administrativa: false,
            salida_no_disponible: false,
            sin_colector: false,
            escucha_no_disponible: false,
            configuracion_sin_firmar: false,
            configuracion_no_verifica: false,
            registro_saturado: false,
            evidencia_en_riesgo: false,
        };

        let linea = String::from_utf8_lossy(&linea_de_latido(&DatosLatido {
            numero: 1,
            sello: "abc",
            condiciones: &calma,
            intervalo_ms: 60_000,
            contador: 1,
            instante_utc: 0,
            maquina: "sensor-1",
            interfaz: "eth0",
        }))
        .into_owned();

        assert!(linea.contains("condiciones=ninguna"), "{linea}");
    }

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
    /// La lista sale de `Condiciones::enumerar`, cuya desestructuracion es
    /// exhaustiva y **sin `..`**: un campo nuevo en `Condiciones` deja de
    /// compilar alli, y en cuanto compila aparece aqui y obliga a decidir si va
    /// al SIEM o si es otra excepcion. No se puede olvidar, que es la unica forma
    /// de que no se olvide.
    ///
    /// Desde RPT-058 los diez nombres se escriben en un solo sitio. Antes esta
    /// prueba los repetia, con lo que la barrera y la cosa vigilada estaban
    /// escritas por la misma mano.
    #[test]
    fn toda_condicion_sale_al_siem_salvo_la_que_no_puede() {
        let condiciones = todas_encendidas();

        // La lista sale de `Condiciones::enumerar`, que es ahora el unico sitio
        // donde se escriben los diez nombres. Repetirlos aqui era tener la
        // barrera y la cosa vigilada escritas por la misma mano.
        let todas = condiciones.enumerar();

        // Las dos excepciones se nombran aqui, una sola vez, y el resto de la
        // prueba se deriva de esta lista. Una tercera excepcion futura tendra que
        // escribirse en este sitio y con su motivo al lado: la barrera protege
        // contra el olvido, no contra la decision.
        const NO_EMISIBLES: [&str; 2] = ["salidaNoDisponible", "sinColector"];

        for (identificador, valor) in todas {
            let emitida = EMISIBLES.iter().any(|(nombre, _)| *nombre == identificador);

            // `valor_de` conoce LAS DIEZ. Es un accesor, no una politica: una
            // condicion que `EMISIBLES` nombra y `valor_de` no conoce se queda
            // apagada para siempre, que es el aviso de su propia documentacion.
            //
            // Desde RPT-058 se deriva de `Condiciones::enumerar`, asi que esto
            // comprueba que la derivacion cubre el identificador de verdad y no
            // uno parecido.
            assert_eq!(
                valor_de(&condiciones, identificador),
                Some(valor),
                "'{identificador}': `valor_de` no lo conoce"
            );

            if NO_EMISIBLES.contains(&identificador) {
                // Las dos excepciones, y ninguna es una decision de estilo:
                // `salidaNoDisponible` exigiria el canal que acaba de fallar y
                // `sinColector` un canal que no existe. Llegan solo por el
                // puente, que es donde VIS-04 las consulta.
                assert!(
                    !emitida,
                    "'{identificador}' no puede salir por un canal que no responde"
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
            todas.len() - NO_EMISIBLES.len(),
            "EMISIBLES debe cubrir todas las condiciones menos las no emisibles"
        );
    }
}
