//! La semilla del emisor, cifrada en reposo.
//!
//! RPT-026, PA-48.
//!
//! # Lo que esto es y lo que no
//!
//! La semilla **es** la clave (RPT-023 §3). Cifrarla no cambia eso: mueve el
//! problema a la frase de paso. Este modulo hace que ese movimiento sea el mas
//! caro posible para quien copie el fichero, y nada mas.
//!
//! RPT-023 §4 ratifico Argon2id para la operativa y **rechazo explicitamente**
//! el fichero protegido solo por permisos del sistema, por lo fragiles que son
//! en despliegues mixtos Windows/Linux.
//!
//! # Disposicion
//!
//! ```text
//! +--------------------------------------------------+
//! | magico       8 bytes  "EJE-SEM1"                  |
//! | version      u16 BE                               |
//! | sal         16 bytes  aleatoria por fichero       |
//! | nonce       12 bytes  aleatorio por CIFRADO       |
//! | cifrado     32 + 16   semilla + etiqueta GCM      |
//! +--------------------------------------------------+
//! ```
//!
//! # La cabecera se autentica
//!
//! Magico, version y sal viajan como datos asociados del AEAD. Sin eso, cambiar
//! la sal de un fichero ajeno y observar si el descifrado falla de una forma u
//! otra seria un principio de oraculo. Autenticada, cualquier alteracion produce
//! el mismo fallo indistinguible.
//!
//! # El nonce es por cifrado, no por fichero
//!
//! Reutilizar un nonce con la misma clave rompe GCM por completo. Como la clave
//! se deriva de (frase, sal), reescribir el fichero con la misma frase y la misma
//! sal daria la misma clave: el nonce **tiene** que cambiar. Aqui se genera uno
//! nuevo en cada escritura, junto con una sal nueva, que es la via barata de no
//! depender de acordarse.

use motor_pqc::reposo::{ClaveSimetrica, LONGITUD_NONCE, cifrar, descifrar};
use motor_pqc::secreto::Secreto;
use motor_pqc::semilla::{LONGITUD_SEMILLA, SemillaFirma};

/// Numero magico del fichero de semilla.
pub const MAGICO_SEMILLA: &[u8; 8] = b"EJE-SEM1";

/// Version del formato.
pub const VERSION_SEMILLA: u16 = 1;

/// Longitud de la sal de derivacion.
///
/// Dieciseis bytes es lo que recomienda la especificacion de Argon2 y el doble
/// del minimo que la implementacion acepta.
pub const LONGITUD_SAL: usize = 16;

/// Bytes de cabecera autenticada: magico, version y sal.
const LONGITUD_CABECERA: usize = 8 + 2 + LONGITUD_SAL;

/// Longitud de la etiqueta de autenticacion de AES-GCM.
const LONGITUD_ETIQUETA: usize = 16;

/// Longitud exacta del fichero.
pub const LONGITUD_FICHERO: usize =
    LONGITUD_CABECERA + LONGITUD_NONCE + LONGITUD_SEMILLA + LONGITUD_ETIQUETA;

/// Fallos del fichero de semilla.
#[derive(Debug, thiserror::Error)]
pub enum ErrorSemilla {
    /// El fichero no empieza por [`MAGICO_SEMILLA`].
    #[error("el fichero no es una semilla de Eje-Latam")]
    MagicoAusente,

    /// Version desconocida.
    #[error("version de semilla {encontrada}; este binario entiende la {VERSION_SEMILLA}")]
    VersionDesconocida {
        /// Version leida.
        encontrada: u16,
    },

    /// La longitud no es la exacta que el formato exige.
    #[error("el fichero de semilla mide {encontrada} bytes; se esperaban {LONGITUD_FICHERO}")]
    LongitudIncorrecta {
        /// Bytes disponibles.
        encontrada: usize,
    },

    /// La frase de paso no abre el fichero, o el fichero fue alterado.
    ///
    /// **No se distinguen los dos casos**, por el mismo motivo que
    /// `reposo::descifrar` no distingue cual de sus entradas fallo: informar
    /// cual daria un oraculo.
    #[error("la frase de paso no abre la semilla, o el fichero fue alterado")]
    NoAbre,

    /// La derivacion Argon2id no pudo completarse.
    #[error("la derivacion de la clave de cifrado fallo")]
    DerivacionFallida,

    /// La frase de paso esta vacia.
    ///
    /// Se rechaza al **crear**, no solo al abrir: una semilla con frase vacia es
    /// la opcion B que RPT-023 §4 rechazo, disfrazada de cifrado.
    #[error("la frase de paso no puede estar vacia")]
    FraseVacia,
}

/// Datos asociados del AEAD: la cabecera entera.
fn asociados(sal: &[u8; LONGITUD_SAL]) -> Vec<u8> {
    let mut salida = Vec::with_capacity(LONGITUD_CABECERA);
    salida.extend_from_slice(MAGICO_SEMILLA);
    salida.extend_from_slice(&VERSION_SEMILLA.to_be_bytes());
    salida.extend_from_slice(sal);
    salida
}

/// Deriva la clave de cifrado de la frase de paso y la sal.
///
/// # Errores
///
/// [`ErrorSemilla::DerivacionFallida`] si Argon2id rechaza los parametros.
fn derivar_clave(frase: &[u8], sal: &[u8; LONGITUD_SAL]) -> Result<ClaveSimetrica, ErrorSemilla> {
    let mut bruta = [0u8; 32];

    argon2::Argon2::default()
        .hash_password_into(frase, sal, &mut bruta)
        .map_err(|_| ErrorSemilla::DerivacionFallida)?;

    Ok(Secreto::nuevo(bruta))
}

/// Cifra una semilla para dejarla en disco.
///
/// La sal y el nonce se reciben en lugar de generarse aqui: este modulo no debe
/// decidir de donde sale la aleatoriedad, y recibirlos permite que la prueba fije
/// valores sin que exista un camino de produccion que use valores fijos.
///
/// # Errores
///
/// [`ErrorSemilla::FraseVacia`] o [`ErrorSemilla::DerivacionFallida`].
pub fn sellar(
    semilla: &SemillaFirma,
    frase: &[u8],
    sal: [u8; LONGITUD_SAL],
    nonce: [u8; LONGITUD_NONCE],
) -> Result<Vec<u8>, ErrorSemilla> {
    if frase.is_empty() {
        return Err(ErrorSemilla::FraseVacia);
    }

    let clave = derivar_clave(frase, &sal)?;
    let cabecera = asociados(&sal);

    let cifrado = cifrar(&clave, &nonce, semilla.exponer(), &cabecera)
        .map_err(|_| ErrorSemilla::DerivacionFallida)?;

    let mut salida = Vec::with_capacity(LONGITUD_FICHERO);
    salida.extend_from_slice(&cabecera);
    salida.extend_from_slice(&nonce);
    salida.extend_from_slice(&cifrado);
    Ok(salida)
}

/// Abre un fichero de semilla con su frase de paso.
///
/// # Errores
///
/// Una variante de [`ErrorSemilla`]. La longitud se exige **exacta**: una cola
/// sobrante admitiria dos lecturas del mismo fichero.
pub fn abrir(bytes: &[u8], frase: &[u8]) -> Result<SemillaFirma, ErrorSemilla> {
    if bytes.len() != LONGITUD_FICHERO {
        return Err(ErrorSemilla::LongitudIncorrecta {
            encontrada: bytes.len(),
        });
    }

    if &bytes[..8] != MAGICO_SEMILLA {
        return Err(ErrorSemilla::MagicoAusente);
    }

    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    if version != VERSION_SEMILLA {
        return Err(ErrorSemilla::VersionDesconocida {
            encontrada: version,
        });
    }

    let mut sal = [0u8; LONGITUD_SAL];
    sal.copy_from_slice(&bytes[10..LONGITUD_CABECERA]);

    let clave = derivar_clave(frase, &sal)?;
    let nonce = &bytes[LONGITUD_CABECERA..LONGITUD_CABECERA + LONGITUD_NONCE];

    let plano = descifrar(
        &clave,
        nonce,
        &bytes[LONGITUD_CABECERA + LONGITUD_NONCE..],
        &asociados(&sal),
    )
    .map_err(|_| ErrorSemilla::NoAbre)?;

    if plano.len() != LONGITUD_SEMILLA {
        return Err(ErrorSemilla::NoAbre);
    }

    let mut bruta = [0u8; LONGITUD_SEMILLA];
    bruta.copy_from_slice(&plano);
    Ok(Secreto::nuevo(bruta))
}
