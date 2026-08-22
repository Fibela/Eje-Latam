//! Emisor de manifiestos firmados.
//!
//! RPT-025, PA-48.
//!
//! # Este crate no se despliega
//!
//! Es la decision que sostiene todo lo demas, y es de empaquetado antes que de
//! codigo.
//!
//! Si el emisor viviera dentro del binario del agente, **cada sensor desplegado
//! llevaria encima la capacidad de firmar inventarios**. Un sensor esta en el
//! armario de planta o en el rack de la clinica, fisicamente accesible, y su
//! modelo de amenaza asume que puede caer. Toda la cadena de cinco eslabones de
//! RPT-011 se apoya en que quien comprometa el agente **no pueda firmar**.
//!
//! De ahi que `eje-manifiesto` sea un crate aparte y que PA-12 —el
//! empaquetador— tenga la obligacion de dejarlo fuera. Una prueba de este crate
//! comprueba lo unico comprobable desde aqui: que `eje-agente` no lo declara
//! como dependencia.
//!
//! # Lo que hace y lo que todavia no
//!
//! Hace: derivar el par de firma de una semilla, leer y **verificar** el
//! manifiesto anterior para tomar su secuencia, construir el inventario y la
//! tabla de segmentos en orden canonico, firmarlos y serializarlos.
//!
//! Y desde RPT-026: leer el TOML del administrador ([`entrada`]), guardar la
//! semilla cifrada con Argon2id ([`reposo_semilla`]) y ofrecer todo eso desde un
//! binario.
//!
//! No hace: generar la clave de recuperacion, ni rotar nada.

pub mod entrada;
pub mod fragmento;
pub mod reposo_semilla;

use guardian_cc::almacen::InventarioLocal;
use guardian_cc::formato::{TECHO_SECUENCIA, serializar};
use guardian_cc::inventario::{
    Centinela, ClaveInventario, DominioClave, Inventario, MarcadoBruto, RaizAnclada,
    mensaje_de_raiz,
};
use guardian_cc::revocacion::RegistroRevocaciones;
use guardian_cc::vlan::{DeclaracionVlan, TablaVlan};
use motor_pqc::firma_hibrida::{ClaveFirmaHibrida, ClaveVerificacionHibrida, firmar};
use motor_pqc::semilla::{SemillaFirma, derivar_par};

/// Fallos de la emision.
#[derive(Debug, thiserror::Error)]
pub enum ErrorEmision {
    /// La secuencia solicitada alcanza o supera el techo.
    #[error("secuencia {solicitada} en el techo o por encima ({TECHO_SECUENCIA}); ver PA-33")]
    TechoAlcanzado {
        /// Secuencia que se pretendia emitir.
        solicitada: u64,
    },

    /// El manifiesto anterior no se pudo verificar.
    ///
    /// **No se degrada a «empezar de cero».** Si el emisor tomara la secuencia
    /// de un fichero sin comprobar su firma, quien editase el `.inv` del
    /// administrador decidiria que se emite a continuacion. Y si ante el fallo
    /// reiniciara en 1, bastaria corromper el fichero para que el siguiente
    /// manifiesto naciera revertido y el agente lo rechazara.
    #[error("el manifiesto anterior no verifica: {detalle}")]
    AnteriorNoVerifica {
        /// Motivo, para el registro.
        detalle: String,
    },

    /// El contenido del manifiesto no forma un inventario valido.
    #[error("inventario invalido: {detalle}")]
    InventarioInvalido {
        /// Motivo.
        detalle: String,
    },

    /// La tabla de segmentos no es valida.
    #[error("tabla de segmentos invalida: {detalle}")]
    SegmentosInvalidos {
        /// Motivo.
        detalle: String,
    },
}

/// Par de firma derivado de una semilla, vivo solo mientras se emite.
pub struct Emisor {
    firmante: ClaveFirmaHibrida,
    verificadora: ClaveVerificacionHibrida,
}

impl Emisor {
    /// Deriva el emisor de su semilla.
    ///
    /// La semilla se consume: no hay motivo para que siga viva despues de
    /// derivar, y dejarla accesible alargaria su vida en memoria sin ganar nada.
    #[must_use]
    pub fn desde_semilla(semilla: SemillaFirma) -> Self {
        let (firmante, verificadora) = derivar_par(semilla);
        Self {
            firmante,
            verificadora,
        }
    }

    /// Clave de verificacion, para aprovisionar el agente.
    #[must_use]
    pub const fn verificacion(&self) -> &ClaveVerificacionHibrida {
        &self.verificadora
    }

    /// Clave de verificacion envuelta en su dominio de custodia.
    #[must_use]
    pub fn como_clave_de_cliente(&self) -> ClaveInventario {
        ClaveInventario::nueva(self.verificadora.clone(), DominioClave::Cliente)
    }

    /// Secuencia que corresponde emitir a continuacion.
    ///
    /// # El manifiesto anterior es entrada, no memoria
    ///
    /// La alternativa —un fichero de estado con el ultimo numero— se pierde, y
    /// perderlo hace que el emisor reinicie en 1: el agente rechazaria por
    /// reversion todo lo que viniera despues. El manifiesto anterior ya lleva la
    /// secuencia dentro y es el unico sitio donde no puede desincronizarse.
    ///
    /// Pero eso lo convierte en entrada de un fichero que puede estar
    /// manipulado, asi que se **verifica antes de creerlo**. El centinela que se
    /// pasa es `Establecido(0)`: aqui no se comprueba frescura —el emisor no
    /// tiene marca de agua propia— sino firma y dominio de clave.
    ///
    /// # Errores
    ///
    /// [`ErrorEmision::AnteriorNoVerifica`] si el fichero no supera la
    /// verificacion, y [`ErrorEmision::TechoAlcanzado`] si la siguiente
    /// secuencia se saldria del rango (PA-33).
    pub fn secuencia_siguiente(&self, anterior: Option<&[u8]>) -> Result<u64, ErrorEmision> {
        let Some(bytes) = anterior else {
            // Primera emision. La secuencia 1 y no la 0 para que «sin manifiesto»
            // y «manifiesto inicial» no compartan numero.
            return Ok(1);
        };

        let clave = self.como_clave_de_cliente();
        let local = InventarioLocal::cargar(
            bytes,
            &clave,
            Centinela::Establecido(0),
            &RegistroRevocaciones::default(),
        )
        .map_err(|error| ErrorEmision::AnteriorNoVerifica {
            detalle: error.to_string(),
        })?;

        let siguiente = local.secuencia().saturating_add(1);
        comprobar_techo(siguiente)?;
        Ok(siguiente)
    }

    /// Firma una configuracion del sensor. RPT-074, PA-79.
    ///
    /// # Por que devuelve la firma y no el fichero
    ///
    /// El fichero lo compone [`guardian_cc::configuracion::serializar`], que es
    /// quien conoce la disposicion en disco. Si esta funcion produjera bytes,
    /// habria **dos** sitios que saben como se escribe una configuracion, y el
    /// dia que uno cambie el otro emitira ficheros que el sensor rechazara como
    /// manipulados.
    ///
    /// Se firma el mensaje canonico, no los bytes: dos codificaciones del mismo
    /// contenido no pueden dar firmas distintas.
    #[must_use]
    pub fn firmar_configuracion(
        &self,
        valores: &guardian_cc::configuracion::Valores,
    ) -> motor_pqc::firma_hibrida::FirmaHibrida {
        motor_pqc::firma_hibrida::firmar(
            &self.firmante,
            &guardian_cc::configuracion::mensaje_de_configuracion(valores),
        )
    }

    /// Emite un manifiesto firmado.
    ///
    /// El orden canonico y el rechazo de duplicados los impone
    /// `Inventario::construir` y `TablaVlan::construir`, que es el **mismo**
    /// codigo que usa el agente al leer. Escribir un constructor propio aqui
    /// permitiria que emisor y sensor discreparan sobre que es valido.
    ///
    /// # Errores
    ///
    /// [`ErrorEmision::TechoAlcanzado`],
    /// [`ErrorEmision::InventarioInvalido`] o
    /// [`ErrorEmision::SegmentosInvalidos`].
    pub fn emitir(
        &self,
        marcados: Vec<MarcadoBruto>,
        segmentos: Vec<DeclaracionVlan>,
        secuencia: u64,
    ) -> Result<Vec<u8>, ErrorEmision> {
        comprobar_techo(secuencia)?;

        let inventario =
            Inventario::construir(marcados).map_err(|error| ErrorEmision::InventarioInvalido {
                detalle: error.to_string(),
            })?;

        let vlans =
            TablaVlan::construir(segmentos).map_err(|error| ErrorEmision::SegmentosInvalidos {
                detalle: error.to_string(),
            })?;

        let raiz = inventario
            .raiz()
            .ok_or_else(|| ErrorEmision::InventarioInvalido {
                detalle: "un inventario vacio no tiene raiz y no significa nada".to_owned(),
            })?;

        let anclada = RaizAnclada {
            raiz,
            vlans: vlans.resumen(),
            secuencia,
        };

        let firma = firmar(&self.firmante, &mensaje_de_raiz(&anclada));

        Ok(serializar(&inventario, &vlans, secuencia, &firma))
    }
}

/// Rechaza una secuencia en el techo o por encima.
///
/// Se comprueba **aqui y en el agente**. La del agente es la defensa —cierra el
/// bloqueo de PA-33 en la puerta por la que llega todo inventario real—; esta es
/// una cortesia, para que el emisor no produzca un fichero que el sensor va a
/// rechazar. Un techo que solo viviera en el emisor no protegeria de nada:
/// quien tenga la clave no usa nuestro emisor.
fn comprobar_techo(secuencia: u64) -> Result<(), ErrorEmision> {
    if secuencia >= TECHO_SECUENCIA {
        return Err(ErrorEmision::TechoAlcanzado {
            solicitada: secuencia,
        });
    }
    Ok(())
}

#[cfg(test)]
mod pruebas;
