//! # Boveda Aislada — AGT-04
//!
//! Cola de eventos cifrados para operar durante un apagon digital, y su
//! reconciliacion contra `eje-almacen` al restablecerse el servicio.
//!
//! ## No es un almacen de secretos
//!
//! La Boveda persiste **eventos pendientes de reconciliacion**. Las credenciales
//! de switch van al almacen de secretos del sistema operativo (RPT-003 §6.2).

#![forbid(unsafe_code)]

use thiserror::Error;

/// Errores de la boveda.
#[derive(Debug, Error)]
pub enum ErrorBoveda {
    /// No queda espacio y la rotacion no pudo liberar suficiente.
    #[error("capacidad agotada: {usado_bytes} de {limite_bytes} bytes")]
    CapacidadAgotada {
        /// Bytes ocupados actualmente.
        usado_bytes: u64,
        /// Limite configurado.
        limite_bytes: u64,
    },

    /// El evento no pudo descifrarse durante la reconciliacion.
    #[error("fallo de integridad al reconciliar el evento {identificador}")]
    IntegridadFallida {
        /// Identificador del evento afectado.
        identificador: u64,
    },
}

/// Politica de retencion de la boveda (RPT-002 §5, AGT-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoliticaRetencion {
    /// Antiguedad maxima de un evento en la cola.
    pub dias_maximos: u32,
    /// Tamano maximo de la cola en bytes.
    pub limite_bytes: u64,
}

impl Default for PoliticaRetencion {
    /// Por defecto: 30 dias o 5 GB, lo que se alcance primero.
    fn default() -> Self {
        Self {
            dias_maximos: 30,
            limite_bytes: 5 * 1024 * 1024 * 1024,
        }
    }
}

/// Accion a tomar cuando la cola alcanza su limite.
///
/// Se descarta el evento mas antiguo **y se emite alerta obligatoria en VIS-04**.
/// Un disco lleno en un nodo hospitalario es una interrupcion, no un detalle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionDesbordamiento {
    /// Rotar descartando el evento mas antiguo, con alerta.
    RotarConAlerta,
}

/// Estado de vigencia de las reglas de respuesta cargadas en el agente.
///
/// Sin conectividad el agente sigue operando, pero las reglas tienen marca
/// temporal. Superado el umbral de obsolescencia, `guardian-cc` degrada a modo
/// solo-deteccion: actuar automaticamente con inteligencia vencida es peor que
/// no actuar (RPT-002 §5, AGT-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VigenciaReglas {
    /// Reglas dentro de la ventana de vigencia.
    Vigentes,
    /// Reglas obsoletas. Deteccion activa, respuesta automatica suspendida.
    Obsoletas,
}

impl VigenciaReglas {
    /// Indica si la respuesta automatica puede ejecutarse.
    #[must_use]
    pub const fn permite_respuesta_automatica(self) -> bool {
        matches!(self, Self::Vigentes)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn reglas_obsoletas_suspenden_respuesta_automatica() {
        assert!(!VigenciaReglas::Obsoletas.permite_respuesta_automatica());
        assert!(VigenciaReglas::Vigentes.permite_respuesta_automatica());
    }

    #[test]
    fn retencion_por_defecto_es_30_dias_y_5_gb() {
        let politica = PoliticaRetencion::default();
        assert_eq!(politica.dias_maximos, 30);
        assert_eq!(politica.limite_bytes, 5 * 1024 * 1024 * 1024);
    }
}
