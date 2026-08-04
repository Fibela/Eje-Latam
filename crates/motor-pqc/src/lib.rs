//! # Motor Poscuantico — AGT-02
//!
//! Envoltorio criptografico hibrido para Eje-Latam.
//!
//! ## Licencia y auditabilidad
//!
//! Este crate es **Apache-2.0 de forma definitiva** por decision ratificada en
//! RPT-003 §2.7. Un motor criptografico cuya seguridad dependa del secreto de su
//! implementacion no es evaluable (principio de Kerckhoffs), y ML-KEM (FIPS 203)
//! y ML-DSA (FIPS 204) son estandares publicos del NIST: no hay propiedad
//! intelectual que proteger.
//!
//! ## Limitacion declarada (RPT-002 §5, AGT-02)
//!
//! El envoltorio protege el extremo cliente. Si el sistema remoto solo habla TLS
//! clasico, el canal hibrido **termina en el proxy local** y el ultimo salto sigue
//! siendo clasico. La proteccion poscuantica de extremo a extremo solo existe
//! cuando ambos extremos ejecutan Eje-Latam o el destino soporta TLS 1.3 hibrido.
//!
//! ## Separacion de responsabilidades
//!
//! ML-KEM y ML-DSA **no cifran datos en reposo**. El cifrado en reposo usa
//! AES-256-GCM con la clave simetrica envuelta mediante ML-KEM.

#![forbid(unsafe_code)]

use thiserror::Error;

/// Errores del motor poscuantico.
#[derive(Debug, Error)]
pub enum ErrorPqc {
    /// El material de clave no corresponde al algoritmo declarado.
    #[error("material de clave invalido para el algoritmo {algoritmo}")]
    ClaveInvalida {
        /// Algoritmo contra el que se intento validar.
        algoritmo: &'static str,
    },

    /// La verificacion de firma no supero la comprobacion.
    #[error("verificacion de firma fallida")]
    FirmaInvalida,

    /// El descifrado autenticado no supero la comprobacion de integridad.
    #[error("fallo de integridad en descifrado autenticado")]
    IntegridadFallida,
}

/// Mecanismo de encapsulado de clave resistente a computacion cuantica.
///
/// Implementacion de referencia prevista: ML-KEM-768 (FIPS 203).
pub trait EncapsuladoClave {
    /// Identificador del algoritmo, para registro y auditoria.
    fn algoritmo(&self) -> &'static str;

    /// Encapsula un secreto compartido contra la clave publica del par.
    ///
    /// Devuelve el texto cifrado de encapsulado y el secreto compartido derivado.
    fn encapsular(&self, clave_publica: &[u8]) -> Result<(Vec<u8>, Vec<u8>), ErrorPqc>;

    /// Recupera el secreto compartido a partir del encapsulado recibido.
    fn desencapsular(&self, clave_privada: &[u8], encapsulado: &[u8]) -> Result<Vec<u8>, ErrorPqc>;
}

/// Esquema de firma digital resistente a computacion cuantica.
///
/// Implementacion de referencia prevista: ML-DSA-65 (FIPS 204).
pub trait FirmaDigital {
    /// Identificador del algoritmo, para registro y auditoria.
    fn algoritmo(&self) -> &'static str;

    /// Firma un mensaje con la clave privada indicada.
    fn firmar(&self, clave_privada: &[u8], mensaje: &[u8]) -> Result<Vec<u8>, ErrorPqc>;

    /// Verifica la firma de un mensaje contra la clave publica indicada.
    fn verificar(&self, clave_publica: &[u8], mensaje: &[u8], firma: &[u8])
    -> Result<(), ErrorPqc>;
}

/// Cifrado autenticado para datos en reposo.
///
/// La clave simetrica se envuelve con [`EncapsuladoClave`]; este trait solo cubre
/// la operacion simetrica (AES-256-GCM).
pub trait CifradoEnReposo {
    /// Cifra y autentica el texto plano bajo la clave y el nonce dados.
    fn cifrar(&self, clave: &[u8], nonce: &[u8], plano: &[u8]) -> Result<Vec<u8>, ErrorPqc>;

    /// Descifra y verifica la integridad del texto cifrado.
    fn descifrar(&self, clave: &[u8], nonce: &[u8], cifrado: &[u8]) -> Result<Vec<u8>, ErrorPqc>;
}

/// Estado de conformidad del motor frente a los vectores oficiales del NIST.
///
/// RPT-003 §9.2 exige vectores ACVP para ML-KEM y ML-DSA antes de considerar
/// verificable cualquier implementacion. Este tipo permite que la CI y el
/// arranque del agente expongan ese estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformidadAcvp {
    /// Los vectores oficiales se ejecutaron y pasaron.
    Verificado,
    /// No se han ejecutado vectores oficiales contra esta implementacion.
    NoVerificado,
}

impl ConformidadAcvp {
    /// Indica si la implementacion puede considerarse apta para produccion.
    #[must_use]
    pub const fn apto_para_produccion(self) -> bool {
        matches!(self, Self::Verificado)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_vectores_acvp_no_es_apto_para_produccion() {
        assert!(!ConformidadAcvp::NoVerificado.apto_para_produccion());
        assert!(ConformidadAcvp::Verificado.apto_para_produccion());
    }
}
