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

// ---------------------------------------------------------------------------
// Forma de la peticion y de la respuesta — RPT-035, PA-41
// ---------------------------------------------------------------------------
//
// Este bloque faltaba y no se noto mientras no hubo transporte. El manifiesto
// declaraba QUE canales existen y QUE campos lleva cada carga, pero no COMO
// viaja el nombre del canal por el cable. Al escribir el servicio, cada extremo
// habria tenido que inventarlo — que es lo que este contrato existe para
// impedir.

/// Bytes del prefijo que declara la longitud del nombre de canal.
pub const PREFIJO_NOMBRE: usize = 2;

/// Cota del nombre de canal, en bytes.
///
/// Ningun identificador declarado se acerca. La cota existe para que un prefijo
/// absurdo no provoque una reserva absurda, por el mismo motivo que la del
/// marco.
pub const NOMBRE_MAXIMO: usize = 64;

/// Primer byte de una respuesta valida.
pub const CODIGO_RESPUESTA: u8 = 0;

/// Primer byte de un rechazo, con el motivo en texto a continuacion.
///
/// # Por que hay codigo y no silencio
///
/// Un canal que devolviera bytes vacios ante un rechazo seria indistinguible de
/// uno que devuelve una lista vacia. Es el tercer estado de RPT-006 §4 en el
/// cable: «no hay nada» y «no pude decirtelo» no son lo mismo.
pub const CODIGO_RECHAZO: u8 = 1;

/// Compone la carga de una peticion: nombre de canal y carga util.
///
/// El nombre va **prefijado en longitud** y no delimitado por un separador: un
/// separador obliga a decidir que pasa si aparece dentro del nombre, y esa
/// decision no deberia existir.
///
/// # Errores
///
/// [`ErrorIpc::CanalNoPermitido`] si el canal no esta en la lista, o
/// [`ErrorIpc::CargaExcesiva`] si la carga supera el limite.
pub fn componer_peticion(canal: Canal, carga: &[u8]) -> Result<Vec<u8>, ErrorIpc> {
    autorizar(canal.identificador(), carga.len())?;

    let nombre = canal.identificador().as_bytes();
    let mut salida = Vec::with_capacity(PREFIJO_NOMBRE + nombre.len() + carga.len());

    // El identificador de un canal del enum nunca excede la cota; se convierte
    // sin desenvolver por si algun dia deja de ser cierto.
    let longitud = u16::try_from(nombre.len()).unwrap_or(u16::MAX);
    salida.extend_from_slice(&longitud.to_be_bytes());
    salida.extend_from_slice(nombre);
    salida.extend_from_slice(carga);
    Ok(salida)
}

/// Descompone la carga de una peticion en canal autorizado y carga util.
///
/// # Orden de comprobaciones
///
/// Cota del nombre, longitud disponible, y solo despues autorizacion. Nada se
/// reserva en funcion de un valor sin validar, y **el canal se autoriza antes de
/// que quien llame vea la carga**.
///
/// # Errores
///
/// [`ErrorIpc::PrefijoTruncado`] si no cabe el prefijo o el nombre,
/// [`ErrorIpc::CanalNoPermitido`] si el nombre no es un canal de la lista o
/// excede la cota, [`ErrorIpc::CargaExcesiva`] si la carga es mayor del limite.
pub fn descomponer_peticion(carga: &[u8]) -> Result<(Canal, &[u8]), ErrorIpc> {
    let Some(prefijo) = carga.get(..PREFIJO_NOMBRE) else {
        return Err(ErrorIpc::PrefijoTruncado);
    };

    let mut bytes = [0u8; PREFIJO_NOMBRE];
    bytes.copy_from_slice(prefijo);
    let longitud = u16::from_be_bytes(bytes) as usize;

    // La cota va antes de indexar: un nombre declarado de sesenta y cinco mil
    // bytes no debe llegar siquiera a la comprobacion de limites.
    if longitud > NOMBRE_MAXIMO {
        return Err(ErrorIpc::CanalNoPermitido);
    }

    let Some(nombre) = carga.get(PREFIJO_NOMBRE..PREFIJO_NOMBRE + longitud) else {
        return Err(ErrorIpc::PrefijoTruncado);
    };

    // Un nombre que no es UTF-8 no puede ser ninguno de los declarados, asi que
    // se rechaza como canal desconocido y no como error de codificacion: el
    // motivo que importa es que no esta permitido.
    let Ok(nombre) = std::str::from_utf8(nombre) else {
        return Err(ErrorIpc::CanalNoPermitido);
    };

    let util = &carga[PREFIJO_NOMBRE + longitud..];
    let canal = autorizar(nombre, util.len())?;
    Ok((canal, util))
}

/// Compone una respuesta valida.
///
/// # Errores
///
/// [`ErrorIpc::CargaExcesiva`] si la carga supera el limite.
pub fn componer_respuesta(carga: &[u8]) -> Result<Vec<u8>, ErrorIpc> {
    if carga.len() >= LONGITUD_MAXIMA_MARCO {
        return Err(ErrorIpc::CargaExcesiva {
            longitud: carga.len(),
        });
    }

    let mut salida = Vec::with_capacity(1 + carga.len());
    salida.push(CODIGO_RESPUESTA);
    salida.extend_from_slice(carga);
    Ok(salida)
}

/// Compone un rechazo con su motivo.
///
/// El motivo se **recorta** en lugar de fallar: quien rechaza ya esta en el
/// camino de error, y un fallo al construir el mensaje de fallo dejaria al otro
/// extremo sin respuesta ninguna.
#[must_use]
pub fn componer_rechazo(motivo: &str) -> Vec<u8> {
    let bytes = motivo.as_bytes();
    let hasta = bytes.len().min(LONGITUD_MAXIMA_MARCO - 1);

    let mut salida = Vec::with_capacity(1 + hasta);
    salida.push(CODIGO_RECHAZO);
    salida.extend_from_slice(&bytes[..hasta]);
    salida
}
