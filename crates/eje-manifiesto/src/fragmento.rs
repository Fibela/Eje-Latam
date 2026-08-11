//! Fichero de fragmento de la clave de recuperacion.
//!
//! RPT-027, PA-54.
//!
//! # Disposicion
//!
//! ```text
//! +--------------------------------------------------+
//! | magico       8 bytes  "EJE-FRG1"                  |
//! | version      u16 BE                               |
//! | indice       u8    custodio, 1..=3                |
//! | umbral       u8    fragmentos necesarios           |
//! | custodios    u8    fragmentos emitidos             |
//! | huella      32 bytes  de la clave PUBLICA derivada |
//! | fragmento   32 bytes                               |
//! +--------------------------------------------------+
//! ```
//!
//! # La huella es lo que convierte a Shamir en verificable
//!
//! El reparto de Shamir **no autentica**: un custodio que entregue un fragmento
//! alterado no hace fallar la reconstruccion, produce **otro secreto**. Y en
//! silencio, que es lo peor: quien reune creeria tener la clave de recuperacion y
//! firmaria un certificado que ningun agente aceptaria, en mitad de un incidente.
//!
//! Cada fragmento lleva la huella de la clave **publica** que se deriva del
//! secreto original. Tras reunir se re-deriva y se compara. No es material
//! secreto —es la misma clave publica que se aprovisiona en el agente— y cierra
//! el hueco.
//!
//! **Lo que no hace: decir quien mintio.** Detecta que el conjunto no cuadra, no
//! cual de los dos fragmentos esta mal. Distinguirlo exigiria un compromiso por
//! fragmento y no lo tenemos; queda escrito para que nadie lo suponga.
//!
//! # Los tres fragmentos declaran umbral y custodios
//!
//! Redundante con el formato, y a proposito: dentro de unos anos, quien
//! encuentre uno de estos ficheros en una caja fuerte necesita saber cuantos
//! hermanos tiene sin depender de que el procedimiento siga escrito en algun
//! sitio.

use guardian_cc::revocacion::IdentificadorClave;
use motor_pqc::reparto::{CUSTODIOS, ErrorReparto, Fragmento, LONGITUD_SECRETO, UMBRAL, reunir};
use motor_pqc::secreto::Secreto;
use motor_pqc::semilla::{SemillaFirma, derivar_verificacion};

/// Numero magico del fichero de fragmento.
pub const MAGICO_FRAGMENTO: &[u8; 8] = b"EJE-FRG1";

/// Version del formato.
pub const VERSION_FRAGMENTO: u16 = 1;

/// Longitud de la huella de la clave publica.
const LONGITUD_HUELLA: usize = 32;

/// Bytes anteriores a la huella.
const LONGITUD_CABECERA: usize = 8 + 2 + 1 + 1 + 1;

/// Longitud exacta del fichero.
pub const LONGITUD_FICHERO: usize = LONGITUD_CABECERA + LONGITUD_HUELLA + LONGITUD_SECRETO;

/// Fallos del fichero de fragmento.
#[derive(Debug, thiserror::Error)]
pub enum ErrorFragmento {
    /// El fichero no empieza por [`MAGICO_FRAGMENTO`].
    #[error("el fichero no es un fragmento de Eje-Latam")]
    MagicoAusente,

    /// Version desconocida.
    #[error("version de fragmento {encontrada}; este binario entiende la {VERSION_FRAGMENTO}")]
    VersionDesconocida {
        /// Version leida.
        encontrada: u16,
    },

    /// La longitud no es la exacta que el formato exige.
    #[error("el fragmento mide {encontrada} bytes; se esperaban {LONGITUD_FICHERO}")]
    LongitudIncorrecta {
        /// Bytes disponibles.
        encontrada: usize,
    },

    /// El esquema declarado no es el que este binario reune.
    #[error(
        "el fragmento declara {umbral}-de-{custodios}; este binario reune {UMBRAL}-de-{CUSTODIOS}"
    )]
    EsquemaDistinto {
        /// Umbral declarado.
        umbral: u8,
        /// Custodios declarados.
        custodios: u8,
    },

    /// Los dos fragmentos pertenecen a claves distintas.
    ///
    /// Mezclar fragmentos de dos repartos produciria un secreto sin sentido, y
    /// sin esta comprobacion lo produciria **en silencio**.
    #[error("los fragmentos pertenecen a repartos distintos")]
    RepartosDistintos,

    /// La clave reconstruida no es la que los fragmentos anuncian.
    ///
    /// Alguno de los dos esta alterado. **No se puede decir cual**: ver el
    /// encabezado del modulo.
    #[error(
        "la clave reconstruida no coincide con la huella declarada; algun fragmento esta alterado"
    )]
    ReconstruccionNoCuadra,

    /// Defecto del reparto.
    #[error(transparent)]
    Reparto(#[from] ErrorReparto),
}

/// Fragmento leido de disco, con su huella.
pub struct FragmentoLeido {
    /// Fragmento propiamente dicho.
    pub fragmento: Fragmento,
    /// Huella de la clave publica del reparto.
    pub huella: [u8; LONGITUD_HUELLA],
}

/// Huella de la clave publica que se deriva de una semilla.
#[must_use]
pub fn huella_de(semilla: SemillaFirma) -> [u8; LONGITUD_HUELLA] {
    let verificacion = derivar_verificacion(semilla);
    let mut huella = [0u8; LONGITUD_HUELLA];
    huella.copy_from_slice(IdentificadorClave::de(&verificacion).resumen().bytes());
    huella
}

/// Serializa un fragmento.
#[must_use]
pub fn serializar(fragmento: &Fragmento, huella: &[u8; LONGITUD_HUELLA]) -> Vec<u8> {
    let mut salida = Vec::with_capacity(LONGITUD_FICHERO);
    salida.extend_from_slice(MAGICO_FRAGMENTO);
    salida.extend_from_slice(&VERSION_FRAGMENTO.to_be_bytes());
    salida.push(fragmento.indice);
    salida.push(UMBRAL);
    salida.push(CUSTODIOS);
    salida.extend_from_slice(huella);
    salida.extend_from_slice(&fragmento.bytes);
    salida
}

/// Analiza un fichero de fragmento.
///
/// # Errores
///
/// Una variante de [`ErrorFragmento`]. La longitud se exige exacta.
pub fn analizar(bytes: &[u8]) -> Result<FragmentoLeido, ErrorFragmento> {
    if bytes.len() != LONGITUD_FICHERO {
        return Err(ErrorFragmento::LongitudIncorrecta {
            encontrada: bytes.len(),
        });
    }

    if &bytes[..8] != MAGICO_FRAGMENTO {
        return Err(ErrorFragmento::MagicoAusente);
    }

    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    if version != VERSION_FRAGMENTO {
        return Err(ErrorFragmento::VersionDesconocida {
            encontrada: version,
        });
    }

    let (umbral, custodios) = (bytes[11], bytes[12]);
    if umbral != UMBRAL || custodios != CUSTODIOS {
        return Err(ErrorFragmento::EsquemaDistinto { umbral, custodios });
    }

    let mut huella = [0u8; LONGITUD_HUELLA];
    huella.copy_from_slice(&bytes[LONGITUD_CABECERA..LONGITUD_CABECERA + LONGITUD_HUELLA]);

    let mut bruto = [0u8; LONGITUD_SECRETO];
    bruto.copy_from_slice(&bytes[LONGITUD_CABECERA + LONGITUD_HUELLA..]);

    Ok(FragmentoLeido {
        fragmento: Fragmento {
            // El indice se valida en `reunir`, que es donde su rango importa.
            indice: bytes[10],
            bytes: bruto,
        },
        huella,
    })
}

/// Reconstruye la semilla y **comprueba** que es la del reparto.
///
/// # Orden
///
/// Se comprueba que los dos fragmentos declaren la misma huella **antes** de
/// reunir: mezclar repartos distintos es un error del operador, no una
/// alteracion, y merece un mensaje que lo diga.
///
/// # Errores
///
/// [`ErrorFragmento::RepartosDistintos`],
/// [`ErrorFragmento::ReconstruccionNoCuadra`] o un [`ErrorReparto`].
pub fn reunir_verificando(
    uno: &FragmentoLeido,
    otro: &FragmentoLeido,
) -> Result<SemillaFirma, ErrorFragmento> {
    if uno.huella != otro.huella {
        return Err(ErrorFragmento::RepartosDistintos);
    }

    let semilla = reunir(&uno.fragmento, &otro.fragmento)?;

    // Lo que Shamir no da. Sin esto, un fragmento alterado produce otra clave y
    // nadie se entera hasta que el agente rechaza el certificado, en mitad del
    // incidente que motivo la reconstruccion.
    if huella_de(Secreto::nuevo(*semilla.exponer())) != uno.huella {
        return Err(ErrorFragmento::ReconstruccionNoCuadra);
    }

    Ok(semilla)
}
