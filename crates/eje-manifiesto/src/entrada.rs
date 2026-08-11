//! Lo que escribe el administrador.
//!
//! RPT-026, PA-48.
//!
//! # `deny_unknown_fields` no es una preferencia de estilo
//!
//! Un campo mal escrito que se ignora en silencio es cómo un marcado de soporte
//! vital acaba emitido como no critico. `clse = "soporte-vital"` sin
//! `deny_unknown_fields` produce un fichero valido que declara el equipo
//! contenible, y el administrador no tiene ninguna forma de notarlo hasta el
//! incidente.
//!
//! Es el mismo mecanismo que `contrato-ipc.toml` usa para los canales y que el
//! formato en disco usa al rechazar bytes sobrantes: una sola lectura posible.
//!
//! # La ausencia de clase significa «declarado no critico»
//!
//! Y eso es una afirmacion fuerte —la unica fuente que puede hacerla es un
//! humano firmante (RPT-009 §3)—, asi que el campo se llama `clase` y su
//! ausencia se documenta aqui en lugar de dejarla implicita.

use guardian_cc::ClaseExcluida;
use guardian_cc::inventario::MarcadoBruto;
use guardian_cc::proveedores::DireccionEnlace;
use guardian_cc::vlan::{DeclaracionVlan, NaturalezaSegmento};
use serde::Deserialize;

/// Vigencia por defecto, en dias.
///
/// Coincide con `clasificacion.vigencia_marcado_dias` de
/// `contrato-contencion.toml`. Duplicarla aqui es deuda conocida: el manifiesto
/// deberia ser la unica fuente, y no lo es porque este crate no lo lee.
pub const VIGENCIA_POR_DEFECTO: u32 = 365;

/// Fallos de la entrada del administrador.
#[derive(Debug, thiserror::Error)]
pub enum ErrorEntrada {
    /// El TOML no se puede interpretar.
    #[error("el fichero de entrada no se puede interpretar: {detalle}")]
    TomlInvalido {
        /// Mensaje del analizador, que ya indica linea y columna.
        detalle: String,
    },

    /// Una direccion de enlace no tiene la forma esperada.
    #[error("la direccion '{texto}' no es una MAC de seis octetos en hexadecimal")]
    MacInvalida {
        /// Texto rechazado.
        texto: String,
    },

    /// Una clase declarada no corresponde a ninguna del vocabulario.
    ///
    /// Se rechaza en lugar de tratarse como «no critico»: la degradacion
    /// silenciosa por un valor no reconocido es exactamente lo que este
    /// analizador existe para impedir.
    #[error(
        "la clase '{texto}' no existe; use soporte-vital, seguridad-funcional o camino-de-gestion"
    )]
    ClaseDesconocida {
        /// Texto rechazado.
        texto: String,
    },

    /// Una naturaleza de segmento no corresponde a ninguna del vocabulario.
    #[error("la naturaleza '{texto}' no existe; use SinDispositivosCriticos o PuedeAlojarCriticos")]
    NaturalezaDesconocida {
        /// Texto rechazado.
        texto: String,
    },
}

/// Fichero de entrada completo.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entrada {
    /// Marcados de dispositivo. Puede estar vacio en el TOML, pero un
    /// inventario sin entradas no se emite: lo rechaza `Emisor::emitir`.
    #[serde(default)]
    pub marcado: Vec<MarcadoEntrada>,

    /// Declaraciones de segmento.
    #[serde(default)]
    pub segmento: Vec<SegmentoEntrada>,
}

/// Un marcado, tal como lo escribe el administrador.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarcadoEntrada {
    /// Direccion de enlace, en hexadecimal con `:` o `-`.
    pub mac: String,

    /// Clase excluida. **Ausente significa «declarado no critico»**.
    #[serde(default)]
    pub clase: Option<String>,

    /// Vigencia en dias.
    #[serde(default = "vigencia_por_defecto")]
    pub vigencia_dias: u32,
}

/// Una declaracion de segmento.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentoEntrada {
    /// Identificador de VLAN.
    pub vlan: u16,

    /// Naturaleza declarada.
    pub naturaleza: String,

    /// Vigencia en dias.
    #[serde(default = "vigencia_por_defecto")]
    pub vigencia_dias: u32,
}

const fn vigencia_por_defecto() -> u32 {
    VIGENCIA_POR_DEFECTO
}

/// Analiza una direccion de enlace escrita a mano.
///
/// Admite `:` y `-` como separadores y no admite su ausencia: doce caracteres
/// hexadecimales seguidos son faciles de transponer sin notarlo, y una MAC
/// transpuesta marca el equipo equivocado.
///
/// # Errores
///
/// [`ErrorEntrada::MacInvalida`].
pub fn analizar_mac(texto: &str) -> Result<DireccionEnlace, ErrorEntrada> {
    let invalida = || ErrorEntrada::MacInvalida {
        texto: texto.to_owned(),
    };

    let octetos: Vec<&str> = texto.split([':', '-']).collect();
    if octetos.len() != 6 {
        return Err(invalida());
    }

    let mut mac: DireccionEnlace = [0u8; 6];
    for (destino, octeto) in mac.iter_mut().zip(octetos) {
        // La comprobacion de digitos hexadecimales va aparte de `from_str_radix`
        // porque este acepta signo: `+1` mide dos caracteres y valdria 1.
        if octeto.len() != 2 || !octeto.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalida());
        }
        *destino = u8::from_str_radix(octeto, 16).map_err(|_| invalida())?;
    }

    Ok(mac)
}

/// Traduce el nombre de una clase al vocabulario cerrado.
///
/// Los identificadores son los de `contrato-contencion.toml`.
fn analizar_clase(texto: &str) -> Result<ClaseExcluida, ErrorEntrada> {
    match texto {
        "soporte-vital" => Ok(ClaseExcluida::SoporteVital),
        "seguridad-funcional" => Ok(ClaseExcluida::SeguridadFuncional),
        "camino-de-gestion" => Ok(ClaseExcluida::CaminoDeGestion),
        _ => Err(ErrorEntrada::ClaseDesconocida {
            texto: texto.to_owned(),
        }),
    }
}

/// Traduce el nombre de una naturaleza de segmento.
fn analizar_naturaleza(texto: &str) -> Result<NaturalezaSegmento, ErrorEntrada> {
    NaturalezaSegmento::TODAS
        .into_iter()
        .find(|naturaleza| naturaleza.identificador() == texto)
        .ok_or_else(|| ErrorEntrada::NaturalezaDesconocida {
            texto: texto.to_owned(),
        })
}

impl Entrada {
    /// Analiza el fichero del administrador.
    ///
    /// # Errores
    ///
    /// [`ErrorEntrada::TomlInvalido`], que ya incluye linea y columna.
    pub fn analizar(contenido: &str) -> Result<Self, ErrorEntrada> {
        toml::from_str(contenido).map_err(|error| ErrorEntrada::TomlInvalido {
            detalle: error.to_string(),
        })
    }

    /// Convierte los marcados al vocabulario del inventario.
    ///
    /// `emitido_en` se recibe en lugar de leerse del reloj aqui: una funcion que
    /// consulta la hora no se puede probar contra un instante fijo, y la vigencia
    /// es justo lo que hay que probar.
    ///
    /// # Errores
    ///
    /// [`ErrorEntrada::MacInvalida`] o [`ErrorEntrada::ClaseDesconocida`].
    pub fn marcados(&self, emitido_en: u64) -> Result<Vec<MarcadoBruto>, ErrorEntrada> {
        self.marcado
            .iter()
            .map(|entrada| {
                let clase = match entrada.clase.as_deref() {
                    Some(texto) => Some(analizar_clase(texto)?),
                    None => None,
                };

                Ok(MarcadoBruto {
                    mac: analizar_mac(&entrada.mac)?,
                    clase,
                    emitido_en,
                    vigencia_dias: entrada.vigencia_dias,
                })
            })
            .collect()
    }

    /// Convierte las declaraciones de segmento.
    ///
    /// El rango de VLAN no se comprueba aqui: lo impone `TablaVlan::construir`,
    /// que es el mismo codigo que usa el agente. Duplicar la comprobacion crearia
    /// dos sitios donde el rango puede divergir.
    ///
    /// # Errores
    ///
    /// [`ErrorEntrada::NaturalezaDesconocida`].
    pub fn segmentos(&self, emitido_en: u64) -> Result<Vec<DeclaracionVlan>, ErrorEntrada> {
        self.segmento
            .iter()
            .map(|entrada| {
                Ok(DeclaracionVlan {
                    vlan: entrada.vlan,
                    naturaleza: analizar_naturaleza(&entrada.naturaleza)?,
                    emitido_en,
                    vigencia_dias: entrada.vigencia_dias,
                })
            })
            .collect()
    }
}
