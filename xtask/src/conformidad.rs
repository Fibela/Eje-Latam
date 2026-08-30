//! Atestado de conformidad poscuantica. RPT-005 §9.3, PA-121.
//!
//! # Que problema resuelve, que no es el que parece
//!
//! Las tres suites de `motor-pqc` —ACVP, Wycheproof y la diferencial contra
//! libcrux— ya existen y ya corren. Lo que no existia es una forma de saber
//! **contra que** corrieron.
//!
//! RPT-005 §9.3 descarto dos mecanismos y dijo por que. Una constante `true` no
//! prueba nada: traslada la afirmacion de sitio. Una variable de entorno o una
//! bandera de compilacion es falsificable, y si va firmada la clave o esta en el
//! repositorio —y entonces no es secreto— o solo en la CI, lo que la convierte
//! en dependencia de PA-14.
//!
//! **El defecto de fondo de las dos es el mismo: tratan la conformidad como una
//! propiedad del evento de compilacion, cuando es una propiedad de las
//! entradas.** Si manana se sube `ml-dsa` en `Cargo.lock`, un binario compilado
//! hoy seguiria portando su bandera de conforme aunque las suites no se hayan
//! ejecutado nunca contra la version nueva. Un atestado del tipo «la CI paso»
//! caduca en silencio, que es el modo de fallo que este proyecto persigue desde
//! RPT-003 §9.5.
//!
//! De ahi que la huella se calcule sobre las entradas: versiones exactas,
//! resumen de los vectores y canal del toolchain. Si cualquiera se mueve sin
//! volver a ejecutar la conformidad, las huellas divergen y la CI se pone roja
//! sola. **El atestado se autoinvalida.** Es el anclaje Merkle de los vectores
//! aplicado al arbol de dependencias, y no necesita `build.rs`, ni variable de
//! entorno, ni clave.
//!
//! # Lo que este mecanismo NO prueba
//!
//! Ata **que** se probo, no **que** se probo. Alguien puede componer la huella
//! correcta sin haber ejecutado ninguna suite: basta escribir el fichero.
//!
//! Cerrarlo exige que la CI sea el unico productor de confianza, con una clave
//! que solo ella posea, y eso es el alcance de PA-14. Queda escrito aqui —y no
//! solo en RPT-005 §9.4— para que nadie lea `CONFORMIDAD.lock` dentro de dos
//! anos y le atribuya una garantia que no da.
//!
//! # Por que el conjunto de dependencias no se escribe a mano
//!
//! RPT-005 nombraba «ml-kem, ml-dsa y libcrux-*». Una lista literal aqui seria
//! el octavo indice escrito a mano de la serie (PA-73, PA-108, PA-119…), y esta
//! familia no falla ruidosamente: se queda corta y sigue pareciendo el total. El
//! dia que entrase una dependencia poscuantica nueva quedaria fuera de la huella
//! sin que nada lo dijera.
//!
//! Se derivan del **grafo ya resuelto** de `Cargo.lock`, asi que una dependencia
//! nueva entra en el atestado sola.
//!
//! # Y por que del grafo resuelto y no del texto del manifiesto
//!
//! La primera version leia `[dependencies]` y `[dev-dependencies]` del
//! `Cargo.toml` de `motor-pqc` y buscaba cada nombre en `Cargo.lock`. Emitio un
//! atestado con **17 entradas para 14 dependencias**: `Cargo.lock` contiene
//! varias versiones mayores del mismo paquete conviviendo —`rand_core` estaba
//! tres veces— y se llevaba todas.
//!
//! El fichero afirmaba entonces que `motor-pqc` se habia probado contra
//! `rand_core 0.6.4`, que es la que arrastra otro crate del arbol. **Un atestado
//! que dice mas de lo que sabe es peor que no tenerlo**, porque parece preciso.
//!
//! `Cargo.lock` ya tiene la respuesta buena: la lista `dependencies` del propio
//! paquete, que incluye las de desarrollo y **desambigua con la version exacta
//! solo donde hace falta** (`"rand 0.9.5"`). Se lee de ahi.

use std::path::Path;

use crate::vectores::resumir;

/// Nombre del fichero de atestado, en la raiz del repositorio.
pub const FICHERO: &str = "CONFORMIDAD.lock";

/// Suites que se exigen en verde antes de emitir nada.
///
/// Son las tres de RPT-005 §9.3. Estan aqui y no derivadas del disco porque
/// `motor-pqc/tests/vectores.rs` tambien es un fichero de pruebas y **no** es una
/// suite de conformidad: comprueba que los vectores esten, no que el motor los
/// pase. Derivarlas del directorio meteria esa y el atestado diria una cosa
/// distinta de la que dice RPT-005.
pub const SUITES: &[&str] = &["acvp", "wycheproof", "diferencial"];

/// Fallos al componer o comprobar el atestado.
#[derive(Debug)]
pub enum ErrorConformidad {
    /// No se pudo leer un fichero de entrada.
    Lectura {
        /// Ruta implicada.
        ruta: String,
        /// Causa.
        detalle: String,
    },

    /// Una dependencia declarada no aparece en `Cargo.lock`.
    ///
    /// No se omite ni se anota como «desconocida»: un atestado al que le falta
    /// una entrada es peor que no tener atestado, porque parece completo.
    SinVersion(String),

    /// `Cargo.lock` no tiene entrada para el paquete que se atestigua.
    SinPaquete(String),

    /// Un nombre resuelve a **varias** versiones y `Cargo.lock` no desambiguo.
    ///
    /// # El tercer estado que faltaba
    ///
    /// La primera version tenia dos: «resuelve» y «no esta». La ambiguedad caia
    /// en el primero y se llevaba todas las versiones al fichero, que es como se
    /// colaron las tres de `rand_core`. RPT-006 §4 otra vez, ahora en el lector
    /// de dependencias: no se elige una, se para.
    Ambiguo {
        /// Nombre implicado.
        nombre: String,
        /// Versiones que resuelven a ese nombre.
        versiones: Vec<String>,
    },

    /// `rust-toolchain.toml` no declara canal.
    SinCanal,

    /// El atestado en disco no coincide con lo que hay en el arbol.
    ///
    /// Solo la construye [`comprobar`], que solo llama la barrera. Sin
    /// `cfg(test)` el compilador la declara muerta y `-D warnings` pone roja la
    /// CI por algo que no lo esta.
    #[cfg(test)]
    Divergencia {
        /// Huella registrada en el fichero.
        registrada: String,
        /// Huella recalculada ahora.
        recalculada: String,
        /// En que se diferencian, en texto legible.
        detalle: String,
    },
}

impl std::fmt::Display for ErrorConformidad {
    fn fmt(&self, formateador: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lectura { ruta, detalle } => {
                write!(formateador, "no se pudo leer {ruta}: {detalle}")
            }

            Self::SinVersion(nombre) => write!(
                formateador,
                "'{nombre}' esta declarada en motor-pqc pero no aparece en Cargo.lock"
            ),

            Self::SinPaquete(nombre) => write!(
                formateador,
                "'{nombre}' no aparece en Cargo.lock: ¿se renombro el crate?"
            ),

            Self::Ambiguo { nombre, versiones } => write!(
                formateador,
                "'{nombre}' resuelve a {} versiones ({}) y Cargo.lock no dice cual.\n  \
                 No se elige una: un atestado con la version equivocada afirma que se \n  \
                 probo contra algo que no se probo.",
                versiones.len(),
                versiones.join(", ")
            ),

            Self::SinCanal => write!(formateador, "rust-toolchain.toml no declara 'channel'"),

            // La misma condicion que la variante. Sin esto, una compilacion sin
            // pruebas intenta nombrar algo que no existe y el compilador lo
            // reporta como «tipo asociado no encontrado», que no se parece en
            // nada a la causa.
            #[cfg(test)]
            Self::Divergencia {
                registrada,
                recalculada,
                detalle,
            } => write!(
                formateador,
                "el atestado NO describe este arbol.\n  \
                 registrada  : {registrada}\n  \
                 recalculada : {recalculada}\n\
                 {detalle}\n  \
                 Alguien movio una entrada sin volver a ejecutar 'cargo xtask conformidad'.\n  \
                 Eso es lo que este fichero existe para detectar: NO se actualiza a mano."
            ),
        }
    }
}

/// Lo que se atestigua, ya resuelto contra el arbol.
#[derive(Debug, PartialEq, Eq)]
pub struct Atestado {
    /// Dependencias de `motor-pqc` con su version exacta, ordenadas por nombre.
    pub paquetes: Vec<(String, String)>,
    /// Resumen SHA-256 del contenido de `FUENTES.lock`.
    pub resumen_fuentes: String,
    /// Canal declarado en `rust-toolchain.toml`.
    pub canal: String,
    /// SHA-256 sobre todo lo anterior.
    pub huella: String,
}

/// Lee un fichero, con la ruta en el error.
fn leer(ruta: &Path) -> Result<String, ErrorConformidad> {
    std::fs::read_to_string(ruta).map_err(|error| ErrorConformidad::Lectura {
        ruta: ruta.display().to_string(),
        detalle: error.to_string(),
    })
}

/// Paquete cuya conformidad se atestigua.
pub const PAQUETE: &str = "motor-pqc";

/// Bloques `[[package]]` de un `Cargo.lock`.
fn bloques(bloqueo: &str) -> impl Iterator<Item = &str> {
    bloqueo.split("[[package]]").skip(1)
}

/// Nombre de un bloque `[[package]]`.
fn nombre_del_bloque(bloque: &str) -> Option<String> {
    bloque
        .lines()
        .find_map(|linea| entrecomillado(linea.trim(), "name = "))
}

/// Version de un bloque `[[package]]`.
fn version_del_bloque(bloque: &str) -> Option<String> {
    bloque
        .lines()
        .find_map(|linea| entrecomillado(linea.trim(), "version = "))
}

/// Dependencias resueltas de `paquete`, tal como las lista `Cargo.lock`.
///
/// Devuelve las entradas **en bruto**: `"aes-gcm"` o `"rand 0.9.5"`. Cargo escribe
/// la version solo cuando el nombre no basta, y esa es exactamente la informacion
/// que hace falta para no llevarse tres `rand_core` al atestado.
///
/// La lista incluye las dependencias de desarrollo, que aqui no es un detalle:
/// `libcrux-ml-kem` y `libcrux-ml-dsa` son la otra mitad de la suite diferencial.
/// Atestiguar sin ellas diria contra que se comparo sin decir con que.
///
/// # Errores
///
/// [`ErrorConformidad::SinPaquete`] si el paquete no esta en el bloqueo.
pub fn dependencias_resueltas(
    bloqueo: &str,
    paquete: &str,
) -> Result<Vec<String>, ErrorConformidad> {
    let bloque = bloques(bloqueo)
        .find(|bloque| nombre_del_bloque(bloque).as_deref() == Some(paquete))
        .ok_or_else(|| ErrorConformidad::SinPaquete(paquete.to_owned()))?;

    let Some((_, tras_abrir)) = bloque.split_once("dependencies = [") else {
        // Un paquete sin dependencias es una lista vacia, no un fallo.
        return Ok(Vec::new());
    };

    let Some((dentro, _)) = tras_abrir.split_once(']') else {
        return Ok(Vec::new());
    };

    Ok(dentro
        .lines()
        .filter_map(|linea| {
            let limpia = linea.trim().trim_end_matches(',').trim();
            let sin_abrir = limpia.strip_prefix('"')?;
            let (entrada, _) = sin_abrir.split_once('"')?;
            Some(entrada.to_owned())
        })
        .collect())
}

/// Resuelve cada entrada a un par nombre-version exacto.
///
/// # Errores
///
/// [`ErrorConformidad::SinVersion`] si el nombre no esta en el bloqueo, y
/// [`ErrorConformidad::Ambiguo`] si resuelve a varias y la entrada no dice cual.
pub fn versiones_de(
    bloqueo: &str,
    entradas: &[String],
) -> Result<Vec<(String, String)>, ErrorConformidad> {
    let catalogo: Vec<(String, String)> = bloques(bloqueo)
        .filter_map(|bloque| Some((nombre_del_bloque(bloque)?, version_del_bloque(bloque)?)))
        .collect();

    let mut resueltas: Vec<(String, String)> = Vec::new();

    for entrada in entradas {
        // Cargo escribe `"rand 0.9.5"` solo cuando hace falta desambiguar.
        let (nombre, pedida) = match entrada.split_once(' ') {
            Some((nombre, version)) => (nombre, Some(version)),
            None => (entrada.as_str(), None),
        };

        let candidatas: Vec<&String> = catalogo
            .iter()
            .filter(|(cada, _)| cada == nombre)
            .map(|(_, version)| version)
            .collect();

        let version = match (pedida, candidatas.as_slice()) {
            (_, []) => return Err(ErrorConformidad::SinVersion(nombre.to_owned())),

            (Some(pedida), _) => {
                if !candidatas.iter().any(|cada| cada.as_str() == pedida) {
                    return Err(ErrorConformidad::SinVersion(entrada.clone()));
                }
                pedida.to_owned()
            }

            (None, [unica]) => (*unica).clone(),

            // Sin version pedida y con varias candidatas no se elige: se para.
            (None, varias) => {
                return Err(ErrorConformidad::Ambiguo {
                    nombre: nombre.to_owned(),
                    versiones: varias.iter().map(|cada| (*cada).clone()).collect(),
                });
            }
        };

        resueltas.push((nombre.to_owned(), version));
    }

    resueltas.sort();
    resueltas.dedup();
    Ok(resueltas)
}

/// Valor entrecomillado tras un prefijo, si la linea lo lleva.
fn entrecomillado(linea: &str, prefijo: &str) -> Option<String> {
    let resto = linea.strip_prefix(prefijo)?.trim();
    let sin_abrir = resto.strip_prefix('"')?;
    let (valor, _) = sin_abrir.split_once('"')?;
    Some(valor.to_owned())
}

/// Canal declarado en `rust-toolchain.toml`.
///
/// # Por que del fichero y no de `rustc --version`
///
/// El recalculo tiene que dar lo mismo en la maquina de cualquiera. Si el canal
/// saliera del binario en ejecucion, la huella cambiaria con la version instalada
/// y la barrera se pondria roja por motivos que no son el que vigila. El fichero
/// pinado es parte del arbol, que es exactamente lo que se atestigua.
fn canal_de(toolchain: &str) -> Option<String> {
    toolchain
        .lines()
        .find_map(|linea| entrecomillado(linea.trim(), "channel = "))
}

/// Texto canonico sobre el que se calcula la huella.
///
/// Lleva prefijos de longitud y etiqueta de dominio por el mismo motivo que
/// `Absorbedor` en `motor-pqc`: sin ellos, dos conjuntos distintos de entradas
/// podrian concatenarse en la misma cadena. Aqui el riesgo es concreto — un
/// paquete llamado `a` en version `b-c` y otro llamado `a-b` en version `c`.
fn mensaje_canonico(paquetes: &[(String, String)], resumen_fuentes: &str, canal: &str) -> Vec<u8> {
    let mut mensaje = Vec::new();
    absorber(&mut mensaje, b"eje-latam/conformidad/v1");

    for (nombre, version) in paquetes {
        absorber(&mut mensaje, nombre.as_bytes());
        absorber(&mut mensaje, version.as_bytes());
    }

    absorber(&mut mensaje, resumen_fuentes.as_bytes());
    absorber(&mut mensaje, canal.as_bytes());
    mensaje
}

/// Anexa un campo con su longitud por delante.
fn absorber(mensaje: &mut Vec<u8>, campo: &[u8]) {
    mensaje.extend_from_slice(&(campo.len() as u64).to_le_bytes());
    mensaje.extend_from_slice(campo);
}

/// Compone el atestado leyendo el arbol.
///
/// **No ejecuta ninguna suite.** Esta funcion es la que usan el comando y la
/// comprobacion, y por eso tiene que ser pura respecto al disco: si compusiera
/// una cosa al emitir y otra al comprobar, la barrera no probaria nada.
///
/// # Errores
///
/// Si falta un fichero de entrada, si una dependencia no esta resuelta en
/// `Cargo.lock`, o si el toolchain no declara canal.
pub fn componer(raiz: &Path) -> Result<Atestado, ErrorConformidad> {
    let bloqueo = leer(&raiz.join("Cargo.lock"))?;
    let toolchain = leer(&raiz.join("rust-toolchain.toml"))?;

    let fuentes = leer(
        &raiz
            .join("crates")
            .join("motor-pqc")
            .join("tests")
            .join("vectores")
            .join("FUENTES.lock"),
    )?;

    let paquetes = versiones_de(&bloqueo, &dependencias_resueltas(&bloqueo, PAQUETE)?)?;
    let canal = canal_de(&toolchain).ok_or(ErrorConformidad::SinCanal)?;
    let resumen_fuentes = resumir(fuentes.as_bytes());

    let huella = resumir(&mensaje_canonico(&paquetes, &resumen_fuentes, &canal));

    Ok(Atestado {
        paquetes,
        resumen_fuentes,
        canal,
        huella,
    })
}

/// Rinde el atestado al formato de `CONFORMIDAD.lock`.
#[must_use]
pub fn rendir(atestado: &Atestado) -> String {
    use std::fmt::Write as _;

    let mut texto = String::new();
    texto.push_str("# Atestado de conformidad poscuantica. RPT-005 §9.3, PA-121.\n");
    texto.push_str("#\n");
    texto.push_str("# Lo emite 'cargo xtask conformidad' SOLO si las tres suites pasan.\n");
    texto.push_str("# NO se edita a mano: 'cargo test -p xtask' recalcula la huella y falla\n");
    texto.push_str("# si no describe este arbol.\n");
    texto.push_str("#\n");
    texto.push_str("# Ata QUE se probo, no QUE se probo (RPT-005 §9.4). Componer esta huella\n");
    texto.push_str("# sin ejecutar nada es posible; cerrarlo es el alcance de PA-14.\n\n");

    let _ = writeln!(texto, "canal  {}", atestado.canal);
    let _ = writeln!(texto, "fuentes  {}", atestado.resumen_fuentes);
    texto.push('\n');

    for (nombre, version) in &atestado.paquetes {
        let _ = writeln!(texto, "paquete  {nombre}  {version}");
    }

    let _ = write!(texto, "\nhuella  {}\n", atestado.huella);
    texto
}

#[cfg(test)]
/// Huella registrada en un `CONFORMIDAD.lock` ya escrito.
#[must_use]
pub fn huella_registrada(contenido: &str) -> Option<String> {
    contenido
        .lines()
        .find_map(|linea| linea.trim().strip_prefix("huella  "))
        .map(|valor| valor.trim().to_owned())
}

#[cfg(test)]
/// Comprueba que el atestado en disco describe el arbol de ahora.
///
/// # Errores
///
/// [`ErrorConformidad::Divergencia`] con el detalle de que cambio, o un error de
/// lectura si el fichero no esta.
pub fn comprobar(raiz: &Path) -> Result<(), ErrorConformidad> {
    let contenido = leer(&raiz.join(FICHERO))?;
    let ahora = componer(raiz)?;

    let registrada = huella_registrada(&contenido).unwrap_or_default();

    if registrada == ahora.huella {
        return Ok(());
    }

    Err(ErrorConformidad::Divergencia {
        registrada,
        recalculada: ahora.huella.clone(),
        detalle: diferencias(&contenido, &ahora),
    })
}

#[cfg(test)]
/// Que entradas cambiaron, en texto para quien lea el fallo.
///
/// Sin esto el mensaje diria «las huellas no coinciden», que obliga a comparar a
/// mano dos ficheros. Un mecanismo que detecta bien y explica mal se acaba
/// desactivando.
fn diferencias(contenido: &str, ahora: &Atestado) -> String {
    use std::fmt::Write as _;

    let mut texto = String::new();
    let antes: Vec<&str> = contenido
        .lines()
        .filter_map(|linea| linea.trim().strip_prefix("paquete  "))
        .collect();

    for (nombre, version) in &ahora.paquetes {
        let entrada = format!("{nombre}  {version}");

        if antes.contains(&entrada.as_str()) {
            continue;
        }

        match antes
            .iter()
            .find(|cada| cada.starts_with(&format!("{nombre}  ")))
        {
            Some(vieja) => {
                let _ = writeln!(texto, "  cambio   : {vieja}  ->  {version}");
            }
            None => {
                let _ = writeln!(texto, "  entro    : {entrada}");
            }
        }
    }

    for vieja in &antes {
        let nombre = vieja.split("  ").next().unwrap_or(*vieja);

        if !ahora.paquetes.iter().any(|(cada, _)| cada == nombre) {
            let _ = writeln!(texto, "  salio    : {vieja}");
        }
    }

    if !contenido.contains(&format!("fuentes  {}", ahora.resumen_fuentes)) {
        let _ = writeln!(
            texto,
            "  cambio   : FUENTES.lock (los vectores no son los mismos)"
        );
    }

    if !contenido.contains(&format!("canal  {}", ahora.canal)) {
        let _ = writeln!(
            texto,
            "  cambio   : canal del toolchain  ->  {}",
            ahora.canal
        );
    }

    if texto.is_empty() {
        texto.push_str("  (ninguna entrada visible cambio: el fichero pudo editarse a mano)\n");
    }

    texto
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Un `Cargo.lock` reducido con la ambiguedad que se nos colo de verdad.
    const BLOQUEO: &str = "\
[[package]]
name = \"motor-pqc\"
version = \"0.1.0\"
dependencies = [
 \"ml-kem\",
 \"rand_core 0.10.1\",
 \"libcrux-ml-kem\",
]

[[package]]
name = \"ml-kem\"
version = \"0.3.2\"

[[package]]
name = \"rand_core\"
version = \"0.6.4\"

[[package]]
name = \"rand_core\"
version = \"0.10.1\"

[[package]]
name = \"libcrux-ml-kem\"
version = \"0.0.10\"
";

    fn entradas(nombres: &[&str]) -> Vec<String> {
        nombres.iter().map(|cada| (*cada).to_owned()).collect()
    }

    #[test]
    fn las_dependencias_salen_del_grafo_resuelto_y_no_de_una_lista() {
        let resueltas = dependencias_resueltas(BLOQUEO, "motor-pqc").expect("el paquete esta");

        assert_eq!(
            resueltas,
            vec!["ml-kem", "rand_core 0.10.1", "libcrux-ml-kem"],
            "se leen tal cual, con la desambiguacion que Cargo escribio"
        );
    }

    /// El defecto que emitio un atestado de 17 entradas para 14 dependencias.
    ///
    /// `rand_core` esta dos veces en el bloqueo. La version anterior se llevaba
    /// las dos y el fichero afirmaba que `motor-pqc` se probo contra `0.6.4`, que
    /// la arrastra otro crate del arbol.
    #[test]
    fn una_version_desambiguada_no_arrastra_a_sus_hermanas() {
        let resueltas = versiones_de(BLOQUEO, &entradas(&["rand_core 0.10.1"])).expect("resuelve");

        assert_eq!(
            resueltas,
            vec![("rand_core".to_owned(), "0.10.1".to_owned())]
        );
        assert!(
            !resueltas.iter().any(|(_, version)| version == "0.6.4"),
            "0.6.4 no es de motor-pqc y no puede entrar en su atestado"
        );
    }

    /// Y si el bloqueo NO desambigua, no se elige: se para.
    #[test]
    fn un_nombre_con_varias_versiones_no_se_resuelve_a_ojo() {
        match versiones_de(BLOQUEO, &entradas(&["rand_core"])) {
            Err(ErrorConformidad::Ambiguo { nombre, versiones }) => {
                assert_eq!(nombre, "rand_core");
                assert_eq!(versiones.len(), 2, "las dos, para que el mensaje sirva");
            }
            otro => panic!("la ambiguedad no puede colapsar con el caso bueno: {otro:?}"),
        }
    }

    /// Una dependencia declarada y sin resolver **para** el atestado.
    ///
    /// La alternativa —omitirla— produciria un fichero que parece completo y no
    /// lo esta, que es el modo de fallo que este comando existe para impedir.
    #[test]
    fn una_dependencia_sin_version_no_se_omite_en_silencio() {
        match versiones_de(BLOQUEO, &entradas(&["ml-kem", "inventada"])) {
            Err(ErrorConformidad::SinVersion(nombre)) => assert_eq!(nombre, "inventada"),
            otro => panic!("tenia que fallar por la que falta, no {otro:?}"),
        }
    }

    #[test]
    fn un_paquete_que_no_esta_en_el_bloqueo_se_dice() {
        match dependencias_resueltas(BLOQUEO, "no-existe") {
            Err(ErrorConformidad::SinPaquete(nombre)) => assert_eq!(nombre, "no-existe"),
            otro => panic!("tenia que acusar al paquete ausente: {otro:?}"),
        }
    }

    #[test]
    fn los_prefijos_de_longitud_impiden_confundir_dos_paquetes() {
        let uno = mensaje_canonico(&[("a".to_owned(), "b-c".to_owned())], "r", "1.85");
        let otro = mensaje_canonico(&[("a-b".to_owned(), "c".to_owned())], "r", "1.85");

        assert_ne!(
            resumir(&uno),
            resumir(&otro),
            "sin prefijos de longitud los dos conjuntos concatenan igual"
        );
    }

    /// La propiedad entera del mecanismo, en una prueba.
    #[test]
    fn mover_cualquier_entrada_cambia_la_huella() {
        let base = mensaje_canonico(&[("ml-dsa".to_owned(), "0.1.0".to_owned())], "res", "1.85");
        let referencia = resumir(&base);

        let version = mensaje_canonico(&[("ml-dsa".to_owned(), "0.1.2".to_owned())], "res", "1.85");
        let fuentes =
            mensaje_canonico(&[("ml-dsa".to_owned(), "0.1.0".to_owned())], "OTRO", "1.85");
        let canal = mensaje_canonico(&[("ml-dsa".to_owned(), "0.1.0".to_owned())], "res", "1.90");

        for (que, movido) in [("version", version), ("fuentes", fuentes), ("canal", canal)] {
            assert_ne!(
                referencia,
                resumir(&movido),
                "mover '{que}' tiene que invalidar el atestado"
            );
        }
    }

    #[test]
    fn el_fichero_rendido_devuelve_su_huella() {
        let atestado = Atestado {
            paquetes: vec![("ml-kem".to_owned(), "0.3.1".to_owned())],
            resumen_fuentes: "abc".to_owned(),
            canal: "1.85".to_owned(),
            huella: "d3adbeef".to_owned(),
        };

        let texto = rendir(&atestado);

        assert_eq!(huella_registrada(&texto).as_deref(), Some("d3adbeef"));
        assert!(texto.contains("paquete  ml-kem  0.3.1"));
        assert!(
            texto.contains("NO se edita a mano"),
            "el fichero tiene que decir que no se toca: si no, alguien lo cuadra a mano"
        );
    }

    #[test]
    fn el_canal_sale_del_fichero_pinado() {
        assert_eq!(
            canal_de("[toolchain]\nchannel = \"1.85\"\n").as_deref(),
            Some("1.85")
        );
        assert_eq!(canal_de("[toolchain]\nprofile = \"default\"\n"), None);
    }

    /// Raiz del repositorio, anclada al manifiesto y no al directorio de trabajo.
    fn raiz() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf()
    }

    #[test]
    fn el_arbol_real_compone_un_atestado_con_las_dos_familias() {
        let atestado = componer(&raiz()).expect("el arbol real tiene que componer");

        for exigida in ["ml-kem", "ml-dsa", "libcrux-ml-kem", "libcrux-ml-dsa"] {
            assert!(
                atestado
                    .paquetes
                    .iter()
                    .any(|(nombre, _)| nombre == exigida),
                "'{exigida}' tiene que estar en el atestado: es una de las dos \
                 implementaciones que la suite diferencial compara"
            );
        }

        assert_eq!(
            atestado.huella.len(),
            64,
            "la huella es un SHA-256 en hexadecimal"
        );

        // Cada nombre una sola vez. Es la prueba de regresion del defecto: si
        // vuelven a colarse las hermanas de `rand_core`, aqui se ve.
        let mut nombres: Vec<&String> =
            atestado.paquetes.iter().map(|(nombre, _)| nombre).collect();
        let antes = nombres.len();
        nombres.sort();
        nombres.dedup();

        assert_eq!(
            nombres.len(),
            antes,
            "un nombre repetido significa que se colaron dos versiones del mismo crate"
        );
    }

    /// **La barrera.** Si alguien sube una dependencia y no vuelve a ejecutar
    /// `cargo xtask conformidad`, esto se pone rojo solo.
    #[test]
    fn el_atestado_en_disco_describe_este_arbol() {
        if let Err(fallo) = comprobar(&raiz()) {
            panic!("{fallo}");
        }
    }
}
