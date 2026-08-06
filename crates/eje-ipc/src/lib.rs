//! # Eje-IPC — contrato entre Eje-Visión y Eje-Agente
//!
//! RPT-006, PA-20.
//!
//! ## El hueco que cierra este crate
//!
//! `eje-vision` declaraba sus canales en TypeScript y `eje-agente` no tenía
//! ninguna definición equivalente. Las dos mitades del puente estaban inventadas
//! por separado y no se habrían encontrado.
//!
//! Rust y TypeScript no pueden compartir tipos. La solución adoptada es la misma
//! que ya funciona para los vectores de prueba: **un manifiesto declarativo único
//! —`contrato-ipc.toml`— y una prueba de paridad en cada extremo** que falla si
//! su definición local diverge. La divergencia deja de ser posible en silencio.
//!
//! ## Transporte
//!
//! Socket de dominio Unix con ACL en Linux y macOS; named pipe con descriptor de
//! seguridad en Windows. **Sin puerto TCP local** (RPT-002 §9.3): un servicio en
//! `localhost` es alcanzable por cualquier proceso local y por cualquier página
//! que el usuario visite.

#![forbid(unsafe_code)]

pub mod mensajes;

#[cfg(test)]
mod pruebas;

use thiserror::Error;

/// Longitud máxima de un marco, en bytes.
///
/// Acota el consumo del proceso privilegiado ante un renderer comprometido.
/// Debe coincidir con `marco.longitud_maxima` de `contrato-ipc.toml`.
pub const LONGITUD_MAXIMA_MARCO: usize = 1_048_576;

/// Bytes del prefijo de longitud que precede a cada carga útil.
pub const PREFIJO_LONGITUD: usize = 4;

/// Errores del contrato IPC.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErrorIpc {
    /// El canal solicitado no figura en la lista de permitidos.
    ///
    /// No se distingue entre "canal inexistente" y "canal prohibido": informar
    /// cuál es cuál daría al renderer un oráculo sobre la superficie interna.
    #[error("canal no permitido")]
    CanalNoPermitido,

    /// La carga útil excede [`LONGITUD_MAXIMA_MARCO`].
    #[error("carga de {longitud} bytes; el maximo es {LONGITUD_MAXIMA_MARCO}")]
    CargaExcesiva {
        /// Longitud declarada o recibida.
        longitud: usize,
    },

    /// El marco está incompleto: faltan bytes por recibir.
    #[error("marco incompleto: se declararon {declarados} bytes y hay {disponibles}")]
    MarcoIncompleto {
        /// Longitud declarada en el prefijo.
        declarados: usize,
        /// Bytes realmente disponibles.
        disponibles: usize,
    },

    /// El prefijo de longitud no llegó completo.
    #[error("prefijo de longitud truncado")]
    PrefijoTruncado,
}

/// Canal permitido del puente.
///
/// La lista es de **permitidos**, no de bloqueados: un canal que no figure aquí
/// se rechaza aunque el preload lo invoque (RPT-004 §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Canal {
    /// VIS-04 — estado resumido del demonio.
    ObtenerEstadoAgente,
    /// VIS-04 — inventario vivo de dispositivos IoT/OT.
    ObtenerInventario,
    /// VIS-04 — ocupación de la Bóveda Aislada.
    ObtenerEstadoBoveda,
    /// VIS-01 — consulta SQL contra ALM-02.
    ConsultarSandbox,
    /// VIS-04 — sucesos de alerta anexados a ALM-01.
    ConsultarAlertas,
    /// VIS-04 — estados degradados vigentes.
    ObtenerCondiciones,
}

impl Canal {
    /// Todos los canales permitidos.
    ///
    /// El orden es estable y coincide con el de `contrato-ipc.toml`.
    pub const TODOS: [Self; 6] = [
        Self::ObtenerEstadoAgente,
        Self::ObtenerInventario,
        Self::ObtenerEstadoBoveda,
        Self::ConsultarSandbox,
        Self::ConsultarAlertas,
        Self::ObtenerCondiciones,
    ];

    /// Identificador estable del canal, tal como viaja por el puente.
    #[must_use]
    pub const fn identificador(self) -> &'static str {
        match self {
            Self::ObtenerEstadoAgente => "obtener-estado-agente",
            Self::ObtenerInventario => "obtener-inventario",
            Self::ObtenerEstadoBoveda => "obtener-estado-boveda",
            Self::ConsultarSandbox => "consultar-sandbox",
            Self::ConsultarAlertas => "consultar-alertas",
            Self::ObtenerCondiciones => "obtener-condiciones",
        }
    }

    /// Resuelve un canal a partir de su identificador.
    ///
    /// Devuelve `None` para cualquier nombre no permitido, incluidos los
    /// explícitamente prohibidos: quien llama no debe poder distinguirlos.
    #[must_use]
    pub fn desde_identificador(texto: &str) -> Option<Self> {
        Self::TODOS
            .into_iter()
            .find(|canal| canal.identificador() == texto)
    }
}

/// Valida una petición entrante del renderer.
///
/// # Errores
///
/// Devuelve [`ErrorIpc::CanalNoPermitido`] si el canal no está en la lista, o
/// [`ErrorIpc::CargaExcesiva`] si la carga supera el límite.
pub fn autorizar(canal: &str, longitud_carga: usize) -> Result<Canal, ErrorIpc> {
    let canal = Canal::desde_identificador(canal).ok_or(ErrorIpc::CanalNoPermitido)?;

    if longitud_carga > LONGITUD_MAXIMA_MARCO {
        return Err(ErrorIpc::CargaExcesiva {
            longitud: longitud_carga,
        });
    }

    Ok(canal)
}

/// Serializa una carga útil como marco con prefijo de longitud.
///
/// El prefijo es de 4 bytes big-endian. Un flujo sin delimitar obligaría al
/// receptor a adivinar dónde termina un mensaje, que es la clase de ambigüedad
/// que ya corregimos en `eje-almacen` y en el combinador poscuántico.
///
/// # Errores
///
/// Devuelve [`ErrorIpc::CargaExcesiva`] si la carga supera el límite.
pub fn enmarcar(carga: &[u8]) -> Result<Vec<u8>, ErrorIpc> {
    let longitud = carga.len();
    if longitud > LONGITUD_MAXIMA_MARCO {
        return Err(ErrorIpc::CargaExcesiva { longitud });
    }

    // El límite garantiza que la conversión no puede desbordar.
    let prefijo = (longitud as u32).to_be_bytes();

    let mut marco = Vec::with_capacity(PREFIJO_LONGITUD + longitud);
    marco.extend_from_slice(&prefijo);
    marco.extend_from_slice(carga);
    Ok(marco)
}

/// Extrae la carga útil de un marco completo.
///
/// La longitud se valida **antes** de reservar memoria: un prefijo malicioso que
/// declare cuatro gigabytes no debe provocar una reserva de cuatro gigabytes.
///
/// # Errores
///
/// Devuelve [`ErrorIpc::PrefijoTruncado`], [`ErrorIpc::CargaExcesiva`] o
/// [`ErrorIpc::MarcoIncompleto`] según el defecto encontrado.
pub fn desenmarcar(marco: &[u8]) -> Result<&[u8], ErrorIpc> {
    let Some(prefijo) = marco.get(..PREFIJO_LONGITUD) else {
        return Err(ErrorIpc::PrefijoTruncado);
    };

    let mut bytes = [0u8; PREFIJO_LONGITUD];
    bytes.copy_from_slice(prefijo);
    let declarados = u32::from_be_bytes(bytes) as usize;

    if declarados > LONGITUD_MAXIMA_MARCO {
        return Err(ErrorIpc::CargaExcesiva {
            longitud: declarados,
        });
    }

    let disponibles = marco.len() - PREFIJO_LONGITUD;
    marco
        .get(PREFIJO_LONGITUD..PREFIJO_LONGITUD + declarados)
        .ok_or(ErrorIpc::MarcoIncompleto {
            declarados,
            disponibles,
        })
}
