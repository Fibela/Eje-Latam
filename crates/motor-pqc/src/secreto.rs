//! Material sensible con borrado seguro.
//!
//! RPT-005 §7.4: toda clave privada y todo secreto compartido se envuelve en un
//! tipo con borrado seguro. Un volcado de memoria de un nodo hospitalario no debe
//! entregar claves privadas ML-KEM.

use zeroize::{Zeroize, ZeroizeOnDrop};

/// Secreto de longitud fija, borrado de memoria al destruirse.
///
/// No implementa `Debug` con contenido ni `Display`: el material sensible no debe
/// poder acabar en un registro por descuido.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Secreto<const N: usize> {
    bytes: [u8; N],
}

impl<const N: usize> Secreto<N> {
    /// Envuelve unos bytes como secreto.
    #[must_use]
    pub const fn nuevo(bytes: [u8; N]) -> Self {
        Self { bytes }
    }

    /// Acceso de solo lectura al material.
    ///
    /// Deliberadamente explícito: quien lo invoca asume la responsabilidad de no
    /// copiar el contenido a un lugar sin borrado seguro.
    #[must_use]
    pub const fn exponer(&self) -> &[u8; N] {
        &self.bytes
    }

    /// Longitud del secreto en bytes.
    #[must_use]
    pub const fn longitud(&self) -> usize {
        N
    }
}

impl<const N: usize> core::fmt::Debug for Secreto<N> {
    /// Nunca imprime el contenido.
    fn fmt(&self, formateador: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formateador, "Secreto<{N}>(oculto)")
    }
}

impl<const N: usize> PartialEq for Secreto<N> {
    /// Comparación en tiempo constante.
    ///
    /// Una comparación que abandona al primer byte distinto filtra por canal
    /// temporal cuántos bytes coincidían, lo que permite reconstruir un secreto
    /// byte a byte.
    fn eq(&self, otro: &Self) -> bool {
        let mut diferencia: u8 = 0;
        for indice in 0..N {
            diferencia |= self.bytes[indice] ^ otro.bytes[indice];
        }
        diferencia == 0
    }
}

impl<const N: usize> Eq for Secreto<N> {}

/// Secreto compartido de 32 bytes, salida del intercambio de claves híbrido.
pub type SecretoCompartido = Secreto<32>;
