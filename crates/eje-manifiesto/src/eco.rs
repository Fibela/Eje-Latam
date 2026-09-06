//! Apagado del eco de la terminal mientras se teclea la frase de paso.
//!
//! PA-53, abierto desde RPT-026 §5 y escalado a critico por RPT-092.
//!
//! # Por que existe este modulo
//!
//! Hasta hoy la herramienta imprimia un aviso —«se vera al teclearla»— y no
//! hacia nada al respecto. El 31 de agosto de 2026 ese aviso se leyo, se
//! entendio, y la frase se tecleo igual, porque **no habia otra forma de
//! darla**. Quedo en pantalla y en el registro de la sesion, y hubo que darla
//! por comprometida.
//!
//! Un aviso que nombra un peligro sin ofrecer salida no reduce el riesgo:
//! traslada la culpa del diseno a quien lo usa.
//!
//! # `unsafe` aqui, y por que no en otro sitio
//!
//! `termios` es FFI y no hay forma de llamarlo sin `unsafe`. Es el **segundo**
//! modulo del workspace que lo admite —el otro es `eje-captura::linux`, con su
//! propia lista acotada— y aqui la lista es de tres llamadas:
//!
//! 1. `isatty` — saber si hay terminal.
//! 2. `tcgetattr` — leer la configuracion actual.
//! 3. `tcsetattr` — quitar `ECHO`, y devolverlo en [`Guardia::drop`].
//!
//! Tres llamadas. Cualquier ampliacion de esa lista deberia costar una
//! revision, y por eso se enumera aqui en lugar de dejarla implicita.
//!
//! # No vive en el agente
//!
//! `eje-manifiesto` no se despliega en el sensor a proposito: un sensor
//! comprometido no debe llevar encima la capacidad de firmar inventarios. La
//! frase de paso solo se teclea en la maquina del administrador, asi que este
//! modulo se queda donde se teclea.

#![allow(unsafe_code)]

use std::mem::MaybeUninit;

/// Descriptor de la entrada estandar.
const ENTRADA: i32 = 0;

/// Que ocurrio al intentar apagar el eco.
///
/// # Tres estados, y ninguno se puede colapsar
///
/// RPT-006 §4. La tentacion es tener dos —«apagado» y «no»— y en cuanto se
/// escribe asi, la herramienta acaba diciendo «no se vera» sobre una terminal
/// en la que si se ve. Eso seria **peor** que el defecto que este modulo
/// arregla: hoy quien teclea sabe que se le ve; con dos estados creeria que no.
pub enum Eco {
    /// Apagado. Mientras este valor viva, la terminal no repite lo tecleado.
    ///
    /// Al soltarse, [`Guardia`] restaura la configuracion anterior.
    Apagado(Guardia),

    /// La entrada **no es una terminal**: una tuberia, un fichero, el arnes de
    /// pruebas.
    ///
    /// No hay eco que apagar y no hay nada que advertir. No es un fallo.
    NoEsTerminal,

    /// Es una terminal y el apagado fallo.
    ///
    /// Se distingue de [`Self::Apagado`] porque son hechos distintos, y de
    /// [`Self::NoEsTerminal`] porque aqui **si hay alguien mirando**. Quien
    /// reciba esto tiene que avisar, que es lo que la herramienta hacia antes
    /// de este modulo y sigue teniendo que hacer cuando no puede hacer mas.
    NoSePudoApagar,
}

/// Devuelve la terminal a como estaba, pase lo que pase.
///
/// # Por que `Drop` y no una llamada al final
///
/// Porque una llamada al final no ocurre si se sale antes por error, y una
/// terminal que se queda muda despues de un fallo es un incidente peor que el
/// eco: el operador no ve lo que teclea y no sabe por que. Con `Drop`, la
/// restauracion la garantiza el compilador y no la memoria de quien escribe.
pub struct Guardia {
    /// Configuracion tal como estaba antes de tocarla.
    anterior: libc::termios,
}

impl Drop for Guardia {
    fn drop(&mut self) {
        // El resultado se ignora **a proposito**: `Drop` no puede devolver un
        // error y no queda nada mejor que intentar. Lo que no se puede es no
        // intentarlo.
        //
        // SAFETY: `anterior` la escribio `tcgetattr` del propio sistema y no se
        // ha modificado desde entonces.
        let _ = unsafe { libc::tcsetattr(ENTRADA, libc::TCSAFLUSH, &self.anterior) };
    }
}

/// Apaga el eco de la entrada estandar.
///
/// El eco vuelve cuando se suelta el [`Guardia`] que devuelve
/// [`Eco::Apagado`]: quien llama no tiene que acordarse de restaurarlo, y por
/// tanto no puede olvidarlo.
#[must_use]
pub fn apagar() -> Eco {
    apagar_en(ENTRADA)
}

/// Igual, sobre un descriptor cualquiera.
///
/// Existe para que la rama de «esto no es una terminal» se pueda probar sin
/// depender de si el arnes de pruebas heredo una tty, que cambia entre correr
/// las pruebas a mano y correrlas en integracion continua. Es la misma razon
/// por la que `leer_frase` toma un `BufRead` en lugar de leer `stdin` (RPT-082,
/// PA-134): **un mecanismo que solo se puede observar en la maquina de alguien
/// no se observa.**
#[must_use]
pub fn apagar_en(descriptor: i32) -> Eco {
    // SAFETY: `isatty` solo consulta el descriptor; no escribe en el ni en
    // memoria del proceso.
    if unsafe { libc::isatty(descriptor) } != 1 {
        return Eco::NoEsTerminal;
    }

    let mut anterior = MaybeUninit::<libc::termios>::uninit();

    // SAFETY: `tcgetattr` rellena la estructura apuntada. El puntero sale de un
    // `MaybeUninit` vivo y del tamano correcto.
    if unsafe { libc::tcgetattr(descriptor, anterior.as_mut_ptr()) } != 0 {
        return Eco::NoSePudoApagar;
    }

    // SAFETY: solo se llega aqui si `tcgetattr` devolvio 0, que es cuando ha
    // inicializado la estructura.
    let anterior = unsafe { anterior.assume_init() };

    let mut sin_eco = anterior;
    sin_eco.c_lflag &= !libc::ECHO;

    // SAFETY: `sin_eco` es copia de una configuracion que el sistema acaba de
    // entregar, con una bandera quitada. `TCSAFLUSH` descarta lo que ya
    // estuviera tecleado, para que nada escrito antes del aviso entre en la
    // frase (el mismo filo que PA-134).
    if unsafe { libc::tcsetattr(descriptor, libc::TCSAFLUSH, &sin_eco) } != 0 {
        return Eco::NoSePudoApagar;
    }

    Eco::Apagado(Guardia { anterior })
}
