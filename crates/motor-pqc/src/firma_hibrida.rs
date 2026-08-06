//! Firma híbrida Ed25519 + ML-DSA-65.
//!
//! Categoría 3 del NIST. **Ed25519 ya está en uso** para los tokens de licencia
//! (RPT-003 §3) y para la firma del paquete empresarial (RPT-004 §5), de modo que
//! la construcción híbrida extiende lo existente en lugar de sustituirlo
//! (RPT-005 §7.4).
//!
//! # La verificación exige ambas firmas
//!
//! Una firma híbrida se acepta **solo si las dos componentes son válidas**. Con
//! la disyunción —aceptar si cualquiera verifica— la seguridad del conjunto sería
//! la de la componente más débil, que es exactamente lo contrario de lo que se
//! busca. Con la conjunción, el atacante debe romper las dos.
//!
//! # Vinculación de dominio
//!
//! Ambos esquemas firman el mismo mensaje canónico, que incorpora una etiqueta de
//! dominio con los algoritmos y la versión. Firmar el mensaje en bruto permitiría
//! reutilizar una de las dos firmas en otro protocolo que también use Ed25519.

use ed25519_dalek::{
    Signature as FirmaEd25519, SigningKey as PrivadaEd25519, VerifyingKey as PublicaEd25519,
};
use ed25519_dalek::{Signer as _, Verifier as _};
use ml_dsa::{
    EncodedSignature, Generate as _, Keypair as _, MlDsa65, Signature as FirmaMlDsa, Signer as _,
    SigningKey as PrivadaMlDsa, Verifier as _, VerifyingKey as PublicaMlDsa,
};
use rand_core::CryptoRng;

use crate::ErrorPqc;
use crate::combinador::mensaje_canonico_de_firma;

/// Identificador del algoritmo, para registro y auditoría.
pub const ALGORITMO: &str = "ed25519+ml-dsa-65";

/// Longitud en bytes de una firma Ed25519.
pub const LONGITUD_FIRMA_CLASICA: usize = 64;

/// Clave de firma híbrida.
pub struct ClaveFirmaHibrida {
    /// Componente poscuántica.
    pub poscuantica: PrivadaMlDsa<MlDsa65>,
    /// Componente clásica.
    pub clasica: PrivadaEd25519,
}

/// Clave de verificación híbrida.
#[derive(Clone)]
pub struct ClaveVerificacionHibrida {
    /// Componente poscuántica.
    pub poscuantica: PublicaMlDsa<MlDsa65>,
    /// Componente clásica.
    pub clasica: PublicaEd25519,
}

/// Firma híbrida: ambas componentes deben verificar.
#[derive(Clone)]
pub struct FirmaHibrida {
    /// Firma ML-DSA-65.
    pub poscuantica: FirmaMlDsa<MlDsa65>,
    /// Firma Ed25519.
    pub clasica: FirmaEd25519,
}

impl FirmaHibrida {
    /// Serializa la firma con la componente clásica al final.
    ///
    /// La componente clásica tiene longitud fija conocida, lo que permite
    /// delimitar sin ambigüedad sin necesidad de prefijos.
    #[must_use]
    pub fn a_bytes(&self) -> Vec<u8> {
        let mut salida = self.poscuantica.encode().to_vec();
        salida.extend_from_slice(&self.clasica.to_bytes());
        salida
    }

    /// Longitud total de una firma serializada.
    ///
    /// Ambas componentes tienen longitud fija, así que la firma también. Se
    /// deriva del tipo en lugar de escribirse a mano: una constante copiada se
    /// desincroniza en silencio si el parámetro cambia.
    #[must_use]
    pub fn longitud_serializada() -> usize {
        EncodedSignature::<MlDsa65>::default().len() + LONGITUD_FIRMA_CLASICA
    }

    /// Reconstruye una firma a partir de su serialización.
    ///
    /// # Errores
    ///
    /// Devuelve [`ErrorPqc::FirmaInvalida`] si la longitud no es exactamente
    /// [`Self::longitud_serializada`] o si la componente poscuántica no decodifica.
    /// Se exige longitud **exacta**: aceptar una entrada más larga y quedarse con
    /// el prefijo dejaría bytes sin interpretar en un dato que llega de un fichero
    /// que se asume manipulable.
    pub fn desde_bytes(bytes: &[u8]) -> Result<Self, ErrorPqc> {
        let longitud_pq = EncodedSignature::<MlDsa65>::default().len();

        if bytes.len() != longitud_pq + LONGITUD_FIRMA_CLASICA {
            return Err(ErrorPqc::FirmaInvalida);
        }

        let (parte_pq, parte_clasica) = bytes.split_at(longitud_pq);

        let codificada =
            EncodedSignature::<MlDsa65>::try_from(parte_pq).map_err(|_| ErrorPqc::FirmaInvalida)?;
        let poscuantica =
            FirmaMlDsa::<MlDsa65>::decode(&codificada).ok_or(ErrorPqc::FirmaInvalida)?;

        let mut bruta = [0u8; LONGITUD_FIRMA_CLASICA];
        bruta.copy_from_slice(parte_clasica);

        Ok(Self {
            poscuantica,
            clasica: FirmaEd25519::from_bytes(&bruta),
        })
    }
}

/// Genera un par de claves de firma híbrido.
pub fn generar_par<R: CryptoRng>(
    generador: &mut R,
) -> (ClaveFirmaHibrida, ClaveVerificacionHibrida) {
    let privada_pq = PrivadaMlDsa::<MlDsa65>::generate_from_rng(generador);
    let publica_pq = privada_pq.verifying_key().clone();

    let mut semilla = [0u8; 32];
    generador.fill_bytes(&mut semilla);
    let privada_clasica = PrivadaEd25519::from_bytes(&semilla);
    let publica_clasica = privada_clasica.verifying_key();

    (
        ClaveFirmaHibrida {
            poscuantica: privada_pq,
            clasica: privada_clasica,
        },
        ClaveVerificacionHibrida {
            poscuantica: publica_pq,
            clasica: publica_clasica,
        },
    )
}

/// Firma un mensaje con ambos esquemas.
#[must_use]
pub fn firmar(clave: &ClaveFirmaHibrida, mensaje: &[u8]) -> FirmaHibrida {
    let canonico = mensaje_canonico_de_firma(mensaje);
    FirmaHibrida {
        poscuantica: clave.poscuantica.sign(&canonico),
        clasica: clave.clasica.sign(&canonico),
    }
}

/// Verifica una firma híbrida.
///
/// # Errores
///
/// Devuelve [`ErrorPqc::FirmaInvalida`] si **cualquiera** de las dos componentes
/// no verifica. No existe modo degradado: aceptar una firma con una sola
/// componente válida reduciría la seguridad del conjunto a la de su parte más
/// débil.
pub fn verificar(
    clave: &ClaveVerificacionHibrida,
    mensaje: &[u8],
    firma: &FirmaHibrida,
) -> Result<(), ErrorPqc> {
    let canonico = mensaje_canonico_de_firma(mensaje);

    let poscuantica_valida = clave
        .poscuantica
        .verify(&canonico, &firma.poscuantica)
        .is_ok();
    let clasica_valida = clave.clasica.verify(&canonico, &firma.clasica).is_ok();

    if poscuantica_valida && clasica_valida {
        Ok(())
    } else {
        Err(ErrorPqc::FirmaInvalida)
    }
}
