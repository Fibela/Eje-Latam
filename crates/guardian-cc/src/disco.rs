//! Acceso al sistema de ficheros para el inventario.
//!
//! RPT-014, PA-29.
//!
//! # Dos reglas
//!
//! ## La lectura se acota en la lectura, no en los metadatos
//!
//! Consultar el tamano con `metadata()` y despues leer es una condicion de
//! carrera: entre ambas llamadas el fichero puede crecer. Y hay rutas que
//! mienten sobre su tamano —un FIFO informa cero— o que crecen mientras se leen.
//!
//! Aqui se lee con `take(LONGITUD_MAXIMA + 1)` y se rechaza si llega el byte de
//! mas. El limite lo impone el lector, no lo que el fichero declare de si mismo.
//!
//! ## La escritura es atomica o no es
//!
//! Un corte de energia a mitad de escritura dejaria un inventario truncado. Como
//! el analizador lo rechazaria —`ErrorFormato::Truncado`—, el agente se quedaria
//! **sin inventario**, y sin inventario no hay marcados, y sin marcados los
//! equipos criticos dejan de estar protegidos.
//!
//! El fallo va en la direccion peligrosa, asi que la escritura se hace en un
//! fichero temporal del **mismo directorio** —para que el renombrado no cruce
//! sistema de ficheros— con sincronizacion antes de renombrar.

use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use crate::formato::LONGITUD_MAXIMA;

/// Fallo de acceso al almacen en disco.
#[derive(Debug, thiserror::Error)]
pub enum ErrorDisco {
    /// El fichero no existe.
    ///
    /// Se distingue de un fallo de lectura: en el primer arranque es esperable,
    /// y despues es un indicio.
    #[error("no existe el fichero {ruta}")]
    NoExiste {
        /// Ruta consultada.
        ruta: String,
    },

    /// El fichero supera [`LONGITUD_MAXIMA`].
    #[error("el fichero supera los {LONGITUD_MAXIMA} bytes admitidos")]
    Excesivo,

    /// Fallo del sistema de ficheros.
    #[error("fallo de entrada/salida en {ruta}: {detalle}")]
    Entrada {
        /// Ruta implicada.
        ruta: String,
        /// Descripcion del error subyacente.
        detalle: String,
    },
}

fn error_de(ruta: &Path, error: &std::io::Error) -> ErrorDisco {
    ErrorDisco::Entrada {
        ruta: ruta.display().to_string(),
        detalle: error.to_string(),
    }
}

/// Ruta del fichero temporal usado para la escritura atomica.
///
/// Vive en el **mismo directorio** que el destino: un renombrado entre sistemas
/// de ficheros distintos no es atomico y degrada a copiar y borrar.
#[must_use]
pub fn ruta_temporal(destino: &Path) -> PathBuf {
    let mut nombre = destino.file_name().unwrap_or_default().to_os_string();
    nombre.push(".parcial");
    destino.with_file_name(nombre)
}

/// Lee el inventario del disco con la longitud acotada.
///
/// # Errores
///
/// [`ErrorDisco::NoExiste`] si el fichero no esta, [`ErrorDisco::Excesivo`] si
/// supera el limite, [`ErrorDisco::Entrada`] ante cualquier otro fallo.
pub fn leer(ruta: &Path) -> Result<Vec<u8>, ErrorDisco> {
    let fichero = match File::open(ruta) {
        Ok(fichero) => fichero,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ErrorDisco::NoExiste {
                ruta: ruta.display().to_string(),
            });
        }
        Err(error) => return Err(error_de(ruta, &error)),
    };

    // Se lee un byte de mas a proposito: si llega, el fichero excede el limite.
    // No se consulta `metadata()`, que puede quedar obsoleta entre la consulta y
    // la lectura.
    let mut contenido = Vec::new();
    fichero
        .take(LONGITUD_MAXIMA as u64 + 1)
        .read_to_end(&mut contenido)
        .map_err(|error| error_de(ruta, &error))?;

    if contenido.len() > LONGITUD_MAXIMA {
        return Err(ErrorDisco::Excesivo);
    }

    Ok(contenido)
}

/// Escribe el inventario de forma atomica.
///
/// # Secuencia
///
/// 1. Escribir el contenido completo en el temporal.
/// 2. `sync_all` sobre el temporal: sin esto, el renombrado puede llegar al disco
///    antes que los datos y el resultado es un fichero de nombre correcto y
///    contenido vacio.
/// 3. Renombrar sobre el destino.
///
/// El renombrado sustituye el destino existente; `std::fs::rename` lo garantiza
/// en las plataformas objetivo.
///
/// # Errores
///
/// [`ErrorDisco::Entrada`] ante cualquier fallo. Si algo falla, el temporal se
/// retira: dejarlo alimentaria una recuperacion manual que tomaria un fichero a
/// medias por bueno.
pub fn escribir_atomico(ruta: &Path, contenido: &[u8]) -> Result<(), ErrorDisco> {
    let temporal = ruta_temporal(ruta);

    let resultado = escribir_temporal(&temporal, contenido);
    if resultado.is_err() {
        let _ = std::fs::remove_file(&temporal);
        return resultado;
    }

    std::fs::rename(&temporal, ruta).map_err(|error| {
        let _ = std::fs::remove_file(&temporal);
        error_de(ruta, &error)
    })
}

fn escribir_temporal(temporal: &Path, contenido: &[u8]) -> Result<(), ErrorDisco> {
    let mut fichero = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(temporal)
        .map_err(|error| error_de(temporal, &error))?;

    fichero
        .write_all(contenido)
        .map_err(|error| error_de(temporal, &error))?;

    // Sin esta sincronizacion el renombrado puede ordenarse antes que los datos.
    fichero
        .sync_all()
        .map_err(|error| error_de(temporal, &error))
}
