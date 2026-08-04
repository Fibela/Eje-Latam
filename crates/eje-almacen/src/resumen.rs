//! Resumenes criptograficos y codificacion canonica.
//!
//! # Por que la codificacion lleva prefijos de longitud
//!
//! Concatenar campos sin delimitar su longitud permite colisiones triviales: los
//! pares `("ab", "c")` y `("a", "bc")` producen la misma secuencia `"abc"` y por
//! tanto el mismo resumen.
//!
//! En un registro de evidencia eso significa que un atacante con capacidad de
//! anexar podria construir dos asientos distintos con identico resumen, y la
//! cadena de custodia dejaria de distinguirlos. Todo campo se absorbe precedido
//! de su longitud en 8 bytes big-endian.

use sha2::{Digest, Sha256};

/// Resumen SHA-256 de 32 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Resumen([u8; 32]);

impl Resumen {
    /// Resumen de todo ceros, usado como antecesor del primer asiento.
    pub const GENESIS: Self = Self([0u8; 32]);

    /// Construye un resumen a partir de sus bytes.
    #[must_use]
    pub const fn desde_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Devuelve los bytes del resumen.
    #[must_use]
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Representacion hexadecimal en minusculas.
    #[must_use]
    pub fn hexadecimal(&self) -> String {
        let mut salida = String::with_capacity(64);
        for byte in self.0 {
            salida.push(digito_hex(byte >> 4));
            salida.push(digito_hex(byte & 0x0f));
        }
        salida
    }

    /// Interpreta una cadena hexadecimal de 64 caracteres.
    ///
    /// Devuelve `None` si la longitud no es 64 o hay caracteres no hexadecimales.
    #[must_use]
    pub fn desde_hexadecimal(texto: &str) -> Option<Self> {
        if texto.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        let crudo = texto.as_bytes();
        for (indice, destino) in bytes.iter_mut().enumerate() {
            let alto = valor_hex(crudo[indice * 2])?;
            let bajo = valor_hex(crudo[indice * 2 + 1])?;
            *destino = (alto << 4) | bajo;
        }
        Some(Self(bytes))
    }
}

const fn digito_hex(valor: u8) -> char {
    match valor {
        0..=9 => (b'0' + valor) as char,
        _ => (b'a' + valor - 10) as char,
    }
}

const fn valor_hex(caracter: u8) -> Option<u8> {
    match caracter {
        b'0'..=b'9' => Some(caracter - b'0'),
        b'a'..=b'f' => Some(caracter - b'a' + 10),
        b'A'..=b'F' => Some(caracter - b'A' + 10),
        _ => None,
    }
}

/// Acumulador de campos con codificacion canonica.
///
/// Cada campo se absorbe precedido de su longitud, de modo que ninguna
/// combinacion distinta de campos pueda producir la misma secuencia de bytes.
pub struct Absorbedor {
    interno: Sha256,
}

impl Absorbedor {
    /// Crea un acumulador vacio con la etiqueta de dominio indicada.
    ///
    /// La etiqueta separa dominios de uso: un resumen de asiento nunca puede
    /// coincidir con un resumen de hoja Merkle aunque los datos sean identicos.
    #[must_use]
    pub fn nuevo(dominio: &[u8]) -> Self {
        let mut interno = Sha256::new();
        interno.update((dominio.len() as u64).to_be_bytes());
        interno.update(dominio);
        Self { interno }
    }

    /// Absorbe un campo de bytes con su longitud.
    pub fn campo(&mut self, valor: &[u8]) -> &mut Self {
        self.interno.update((valor.len() as u64).to_be_bytes());
        self.interno.update(valor);
        self
    }

    /// Absorbe un entero sin signo de 64 bits.
    pub fn entero(&mut self, valor: u64) -> &mut Self {
        self.campo(&valor.to_be_bytes())
    }

    /// Absorbe un entero con signo de 64 bits.
    pub fn entero_con_signo(&mut self, valor: i64) -> &mut Self {
        self.campo(&valor.to_be_bytes())
    }

    /// Absorbe un resumen previo.
    pub fn resumen(&mut self, valor: &Resumen) -> &mut Self {
        self.campo(valor.bytes())
    }

    /// Cierra el acumulador y devuelve el resumen.
    #[must_use]
    pub fn finalizar(self) -> Resumen {
        Resumen(self.interno.finalize().into())
    }
}
