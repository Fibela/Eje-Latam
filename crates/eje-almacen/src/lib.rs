//! # Eje-Almacen — ALM-01 y ALM-02
//!
//! Motor de persistencia local sobre libSQL, con dos bases estrictamente separadas.
//!
//! ## Por que dos bases
//!
//! El corpus original permitia al analista ejecutar `ALTER` y `DROP` sobre las
//! mismas tablas que custodian la evidencia, lo que destruye su valor probatorio
//! (RPT-002 §5, ALM-01/ALM-02).
//!
//! | | ALM-01 Registro de Evidencia | ALM-02 Sandbox del Analista |
//! |---|---|---|
//! | Escritura | Solo anexado, exclusiva del agente | Libre |
//! | DDL desde la GUI | Imposible | Permitido |
//! | Integridad | Cadena de hashes (Merkle) | No aplica |
//! | Origen | Agente | Copia de solo lectura de ALM-01 |
//!
//! ## La cadena Merkle es Apache-2.0 por necesidad
//!
//! El valor probatorio de una cadena de custodia depende de que su algoritmo sea
//! auditable. Una cadena forense cerrada no sirve en un proceso judicial
//! (RPT-003 §2.5).
//!
//! ## Los secretos no viven aqui
//!
//! Las credenciales de switch **no se almacenan en ALM-01**: rotan, y un registro
//! solo-anexado no permite eliminarlas; ademas se exportarian junto con la
//! evidencia en un proceso judicial (RPT-003 §6.1). Van al almacen de secretos
//! del sistema operativo.

#![forbid(unsafe_code)]

use thiserror::Error;

/// Errores del almacen.
#[derive(Debug, Error)]
pub enum ErrorAlmacen {
    /// Se intento una operacion de modificacion sobre el registro de evidencia.
    #[error("operacion '{operacion}' prohibida sobre el registro de evidencia")]
    EscrituraProhibida {
        /// Operacion rechazada.
        operacion: &'static str,
    },

    /// La verificacion de la cadena de hashes detecto una discontinuidad.
    #[error("cadena de custodia rota en el asiento {asiento}")]
    CadenaRota {
        /// Numero de asiento donde se detecto la discontinuidad.
        asiento: u64,
    },

    /// Se intento persistir un secreto en el registro de evidencia.
    #[error("los secretos no se almacenan en el registro de evidencia (RPT-003 §6.1)")]
    SecretoEnEvidencia,
}

/// Base de datos destino de una operacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseDestino {
    /// ALM-01. Solo anexado, encadenada, inmutable.
    RegistroEvidencia,
    /// ALM-02. Modificable por el analista.
    SandboxAnalista,
}

/// Clase de operacion SQL solicitada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaseOperacion {
    /// Lectura.
    Consulta,
    /// Anexado de un nuevo asiento.
    Anexado,
    /// Modificacion de filas existentes.
    Modificacion,
    /// Cambio de esquema (`ALTER`, `DROP`, `CREATE`).
    DefinicionEsquema,
}

impl ClaseOperacion {
    /// Nombre estable de la operacion, para mensajes de error y registro.
    #[must_use]
    pub const fn nombre(self) -> &'static str {
        match self {
            Self::Consulta => "consulta",
            Self::Anexado => "anexado",
            Self::Modificacion => "modificacion",
            Self::DefinicionEsquema => "definicion de esquema",
        }
    }
}

/// Modo de esquema seleccionado en el lanzador VIS-03.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoEsquema {
    /// Esquemas preconfigurados: marcas de tiempo, IoC regionales, registros de red.
    Estandar,
    /// Persistencia reducida para nodos de recursos limitados.
    IotLigero,
    /// Esquema libre del analista. Opera **solo sobre ALM-02**.
    Personalizado,
}

/// Decide si una operacion es admisible sobre la base indicada.
///
/// El registro de evidencia solo admite consulta y anexado. Cualquier
/// modificacion o cambio de esquema se rechaza, sin excepcion ni bandera de
/// configuracion que lo habilite.
///
/// # Errores
///
/// Devuelve [`ErrorAlmacen::EscrituraProhibida`] si se intenta modificar el
/// registro de evidencia.
pub const fn autorizar(base: BaseDestino, operacion: ClaseOperacion) -> Result<(), ErrorAlmacen> {
    match base {
        BaseDestino::SandboxAnalista => Ok(()),
        BaseDestino::RegistroEvidencia => match operacion {
            ClaseOperacion::Consulta | ClaseOperacion::Anexado => Ok(()),
            ClaseOperacion::Modificacion | ClaseOperacion::DefinicionEsquema => {
                Err(ErrorAlmacen::EscrituraProhibida {
                    operacion: operacion.nombre(),
                })
            }
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn evidencia_admite_consulta_y_anexado() {
        assert!(autorizar(BaseDestino::RegistroEvidencia, ClaseOperacion::Consulta).is_ok());
        assert!(autorizar(BaseDestino::RegistroEvidencia, ClaseOperacion::Anexado).is_ok());
    }

    #[test]
    fn evidencia_rechaza_modificacion_y_ddl() {
        let evidencia = BaseDestino::RegistroEvidencia;
        assert!(autorizar(evidencia, ClaseOperacion::Modificacion).is_err());
        assert!(autorizar(evidencia, ClaseOperacion::DefinicionEsquema).is_err());
    }

    #[test]
    fn sandbox_admite_todo() {
        for operacion in [
            ClaseOperacion::Consulta,
            ClaseOperacion::Anexado,
            ClaseOperacion::Modificacion,
            ClaseOperacion::DefinicionEsquema,
        ] {
            assert!(autorizar(BaseDestino::SandboxAnalista, operacion).is_ok());
        }
    }
}
