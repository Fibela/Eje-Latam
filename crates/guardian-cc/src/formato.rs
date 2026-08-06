//! Formato canonico en disco del inventario firmado.
//!
//! RPT-013, PA-24.
//!
//! # Este es el primer frente
//!
//! El analizador de este fichero es **codigo no autenticado**: corre antes de que
//! ninguna firma se verifique, sobre un fichero que el modelo de amenazas asume
//! manipulable. Toda la cadena de cinco eslabones de RPT-011 y RPT-012 se apoya
//! en que este modulo no se caiga, no reserve memoria a peticion del atacante y
//! no admita dos lecturas del mismo fichero.
//!
//! # Disposicion
//!
//! ```text
//! +--------------------------------------------------+
//! | magico       8 bytes  "EJE-INV1"                  |
//! | version      u16 BE                               |
//! | secuencia    u64 BE                               |
//! | entradas     u32 BE   numero de marcados          |
//! +--------------------------------------------------+
//! | por cada marcado, 19 bytes de ancho fijo:         |
//! |   mac            6 bytes                          |
//! |   clase          u8    escalar cerrado            |
//! |   emitido_en     u64 BE                           |
//! |   vigencia_dias  u32 BE                           |
//! +--------------------------------------------------+
//! | firma        longitud fija, ML-DSA-65 + Ed25519   |
//! +--------------------------------------------------+
//! ```
//!
//! # Tres decisiones que merecen justificacion
//!
//! ## La raiz **no** se almacena
//!
//! Se recalcula a partir de las entradas. Guardarla crearia una pregunta que no
//! debe existir: si la raiz del fichero y la recalculada discrepan, ¿cual vale?
//! Cualquiera de las dos respuestas es explotable. Al no almacenarla, alterar una
//! entrada cambia la raiz recalculada y la firma deja de verificar.
//!
//! ## Las entradas son de ancho fijo
//!
//! No es una preferencia de estilo. Con ancho fijo, el numero declarado de
//! entradas se puede validar **contra los bytes que quedan** antes de reservar
//! nada. Con ancho variable habria que recorrer la lista para saber si cabe, y el
//! recorrido ya es trabajo a peticion del atacante.
//!
//! Es la misma leccion que `eje-ipc`: alli el prefijo de longitud se valida antes
//! de reservar, porque un prefijo que declare cuatro gigabytes no debe provocar
//! una reserva de cuatro gigabytes.
//!
//! ## Los bytes sobrantes se rechazan
//!
//! Un fichero cuya cola no se interpreta admite dos lecturas: la del analizador y
//! la de quien anadio los bytes. Es la misma clase de ambiguedad que
//! `deny_unknown_fields` cierra en el contrato IPC.

use eje_almacen::resumen::Resumen;
use motor_pqc::firma_hibrida::FirmaHibrida;

use crate::ClaseExcluida;
use crate::inventario::{ErrorInventario, Inventario, MarcadoBruto, RaizAnclada};
use crate::proveedores::DireccionEnlace;

/// Numero magico que abre todo fichero de inventario.
pub const MAGICO: &[u8; 8] = b"EJE-INV1";

/// Version del formato que este modulo entiende.
pub const VERSION: u16 = 1;

/// Bytes de cabecera: magico, version, secuencia y numero de entradas.
const LONGITUD_CABECERA: usize = 8 + 2 + 8 + 4;

/// Bytes de una entrada: mac, clase, emision y vigencia.
const LONGITUD_ENTRADA: usize = 6 + 1 + 8 + 4;

/// Cota superior del fichero completo, en bytes.
///
/// Un inventario razonable de un hospital grande ronda las decenas de miles de
/// entradas. Ocho megabytes dan margen de sobra y acotan el consumo ante un
/// fichero hostil.
pub const LONGITUD_MAXIMA: usize = 8 * 1024 * 1024;

/// Numero maximo de entradas admitido.
pub const ENTRADAS_MAXIMAS: usize = 200_000;

/// Defectos de estructura detectables **antes** de cualquier comprobacion
/// criptografica.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorFormato {
    /// El fichero excede [`LONGITUD_MAXIMA`].
    #[error("el fichero declara {longitud} bytes; el maximo es {LONGITUD_MAXIMA}")]
    FicheroExcesivo {
        /// Longitud observada.
        longitud: usize,
    },

    /// El fichero no empieza por [`MAGICO`].
    #[error("el fichero no es un inventario de Eje-Latam")]
    MagicoAusente,

    /// La version del formato no es la esperada.
    #[error("version de formato {encontrada}; este binario entiende la {VERSION}")]
    VersionDesconocida {
        /// Version leida del fichero.
        encontrada: u16,
    },

    /// El fichero termina antes de lo que su estructura exige.
    #[error("fichero truncado: se esperaban {esperados} bytes y hay {disponibles}")]
    Truncado {
        /// Bytes que la estructura exige.
        esperados: usize,
        /// Bytes realmente disponibles.
        disponibles: usize,
    },

    /// Quedaron bytes sin interpretar al final.
    #[error("{sobrantes} bytes sobrantes al final del fichero")]
    BytesSobrantes {
        /// Bytes no interpretados.
        sobrantes: usize,
    },

    /// El numero de entradas declarado excede el limite.
    #[error("se declaran {declaradas} entradas; el maximo es {ENTRADAS_MAXIMAS}")]
    DemasiadasEntradas {
        /// Numero declarado en la cabecera.
        declaradas: usize,
    },

    /// Un codigo de clase no corresponde a ninguna variante conocida.
    #[error("codigo de clase {codigo} desconocido")]
    ClaseDesconocida {
        /// Codigo leido.
        codigo: u8,
    },

    /// Un inventario sin entradas no tiene raiz y no significa nada.
    #[error("el inventario esta vacio")]
    InventarioVacio,

    /// La firma no decodifica.
    #[error("la firma del fichero no decodifica")]
    FirmaMalformada,

    /// Defecto detectado al construir el inventario en orden canonico.
    #[error(transparent)]
    Inventario(#[from] ErrorInventario),
}

/// Contenido estructural de un fichero de inventario, **sin verificar**.
///
/// Que este tipo exista solo significa que el fichero esta bien formado. No dice
/// nada sobre firmas: para eso hace falta `RaizVerificada`.
///
/// No deriva `Debug` ni `PartialEq`: [`FirmaHibrida`] no los implementa, y
/// anadirselos alli para conveniencia de este tipo pondria material
/// criptografico en los registros de depuracion.
#[derive(Clone)]
pub struct FicheroInventario {
    /// Inventario en orden canonico.
    pub inventario: Inventario,
    /// Raiz recalculada y secuencia leida.
    pub anclada: RaizAnclada,
    /// Firma que acompana al fichero.
    pub firma: FirmaHibrida,
}

/// Clase a partir de su codigo escalar.
const fn clase_desde_codigo(codigo: u8) -> Option<Option<ClaseExcluida>> {
    match codigo {
        0 => Some(None),
        1 => Some(Some(ClaseExcluida::SoporteVital)),
        2 => Some(Some(ClaseExcluida::SeguridadFuncional)),
        3 => Some(Some(ClaseExcluida::CaminoDeGestion)),
        _ => None,
    }
}

/// Codigo escalar de una clase.
const fn codigo_de_clase(clase: Option<ClaseExcluida>) -> u8 {
    match clase {
        None => 0,
        Some(ClaseExcluida::SoporteVital) => 1,
        Some(ClaseExcluida::SeguridadFuncional) => 2,
        Some(ClaseExcluida::CaminoDeGestion) => 3,
    }
}

/// Serializa un inventario y su firma al formato en disco.
///
/// La raiz no se escribe: se recalcula al leer.
#[must_use]
pub fn serializar(inventario: &Inventario, secuencia: u64, firma: &FirmaHibrida) -> Vec<u8> {
    let marcados = inventario.marcados();
    let mut salida =
        Vec::with_capacity(LONGITUD_CABECERA + marcados.len() * LONGITUD_ENTRADA + 4096);

    salida.extend_from_slice(MAGICO);
    salida.extend_from_slice(&VERSION.to_be_bytes());
    salida.extend_from_slice(&secuencia.to_be_bytes());
    salida.extend_from_slice(&(marcados.len() as u32).to_be_bytes());

    for marcado in marcados {
        salida.extend_from_slice(&marcado.mac);
        salida.push(codigo_de_clase(marcado.clase));
        salida.extend_from_slice(&marcado.emitido_en.to_be_bytes());
        salida.extend_from_slice(&marcado.vigencia_dias.to_be_bytes());
    }

    salida.extend_from_slice(&firma.a_bytes());
    salida
}

/// Analiza un fichero de inventario.
///
/// # Orden de comprobaciones
///
/// Cota global, magico, version, y solo despues cualquier cosa que dependa de
/// datos del fichero. Nada se reserva en funcion de un valor sin validar.
///
/// # Errores
///
/// Una variante de [`ErrorFormato`] por defecto detectado. Se distinguen a
/// proposito: un fichero truncado es un disco lleno y un magico ausente es otra
/// cosa.
pub fn analizar(bytes: &[u8]) -> Result<FicheroInventario, ErrorFormato> {
    if bytes.len() > LONGITUD_MAXIMA {
        return Err(ErrorFormato::FicheroExcesivo {
            longitud: bytes.len(),
        });
    }

    if bytes.len() < LONGITUD_CABECERA {
        return Err(ErrorFormato::Truncado {
            esperados: LONGITUD_CABECERA,
            disponibles: bytes.len(),
        });
    }

    if &bytes[..8] != MAGICO {
        return Err(ErrorFormato::MagicoAusente);
    }

    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    if version != VERSION {
        return Err(ErrorFormato::VersionDesconocida {
            encontrada: version,
        });
    }

    let mut secuencia_bruta = [0u8; 8];
    secuencia_bruta.copy_from_slice(&bytes[10..18]);
    let secuencia = u64::from_be_bytes(secuencia_bruta);

    let mut entradas_brutas = [0u8; 4];
    entradas_brutas.copy_from_slice(&bytes[18..22]);
    let entradas = u32::from_be_bytes(entradas_brutas) as usize;

    // Se acota el numero declarado ANTES de multiplicar o reservar. Sin esto, un
    // fichero de veintidos bytes puede declarar cuatro mil millones de entradas.
    if entradas > ENTRADAS_MAXIMAS {
        return Err(ErrorFormato::DemasiadasEntradas {
            declaradas: entradas,
        });
    }

    if entradas == 0 {
        return Err(ErrorFormato::InventarioVacio);
    }

    // El ancho fijo permite conocer el tamano exacto sin recorrer nada.
    let longitud_firma = FirmaHibrida::longitud_serializada();
    let esperados = LONGITUD_CABECERA + entradas * LONGITUD_ENTRADA + longitud_firma;

    if bytes.len() < esperados {
        return Err(ErrorFormato::Truncado {
            esperados,
            disponibles: bytes.len(),
        });
    }

    if bytes.len() > esperados {
        return Err(ErrorFormato::BytesSobrantes {
            sobrantes: bytes.len() - esperados,
        });
    }

    let mut marcados = Vec::with_capacity(entradas);
    let mut desplazamiento = LONGITUD_CABECERA;

    for _ in 0..entradas {
        let entrada = &bytes[desplazamiento..desplazamiento + LONGITUD_ENTRADA];

        let mut mac: DireccionEnlace = [0u8; 6];
        mac.copy_from_slice(&entrada[..6]);

        let codigo = entrada[6];
        let Some(clase) = clase_desde_codigo(codigo) else {
            return Err(ErrorFormato::ClaseDesconocida { codigo });
        };

        let mut emision = [0u8; 8];
        emision.copy_from_slice(&entrada[7..15]);

        let mut vigencia = [0u8; 4];
        vigencia.copy_from_slice(&entrada[15..19]);

        marcados.push(MarcadoBruto {
            mac,
            clase,
            emitido_en: u64::from_be_bytes(emision),
            vigencia_dias: u32::from_be_bytes(vigencia),
        });

        desplazamiento += LONGITUD_ENTRADA;
    }

    let firma = FirmaHibrida::desde_bytes(&bytes[desplazamiento..])
        .map_err(|_| ErrorFormato::FirmaMalformada)?;

    // `construir` reordena y rechaza duplicados. Un fichero escrito en otro orden
    // produce la misma raiz; uno con la misma direccion dos veces se rechaza.
    let inventario = Inventario::construir(marcados)?;
    let raiz = inventario.raiz().ok_or(ErrorFormato::InventarioVacio)?;

    Ok(FicheroInventario {
        inventario,
        anclada: RaizAnclada { raiz, secuencia },
        firma,
    })
}

/// Recalcula la raiz de un inventario ya analizado.
///
/// Existe para que quien audite pueda comprobar que la raiz del
/// [`FicheroInventario`] no viene del fichero sino del contenido.
#[must_use]
pub fn raiz_recalculada(fichero: &FicheroInventario) -> Option<Resumen> {
    fichero.inventario.raiz()
}
