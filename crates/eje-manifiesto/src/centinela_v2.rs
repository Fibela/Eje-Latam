//! Lector del centinela **version 2**, para migrarlo a la 3.
//!
//! PA-150.
//!
//! # Por que este lector vive aqui y no en `guardian-cc`
//!
//! Porque `guardian-cc` es el crate que viaja dentro del sensor, y la
//! documentacion de `VERSION_CENTINELA` dice —desde que se escribio la version
//! 2, meses antes de que hiciera falta— que *«la migracion tendra que ser una
//! operacion deliberada del emisor, no una lectura tolerante del agente»*.
//!
//! Poniendo el lector en el emisor, esa frase deja de ser una intencion y pasa a
//! ser una propiedad del reparto: **no existe ningun camino en el binario del
//! agente capaz de interpretar un centinela de version 2.** Un fichero viejo en
//! un sensor no se degrada ni se tolera: se rechaza como corrupto, y alguien
//! tiene que venir a migrarlo a mano.
//!
//! # Lo que este modulo no hace
//!
//! No escribe. Devuelve las dos marcas y ya; componer el v3 y escribirlo es
//! trabajo de `migrar_centinela` en `main.rs`, que ademas necesita la clave de
//! recuperacion para el ancla y por tanto no puede vivir aqui.

use guardian_cc::arranque::MAGICO_CENTINELA;
use guardian_cc::inventario::Centinela;

/// Longitud exacta del centinela de version 2.
const LONGITUD_V2: usize = 8 + 2 + (1 + 8) + (1 + 8);

/// Version que este modulo sabe leer, y la unica.
const VERSION_V2: u16 = 2;

/// Fallos al leer un centinela de version 2.
#[derive(Debug, thiserror::Error)]
pub enum ErrorCentinelaV2 {
    /// Longitud, magico o marcas que no cuadran.
    #[error("el centinela no es un fichero de version 2 valido")]
    NoEsV2,

    /// Es un centinela valido, pero de otra version.
    ///
    /// Se distingue de [`Self::NoEsV2`] a proposito: «esto no es un centinela»
    /// y «esto ya esta migrado» piden cosas distintas de quien lo lee, y
    /// colapsarlas haria que migrar dos veces se leyera como fichero corrupto.
    #[error("el centinela es de version {encontrada}, no de la 2")]
    OtraVersion {
        /// Version que declara el fichero.
        encontrada: u16,
    },
}

/// Las dos marcas de un centinela de version 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarcasV2 {
    /// Secuencia mas alta de inventario aceptada.
    pub inventario: Centinela,
    /// Secuencia mas alta de configuracion firmada aceptada.
    pub configuracion: Centinela,
}

/// Lee un centinela de version 2.
///
/// # Errores
///
/// [`ErrorCentinelaV2::OtraVersion`] si el fichero declara otra version —lo que
/// incluye un v3 ya migrado—, y [`ErrorCentinelaV2::NoEsV2`] si no cuadra nada
/// mas.
pub fn analizar_v2(bytes: &[u8]) -> Result<MarcasV2, ErrorCentinelaV2> {
    // La version se mira ANTES que la longitud. Un v3 mide sesenta y un bytes y
    // fallaria por longitud, y el mensaje diria «esto no es un centinela» sobre
    // un fichero perfectamente sano y ya migrado.
    if bytes.len() >= 10 && &bytes[..8] == MAGICO_CENTINELA {
        let version = u16::from_be_bytes([bytes[8], bytes[9]]);
        if version != VERSION_V2 {
            return Err(ErrorCentinelaV2::OtraVersion {
                encontrada: version,
            });
        }
    }

    if bytes.len() != LONGITUD_V2 || &bytes[..8] != MAGICO_CENTINELA {
        return Err(ErrorCentinelaV2::NoEsV2);
    }

    let (Some(inventario), Some(configuracion)) =
        (leer_marca(&bytes[10..19]), leer_marca(&bytes[19..28]))
    else {
        return Err(ErrorCentinelaV2::NoEsV2);
    };

    Ok(MarcasV2 {
        inventario,
        configuracion,
    })
}

/// Lee una marca de nueve bytes: presencia y valor.
///
/// Es la misma codificacion que la version 3, y se repite aqui en lugar de
/// reutilizarse porque **este lector no debe seguir al otro si el otro cambia**.
/// Un lector de formato antiguo que se mueve con el formato nuevo deja de leer
/// el formato antiguo, que es lo unico que existe para leer.
fn leer_marca(bytes: &[u8]) -> Option<Centinela> {
    let (presencia, valor) = bytes.split_first()?;
    let valor: [u8; 8] = valor.try_into().ok()?;

    match *presencia {
        0 if valor == [0u8; 8] => Some(Centinela::SinEstablecer),
        1 => Some(Centinela::Establecido(u64::from_be_bytes(valor))),
        _ => None,
    }
}
