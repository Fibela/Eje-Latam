//! Revocacion de la clave de inventario.
//!
//! RPT-015, PA-33.
//!
//! # Que resuelve
//!
//! La autoridad del inventario descansa en una sola clave. Si se filtra, el
//! atacante emite secuencias crecientes igual que el legitimo: la monotonia de
//! RPT-012 no le estorba.
//!
//! # Tres decisiones que se hacen mal por defecto
//!
//! ## La revocacion no es total
//!
//! Invalidar «todo lo firmado por K» invalida tambien los inventarios legitimos
//! anteriores al compromiso. El agente se quedaria sin marcados, y sin marcados
//! los equipos criticos dejan de estar protegidos: seria provocarnos la perdida
//! que el atacante buscaba.
//!
//! Por eso el certificado lleva una **secuencia de corte**: cae lo firmado por
//! encima de ella, sobrevive lo de por debajo.
//!
//! ## Quien firma no puede ser ninguna de las dos claves conocidas
//!
//! Ni la operativa —el atacante la tiene— ni la de PremosCorp, porque
//! [`DominioClave`] existe desde RPT-011 para que el proveedor no pueda decidir
//! que equipos del cliente son criticos. Hace falta un tercer dominio,
//! [`DominioClave::ClienteRecuperacion`], en custodia del cliente y fuera de
//! linea.
//!
//! ## El certificado **baja** el centinela
//!
//! Es la enmienda de RPT-015 §6.1 a la regla «el centinela nunca retrocede».
//!
//! Sin ella, un atacante con la clave operativa emite un inventario con secuencia
//! `u64::MAX`, el agente lo acepta —la firma es valida— y ningun inventario
//! legitimo puede ya superarlo. El inventario queda congelado **para siempre**,
//! con un solo mensaje, y revocar no lo arregla porque el centinela sigue arriba.
//!
//! El resultado seria peor que el compromiso. De ahi que un certificado valido
//! reinicie el centinela a su secuencia de corte: es la unica operacion
//! autorizada a bajar la marca de agua, y es segura porque exige la clave que el
//! atacante no tiene.

use eje_almacen::resumen::{Absorbedor, Resumen};
use motor_pqc::firma_hibrida::{ClaveVerificacionHibrida, FirmaHibrida, verificar};

use crate::inventario::{ClaveInventario, DominioClave};

/// Dominio del resumen que identifica una clave.
const DOMINIO_IDENTIFICADOR: &[u8] = b"eje-latam/agt-01/identificador-clave/v1";

/// Dominio del mensaje firmado de un certificado de revocacion.
///
/// Separado del de la raiz del inventario: sin etiquetas distintas, una firma
/// sobre un certificado podria presentarse como firma de raiz.
const DOMINIO_CERTIFICADO: &[u8] = b"eje-latam/agt-01/certificado-revocacion/v1";

/// Identificador estable de una clave de verificacion.
///
/// Es el resumen de su forma serializada. Se usa en lugar de la clave completa
/// porque el registro de revocaciones debe poder nombrar una clave que ya no se
/// conserva.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdentificadorClave(Resumen);

impl IdentificadorClave {
    /// Deriva el identificador de una clave de verificacion.
    #[must_use]
    pub fn de(clave: &ClaveVerificacionHibrida) -> Self {
        let mut absorbedor = Absorbedor::nuevo(DOMINIO_IDENTIFICADOR);
        absorbedor.campo(&clave.a_bytes());
        Self(absorbedor.finalizar())
    }

    /// Resumen subyacente, para registro forense.
    #[must_use]
    pub const fn resumen(&self) -> &Resumen {
        &self.0
    }
}

/// Errores de la revocacion.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorRevocacion {
    /// El certificado no lo firma una clave del dominio de recuperacion.
    ///
    /// Admitir aqui la clave operativa dejaria que el atacante se
    /// «autorrevocase» a una secuencia de corte alta, que es lo contrario de una
    /// revocacion.
    #[error("el certificado exige una clave de recuperacion; se presento {encontrado:?}")]
    DominioDeClaveIncorrecto {
        /// Dominio de la clave presentada.
        encontrado: DominioClave,
    },

    /// La firma del certificado no verifica.
    #[error("la firma del certificado de revocacion no verifica")]
    FirmaInvalida,

    /// El certificado se revoca a si mismo.
    ///
    /// Revocar la clave sucesora en el mismo acto dejaria al cliente sin
    /// autoridad ninguna sobre su inventario.
    #[error("el certificado declara la misma clave como revocada y como sucesora")]
    SucesoraEsLaRevocada,
}

/// Certificado tal como llega, **sin verificar**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertificadoRevocacion {
    /// Clave que deja de valer por encima del corte.
    pub revocada: IdentificadorClave,
    /// Secuencia de corte. Lo firmado por `revocada` **por encima** de este
    /// valor deja de aceptarse; lo de por debajo sigue siendo valido.
    pub hasta_secuencia: u64,
    /// Clave que sustituye a la revocada.
    pub sucesora: IdentificadorClave,
    /// Instante de emision, en segundos desde la epoca.
    pub emitido_en: u64,
}

/// Mensaje canonico que la clave de recuperacion firma.
#[must_use]
pub fn mensaje_de_certificado(certificado: &CertificadoRevocacion) -> Vec<u8> {
    let mut absorbedor = Absorbedor::nuevo(DOMINIO_CERTIFICADO);
    absorbedor
        .resumen(certificado.revocada.resumen())
        .entero(certificado.hasta_secuencia)
        .resumen(certificado.sucesora.resumen())
        .entero(certificado.emitido_en);
    absorbedor.finalizar().bytes().to_vec()
}

/// Certificado cuya firma y dominio de clave ya se comprobaron.
///
/// Campos privados y sin constructor publico salvo [`Self::verificar`]. Que
/// exista **es** la prueba de que lo firmo la clave de recuperacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertificadoVerificado {
    revocada: IdentificadorClave,
    hasta_secuencia: u64,
    sucesora: IdentificadorClave,
}

impl CertificadoVerificado {
    /// Comprueba dominio de clave, coherencia y firma.
    ///
    /// # Errores
    ///
    /// [`ErrorRevocacion::DominioDeClaveIncorrecto`],
    /// [`ErrorRevocacion::SucesoraEsLaRevocada`] o
    /// [`ErrorRevocacion::FirmaInvalida`].
    pub fn verificar(
        certificado: CertificadoRevocacion,
        firma: &FirmaHibrida,
        clave: &ClaveInventario,
    ) -> Result<Self, ErrorRevocacion> {
        if clave.dominio() != DominioClave::ClienteRecuperacion {
            return Err(ErrorRevocacion::DominioDeClaveIncorrecto {
                encontrado: clave.dominio(),
            });
        }

        if certificado.revocada == certificado.sucesora {
            return Err(ErrorRevocacion::SucesoraEsLaRevocada);
        }

        verificar(
            clave.verificacion(),
            &mensaje_de_certificado(&certificado),
            firma,
        )
        .map_err(|_| ErrorRevocacion::FirmaInvalida)?;

        Ok(Self {
            revocada: certificado.revocada,
            hasta_secuencia: certificado.hasta_secuencia,
            sucesora: certificado.sucesora,
        })
    }

    /// Clave revocada.
    #[must_use]
    pub const fn revocada(&self) -> IdentificadorClave {
        self.revocada
    }

    /// Secuencia de corte.
    #[must_use]
    pub const fn hasta_secuencia(&self) -> u64 {
        self.hasta_secuencia
    }

    /// Clave sucesora.
    #[must_use]
    pub const fn sucesora(&self) -> IdentificadorClave {
        self.sucesora
    }
}

/// Registro local de claves revocadas.
///
/// # Por que basta un fichero y no hace falta el ancla de PA-28
///
/// A diferencia del centinela de frescura, este conjunto **solo crece** y el
/// certificado se puede **volver a presentar**. Perderlo devuelve el sistema al
/// estado de antes de la revocacion, que es el estado en el que ya vivimos; no
/// por debajo. El cliente conserva el certificado y reponerlo es presentarlo de
/// nuevo (RPT-015 §5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistroRevocaciones {
    entradas: Vec<(IdentificadorClave, u64)>,
}

impl RegistroRevocaciones {
    /// Registro vacio.
    #[must_use]
    pub const fn nuevo() -> Self {
        Self {
            entradas: Vec::new(),
        }
    }

    /// Anota un certificado verificado.
    ///
    /// Si la clave ya figuraba, **se conserva el corte mas bajo**. Un corte
    /// posterior mas alto aflojaria una revocacion existente, y una revocacion
    /// que se puede aflojar no es una revocacion.
    pub fn anotar(&mut self, certificado: &CertificadoVerificado) {
        let revocada = certificado.revocada();
        let corte = certificado.hasta_secuencia();

        if let Some(entrada) = self
            .entradas
            .iter_mut()
            .find(|(identificador, _)| *identificador == revocada)
        {
            entrada.1 = entrada.1.min(corte);
            return;
        }

        self.entradas.push((revocada, corte));
    }

    /// Corte anotado para una clave, si esta revocada.
    #[must_use]
    pub fn corte_de(&self, identificador: &IdentificadorClave) -> Option<u64> {
        self.entradas
            .iter()
            .find(|(anotado, _)| anotado == identificador)
            .map(|(_, corte)| *corte)
    }

    /// Indica si la clave puede haber firmado esa secuencia.
    ///
    /// Una clave no revocada puede firmar cualquiera. Una revocada, solo hasta su
    /// corte inclusive.
    #[must_use]
    pub fn admite(&self, identificador: &IdentificadorClave, secuencia: u64) -> bool {
        match self.corte_de(identificador) {
            None => true,
            Some(corte) => secuencia <= corte,
        }
    }

    /// Numero de claves revocadas.
    #[must_use]
    pub fn anotadas(&self) -> usize {
        self.entradas.len()
    }
}
