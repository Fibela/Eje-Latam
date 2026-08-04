//! Cifrado autenticado de datos en reposo con AES-256-GCM.
//!
//! # Separación de responsabilidades
//!
//! ML-KEM y ML-DSA **no cifran datos en reposo**. El corpus original atribuía esa
//! función a ML-KEM/ML-DSA, y se corrigió en RPT-002 §5: el cifrado en reposo es
//! AES-256-GCM, con la clave simétrica envuelta mediante ML-KEM.
//!
//! # Nonce
//!
//! GCM falla de forma catastrófica ante reutilización de nonce con la misma
//! clave: se filtra la clave de autenticación y se compromete la integridad de
//! todos los mensajes bajo esa clave. Por eso el nonce **no se genera dentro de
//! este módulo**: lo aporta quien llama, que es quien conoce la política de
//! unicidad de su contexto.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};

use crate::ErrorPqc;
use crate::secreto::Secreto;

/// Identificador del algoritmo, para registro y auditoría.
pub const ALGORITMO: &str = "aes-256-gcm";

/// Longitud del nonce en bytes.
pub const LONGITUD_NONCE: usize = 12;

/// Clave simétrica de 256 bits con borrado seguro.
pub type ClaveSimetrica = Secreto<32>;

/// Cifra y autentica datos en reposo.
///
/// `asociados` se autentica sin cifrarse: sirve para vincular el texto cifrado a
/// su contexto —identificador de fichero, número de asiento, versión de
/// esquema— de modo que no pueda trasplantarse a otro lugar.
///
/// # Errores
///
/// Devuelve [`ErrorPqc::CifradoFallido`] si el nonce no mide
/// [`LONGITUD_NONCE`] bytes o si la operación falla.
pub fn cifrar(
    clave: &ClaveSimetrica,
    nonce: &[u8],
    plano: &[u8],
    asociados: &[u8],
) -> Result<Vec<u8>, ErrorPqc> {
    if nonce.len() != LONGITUD_NONCE {
        return Err(ErrorPqc::CifradoFallido);
    }

    let clave_gcm = Key::<Aes256Gcm>::from_slice(clave.exponer());
    let cifrador = Aes256Gcm::new(clave_gcm);

    cifrador
        .encrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: plano,
                aad: asociados,
            },
        )
        .map_err(|_| ErrorPqc::CifradoFallido)
}

/// Descifra y verifica la integridad de datos en reposo.
///
/// # Errores
///
/// Devuelve [`ErrorPqc::IntegridadFallida`] si el texto cifrado, el nonce o los
/// datos asociados fueron alterados. **No distingue entre los tres casos**:
/// informar cuál falló daría al atacante un oráculo.
pub fn descifrar(
    clave: &ClaveSimetrica,
    nonce: &[u8],
    cifrado: &[u8],
    asociados: &[u8],
) -> Result<Vec<u8>, ErrorPqc> {
    if nonce.len() != LONGITUD_NONCE {
        return Err(ErrorPqc::IntegridadFallida);
    }

    let clave_gcm = Key::<Aes256Gcm>::from_slice(clave.exponer());
    let cifrador = Aes256Gcm::new(clave_gcm);

    cifrador
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: cifrado,
                aad: asociados,
            },
        )
        .map_err(|_| ErrorPqc::IntegridadFallida)
}
