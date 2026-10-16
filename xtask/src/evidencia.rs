//! Ninguna captura de `docs/evidencia/` puede repetir el contenido de otra.
//!
//! # Por que existe esta barrera
//!
//! PA-159. El 19 de septiembre de 2026 se archivo en `docs/evidencia/` una
//! captura de pantalla con el nombre y la fecha de ese dia. Era la captura del
//! **8 de septiembre**: mismo SHA-256, byte a byte, que
//! `RPT-095_Sin-Clave-De-Recuperacion_2026-09-08.png`. La orden que la copio fue
//! un `cp` de un fichero del directorio personal que se creia reciente y no lo
//! era.
//!
//! No la caza ninguna prueba del producto, ni `xtask verificar`, ni la revision
//! de un reporte: el fichero existe, se abre, y ensena un tablero de VIS-04
//! perfectamente plausible. Lo unico que delata que no es de ese dia es que ya
//! estaba ahi con otro nombre.
//!
//! # Por que importa mas de lo que parece
//!
//! `docs/evidencia/` es lo que este proyecto ofrece cuando alguien pregunta «¿y
//! como se que eso que dice el reporte pasó de verdad?». Es la respuesta a la
//! pregunta del auditor. Una captura repetida no es un fichero de mas: es una
//! afirmacion falsa dentro del unico sitio del repositorio que existe para
//! sostener afirmaciones.
//!
//! Y el dano no se ve el dia que se comete. Se ve dentro de seis meses, cuando
//! alguien abra un fichero fechado en septiembre, cuente dieciseis condiciones
//! en una imagen que el reporte dice que tiene veintiuna, y **no tenga forma de
//! saber cual de las dos cosas es mentira**. A partir de ahi, todo el directorio
//! vale menos.
//!
//! # La regla, y por que es exactamente esta
//!
//! Una captura que se repite es, por definicion, una captura que no se tomo. Dos
//! pantallas fotografiadas en momentos distintos difieren siempre —el reloj del
//! escritorio, el contador de vueltas, un pixel del cursor—, asi que dos ficheros
//! identicos al byte no pueden ser dos observaciones. Son una observacion y una
//! copia.
//!
//! No se comprueba que los nombres sean distintos, ni las fechas, ni los
//! tamanos: todo eso lo puede satisfacer una copia. Se compara el contenido.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::vectores::resumir;

/// Directorio que se audita, relativo a la raiz del repositorio.
pub const DIRECTORIO: &str = "docs/evidencia";

/// Fallos al auditar el directorio de evidencia.
///
/// `Display` se escribe a mano y no con `thiserror`, como el resto de `xtask`:
/// esta herramienta no se distribuye y no se le traen cajas por comodidad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorEvidencia {
    /// No se pudo recorrer el directorio.
    Directorio {
        /// Ruta implicada.
        ruta: String,
        /// Causa.
        detalle: String,
    },

    /// No se pudo leer un fichero del directorio.
    ///
    /// **No se ignora en silencio.** Un fichero ilegible es un fichero cuyo
    /// contenido no se ha comparado con nada, y dar el directorio por limpio sin
    /// haberlo mirado entero seria la misma clase de afirmacion sin respaldo que
    /// esta barrera existe para impedir.
    Fichero {
        /// Ruta implicada.
        ruta: String,
        /// Causa.
        detalle: String,
    },
}

impl std::fmt::Display for ErrorEvidencia {
    fn fmt(&self, formateador: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Directorio { ruta, detalle } => {
                write!(formateador, "no se pudo recorrer {ruta}: {detalle}")
            }
            Self::Fichero { ruta, detalle } => {
                write!(formateador, "no se pudo leer {ruta}: {detalle}")
            }
        }
    }
}

/// Un contenido que aparece bajo mas de un nombre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repetido {
    /// SHA-256 del contenido, en hexadecimal.
    pub huella: String,
    /// Nombres bajo los que aparece, en orden alfabetico.
    ///
    /// Se conservan **todos** y no solo el par: si una captura se copio tres
    /// veces, quien lo arregle tiene que ver las tres.
    pub nombres: Vec<String>,
}

/// Busca contenidos repetidos en el directorio de evidencia.
///
/// Devuelve la lista vacia cuando cada fichero es unico. Un directorio que no
/// existe tampoco es un fallo: un repositorio recien clonado sin evidencia
/// todavia no tiene nada que repetir, y hacer fallar la barrera ahi la volveria
/// molesta sin volverla util.
///
/// # Errores
///
/// [`ErrorEvidencia::Directorio`] si el directorio existe y no se puede
/// recorrer, y [`ErrorEvidencia::Fichero`] si alguna entrada no se puede leer.
pub fn repetidos(raiz: &Path) -> Result<Vec<Repetido>, ErrorEvidencia> {
    let directorio = raiz.join(DIRECTORIO);

    if !directorio.is_dir() {
        return Ok(Vec::new());
    }

    let entradas = std::fs::read_dir(&directorio).map_err(|error| ErrorEvidencia::Directorio {
        ruta: directorio.display().to_string(),
        detalle: error.to_string(),
    })?;

    // `BTreeMap` y no `HashMap`: el orden de la salida tiene que ser el mismo en
    // cualquier maquina, o dos ejecuciones de la misma barrera dirian lo mismo
    // en distinto orden y nadie sabria si cambio algo.
    let mut por_huella: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for entrada in entradas {
        let ruta: PathBuf = entrada
            .map_err(|error| ErrorEvidencia::Directorio {
                ruta: directorio.display().to_string(),
                detalle: error.to_string(),
            })?
            .path();

        // Los subdirectorios se saltan sin recorrerlos. Hoy no hay ninguno; si
        // algun dia los hay, esta linea es lo que habra que decidir, y por eso
        // esta escrita en lugar de dejar que `read` falle.
        if !ruta.is_file() {
            continue;
        }

        let bytes = std::fs::read(&ruta).map_err(|error| ErrorEvidencia::Fichero {
            ruta: ruta.display().to_string(),
            detalle: error.to_string(),
        })?;

        let nombre = ruta.file_name().map_or_else(
            || ruta.display().to_string(),
            |n| n.to_string_lossy().into(),
        );

        por_huella.entry(resumir(&bytes)).or_default().push(nombre);
    }

    Ok(por_huella
        .into_iter()
        .filter(|(_, nombres)| nombres.len() > 1)
        .map(|(huella, mut nombres)| {
            nombres.sort();
            Repetido { huella, nombres }
        })
        .collect())
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::{DIRECTORIO, repetidos};

    /// Raiz de prueba con su directorio de evidencia ya creado.
    fn raiz(nombre: &str) -> std::path::PathBuf {
        let ruta = std::env::temp_dir().join(format!(
            "eje-pa159-{}-{nombre}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&ruta);
        std::fs::create_dir_all(ruta.join(DIRECTORIO)).expect("temporal");
        ruta
    }

    fn escribir(raiz: &std::path::Path, nombre: &str, contenido: &[u8]) {
        std::fs::write(raiz.join(DIRECTORIO).join(nombre), contenido).expect("escribe");
    }

    /// El defecto exacto del 19 de septiembre de 2026.
    #[test]
    fn dos_capturas_identicas_se_denuncian() {
        let raiz = raiz("identicas");
        escribir(&raiz, "RPT-095_algo_2026-09-08.png", b"una pantalla");
        escribir(&raiz, "RPT-103_otra-cosa_2026-09-19.png", b"una pantalla");

        let hallados = repetidos(&raiz).expect("recorre");

        assert_eq!(hallados.len(), 1, "hay un contenido repetido y solo uno");
        assert_eq!(
            hallados[0].nombres,
            vec![
                "RPT-095_algo_2026-09-08.png".to_owned(),
                "RPT-103_otra-cosa_2026-09-19.png".to_owned()
            ],
            "se nombran los dos: quien lo arregle necesita saber cual copio a cual"
        );

        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Anti-vacuidad, y no es decorativa.
    ///
    /// Sin esto, una version que denunciara **todo** fichero pasaria la prueba de
    /// arriba. Y una barrera que se pone roja siempre se desactiva en una semana,
    /// que es peor que no tenerla: deja el hueco y ademas ensena a ignorar avisos
    /// (PA-45).
    #[test]
    fn capturas_distintas_no_se_denuncian() {
        let raiz = raiz("distintas");
        escribir(&raiz, "una.png", b"pantalla del lunes");
        escribir(&raiz, "otra.png", b"pantalla del martes");
        escribir(&raiz, "tercera.png", b"pantalla del miercoles");

        assert!(repetidos(&raiz).expect("recorre").is_empty());

        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Tres copias del mismo contenido salen las tres, no dos.
    ///
    /// Quien arregla esto tiene que ver el alcance entero. Informar de un par
    /// cuando hay tres deja una copia dentro y la barrera volveria a saltar
    /// manana, con el arreglo ya dado por hecho.
    #[test]
    fn tres_copias_se_nombran_las_tres() {
        let raiz = raiz("tres");
        escribir(&raiz, "a.png", b"misma pantalla");
        escribir(&raiz, "b.png", b"misma pantalla");
        escribir(&raiz, "c.png", b"misma pantalla");

        let hallados = repetidos(&raiz).expect("recorre");

        assert_eq!(hallados.len(), 1);
        assert_eq!(hallados[0].nombres.len(), 3);

        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Un repositorio sin evidencia todavia no es un fallo.
    #[test]
    fn sin_directorio_no_hay_nada_que_denunciar() {
        let ruta = std::env::temp_dir().join(format!("eje-pa159-vacio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ruta);
        std::fs::create_dir_all(&ruta).expect("temporal");

        assert!(repetidos(&ruta).expect("no falla").is_empty());

        let _ = std::fs::remove_dir_all(&ruta);
    }
}
