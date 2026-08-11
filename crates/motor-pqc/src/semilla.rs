//! Derivacion determinista de un par de firma a partir de una semilla.
//!
//! RPT-025, PA-48.
//!
//! # Por que existe
//!
//! [`ClaveFirmaHibrida`](crate::firma_hibrida::ClaveFirmaHibrida) **no tiene
//! serializacion**, y la ausencia es deliberada: seria la funcion que permite al
//! material privado salir del proceso, justo lo que RPT-021 §3 obliga a evitar.
//!
//! Pero un emisor de manifiestos necesita la misma clave entre ejecuciones. La
//! salida no es anadir esa funcion sino observar que
//! [`generar_par`](crate::firma_hibrida::generar_par) es **determinista respecto
//! del generador que recibe**: bastan 32 bytes de semilla para reconstruir el par
//! entero. La clave vive en memoria lo que dura la firma.
//!
//! La semilla **es** la clave. Cifrarla no cambia eso: solo mueve el problema a
//! lo que la descifra.
//!
//! # Por que no se usa `rand_chacha`
//!
//! RPT-023 §3 nombraba `rand_chacha`, y al ir a anadirlo aparecio que no sirve:
//! `rand_chacha 0.9` implementa los rasgos de `rand_core 0.9`, y `generar_par`
//! exige el `CryptoRng` de `rand_core 0.10`. Meter una version mas de `rand_core`
//! en este arbol tiene precedente y no bueno — el comentario de
//! `guardian-cc/Cargo.toml` dice literalmente que tres versiones conviviendo ya
//! costaron una sesion.
//!
//! El expansor se construye aqui sobre `sha2`, que ya es dependencia. La
//! construccion es MGF1 —`SHA-256(etiqueta ‖ semilla ‖ contador)`, con los campos
//! prefijados en longitud como en el resto del proyecto—, que es la misma familia
//! que PKCS#1 estandariza para expandir una semilla en un flujo.
//!
//! Tiene una ventaja adicional sobre traer una dependencia: **ninguna llamada es
//! falible**, asi que no hay ningun `expect` que justificar en una ruta que
//! produce material de clave.

use sha2::{Digest as _, Sha256};

use crate::firma_hibrida::{ClaveFirmaHibrida, ClaveVerificacionHibrida, generar_par};
use crate::secreto::Secreto;

/// Longitud de la semilla, en bytes.
pub const LONGITUD_SEMILLA: usize = 32;

/// Semilla de la que se deriva un par de firma.
///
/// Es [`Secreto`], no `[u8; 32]`: el tipo impide que aparezca en un registro de
/// depuracion y se limpia al soltarse.
pub type SemillaFirma = Secreto<LONGITUD_SEMILLA>;

/// Etiqueta de dominio del expansor.
///
/// Sin ella, la misma semilla usada para otra cosa produciria el mismo flujo.
pub const ETIQUETA_SEMILLA: &[u8] = b"eje-latam/pqc/expansor-semilla/sha-256/v1";

/// Generador determinista a partir de una semilla.
///
/// # Alcance
///
/// Sirve **solo** para re-derivar un par de firma a partir de una semilla que ya
/// tiene 256 bits de entropia. No es un sustituto del generador del sistema para
/// **crear** esa semilla: eso exige `rand_core::OsRng` o equivalente, y este tipo
/// no lo hace por uno.
pub struct ExpansorSemilla {
    semilla: SemillaFirma,
    contador: u64,
    reserva: [u8; 32],
    consumidos: usize,
}

impl ExpansorSemilla {
    /// Expansor sobre la semilla dada.
    #[must_use]
    pub const fn nuevo(semilla: SemillaFirma) -> Self {
        Self {
            semilla,
            contador: 0,
            reserva: [0u8; 32],
            // Fuerza a producir el primer bloque en la primera lectura.
            consumidos: 32,
        }
    }

    /// Bloque `indice` del flujo.
    ///
    /// Los campos van prefijados en longitud por el mismo motivo que en
    /// `Absorbedor`: sin ellos, dos entradas distintas podrian concatenarse
    /// igual.
    fn bloque(&self, indice: u64) -> [u8; 32] {
        let mut resumen = Sha256::new();
        resumen.update((ETIQUETA_SEMILLA.len() as u64).to_be_bytes());
        resumen.update(ETIQUETA_SEMILLA);
        resumen.update((LONGITUD_SEMILLA as u64).to_be_bytes());
        resumen.update(self.semilla.exponer());
        resumen.update(indice.to_be_bytes());
        resumen.finalize().into()
    }

    /// Siguiente byte del flujo.
    fn siguiente_byte(&mut self) -> u8 {
        if self.consumidos >= self.reserva.len() {
            self.reserva = self.bloque(self.contador);
            self.contador = self.contador.wrapping_add(1);
            self.consumidos = 0;
        }

        let byte = self.reserva[self.consumidos];
        self.consumidos += 1;
        byte
    }
}

impl rand_core::TryRng for ExpansorSemilla {
    type Error = rand_core::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let mut bruto = [0u8; 4];
        for byte in &mut bruto {
            *byte = self.siguiente_byte();
        }
        Ok(u32::from_le_bytes(bruto))
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let mut bruto = [0u8; 8];
        for byte in &mut bruto {
            *byte = self.siguiente_byte();
        }
        Ok(u64::from_le_bytes(bruto))
    }

    fn try_fill_bytes(&mut self, destino: &mut [u8]) -> Result<(), Self::Error> {
        for byte in destino.iter_mut() {
            *byte = self.siguiente_byte();
        }
        Ok(())
    }
}

impl rand_core::TryCryptoRng for ExpansorSemilla {}

/// Deriva el par de firma correspondiente a una semilla.
///
/// La misma semilla produce siempre el mismo par. Semillas distintas producen
/// pares distintos.
#[must_use]
pub fn derivar_par(semilla: SemillaFirma) -> (ClaveFirmaHibrida, ClaveVerificacionHibrida) {
    generar_par(&mut ExpansorSemilla::nuevo(semilla))
}

/// Deriva **solo** la clave de verificacion de una semilla.
///
/// Existe para el aprovisionamiento: escribir el fichero `.pub` del agente no
/// necesita tener la privada viva mas alla de lo imprescindible.
#[must_use]
pub fn derivar_verificacion(semilla: SemillaFirma) -> ClaveVerificacionHibrida {
    derivar_par(semilla).1
}
