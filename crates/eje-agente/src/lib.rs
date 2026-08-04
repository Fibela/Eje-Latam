//! # Eje-Agente
//!
//! Demonio local soberano de Eje-Latam. Integra los modulos AGT-01 a AGT-07 y
//! opera con capacidad plena **sin depender de conectividad ni de infraestructura
//! de PremosCorp** (RPT-002 §1).
//!
//! ## Principio de producto (RPT-003 §3.1)
//!
//! Ninguna condicion comercial degrada jamas una funcion de seguridad. Una licencia
//! vencida no desactiva deteccion ni contencion.

#![forbid(unsafe_code)]

use eje_almacen::ModoEsquema;
use eje_red::ConfiguracionRed;
use guardian_cc::PerfilSegmento;

/// Version del agente, tomada del manifiesto del paquete.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Estado de la licencia del nodo.
///
/// Ver RPT-003 §3.4 para la matriz completa de degradacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoLicencia {
    /// Licencia dentro de su periodo de validez.
    Vigente,
    /// Licencia expirada, sin incidente en curso.
    VencidaSinIncidente,
    /// Licencia expirada con incidente activo. VIS-02 opera completo.
    VencidaConIncidenteActivo,
}

impl EstadoLicencia {
    /// Indica si las funciones de seguridad operan al completo.
    ///
    /// Devuelve `true` **siempre**: AGT-01 a AGT-07 nunca se degradan por motivo
    /// comercial. La funcion existe para que la invariante quede explicita y
    /// verificable en pruebas, no porque pueda devolver `false`.
    #[must_use]
    pub const fn seguridad_completa(self) -> bool {
        true
    }

    /// Indica si VIS-02 puede exportar reportes y comparativas historicas.
    #[must_use]
    pub const fn permite_exportacion_de_reportes(self) -> bool {
        matches!(self, Self::Vigente)
    }

    /// Indica si VIS-02 muestra el estado operativo en vivo.
    ///
    /// Durante un incidente activo se muestra aunque la licencia este vencida:
    /// dejar a un comite de crisis hospitalario sin tablero por una fecha de
    /// facturacion es un fallo de producto con consecuencias reales.
    #[must_use]
    pub const fn permite_tablero_en_vivo(self) -> bool {
        matches!(self, Self::Vigente | Self::VencidaConIncidenteActivo)
    }
}

/// Configuracion de arranque del agente, fijada por el lanzador VIS-03.
#[derive(Debug, Clone)]
pub struct ConfiguracionAgente {
    /// Perfil del segmento vigilado.
    pub perfil: PerfilSegmento,
    /// Modo de esquema de la base local.
    pub modo_esquema: ModoEsquema,
    /// Configuracion de la capa de red.
    pub red: ConfiguracionRed,
    /// Estado de licencia del nodo.
    pub licencia: EstadoLicencia,
}

impl ConfiguracionAgente {
    /// Construye la configuracion por defecto para un segmento dado.
    ///
    /// El perfil OT aplica las restricciones de descubrimiento y Capa B sin
    /// necesidad de configuracion adicional.
    #[must_use]
    pub fn para_segmento(perfil: PerfilSegmento) -> Self {
        Self {
            perfil,
            modo_esquema: ModoEsquema::Estandar,
            red: ConfiguracionRed {
                perfil,
                capa_b_autorizada: false,
            },
            licencia: EstadoLicencia::Vigente,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn ninguna_condicion_comercial_degrada_la_seguridad() {
        for estado in [
            EstadoLicencia::Vigente,
            EstadoLicencia::VencidaSinIncidente,
            EstadoLicencia::VencidaConIncidenteActivo,
        ] {
            assert!(
                estado.seguridad_completa(),
                "la seguridad se degrado con licencia {estado:?}"
            );
        }
    }

    #[test]
    fn incidente_activo_conserva_el_tablero_en_vivo() {
        assert!(EstadoLicencia::VencidaConIncidenteActivo.permite_tablero_en_vivo());
        assert!(!EstadoLicencia::VencidaSinIncidente.permite_tablero_en_vivo());
    }

    #[test]
    fn licencia_vencida_bloquea_exportacion_pero_no_seguridad() {
        let estado = EstadoLicencia::VencidaSinIncidente;
        assert!(!estado.permite_exportacion_de_reportes());
        assert!(estado.seguridad_completa());
    }

    #[test]
    fn perfil_ot_arranca_con_capa_b_deshabilitada() {
        let configuracion = ConfiguracionAgente::para_segmento(PerfilSegmento::Ot);
        assert!(configuracion.red.autorizar_capa_b().is_err());
    }
}
