//! # Eje-Red — RED-01 y RED-02
//!
//! Capa A: descubrimiento de nodos vecinos en la red local.
//! Capa B: enlace punto a punto entre sedes con atravesamiento de NAT.
//!
//! ## Correccion de la Capa A
//!
//! El corpus original incluia "enlaces BGP dedicados" en la Capa A. BGP es un
//! protocolo de enrutamiento entre sistemas autonomos, no un mecanismo de
//! descubrimiento en LAN. Si el cliente posee enlaces dedicados entre sedes,
//! corresponden a la Capa B como transporte (RPT-002 §9.2).
//!
//! ## Honestidad sobre la Capa B
//!
//! La P2P verdaderamente sin infraestructura externa aplica **solo a subredes con
//! ruteo directo**. El atravesamiento fiable de NAT requiere punto de encuentro:
//! sin el se falla frente a CGNAT y NAT simetrico, que dominan el acceso
//! residencial y de pyme en la region (RPT-002 §5, RED-02).

#![forbid(unsafe_code)]

use guardian_cc::PerfilSegmento;
use thiserror::Error;

/// Errores de la capa de red.
#[derive(Debug, Error)]
pub enum ErrorRed {
    /// Se intento emitir descubrimiento activo en un segmento OT.
    #[error("descubrimiento activo prohibido en segmento OT")]
    DescubrimientoActivoEnOt,

    /// Se intento habilitar la Capa B en un segmento OT sin autorizacion explicita.
    #[error("la Capa B esta deshabilitada por defecto en segmento OT")]
    CapaBDeshabilitadaEnOt,

    /// No se alcanzo el servidor de senalizacion y el NAT requiere relevo.
    #[error("senalizacion inalcanzable en {servidor}: el NAT requiere relevo")]
    SenalizacionInalcanzable {
        /// Servidor de senalizacion configurado.
        servidor: String,
    },
}

/// Rol del servidor de asistencia para la Capa B.
///
/// STUN y DERP **no son lo mismo**: STUN descubre la direccion publica y no
/// resuelve NAT simetrico; DERP transporta el trafico y tiene costo real de ancho
/// de banda (RPT-003 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolAsistencia {
    /// Descubrimiento de direccion publica. Sin costo de transito.
    Stun,
    /// Relevo de trafico. Necesario ante NAT simetrico. Con costo de transito.
    Derp,
}

/// Alojamiento del servidor de asistencia.
///
/// La opcion propia debe ser **prominente y sin coste adicional**: un servidor
/// operado por PremosCorp observa las direcciones IP publicas de todos los
/// clientes, metadato sensible en un producto que vende soberania del dato
/// (RPT-003 §7, matiz 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlojamientoAsistencia {
    /// Instancia oficial de PremosCorp.
    Oficial,
    /// Instancia desplegada en la infraestructura del cliente.
    Propia {
        /// Punto final configurado por el cliente.
        punto_final: String,
    },
}

/// Transporte del canal de senalizacion.
///
/// El puerto 3478/UDP se bloquea con frecuencia en redes corporativas y casi
/// siempre en OT, por lo que la ruta sobre TLS/443 no es opcional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransporteSenalizacion {
    /// UDP en el puerto estandar 3478.
    Udp3478,
    /// TLS sobre 443, ruta alternativa obligatoria.
    Tls443,
}

/// Configuracion de la capa de red para un segmento.
#[derive(Debug, Clone)]
pub struct ConfiguracionRed {
    /// Perfil del segmento vigilado.
    pub perfil: PerfilSegmento,
    /// Si la Capa B fue habilitada de forma deliberada por el cliente.
    pub capa_b_autorizada: bool,
}

impl ConfiguracionRed {
    /// Comprueba si puede emitirse descubrimiento activo (mDNS / difusion UDP).
    ///
    /// # Errores
    ///
    /// Devuelve [`ErrorRed::DescubrimientoActivoEnOt`] en segmentos OT: la difusion
    /// puede degradar PLC antiguos (RPT-002 §9.2).
    pub const fn autorizar_descubrimiento_activo(&self) -> Result<(), ErrorRed> {
        if self.perfil.permite_descubrimiento_activo() {
            Ok(())
        } else {
            Err(ErrorRed::DescubrimientoActivoEnOt)
        }
    }

    /// Comprueba si la Capa B puede establecerse.
    ///
    /// En segmento OT esta deshabilitada por defecto: una conexion saliente a
    /// internet desde un segmento industrial puede vulnerar la segmentacion en
    /// zonas y conductos que exige IEC 62443 (RPT-003 §7, matiz 4).
    ///
    /// # Errores
    ///
    /// Devuelve [`ErrorRed::CapaBDeshabilitadaEnOt`] si el segmento es OT y el
    /// cliente no la autorizo de forma deliberada.
    pub const fn autorizar_capa_b(&self) -> Result<(), ErrorRed> {
        match self.perfil {
            PerfilSegmento::Corporativo => Ok(()),
            PerfilSegmento::Ot => {
                if self.capa_b_autorizada {
                    Ok(())
                } else {
                    Err(ErrorRed::CapaBDeshabilitadaEnOt)
                }
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn ot_bloquea_descubrimiento_activo() {
        let configuracion = ConfiguracionRed {
            perfil: PerfilSegmento::Ot,
            capa_b_autorizada: false,
        };
        assert!(configuracion.autorizar_descubrimiento_activo().is_err());
    }

    #[test]
    fn ot_bloquea_capa_b_sin_autorizacion_explicita() {
        let configuracion = ConfiguracionRed {
            perfil: PerfilSegmento::Ot,
            capa_b_autorizada: false,
        };
        assert!(configuracion.autorizar_capa_b().is_err());
    }

    #[test]
    fn ot_permite_capa_b_con_autorizacion_deliberada() {
        let configuracion = ConfiguracionRed {
            perfil: PerfilSegmento::Ot,
            capa_b_autorizada: true,
        };
        assert!(configuracion.autorizar_capa_b().is_ok());
    }

    #[test]
    fn corporativo_permite_ambas() {
        let configuracion = ConfiguracionRed {
            perfil: PerfilSegmento::Corporativo,
            capa_b_autorizada: false,
        };
        assert!(configuracion.autorizar_descubrimiento_activo().is_ok());
        assert!(configuracion.autorizar_capa_b().is_ok());
    }
}
