//! Descarga y anclaje de los vectores de prueba de `motor-pqc`.
//!
//! RPT-005 §4.3, PA-17.
//!
//! # Por qué se anclan por resumen
//!
//! `.gitleaks.toml` exonera `crates/motor-pqc/tests/vectores/` del escaneo de
//! secretos, porque los vectores contienen material de clave por diseño. Exonerar
//! un directorio crea un punto ciego permanente: quien deposite ahí una clave
//! real no será detectado.
//!
//! El anclaje cierra ese hueco. `FUENTES.lock` registra el resumen SHA-256 de
//! cada fichero y se versiona en el repositorio. Cualquier alteración del
//! contenido —incluida la inserción de un secreto real— cambia el resumen y falla
//! la verificación antes de que gitleaks entre en juego.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

/// Directorio de vectores, relativo a la raíz del workspace.
const DIRECTORIO: &str = "crates/motor-pqc/tests/vectores";

/// Tamaño máximo admitido por fichero.
///
/// Los `internalProjection.json` de ACVP cubren todos los conjuntos de
/// parámetros del algoritmo y pueden ser muy grandes. Vendorizar cientos de
/// megabytes en el repositorio no es aceptable: por encima de este límite hay
/// que podar a los conjuntos que realmente usamos antes de versionar.
const TAMANO_MAXIMO_BYTES: u64 = 64 * 1024 * 1024;

/// Fichero declarado en `FUENTES.toml`.
struct Fuente {
    nombre: String,
    url: String,
}

/// Error de la descarga o del anclaje.
#[derive(Debug)]
pub enum ErrorVectores {
    /// No se pudo leer o escribir un fichero.
    Ficheros(String),
    /// La descarga falló.
    Descarga(String),
    /// El resumen no coincide con el anclaje.
    AnclajeRoto {
        /// Fichero afectado.
        nombre: String,
        /// Resumen esperado según el anclaje.
        esperado: String,
        /// Resumen calculado sobre el contenido descargado.
        obtenido: String,
    },
    /// El fichero descargado excede el límite admitido.
    DemasiadoGrande {
        /// Fichero afectado.
        nombre: String,
        /// Tamaño obtenido en bytes.
        bytes: u64,
    },
}

impl std::fmt::Display for ErrorVectores {
    fn fmt(&self, formateador: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ficheros(detalle) => write!(formateador, "error de ficheros: {detalle}"),
            Self::Descarga(detalle) => write!(formateador, "error de descarga: {detalle}"),
            Self::AnclajeRoto {
                nombre,
                esperado,
                obtenido,
            } => write!(
                formateador,
                "el resumen de '{nombre}' no coincide con el anclaje.\n  \
                 esperado: {esperado}\n  obtenido: {obtenido}\n  \
                 El contenido cambio respecto a lo versionado. NO se acepta sin revision \
                 manual: este anclaje es lo que hace segura la exoneracion de gitleaks.\n  \
                 Si el cambio es deliberado —una correccion de FUENTES.toml, por ejemplo—: \
                 cargo xtask vectores --actualizar"
            ),
            Self::DemasiadoGrande { nombre, bytes } => write!(
                formateador,
                "'{nombre}' ocupa {} MB y supera el limite de {} MB.\n  \
                 Vendorizar un fichero de ese tamano en el repositorio no es aceptable.\n  \
                 Pode el conjunto a los parametros que realmente se usan (ML-KEM-768 y \
                 ML-DSA-65) antes de versionarlo.",
                bytes / (1024 * 1024),
                TAMANO_MAXIMO_BYTES / (1024 * 1024)
            ),
        }
    }
}

/// Extrae las fuentes declaradas en `FUENTES.toml`.
///
/// Analizador deliberadamente simple: el formato lo controla este proyecto y
/// añadir una dependencia TOML a la herramienta de desarrollo no se justifica.
fn leer_fuentes(contenido: &str) -> Vec<Fuente> {
    let mut fuentes = Vec::new();
    let mut nombre: Option<String> = None;

    for linea in contenido.lines() {
        let limpia = linea.trim();
        if limpia.starts_with('#') {
            continue;
        }
        if let Some(valor) = entrecomillado(limpia, "nombre = ") {
            nombre = Some(valor);
        } else if let Some(url) = entrecomillado(limpia, "url = ") {
            if let Some(actual) = nombre.take() {
                fuentes.push(Fuente {
                    nombre: actual,
                    url,
                });
            }
        }
    }

    fuentes
}

/// Extrae el valor entrecomillado de una línea `clave = "valor"`.
fn entrecomillado(linea: &str, prefijo: &str) -> Option<String> {
    let resto = linea.strip_prefix(prefijo)?;
    let sin_inicio = resto.strip_prefix('"')?;
    let fin = sin_inicio.find('"')?;
    Some(sin_inicio[..fin].to_owned())
}

/// Resumen SHA-256 en hexadecimal.
///
/// Compartida con [`crate::empaquetar`] (RPT-073, PA-126): el manifiesto de
/// integridad del artefacto necesita exactamente esto. Vive aqui por haber
/// llegado antes, no por pertenecer a los vectores; duplicarla habria sido la
/// septima lista escrita a mano de la semana, y ademas una en la que dos copias
/// podrian discrepar en el formato del hexadecimal sin que nada lo notara.
pub(crate) fn resumir(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digestion = Sha256::digest(bytes);
    digestion
        .iter()
        .fold(String::with_capacity(64), |mut texto, byte| {
            let _ = write!(texto, "{byte:02x}");
            texto
        })
}

/// Descarga un fichero con `curl`.
///
/// Se usa `curl` en lugar de una biblioteca HTTP para no añadir una dependencia
/// de red a la herramienta de desarrollo. Está disponible en Linux, macOS y en
/// Windows 10 en adelante.
fn descargar(url: &str, destino: &Path) -> Result<Vec<u8>, ErrorVectores> {
    let salida = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--location",
            "--max-time",
            "120",
            "--output",
        ])
        .arg(destino)
        .arg(url)
        .output()
        .map_err(|error| ErrorVectores::Descarga(format!("no se pudo ejecutar curl: {error}")))?;

    if !salida.status.success() {
        return Err(ErrorVectores::Descarga(format!(
            "{url}: {}",
            String::from_utf8_lossy(&salida.stderr).trim()
        )));
    }

    fs::read(destino)
        .map_err(|error| ErrorVectores::Ficheros(format!("{}: {error}", destino.display())))
}

/// Lee el anclaje existente, si lo hay.
fn leer_anclaje(ruta: &Path) -> Vec<(String, String)> {
    let Ok(contenido) = fs::read_to_string(ruta) else {
        return Vec::new();
    };

    contenido
        .lines()
        .filter_map(|linea| {
            let (nombre, resumen) = linea.split_once("  ")?;
            Some((nombre.trim().to_owned(), resumen.trim().to_owned()))
        })
        .collect()
}

/// Descarga los vectores y verifica o crea el anclaje.
///
/// # Errores
///
/// Devuelve error si la descarga falla o si el resumen de un fichero no coincide
/// con el anclaje versionado.
pub fn sincronizar(raiz: &Path, actualizar: bool) -> Result<(), ErrorVectores> {
    let directorio = raiz.join(DIRECTORIO);
    let declaracion = directorio.join("FUENTES.toml");
    let anclaje = directorio.join("FUENTES.lock");

    let contenido = fs::read_to_string(&declaracion)
        .map_err(|error| ErrorVectores::Ficheros(format!("{}: {error}", declaracion.display())))?;
    let fuentes = leer_fuentes(&contenido);

    if fuentes.is_empty() {
        return Err(ErrorVectores::Ficheros(
            "FUENTES.toml no declara ninguna fuente".to_owned(),
        ));
    }

    let previo = if actualizar {
        Vec::new()
    } else {
        leer_anclaje(&anclaje)
    };
    let primera_vez = previo.is_empty();

    if actualizar {
        println!("Modo --actualizar: se REESCRIBE el anclaje sin verificar el previo.");
        println!("Revise el diff de FUENTES.lock antes de versionarlo.\n");
    } else if primera_vez {
        println!("No hay anclaje previo: se creara FUENTES.lock.");
        println!("Revise el contenido descargado antes de versionarlo.\n");
    } else {
        println!("Verificando contra el anclaje versionado.\n");
    }

    let mut nuevo: Vec<(String, String)> = Vec::with_capacity(fuentes.len());

    for fuente in &fuentes {
        let destino: PathBuf = directorio.join(&fuente.nombre);
        print!("  {} ... ", fuente.nombre);

        let bytes = descargar(&fuente.url, &destino)?;

        if bytes.len() as u64 > TAMANO_MAXIMO_BYTES {
            println!("DEMASIADO GRANDE");
            let _ = fs::remove_file(&destino);
            return Err(ErrorVectores::DemasiadoGrande {
                nombre: fuente.nombre.clone(),
                bytes: bytes.len() as u64,
            });
        }

        let resumen = resumir(&bytes);

        if let Some((_, esperado)) = previo.iter().find(|(nombre, _)| *nombre == fuente.nombre) {
            if esperado != &resumen {
                println!("ANCLAJE ROTO");
                return Err(ErrorVectores::AnclajeRoto {
                    nombre: fuente.nombre.clone(),
                    esperado: esperado.clone(),
                    obtenido: resumen,
                });
            }
            println!("verificado ({} bytes)", bytes.len());
        } else {
            println!("anclado ({} bytes)", bytes.len());
        }

        nuevo.push((fuente.nombre.clone(), resumen));
    }

    use std::fmt::Write as _;
    let cuerpo = nuevo
        .iter()
        .fold(String::new(), |mut texto, (nombre, resumen)| {
            let _ = writeln!(texto, "{nombre}  {resumen}");
            texto
        });

    fs::write(&anclaje, cuerpo)
        .map_err(|error| ErrorVectores::Ficheros(format!("{}: {error}", anclaje.display())))?;

    println!("\n{} ficheros en {}", nuevo.len(), directorio.display());
    if primera_vez {
        println!("Versione FUENTES.lock: es lo que ancla el contenido y hace segura");
        println!("la exoneracion de gitleaks sobre este directorio.");
    }

    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_leen_las_fuentes_declaradas() {
        let contenido = r#"
# comentario
[[acvp]]
nombre = "uno.json"
url = "https://ejemplo/uno.json"

[[wycheproof]]
nombre = "dos.json"
url = "https://ejemplo/dos.json"
motivo = "regresion"
"#;
        let fuentes = leer_fuentes(contenido);
        assert_eq!(fuentes.len(), 2);
        assert_eq!(fuentes[0].nombre, "uno.json");
        assert_eq!(fuentes[1].url, "https://ejemplo/dos.json");
    }

    #[test]
    fn una_entrada_sin_url_no_produce_fuente() {
        let fuentes = leer_fuentes("nombre = \"huerfano.json\"\n");
        assert!(fuentes.is_empty());
    }

    #[test]
    fn el_resumen_es_estable() {
        assert_eq!(
            resumir(b"eje-latam"),
            resumir(b"eje-latam"),
            "el resumen debe ser determinista"
        );
        assert_ne!(resumir(b"a"), resumir(b"b"));
        assert_eq!(resumir(b"").len(), 64);
    }

    #[test]
    fn el_anclaje_ausente_se_lee_como_vacio() {
        assert!(leer_anclaje(Path::new("/no/existe/FUENTES.lock")).is_empty());
    }
}
