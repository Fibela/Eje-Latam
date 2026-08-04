//! Intercambio de claves híbrido X25519 + ML-KEM-768.
//!
//! Categoría 3 del NIST. La componente clásica garantiza que el canal no queda
//! peor que hoy si apareciera un ataque práctico contra retículos; la componente
//! poscuántica lo protege frente a la recolección presente para descifrado
//! futuro (RPT-005 §7.4).
//!
//! # Aleatoriedad
//!
//! Todas las operaciones reciben el generador como parámetro. **No se activa la
//! característica `getrandom` de ninguna dependencia**: la fuente de aleatoriedad
//! es una decisión abierta (PA-18) y no debe quedar fijada por descuido en la
//! elección de una bandera de característica.
//!
//! # Nota sobre `rand_core`
//!
//! `ml-kem` 0.3 usa `rand_core` 0.9 y `x25519-dalek` 2.0 usa `rand_core` 0.6.
//! Ambas versiones conviven en el árbol de dependencias y sus traits **no son
//! intercambiables**. Este módulo nunca invoca los constructores aleatorios de
//! `x25519-dalek`: rellena los 32 bytes desde el generador propio y construye el
//! secreto con `StaticSecret::from`. Así el conflicto de versiones no aparece, y
//! además queda explícito de dónde sale cada byte de aleatoriedad.

use ml_kem::{Ciphertext, Decapsulate, Encapsulate, Kem, MlKem768};
use rand_core::CryptoRng;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::ErrorPqc;
use crate::combinador::derivar_secreto_hibrido;
use crate::secreto::SecretoCompartido;

/// Identificador del algoritmo, para registro y auditoría.
pub const ALGORITMO: &str = "x25519+ml-kem-768";

/// Clave de encapsulado ML-KEM-768.
pub type PublicaPoscuantica = ml_kem::EncapsulationKey<MlKem768>;

/// Clave de desencapsulado ML-KEM-768.
pub type PrivadaPoscuantica = ml_kem::DecapsulationKey<MlKem768>;

/// Clave pública híbrida del receptor.
#[derive(Clone)]
pub struct ClavePublicaHibrida {
    /// Componente poscuántica.
    pub poscuantica: PublicaPoscuantica,
    /// Componente clásica.
    pub clasica: PublicKey,
}

/// Clave privada híbrida del receptor.
pub struct ClavePrivadaHibrida {
    /// Componente poscuántica.
    pub poscuantica: PrivadaPoscuantica,
    /// Componente clásica.
    pub clasica: StaticSecret,
}

/// Material transmitido al receptor para establecer el secreto compartido.
#[derive(Clone)]
pub struct Encapsulado {
    /// Texto cifrado de encapsulado ML-KEM-768.
    pub cifrado_poscuantico: Ciphertext<MlKem768>,
    /// Clave pública efímera X25519 del emisor.
    pub publica_clasica: PublicKey,
}

impl Encapsulado {
    /// Serializa el encapsulado para su transmisión.
    #[must_use]
    pub fn a_bytes(&self) -> Vec<u8> {
        let mut salida = Vec::new();
        salida.extend_from_slice(&self.cifrado_poscuantico);
        salida.extend_from_slice(self.publica_clasica.as_bytes());
        salida
    }
}

/// Genera un par de claves híbrido.
pub fn generar_par<R: CryptoRng>(generador: &mut R) -> (ClavePrivadaHibrida, ClavePublicaHibrida) {
    let (privada_pq, publica_pq) = MlKem768::generate_keypair_from_rng(generador);

    let mut semilla = [0u8; 32];
    generador.fill_bytes(&mut semilla);
    let privada_clasica = StaticSecret::from(semilla);
    let publica_clasica = PublicKey::from(&privada_clasica);

    (
        ClavePrivadaHibrida {
            poscuantica: privada_pq,
            clasica: privada_clasica,
        },
        ClavePublicaHibrida {
            poscuantica: publica_pq,
            clasica: publica_clasica,
        },
    )
}

/// Establece un secreto compartido contra la clave pública del receptor.
///
/// # Errores
///
/// Propaga [`ErrorPqc::DerivacionFallida`] del combinador.
pub fn encapsular<R: CryptoRng>(
    publica: &ClavePublicaHibrida,
    generador: &mut R,
) -> Result<(Encapsulado, SecretoCompartido), ErrorPqc> {
    let (cifrado_poscuantico, secreto_pq) = publica.poscuantica.encapsulate_with_rng(generador);

    let mut semilla = [0u8; 32];
    generador.fill_bytes(&mut semilla);
    let efimera = StaticSecret::from(semilla);
    let publica_clasica = PublicKey::from(&efimera);
    let secreto_clasico = efimera.diffie_hellman(&publica.clasica);

    let encapsulado = Encapsulado {
        cifrado_poscuantico,
        publica_clasica,
    };

    let compartido = derivar_secreto_hibrido(
        &secreto_pq,
        secreto_clasico.as_bytes(),
        &encapsulado.cifrado_poscuantico,
        encapsulado.publica_clasica.as_bytes(),
    )?;

    Ok((encapsulado, compartido))
}

/// Recupera el secreto compartido a partir del encapsulado recibido.
///
/// # Errores
///
/// Propaga [`ErrorPqc::DerivacionFallida`] del combinador.
pub fn desencapsular(
    privada: &ClavePrivadaHibrida,
    encapsulado: &Encapsulado,
) -> Result<SecretoCompartido, ErrorPqc> {
    let secreto_pq = privada
        .poscuantica
        .decapsulate(&encapsulado.cifrado_poscuantico);
    let secreto_clasico = privada.clasica.diffie_hellman(&encapsulado.publica_clasica);

    derivar_secreto_hibrido(
        &secreto_pq,
        secreto_clasico.as_bytes(),
        &encapsulado.cifrado_poscuantico,
        encapsulado.publica_clasica.as_bytes(),
    )
}
