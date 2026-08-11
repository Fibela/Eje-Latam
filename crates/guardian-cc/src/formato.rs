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
//! | segmentos    u16 BE   numero de declaraciones     |
//! +--------------------------------------------------+
//! | por cada marcado, 19 bytes de ancho fijo:         |
//! |   mac            6 bytes                          |
//! |   clase          u8    escalar cerrado            |
//! |   emitido_en     u64 BE                           |
//! |   vigencia_dias  u32 BE                           |
//! +--------------------------------------------------+
//! | por cada segmento, 15 bytes de ancho fijo:        |
//! |   vlan           u16 BE  1..=4094                 |
//! |   naturaleza     u8    escalar cerrado, 0 invalido|
//! |   emitido_en     u64 BE                           |
//! |   vigencia_dias  u32 BE                           |
//! +--------------------------------------------------+
//! | firma        longitud fija, ML-DSA-65 + Ed25519   |
//! +--------------------------------------------------+
//! ```
//!
//! # Que cubre la firma
//!
//! La firma no se calcula sobre estos bytes sino sobre el mensaje canonico de
//! [`mensaje_de_raiz`](crate::inventario::mensaje_de_raiz), que ancla **la raiz
//! Merkle de los marcados, el resumen de la tabla de segmentos y la secuencia**.
//! Los dos bloques quedan cubiertos por caminos distintos pero equivalentes:
//! alterar un marcado cambia la raiz, alterar una declaracion cambia el resumen,
//! y en ambos casos la firma deja de verificar (RPT-022 §2).
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
use crate::vlan::{
    DeclaracionVlan, ErrorVlan, NaturalezaSegmento, TablaVlan, VLAN_MAXIMA, VLAN_MINIMA,
    VLANS_MAXIMAS,
};

/// Numero magico que abre todo fichero de inventario.
pub const MAGICO: &[u8; 8] = b"EJE-INV1";

/// Version del formato que este modulo entiende.
///
/// La `2` incorpora el bloque de declaraciones de segmento (RPT-022, PA-45).
/// Subirla deja obsoletos todos los inventarios en version 1, que es exactamente
/// lo que [`ErrorFormato::FormatoObsoleto`] existe para que **no** se lea como un
/// ataque.
pub const VERSION: u16 = 2;

/// Bytes de cabecera: magico, version, secuencia, entradas y segmentos.
const LONGITUD_CABECERA: usize = 8 + 2 + 8 + 4 + 2;

/// Bytes de una entrada: mac, clase, emision y vigencia.
const LONGITUD_ENTRADA: usize = 6 + 1 + 8 + 4;

/// Bytes de una declaracion de segmento: vlan, naturaleza, emision y vigencia.
const LONGITUD_SEGMENTO: usize = 2 + 1 + 8 + 4;

/// Cota superior del fichero completo, en bytes.
///
/// Un inventario razonable de un hospital grande ronda las decenas de miles de
/// entradas. Ocho megabytes dan margen de sobra y acotan el consumo ante un
/// fichero hostil.
pub const LONGITUD_MAXIMA: usize = 8 * 1024 * 1024;

/// Numero maximo de entradas admitido.
pub const ENTRADAS_MAXIMAS: usize = 200_000;

/// Primera secuencia que un fichero **no** puede declarar.
///
/// # Por que hay techo
///
/// PA-33 describe el ataque de un solo mensaje: quien tenga la clave operativa
/// emite un inventario con secuencia `u64::MAX`, el agente lo acepta —la firma es
/// valida— y **ningun inventario legitimo puede ya superarlo**. El parque queda
/// congelado para siempre y revocar no lo arregla, porque el centinela sigue
/// arriba.
///
/// `Centinela::reiniciar_por` era la salida, y sigue existiendo. Pero es una
/// recuperacion: exige la clave de recuperacion fuera de linea y un humano. Es
/// mejor que el ataque no llegue a ocurrir.
///
/// Cuatro mil millones de emisiones son varios ordenes de magnitud mas de las que
/// un parque genera en la vida del producto. Por encima de eso no hay
/// crecimiento: hay una senal.
///
/// # Donde se comprueba, y donde no
///
/// Aqui, en el analizador de fichero, **antes** de tocar criptografia. Es el
/// camino por el que llega todo inventario real, asi que cierra el ataque en la
/// puerta.
///
/// No se comprueba en `RaizVerificada::verificar`, que es el paso en memoria.
/// Quien construya una `RaizAnclada` desde otra fuente —hoy nadie fuera de las
/// pruebas— se lo salta. Queda escrito para que no se confunda una defensa de
/// perimetro con un invariante del tipo.
pub const TECHO_SECUENCIA: u64 = 1 << 32;

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

    /// El fichero usa una version **anterior** del formato.
    ///
    /// # Por que no es lo mismo que no verificar
    ///
    /// Un inventario de formato anterior es un fichero legitimo que el
    /// administrador emitio cuando el agente entendia otra version. Tratarlo
    /// como manipulacion —que es lo que hacia la version anterior de este
    /// analizador— convertiria **cada actualizacion rutinaria del agente en una
    /// alerta maxima de ataque**, y eso ensena al operador a ignorar esa alerta
    /// justo antes de que sea real.
    ///
    /// La accion correcta es pedir reemision, no dar la voz de alarma.
    #[error(
        "formato en version {encontrada}; este binario emite la {VERSION}. Reemitir el inventario"
    )]
    FormatoObsoleto {
        /// Version leida del fichero.
        encontrada: u16,
    },

    /// El fichero usa una version **posterior** o desconocida del formato.
    ///
    /// Se rechaza sin interpretar: adivinar la disposicion de campos que no se
    /// conocen es exactamente lo que un analizador de entrada hostil no debe
    /// hacer.
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

    /// La secuencia declarada alcanza o supera [`TECHO_SECUENCIA`].
    ///
    /// Se rechaza como **malformacion**, no como politica: un fichero asi no
    /// puede proceder de un uso legitimo, y aceptarlo es el bloqueo permanente
    /// de PA-33.
    #[error("secuencia {declarada} en el techo o por encima ({TECHO_SECUENCIA}); ver PA-33")]
    SecuenciaFueraDeRango {
        /// Secuencia leida de la cabecera.
        declarada: u64,
    },

    /// El numero de entradas declarado excede el limite.
    #[error("se declaran {declaradas} entradas; el maximo es {ENTRADAS_MAXIMAS}")]
    DemasiadasEntradas {
        /// Numero declarado en la cabecera.
        declaradas: usize,
    },

    /// El numero de declaraciones de segmento excede el espacio declarable.
    #[error("se declaran {declaradas} segmentos; el maximo es {VLANS_MAXIMAS}")]
    DemasiadosSegmentos {
        /// Numero declarado en la cabecera.
        declaradas: usize,
    },

    /// Un codigo de clase no corresponde a ninguna variante conocida.
    #[error("codigo de clase {codigo} desconocido")]
    ClaseDesconocida {
        /// Codigo leido.
        codigo: u8,
    },

    /// Un codigo de naturaleza de segmento no corresponde a ninguna variante.
    ///
    /// El `0` cae aqui a proposito: un bloque de ceros no debe analizarse como
    /// una tabla de declaraciones validas.
    #[error("codigo de naturaleza de segmento {codigo} desconocido")]
    NaturalezaDesconocida {
        /// Codigo leido.
        codigo: u8,
    },

    /// Un identificador de VLAN queda fuera del rango declarable.
    #[error("la vlan {vlan} esta fuera del rango declarable {VLAN_MINIMA}..={VLAN_MAXIMA}")]
    VlanFueraDeRango {
        /// Identificador leido.
        vlan: u16,
    },

    /// Las declaraciones no vienen en orden ascendente de VLAN.
    ///
    /// Mismo motivo que [`Self::EntradasDesordenadas`]: reordenar en silencio
    /// haria que dos ficheros distintos produjeran la misma tabla, y con ella el
    /// mismo resumen firmado.
    #[error("la declaracion {posicion} rompe el orden ascendente de vlans")]
    SegmentosDesordenados {
        /// Indice de la primera declaracion fuera de orden.
        posicion: usize,
    },

    /// Defecto detectado al construir la tabla de segmentos.
    #[error(transparent)]
    Vlan(#[from] ErrorVlan),

    /// Un inventario sin entradas no tiene raiz y no significa nada.
    #[error("el inventario esta vacio")]
    InventarioVacio,

    /// Las entradas no vienen en orden ascendente de direccion.
    ///
    /// El orden canonico se comprueba **al leer**, no solo al construir. Si el
    /// analizador reordenase en silencio, dos ficheros distintos producirian el
    /// mismo inventario y la codificacion en disco dejaria de ser unica —la misma
    /// ambiguedad que cierran el rechazo de bytes sobrantes y
    /// `deny_unknown_fields` en el contrato IPC—.
    #[error("la entrada {posicion} rompe el orden ascendente de direcciones")]
    EntradasDesordenadas {
        /// Indice de la primera entrada fuera de orden.
        posicion: usize,
    },

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
    /// Tabla de segmentos en orden canonico, **sin verificar**.
    ///
    /// Que este bien formada no significa que sea la que el administrador firmo.
    /// Eso lo decide
    /// [`TablaVlanVerificada::verificar_e_instanciar`](crate::vlan::TablaVlanVerificada::verificar_e_instanciar).
    pub vlans: TablaVlan,
    /// Raiz recalculada, resumen de segmentos recalculado y secuencia leida.
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

/// Serializa un inventario, su tabla de segmentos y su firma al formato en
/// disco.
///
/// Ni la raiz ni el resumen de la tabla se escriben: ambos se recalculan al leer,
/// por el motivo del §«La raiz **no** se almacena».
#[must_use]
pub fn serializar(
    inventario: &Inventario,
    vlans: &TablaVlan,
    secuencia: u64,
    firma: &FirmaHibrida,
) -> Vec<u8> {
    let marcados = inventario.marcados();
    let declaraciones = vlans.declaraciones();
    let mut salida = Vec::with_capacity(
        LONGITUD_CABECERA
            + marcados.len() * LONGITUD_ENTRADA
            + declaraciones.len() * LONGITUD_SEGMENTO
            + 4096,
    );

    salida.extend_from_slice(MAGICO);
    salida.extend_from_slice(&VERSION.to_be_bytes());
    salida.extend_from_slice(&secuencia.to_be_bytes());
    salida.extend_from_slice(&(marcados.len() as u32).to_be_bytes());
    salida.extend_from_slice(&(declaraciones.len() as u16).to_be_bytes());

    for marcado in marcados {
        salida.extend_from_slice(&marcado.mac);
        salida.push(codigo_de_clase(marcado.clase));
        salida.extend_from_slice(&marcado.emitido_en.to_be_bytes());
        salida.extend_from_slice(&marcado.vigencia_dias.to_be_bytes());
    }

    for declaracion in declaraciones {
        salida.extend_from_slice(&declaracion.vlan.to_be_bytes());
        salida.push(declaracion.naturaleza.codigo());
        salida.extend_from_slice(&declaracion.emitido_en.to_be_bytes());
        salida.extend_from_slice(&declaracion.vigencia_dias.to_be_bytes());
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

    // Las dos direcciones no significan lo mismo. Un fichero anterior es
    // legitimo y caduco; uno posterior es ilegible. Colapsarlos hacia
    // «desconocida» hacia que una actualizacion pareciera un ataque.
    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    match version.cmp(&VERSION) {
        std::cmp::Ordering::Less => {
            return Err(ErrorFormato::FormatoObsoleto {
                encontrada: version,
            });
        }
        std::cmp::Ordering::Greater => {
            return Err(ErrorFormato::VersionDesconocida {
                encontrada: version,
            });
        }
        std::cmp::Ordering::Equal => {}
    }

    let mut secuencia_bruta = [0u8; 8];
    secuencia_bruta.copy_from_slice(&bytes[10..18]);
    let secuencia = u64::from_be_bytes(secuencia_bruta);

    // El techo se comprueba aqui, antes de la firma. Si se comprobara despues,
    // un inventario saturado y correctamente firmado seria «valido pero
    // rechazado», y esa distincion invita a que alguien la relaje.
    if secuencia >= TECHO_SECUENCIA {
        return Err(ErrorFormato::SecuenciaFueraDeRango {
            declarada: secuencia,
        });
    }

    let mut entradas_brutas = [0u8; 4];
    entradas_brutas.copy_from_slice(&bytes[18..22]);
    let entradas = u32::from_be_bytes(entradas_brutas) as usize;

    let segmentos = u16::from_be_bytes([bytes[22], bytes[23]]) as usize;

    // Se acotan los dos numeros declarados ANTES de multiplicar o reservar. Sin
    // esto, un fichero de veinticuatro bytes puede declarar cuatro mil millones
    // de entradas.
    if entradas > ENTRADAS_MAXIMAS {
        return Err(ErrorFormato::DemasiadasEntradas {
            declaradas: entradas,
        });
    }

    if segmentos > VLANS_MAXIMAS {
        return Err(ErrorFormato::DemasiadosSegmentos {
            declaradas: segmentos,
        });
    }

    if entradas == 0 {
        return Err(ErrorFormato::InventarioVacio);
    }

    // Una tabla de segmentos vacia SI es legitima: un cliente puede no haber
    // declarado ninguna VLAN todavia. Su resumen esta definido y queda firmado
    // igual, asi que borrar el bloque entero tampoco pasa desapercibido.

    // El ancho fijo permite conocer el tamano exacto sin recorrer nada.
    let longitud_firma = FirmaHibrida::longitud_serializada();
    let esperados = LONGITUD_CABECERA
        + entradas * LONGITUD_ENTRADA
        + segmentos * LONGITUD_SEGMENTO
        + longitud_firma;

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
    let mut anterior: Option<DireccionEnlace> = None;

    for posicion in 0..entradas {
        let entrada = &bytes[desplazamiento..desplazamiento + LONGITUD_ENTRADA];

        let mut mac: DireccionEnlace = [0u8; 6];
        mac.copy_from_slice(&entrada[..6]);

        // Estrictamente ascendente: cubre el desorden y, de paso, la direccion
        // repetida, que `Inventario::construir` tambien rechaza.
        if let Some(previa) = anterior {
            if mac <= previa {
                return Err(ErrorFormato::EntradasDesordenadas { posicion });
            }
        }
        anterior = Some(mac);

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

    let mut declaraciones = Vec::with_capacity(segmentos);
    let mut vlan_anterior: Option<u16> = None;

    for posicion in 0..segmentos {
        let registro = &bytes[desplazamiento..desplazamiento + LONGITUD_SEGMENTO];

        let vlan = u16::from_be_bytes([registro[0], registro[1]]);

        if !(VLAN_MINIMA..=VLAN_MAXIMA).contains(&vlan) {
            return Err(ErrorFormato::VlanFueraDeRango { vlan });
        }

        if let Some(previa) = vlan_anterior {
            if vlan <= previa {
                return Err(ErrorFormato::SegmentosDesordenados { posicion });
            }
        }
        vlan_anterior = Some(vlan);

        let codigo = registro[2];
        let Some(naturaleza) = NaturalezaSegmento::desde_codigo(codigo) else {
            return Err(ErrorFormato::NaturalezaDesconocida { codigo });
        };

        let mut emision = [0u8; 8];
        emision.copy_from_slice(&registro[3..11]);

        let mut vigencia = [0u8; 4];
        vigencia.copy_from_slice(&registro[11..15]);

        declaraciones.push(DeclaracionVlan {
            vlan,
            naturaleza,
            emitido_en: u64::from_be_bytes(emision),
            vigencia_dias: u32::from_be_bytes(vigencia),
        });

        desplazamiento += LONGITUD_SEGMENTO;
    }

    let firma = FirmaHibrida::desde_bytes(&bytes[desplazamiento..])
        .map_err(|_| ErrorFormato::FirmaMalformada)?;

    // `construir` reordena y rechaza duplicados. Un fichero escrito en otro orden
    // produce la misma raiz; uno con la misma direccion dos veces se rechaza.
    let inventario = Inventario::construir(marcados)?;
    let raiz = inventario.raiz().ok_or(ErrorFormato::InventarioVacio)?;

    let vlans = TablaVlan::construir(declaraciones)?;
    let resumen_vlans = vlans.resumen();

    Ok(FicheroInventario {
        inventario,
        vlans,
        anclada: RaizAnclada {
            raiz,
            vlans: resumen_vlans,
            secuencia,
        },
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
