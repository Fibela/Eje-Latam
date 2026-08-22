//! Guardian de implementaciones inconclusas y datos simulados.
//!
//! Implementa RPT-003 §9.4 y §9.5. Sustituye a `scripts/verificar-inconclusos.sh`
//! y hace innecesaria la version PowerShell.
//!
//! # Por que un crate y no un script
//!
//! Un guardian que nunca falla no sirve, y no hay forma de saber que esta vivo
//! sin provocarlo. Durante el desarrollo de esta plataforma dos guardianes
//! distintos pasaron en verde con la violacion presente:
//!
//! - la version PowerShell abandonaba el fichero al primer `#[cfg(test)]`;
//! - la configuracion de dependency-cruiser excluia `dist` del grafo, dejando
//!   inerte la regla que protegia la frontera open-core.
//!
//! Ambos se detectaron solo con pruebas negativas. Un crate del workspace se
//! prueba con `cargo test`; un script suelto, no.

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::exclusion::{codigo_sin_comentarios, lineas_de_prueba};

/// Sobre que texto se aplica una comprobacion.
///
/// RPT-076, PA-129. **No todas miran lo mismo, y esa es la correccion entera.**
///
/// Al arreglar los falsos positivos estuvo a punto de irse un guardian ciego:
/// quitar los comentarios para todas las comprobaciones habria dejado sin
/// detectar los marcadores `// TODO`, que viven **exactamente** ahi. Lo cazo la
/// prueba que ya existia.
///
/// Asi que cada comprobacion declara su ambito, y el que se equivoque de ambito
/// falla de forma visible en lugar de dejar de mirar en silencio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ambito {
    /// El codigo, con los comentarios sustituidos por espacios.
    ///
    /// Lo que se busca es una **instruccion**: un `todo!()`, un `mock`, una IP
    /// fija. Un comentario que los mencione los explica, no los comete.
    Codigo,
    /// La linea entera, comentarios incluidos.
    ///
    /// Para lo que **es** un comentario por naturaleza. Un `// TODO` no aparece
    /// en ningun otro sitio.
    LineaEntera,
}

/// Comprobacion aplicada a cada linea de codigo de produccion.
pub struct Comprobacion {
    /// Nombre mostrado en el informe.
    pub etiqueta: &'static str,
    /// Motivo por el que la coincidencia es inaceptable en produccion.
    pub motivo: &'static str,
    /// Expresion que la detecta.
    pub patron: Regex,
    /// Sobre que texto se aplica.
    pub ambito: Ambito,
}

/// Coincidencia encontrada en el codigo de produccion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hallazgo {
    /// Fichero donde se encontro.
    pub fichero: PathBuf,
    /// Numero de linea, comenzando en 1.
    pub linea: usize,
    /// Etiqueta de la comprobacion que se activo.
    pub etiqueta: &'static str,
    /// Contenido de la linea, recortado.
    pub contenido: String,
}

/// Error de configuracion del guardian.
#[derive(Debug)]
pub enum ErrorGuardian {
    /// Una expresion no compila. Solo puede ocurrir al editar este fichero.
    PatronInvalido(String),
    /// La ruta indicada no existe o no pudo leerse.
    RutaIlegible(String),
}

impl std::fmt::Display for ErrorGuardian {
    fn fmt(&self, formateador: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PatronInvalido(detalle) => {
                write!(formateador, "expresion regular invalida: {detalle}")
            }
            Self::RutaIlegible(detalle) => write!(formateador, "ruta ilegible: {detalle}"),
        }
    }
}

/// Construye el conjunto de comprobaciones.
///
/// # Errores
///
/// Devuelve [`ErrorGuardian::PatronInvalido`] si alguna expresion no compila.
pub fn comprobaciones() -> Result<Vec<Comprobacion>, ErrorGuardian> {
    let definiciones: [(&'static str, &'static str, &str, Ambito); 6] = [
        (
            "Implementaciones inconclusas",
            "Un macro de este tipo en ruta de produccion no devuelve un error controlado: aborta el proceso que vigila la red de una fabrica",
            r"(^|[^\w])(todo|unimplemented)!",
            Ambito::Codigo,
        ),
        (
            "Panicos con marcador pendiente",
            "Un panico deliberado con marcador es trabajo sin terminar disfrazado de control de errores",
            r#"panic!\(\s*"(TODO|FIXME|PENDIENTE|pendiente)"#,
            Ambito::Codigo,
        ),
        (
            "Marcadores de trabajo pendiente",
            "La rama Principal refleja solo estado funcional probado: elimine el comentario y abra un issue (RPT-003 §9.5)",
            r"//\s*(TODO|FIXME|XXX|HACK|PENDIENTE)",
            // Un marcador de trabajo pendiente ES un comentario: mirarlo en el
            // codigo seria no mirarlo nunca.
            Ambito::LineaEntera,
        ),
        (
            "Rutas sin implementar",
            "NoImplementado significa 'pendiente'. Para una operacion que no existe por diseno use NoSoportado (RPT-003 §9.5)",
            r"(NotImplemented|NoImplementado|501\s*Not\s*Implemented)",
            Ambito::Codigo,
        ),
        (
            "Datos simulados fuera de pruebas",
            "Un mock que devuelve exito valida el mock, no la operacion. Prohibido en la ruta de contencion (RPT-003 §9.2)",
            r"(^|[^\w])(mock|Mock|MOCK|dummy|Dummy|stub_|fake_|Fake)",
            Ambito::Codigo,
        ),
        (
            "Puntos finales y credenciales de ejemplo",
            "Ninguna IP ni puerto fijo en las bibliotecas base: use configuracion inyectada (RPT-003 §9.5)",
            r"(localhost:\d+|127\.0\.0\.1|example\.com|cambiame|changeme|contrasena123)",
            Ambito::Codigo,
        ),
    ];

    let mut resultado = Vec::with_capacity(definiciones.len());
    for (etiqueta, motivo, expresion, ambito) in definiciones {
        match Regex::new(expresion) {
            Ok(patron) => resultado.push(Comprobacion {
                etiqueta,
                motivo,
                patron,
                ambito,
            }),
            Err(error) => return Err(ErrorGuardian::PatronInvalido(error.to_string())),
        }
    }
    Ok(resultado)
}

/// Analiza un fuente Rust y devuelve las coincidencias en codigo de produccion.
///
/// Las lineas pertenecientes a bloques `#[cfg(test)]` se omiten: mocks y URL de
/// prueba son legitimos ahi (RPT-003 §9.5).
#[must_use]
pub fn analizar(fichero: &Path, fuente: &str, comprobaciones: &[Comprobacion]) -> Vec<Hallazgo> {
    let excluidas = lineas_de_prueba(fuente);

    // RPT-076, PA-129. Se busca en el CODIGO, no en la linea cruda. El guardian
    // acuso dos veces en dos dias a la prosa que explica el diseño —un comentario
    // sobre por que no hay punto de escucha por omision, y otro sobre por que un
    // estado degradado no es un mock— y la salida facil era reescribir la prosa.
    //
    // Las cadenas siguen mirandose: `bind("127.0.0.1:5514")` es codigo.
    let codigo = codigo_sin_comentarios(fuente);
    let mut hallazgos = Vec::new();

    for (indice, contenido) in fuente.lines().enumerate() {
        if excluidas.get(indice).copied().unwrap_or(false) {
            continue;
        }

        // Si la linea no aparece en `codigo` es que no existe, y una linea que no
        // existe no se registra como conforme: se mira la cruda, que es el
        // comportamiento anterior. No poder mirar no es haber mirado.
        let solo_codigo = codigo.get(indice).map_or(contenido, String::as_str);

        for comprobacion in comprobaciones {
            let buscado = match comprobacion.ambito {
                Ambito::Codigo => solo_codigo,
                Ambito::LineaEntera => contenido,
            };

            if comprobacion.patron.is_match(buscado) {
                hallazgos.push(Hallazgo {
                    fichero: fichero.to_path_buf(),
                    linea: indice + 1,
                    etiqueta: comprobacion.etiqueta,
                    contenido: contenido.trim().to_owned(),
                });
            }
        }
    }

    hallazgos
}

/// Recolecta los ficheros `.rs` bajo una ruta, omitiendo `target`.
///
/// # Errores
///
/// Devuelve [`ErrorGuardian::RutaIlegible`] si la ruta no puede recorrerse.
pub fn recolectar_fuentes(raiz: &Path) -> Result<Vec<PathBuf>, ErrorGuardian> {
    let mut ficheros = Vec::new();
    recolectar(raiz, &mut ficheros)?;
    ficheros.sort();
    Ok(ficheros)
}

fn recolectar(ruta: &Path, acumulador: &mut Vec<PathBuf>) -> Result<(), ErrorGuardian> {
    let entradas = fs::read_dir(ruta)
        .map_err(|error| ErrorGuardian::RutaIlegible(format!("{}: {error}", ruta.display())))?;

    for entrada in entradas {
        let entrada = entrada
            .map_err(|error| ErrorGuardian::RutaIlegible(format!("{}: {error}", ruta.display())))?;
        let camino = entrada.path();
        let nombre = entrada.file_name();
        let nombre = nombre.to_string_lossy();

        let es_rust = camino.extension().is_some_and(|ext| ext == "rs");

        if camino.is_dir() {
            if nombre == "target" || nombre == ".git" {
                continue;
            }
            recolectar(&camino, acumulador)?;
        } else if es_rust {
            acumulador.push(camino);
        }
    }
    Ok(())
}

/// Ejecuta el guardian sobre una ruta.
///
/// # Errores
///
/// Propaga errores de compilacion de patrones o de lectura de la ruta.
pub fn verificar(raiz: &Path) -> Result<Vec<Hallazgo>, ErrorGuardian> {
    let comprobaciones = comprobaciones()?;
    let fuentes = recolectar_fuentes(raiz)?;
    let mut hallazgos = Vec::new();

    for fichero in fuentes {
        let fuente = fs::read_to_string(&fichero).map_err(|error| {
            ErrorGuardian::RutaIlegible(format!("{}: {error}", fichero.display()))
        })?;
        hallazgos.extend(analizar(&fichero, &fuente, &comprobaciones));
    }

    Ok(hallazgos)
}
