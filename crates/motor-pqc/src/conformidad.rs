//! Estado de conformidad de la implementación criptográfica.
//!
//! # Enmienda de RPT-005 §4.3
//!
//! La versión anterior de este módulo solo registraba la ejecución de los
//! vectores ACVP del NIST. **Era insuficiente.**
//!
//! CVE-2026-24850 —maleabilidad de firma en el verificador de ML-DSA de
//! RustCrypto— pasaba los vectores ACVP al completo. Se detectó únicamente con
//! los vectores de Wycheproof. La distinción es de propósito:
//!
//! | Conjunto | Qué prueba |
//! |---|---|
//! | ACVP (NIST) | Que se calcula **correctamente** lo que se debe calcular |
//! | Wycheproof (C2SP) | Que se **rechaza** lo que se debe rechazar |
//!
//! Y como ninguna implementación PQC en Rust tiene auditoría independiente
//! (RPT-005 §2.2), se exige además el contraste contra una segunda
//! implementación independiente.

/// Comprobaciones exigidas antes de considerar apto un motor criptográfico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Conformidad {
    /// Vectores oficiales ACVP del NIST ejecutados y superados.
    pub acvp: bool,
    /// Vectores adversarios de Wycheproof ejecutados y superados.
    pub wycheproof: bool,
    /// Contraste diferencial contra una segunda implementación superado.
    pub contraste_diferencial: bool,
}

impl Conformidad {
    /// Conformidad completa, con las tres comprobaciones superadas.
    pub const COMPLETA: Self = Self {
        acvp: true,
        wycheproof: true,
        contraste_diferencial: true,
    };

    /// Indica si la implementación puede considerarse apta para producción.
    ///
    /// Las tres comprobaciones son necesarias. Ninguna sustituye a otra.
    #[must_use]
    pub const fn apto_para_produccion(self) -> bool {
        self.acvp && self.wycheproof && self.contraste_diferencial
    }

    /// Comprobaciones pendientes, para el informe de arranque del agente.
    #[must_use]
    pub fn pendientes(self) -> Vec<&'static str> {
        let mut faltan = Vec::new();
        if !self.acvp {
            faltan.push("vectores ACVP del NIST");
        }
        if !self.wycheproof {
            faltan.push("vectores adversarios de Wycheproof");
        }
        if !self.contraste_diferencial {
            faltan.push("contraste diferencial contra segunda implementacion");
        }
        faltan
    }
}
