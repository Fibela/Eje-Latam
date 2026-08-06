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

/// Retira el fichero temporal en cualquier salida, salvo desarme explicito.
///
/// # Que cubre y que no
///
/// Cubre **todo camino de salida por retorno**: `Err` temprano, `?`, o salida
/// normal sin desarmar. Eso es lo que una comprobacion `if resultado.is_err()`
/// no daba, porque solo veia el camino que quien la escribio recordo.
///
/// **No cubre el panico en compilacion de release.** El perfil de release de este
/// workspace declara `panic = "abort"`, asi que un panico aborta el proceso sin
/// desenrollar la pila y este destructor no llega a ejecutarse. En `dev` y en las
/// pruebas si se ejecuta, porque alli el panico desenrolla.
///
/// La consecuencia practica es benigna y conviene dejarla escrita: un `.parcial`
/// huerfano tras un aborto **no es corrupcion**, porque el destino no se toco. Es
/// basura, y ni siquiera se acumula: [`ruta_temporal`] es determinista y la
/// siguiente escritura lo trunca. El cargador lee una ruta fija y nunca puede
/// confundir el temporal con el inventario.
struct LimpiadorTemporal<'a> {
    ruta: &'a Path,
    armado: bool,
}

impl<'a> LimpiadorTemporal<'a> {
    const fn nuevo(ruta: &'a Path) -> Self {
        Self { ruta, armado: true }
    }

    /// Desarma la guarda tras un renombrado exitoso.
    ///
    /// Consume la guarda: una vez renombrado, el fichero temporal ya no existe
    /// con ese nombre y borrarlo destruiria el inventario recien colocado.
    fn desarmar(mut self) {
        self.armado = false;
    }
}

impl Drop for LimpiadorTemporal<'_> {
    fn drop(&mut self) {
        if self.armado {
            // El error se descarta a proposito: estamos en un camino de fallo y
            // no hay nada mejor que hacer que intentarlo.
            let _ = std::fs::remove_file(self.ruta);
        }
    }
}

/// Escribe el inventario de forma atomica.
///
/// # Secuencia
///
/// 1. Escribir el contenido completo en el temporal.
/// 2. `sync_all` sobre el temporal: sin esto, el renombrado puede llegar al disco
///    antes que los datos y el resultado es un fichero de nombre correcto y
///    contenido vacio.
/// 3. Renombrar sobre el destino y desarmar la guarda.
///
/// El renombrado sustituye el destino existente; `std::fs::rename` lo garantiza
/// en las plataformas objetivo.
///
/// # Por que importa que sea atomica
///
/// Un corte de energia a mitad de escritura dejaria un inventario truncado, que
/// el analizador rechazaria. El agente se quedaria **sin inventario**, y sin
/// inventario no hay marcados, y sin marcados los equipos criticos dejan de estar
/// protegidos. El fallo va en la direccion peligrosa.
///
/// # Errores
///
/// [`ErrorDisco::Entrada`] ante cualquier fallo. El temporal se retira por
/// [`LimpiadorTemporal`]; dejarlo alimentaria una recuperacion manual que tomaria
/// un fichero a medias por bueno.
pub fn escribir_atomico(ruta: &Path, contenido: &[u8]) -> Result<(), ErrorDisco> {
    let temporal = ruta_temporal(ruta);
    let limpiador = LimpiadorTemporal::nuevo(&temporal);

    escribir_temporal(&temporal, contenido)?;
    std::fs::rename(&temporal, ruta).map_err(|error| error_de(ruta, &error))?;

    limpiador.desarmar();
    Ok(())
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

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Directorio de trabajo aislado, retirado al terminar.
    ///
    /// No se usa `tempfile`: una dependencia mas en un crate que ya arrastra
    /// criptografia, para algo que son doce lineas.
    struct DirectorioDePrueba {
        ruta: PathBuf,
    }

    impl DirectorioDePrueba {
        fn nuevo(nombre: &str) -> Self {
            let ruta = std::env::temp_dir().join(format!("eje-latam-{nombre}"));
            let _ = std::fs::remove_dir_all(&ruta);
            std::fs::create_dir_all(&ruta).expect("se puede crear el directorio de prueba");
            Self { ruta }
        }

        fn junto(&self, nombre: &str) -> PathBuf {
            self.ruta.join(nombre)
        }
    }

    impl Drop for DirectorioDePrueba {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.ruta);
        }
    }

    #[test]
    fn el_ciclo_de_escritura_y_lectura_conserva_los_bytes() {
        let directorio = DirectorioDePrueba::nuevo("ciclo");
        let destino = directorio.junto("inventario.inv");
        let contenido = b"EJE-INV1 contenido cualquiera".to_vec();

        escribir_atomico(&destino, &contenido).expect("la escritura debe completarse");
        assert_eq!(
            leer(&destino).expect("la lectura debe completarse"),
            contenido
        );
    }

    #[test]
    fn la_escritura_exitosa_no_deja_el_parcial() {
        let directorio = DirectorioDePrueba::nuevo("sin-parcial");
        let destino = directorio.junto("inventario.inv");

        escribir_atomico(&destino, b"contenido").expect("escritura");

        assert!(
            !ruta_temporal(&destino).exists(),
            "el temporal debe desaparecer al renombrarse"
        );
    }

    #[test]
    fn una_escritura_sobre_otra_sustituye_el_destino() {
        // `rename` debe sustituir el destino existente; si no lo hiciera, la
        // segunda escritura fallaria y el inventario quedaria congelado en la
        // primera version.
        let directorio = DirectorioDePrueba::nuevo("sustituye");
        let destino = directorio.junto("inventario.inv");

        escribir_atomico(&destino, b"primera").expect("primera escritura");
        escribir_atomico(&destino, b"segunda").expect("segunda escritura");

        assert_eq!(leer(&destino).unwrap(), b"segunda");
    }

    // --- Camino B: fallo de renombrado ---

    #[test]
    fn un_fallo_de_renombrado_limpia_el_parcial() {
        // El destino es un directorio existente: `rename` no puede sustituirlo.
        // Es la unica forma portable de provocar el fallo sin depender de
        // permisos, que se comportan distinto en Windows y en Unix.
        let directorio = DirectorioDePrueba::nuevo("rename-falla");
        let destino = directorio.junto("soy-un-directorio");
        std::fs::create_dir(&destino).expect("se crea el directorio");

        let resultado = escribir_atomico(&destino, b"contenido");

        assert!(
            resultado.is_err(),
            "renombrar sobre un directorio debe fallar"
        );
        assert!(
            !ruta_temporal(&destino).exists(),
            "la guarda debe haber retirado el temporal"
        );
    }

    // --- Camino A: salida temprana y panico ---

    #[test]
    fn la_guarda_limpia_ante_salida_temprana() {
        let directorio = DirectorioDePrueba::nuevo("salida-temprana");
        let temporal = directorio.junto("algo.parcial");
        std::fs::write(&temporal, b"a medias").expect("se crea el temporal");

        {
            let _limpiador = LimpiadorTemporal::nuevo(&temporal);
            // Salida del ambito sin desarmar, que es lo que ocurre en todo
            // camino de error de `escribir_atomico`.
        }

        assert!(!temporal.exists(), "la guarda debe retirar el temporal");
    }

    #[test]
    fn la_guarda_limpia_ante_panico_cuando_hay_desenrollado() {
        // En `dev` y en pruebas el panico desenrolla y el destructor corre. En
        // release este workspace usa `panic = "abort"` y NO corre: la prueba
        // documenta el mecanismo, no promete la garantia en el binario enviado.
        let directorio = DirectorioDePrueba::nuevo("panico");
        let temporal = directorio.junto("algo.parcial");
        std::fs::write(&temporal, b"a medias").expect("se crea el temporal");

        let ruta = temporal.clone();
        let resultado = std::panic::catch_unwind(move || {
            let _limpiador = LimpiadorTemporal::nuevo(&ruta);
            panic!("fallo simulado a mitad de escritura");
        });

        assert!(resultado.is_err(), "el panico debe capturarse");
        assert!(
            !temporal.exists(),
            "al desenrollar, la guarda debe retirar el temporal"
        );
    }

    #[test]
    fn desarmar_conserva_el_fichero() {
        let directorio = DirectorioDePrueba::nuevo("desarmar");
        let fichero = directorio.junto("conservado");
        std::fs::write(&fichero, b"contenido").expect("se crea");

        LimpiadorTemporal::nuevo(&fichero).desarmar();

        assert!(
            fichero.exists(),
            "desarmar debe impedir el borrado; si no, el renombrado exitoso \
             destruiria el inventario recien colocado"
        );
    }

    // --- Lectura acotada ---

    #[test]
    fn un_fichero_ausente_se_distingue_de_un_fallo() {
        let directorio = DirectorioDePrueba::nuevo("ausente");

        assert!(matches!(
            leer(&directorio.junto("no-existe.inv")),
            Err(ErrorDisco::NoExiste { .. })
        ));
    }

    #[test]
    fn un_fichero_que_excede_el_limite_se_rechaza() {
        // Un byte por encima del maximo. El limite lo impone el lector, no lo
        // que el fichero declare de si mismo.
        let directorio = DirectorioDePrueba::nuevo("excesivo");
        let destino = directorio.junto("grande.inv");
        std::fs::write(&destino, vec![0u8; LONGITUD_MAXIMA + 1]).expect("se escribe");

        assert!(matches!(leer(&destino), Err(ErrorDisco::Excesivo)));
    }

    #[test]
    fn un_fichero_en_el_limite_exacto_se_admite() {
        // La frontera se comprueba por los dos lados: un limite que rechace lo
        // legitimo es tan defecto como uno que admita lo excesivo.
        let directorio = DirectorioDePrueba::nuevo("limite");
        let destino = directorio.junto("justo.inv");
        std::fs::write(&destino, vec![0u8; LONGITUD_MAXIMA]).expect("se escribe");

        assert_eq!(
            leer(&destino).expect("debe admitirse").len(),
            LONGITUD_MAXIMA
        );
    }

    #[test]
    fn el_temporal_vive_junto_al_destino() {
        // Si el temporal cayera en otro directorio, el renombrado podria cruzar
        // sistema de ficheros y dejar de ser atomico.
        let destino = Path::new("/un/directorio/inventario.inv");
        let temporal = ruta_temporal(destino);

        assert_eq!(temporal.parent(), destino.parent());
        assert_eq!(
            temporal.file_name().unwrap(),
            std::ffi::OsStr::new("inventario.inv.parcial")
        );
    }
}
