//! Fichero de clave de verificacion aprovisionada.
//!
//! RPT-024, PA-49.
//!
//! # La mitad que faltaba
//!
//! RPT-011 construyo cinco eslabones para que un marcado solo valga si lo firmo
//! el administrador del cliente. Pero `arrancar` recibe la [`ClaveInventario`]
//! **como parametro**, y nadie se la da: `eje-agente` opera en primer arranque
//! porque no hay forma de que el sensor sepa con que clave verificar.
//!
//! Emitir manifiestos firmados (PA-48) no sirve de nada si el sensor no puede
//! comprobarlos. Este modulo es la otra mitad de esa funcion.
//!
//! # Disposicion
//!
//! ```text
//! +--------------------------------------------------+
//! | magico       8 bytes  "EJE-PUB1"                  |
//! | version      u16 BE                               |
//! | dominio      u8    escalar cerrado, 0 invalido    |
//! | clave        longitud fija, ML-DSA-65 + Ed25519   |
//! +--------------------------------------------------+
//! ```
//!
//! # El dominio viaja en el fichero
//!
//! Es la decision que sostiene todo lo demas. [`DominioClave`] existe desde
//! RPT-011 §4 para que la clave con la que PremosCorp firma binarios no pueda
//! declarar que equipos del cliente son criticos, y al reves. Si el dominio se
//! declarase en el codigo que carga el fichero —«esta ruta es la operativa»—,
//! **la separacion dependeria de que nadie confunda dos rutas**, que es
//! exactamente la clase de garantia que este proyecto se niega a aceptar.
//!
//! Al viajar dentro, un fichero de recuperacion colocado donde va el operativo
//! se rechaza por lo que **es**, no por donde esta.
//!
//! # Este fichero no esta firmado, y no puede estarlo
//!
//! Conviene decirlo antes de que alguien lo suponga. Es el ancla de confianza:
//! firmarlo exigiria otra clave para verificar la firma, y esa otra habria que
//! aprovisionarla igual. La regresion no termina.
//!
//! Lo que protege a este fichero no es criptografia sino **el momento**: se
//! escribe durante la instalacion, con un humano presente, y a partir de ahi
//! [`Centinela`](crate::inventario::Centinela) detecta que desaparezca. Quien
//! pueda sustituirlo despues ya tiene escritura en el almacen del agente, que es
//! un compromiso mas grave que el que este fichero podria evitar.

use motor_pqc::firma_hibrida::ClaveVerificacionHibrida;

use crate::inventario::{ClaveInventario, DominioClave};

/// Numero magico que abre todo fichero de clave aprovisionada.
pub const MAGICO_CLAVE: &[u8; 8] = b"EJE-PUB1";

/// Version del formato de clave.
pub const VERSION_CLAVE: u16 = 1;

/// Bytes de cabecera: magico, version y dominio.
const LONGITUD_CABECERA: usize = 8 + 2 + 1;

/// Defectos del fichero de clave.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorClave {
    /// El fichero no empieza por [`MAGICO_CLAVE`].
    #[error("el fichero no es una clave de Eje-Latam")]
    MagicoAusente,

    /// Version desconocida.
    #[error("version de clave {encontrada}; este binario entiende la {VERSION_CLAVE}")]
    VersionDesconocida {
        /// Version leida.
        encontrada: u16,
    },

    /// La longitud no es la exacta que el formato exige.
    #[error("el fichero de clave declara {encontrada} bytes; se esperaban {esperados}")]
    LongitudIncorrecta {
        /// Bytes disponibles.
        encontrada: usize,
        /// Bytes que el formato exige.
        esperados: usize,
    },

    /// El codigo de dominio no corresponde a ninguna variante.
    ///
    /// El `0` cae aqui a proposito, igual que en el resto de escalares del
    /// proyecto: un fichero de ceros no debe analizarse como una clave valida.
    #[error("codigo de dominio de clave {codigo} desconocido")]
    DominioDesconocido {
        /// Codigo leido.
        codigo: u8,
    },

    /// El material de clave no decodifica.
    #[error("el material de clave no decodifica")]
    ClaveMalformada,

    /// El fichero es de otro dominio del que se esperaba.
    ///
    /// No es un defecto del fichero sino de su colocacion, y por eso el error lo
    /// nombra: aprovisionar la clave de recuperacion donde va la operativa
    /// permitiria que la clave de emergencia firmase inventarios corrientes.
    #[error("clave del dominio {encontrado:?}; esta ruta exige {esperado:?}")]
    DominioInesperado {
        /// Dominio declarado en el fichero.
        encontrado: DominioClave,
        /// Dominio que la ruta exige.
        esperado: DominioClave,
    },
}

/// Codigo escalar de un dominio de clave.
///
/// Empieza en 1: ver [`ErrorClave::DominioDesconocido`].
#[must_use]
pub const fn codigo_de_dominio(dominio: DominioClave) -> u8 {
    match dominio {
        DominioClave::Cliente => 1,
        DominioClave::PremosCorp => 2,
        DominioClave::ClienteRecuperacion => 3,
    }
}

/// Dominio a partir de su codigo escalar.
#[must_use]
pub const fn dominio_desde_codigo(codigo: u8) -> Option<DominioClave> {
    match codigo {
        1 => Some(DominioClave::Cliente),
        2 => Some(DominioClave::PremosCorp),
        3 => Some(DominioClave::ClienteRecuperacion),
        _ => None,
    }
}

/// Longitud exacta del fichero.
#[must_use]
pub fn longitud_fichero() -> usize {
    LONGITUD_CABECERA + ClaveVerificacionHibrida::longitud_serializada()
}

/// Serializa una clave de verificacion con su dominio.
#[must_use]
pub fn serializar(clave: &ClaveVerificacionHibrida, dominio: DominioClave) -> Vec<u8> {
    let mut salida = Vec::with_capacity(longitud_fichero());
    salida.extend_from_slice(MAGICO_CLAVE);
    salida.extend_from_slice(&VERSION_CLAVE.to_be_bytes());
    salida.push(codigo_de_dominio(dominio));
    salida.extend_from_slice(&clave.a_bytes());
    salida
}

/// Analiza un fichero de clave y comprueba que su dominio es el esperado.
///
/// # Orden de comprobaciones
///
/// Magico, version, longitud, dominio y material, en ese orden. El dominio va
/// **antes** que el material por el mismo motivo que en
/// [`RaizVerificada::verificar`](crate::inventario::RaizVerificada::verificar):
/// una clave del dominio equivocado no debe llegar a gastar ciclos de
/// decodificacion.
///
/// # Errores
///
/// Una variante de [`ErrorClave`] por defecto detectado.
pub fn analizar(bytes: &[u8], esperado: DominioClave) -> Result<ClaveInventario, ErrorClave> {
    let esperados = longitud_fichero();

    if bytes.len() < LONGITUD_CABECERA {
        return Err(ErrorClave::LongitudIncorrecta {
            encontrada: bytes.len(),
            esperados,
        });
    }

    if &bytes[..8] != MAGICO_CLAVE {
        return Err(ErrorClave::MagicoAusente);
    }

    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    if version != VERSION_CLAVE {
        return Err(ErrorClave::VersionDesconocida {
            encontrada: version,
        });
    }

    // Longitud exacta: ni corta ni larga. Una cola sobrante admitiria dos
    // lecturas del mismo fichero, que es la ambiguedad que el resto del proyecto
    // cierra.
    if bytes.len() != esperados {
        return Err(ErrorClave::LongitudIncorrecta {
            encontrada: bytes.len(),
            esperados,
        });
    }

    let codigo = bytes[10];
    let Some(encontrado) = dominio_desde_codigo(codigo) else {
        return Err(ErrorClave::DominioDesconocido { codigo });
    };

    if encontrado != esperado {
        return Err(ErrorClave::DominioInesperado {
            encontrado,
            esperado,
        });
    }

    let clave = ClaveVerificacionHibrida::desde_bytes(&bytes[LONGITUD_CABECERA..])
        .map_err(|_| ErrorClave::ClaveMalformada)?;

    Ok(ClaveInventario::nueva(clave, encontrado))
}
