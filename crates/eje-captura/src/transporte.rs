//! Extraccion de cabeceras hasta la capa de transporte.
//!
//! RPT-020, PA-44.
//!
//! # Lo que esto es y lo que no
//!
//! Es **extraccion pasiva de cabeceras**: direcciones de enlace, etiqueta VLAN si
//! la hay, y puertos de origen y destino. Nada mas.
//!
//! **No es inspeccion profunda ni analisis de firma de aplicacion.** Un puerto no
//! prueba un protocolo: Modbus movido al 10502 se escapa, y cualquiera puede
//! abrir el 502 y hablar otra cosa. Llamar «huella» a esto seria exagerar.
//!
//! Se admite porque alimenta una fuente **inferida**, y por RPT-009 §3 una fuente
//! inferida solo puede sugerir criticidad, nunca descartarla. Un falso negativo
//! —Modbus en puerto raro— deja el dispositivo sin indicio, que es donde ya
//! estaba. Un falso positivo lleva a ambiguedad y a un humano. Las dos
//! direcciones de error son tolerables; no lo serian si esto pudiera declarar un
//! equipo «no critico».
//!
//! # Toda lectura esta acotada
//!
//! La trama viene de la red y su longitud no es una promesa. Cada acceso pasa por
//! `get`, y la funcion devuelve `None` en lugar de indexar a ciegas: un panico
//! aqui seria a peticion de quien emita la trama.

use crate::{DireccionEnlace, Trama};

/// Tipo de trama para IPv4.
const TIPO_IPV4: u16 = 0x0800;

/// Tipo de trama para etiqueta 802.1Q.
const TIPO_VLAN: u16 = 0x8100;

/// Protocolo IP para TCP.
const IP_TCP: u8 = 6;

/// Protocolo IP para UDP.
const IP_UDP: u8 = 17;

/// Longitud de la cabecera Ethernet II sin etiqueta.
const CABECERA_ETHERNET: usize = 14;

/// Longitud de la etiqueta 802.1Q, incluido el tipo que la sigue.
const ETIQUETA_VLAN: usize = 4;

/// Capa de transporte observada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transporte {
    /// TCP, con puertos de origen y destino.
    Tcp {
        /// Puerto de origen.
        origen: u16,
        /// Puerto de destino.
        destino: u16,
    },
    /// UDP, con puertos de origen y destino.
    Udp {
        /// Puerto de origen.
        origen: u16,
        /// Puerto de destino.
        destino: u16,
    },
}

impl Transporte {
    /// Puertos de origen y destino, sea cual sea el protocolo.
    #[must_use]
    pub const fn puertos(self) -> (u16, u16) {
        match self {
            Self::Tcp { origen, destino } | Self::Udp { origen, destino } => (origen, destino),
        }
    }
}

/// Cabeceras extraidas de una trama.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extraida {
    /// Direccion de enlace de origen.
    pub origen: DireccionEnlace,
    /// Direccion de enlace de destino.
    pub destino: DireccionEnlace,
    /// Identificador de VLAN, si la trama viene etiquetada.
    ///
    /// # Por que importa
    ///
    /// RPT-018 §8.3 anticipaba que `ProveedorSegmento` y `ProveedorHuella`
    /// podrian dejar de ser independientes. Aqui esta la razon concreta: **el
    /// segmento de un dispositivo se conoce por la misma trama que su
    /// protocolo**. En un puerto espejo sin etiquetar, esto es `None` y el
    /// segmento tiene que venir de otra parte.
    pub vlan: Option<u16>,
    /// Capa de transporte, si la trama llega hasta ella.
    pub transporte: Option<Transporte>,
}

/// Lee dos bytes en orden de red.
fn leer_u16(bytes: &[u8], desde: usize) -> Option<u16> {
    let par = bytes.get(desde..desde + 2)?;
    Some(u16::from_be_bytes([par[0], par[1]]))
}

/// Extrae las cabeceras de una trama.
///
/// Devuelve `None` si la trama no alcanza siquiera para las direcciones de
/// enlace. Una trama que llega hasta el enlace pero no mas alla devuelve
/// `Some` con `transporte: None`: **no observar transporte no es un fallo**, es
/// una trama que no lo lleva.
#[must_use]
pub fn extraer(trama: &Trama) -> Option<Extraida> {
    let bytes = trama.bytes.as_slice();

    let origen = trama.origen()?;
    let destino = trama.destino()?;

    let mut tipo = leer_u16(bytes, 12)?;
    let mut desplazamiento = CABECERA_ETHERNET;
    let mut vlan = None;

    if tipo == TIPO_VLAN {
        let control = leer_u16(bytes, CABECERA_ETHERNET)?;
        // Los doce bits bajos son el identificador; los otros cuatro son
        // prioridad y elegibilidad de descarte, que aqui no interesan.
        vlan = Some(control & 0x0FFF);
        tipo = leer_u16(bytes, CABECERA_ETHERNET + 2)?;
        desplazamiento += ETIQUETA_VLAN;
    }

    let extraida = Extraida {
        origen,
        destino,
        vlan,
        transporte: None,
    };

    if tipo != TIPO_IPV4 {
        return Some(extraida);
    }

    let Some(&primero) = bytes.get(desplazamiento) else {
        return Some(extraida);
    };

    // Los cuatro bits bajos son la longitud de cabecera en palabras de 32 bits.
    // El minimo legal es 5; por debajo la cabecera esta malformada y no se
    // interpreta el resto.
    let palabras = usize::from(primero & 0x0F);
    if palabras < 5 {
        return Some(extraida);
    }
    let longitud_ip = palabras * 4;

    let Some(&protocolo) = bytes.get(desplazamiento + 9) else {
        return Some(extraida);
    };

    let inicio = desplazamiento + longitud_ip;
    let (Some(puerto_origen), Some(puerto_destino)) =
        (leer_u16(bytes, inicio), leer_u16(bytes, inicio + 2))
    else {
        return Some(extraida);
    };

    let transporte = match protocolo {
        IP_TCP => Some(Transporte::Tcp {
            origen: puerto_origen,
            destino: puerto_destino,
        }),
        IP_UDP => Some(Transporte::Udp {
            origen: puerto_origen,
            destino: puerto_destino,
        }),
        _ => None,
    };

    Some(Extraida {
        transporte,
        ..extraida
    })
}
