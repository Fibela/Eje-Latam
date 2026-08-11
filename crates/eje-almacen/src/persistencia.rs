//! El registro de evidencia, en disco.
//!
//! RPT-029, PA-56.
//!
//! # Por que existe
//!
//! RPT-028 cableó los manejadores de alerta y dejó el registro **en memoria**.
//! Una alerta que no sobrevive a un reinicio del agente no es una alerta: el
//! sensor se reinicia por una actualizacion, por un corte de luz o porque
//! alguien lo reinicia, y con el se va la unica constancia de que hubo una
//! amenaza incontenible.
//!
//! # Disposicion
//!
//! ```text
//! +--------------------------------------------------+
//! | magico       8 bytes  "EJE-ALM1"                  |
//! | version      u16 BE                               |
//! | asientos     u32 BE                               |
//! +--------------------------------------------------+
//! | por cada asiento, ancho variable:                 |
//! |   numero        u64 BE                            |
//! |   instante_utc  i64 BE                            |
//! |   clase         u16 BE longitud + bytes           |
//! |   nodo          u16 BE longitud + bytes           |
//! |   detalle       u32 BE longitud + bytes           |
//! +--------------------------------------------------+
//! ```
//!
//! # Los resumenes **no** se almacenan
//!
//! Ni el propio ni el del anterior. Es el mismo argumento que en el inventario
//! firmado (RPT-013): guardarlos crearia una pregunta sin respuesta segura —si
//! el resumen escrito y el recalculado discrepan, ¿cual vale?—, y cualquiera de
//! las dos respuestas es explotable.
//!
//! Al no guardarlos, la cadena se **reconstruye** al cargar, encadenando cada
//! asiento con el anterior igual que hizo el agente al anexarlo. Alterar un
//! campo cambia el resumen recalculado y rompe el enlace del siguiente.
//!
//! # El numero de asiento si se almacena, y se comprueba
//!
//! Tambien es derivable —`anexar` lo asigna—, y ahi esta el motivo de guardarlo:
//! si no se guardara, **borrar un asiento intermedio pasaria desapercibido**
//! porque la reconstruccion renumeraria los siguientes y la cadena cuadraria.
//!
//! Con el numero escrito, la reconstruccion compara lo que asigna con lo que el
//! fichero declara, y la supresion sale como
//! [`ErrorPersistencia::NumeracionAlterada`]. Es la misma leccion que RPT-010
//! §4: firmar entrada por entrada no protege contra la supresion, y aqui el
//! papel de la firma lo hace la numeracion consecutiva.

use crate::cadena::RegistroEvidencia;
use crate::esquema::ClaseEvento;
use crate::resumen::Resumen;

/// Numero magico del fichero de registro.
pub const MAGICO_REGISTRO: &[u8; 8] = b"EJE-ALM1";

/// Version del formato.
///
/// La **1** no se rechaza: un fichero v1 *es* un segmento con `base = 1` y
/// `genesis = GENESIS`, asi que se interpreta en lugar de migrarse. Rechazarlo
/// lo convertiria en `ViolacionDetectada` y el agente acusaria de manipulacion a
/// quien solo actualizo el ejecutable (RPT-040 §2).
pub const VERSION_REGISTRO: u16 = 2;

/// Primera version del formato, sin base ni genesis explicitos.
pub const VERSION_SIN_SEGMENTOS: u16 = 1;

/// Bytes de cabecera v1: magico, version y numero de asientos.
const LONGITUD_CABECERA_V1: usize = 8 + 2 + 4;

/// Bytes de cabecera v2: magico, version, base, genesis y numero de asientos.
const LONGITUD_CABECERA: usize = 8 + 2 + 8 + 32 + 4;

/// Asientos que caben en un segmento antes de rotar.
///
/// RPT-040 §3, PA-59.
///
/// # Esto acota el coste de escritura, no la organizacion del directorio
///
/// RPT-029 §5 reescribe el fichero **entero** en cada persistencia. Con
/// segmentos de [`ASIENTOS_MAXIMOS`] serian unos 100 MB reescritos por cada
/// alerta anexada. El tamano de segmento es la cota de ese coste; la
/// granularidad de purga que da a la futura poda es una consecuencia agradable,
/// no el motivo.
///
/// # Es una hipotesis, no una medida
///
/// A unos 200 bytes por asiento son ~2 MB por reescritura. La cadencia real de
/// eventos es PA-41 y **sigue sin medir** porque depende de PA-40: este numero
/// esta razonado, no calculado, y hay que recalibrarlo cuando exista la medida.
pub const ASIENTOS_POR_SEGMENTO: usize = 10_000;

/// Cota superior del fichero completo, en bytes.
///
/// Se comprueba **antes** de interpretar nada. Un registro forense crece con el
/// tiempo, asi que la cota es holgada; lo que acota de verdad el consumo es
/// [`ASIENTOS_MAXIMOS`].
pub const LONGITUD_MAXIMA: usize = 64 * 1024 * 1024;

/// Numero maximo de asientos admitido.
pub const ASIENTOS_MAXIMOS: usize = 500_000;

/// Longitud maxima de un campo de texto, en bytes.
///
/// El detalle de un asiento lo escribe el agente, no un tercero, pero este
/// analizador corre sobre un fichero que el modelo de amenazas asume
/// manipulable: la cota existe para que un prefijo de longitud absurdo no
/// provoque una reserva absurda.
pub const TEXTO_MAXIMO: usize = 64 * 1024;

/// Defectos del fichero de registro.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorPersistencia {
    /// El fichero excede [`LONGITUD_MAXIMA`].
    #[error("el registro declara {longitud} bytes; el maximo es {LONGITUD_MAXIMA}")]
    FicheroExcesivo {
        /// Longitud observada.
        longitud: usize,
    },

    /// El fichero no empieza por [`MAGICO_REGISTRO`].
    #[error("el fichero no es un registro de evidencia de Eje-Latam")]
    MagicoAusente,

    /// La cabecera declara que el segmento empieza en el asiento cero.
    #[error("un segmento no puede empezar en el asiento 0")]
    BaseInvalida,

    /// Version desconocida.
    #[error("version de registro {encontrada}; este binario entiende la {VERSION_REGISTRO}")]
    VersionDesconocida {
        /// Version leida.
        encontrada: u16,
    },

    /// El fichero termina antes de lo que su estructura exige.
    ///
    /// Es el defecto esperable de un corte de energia durante el anexado, y por
    /// eso tiene variante propia: distinguirlo de una alteracion permite que el
    /// operador sepa si perdio el ultimo asiento o si alguien toco el fichero.
    #[error("registro truncado en el asiento {posicion}")]
    Truncado {
        /// Indice del asiento incompleto.
        posicion: usize,
    },

    /// Quedaron bytes sin interpretar al final.
    #[error("{sobrantes} bytes sobrantes al final del registro")]
    BytesSobrantes {
        /// Bytes no interpretados.
        sobrantes: usize,
    },

    /// El numero de asientos declarado excede el limite.
    #[error("se declaran {declarados} asientos; el maximo es {ASIENTOS_MAXIMOS}")]
    DemasiadosAsientos {
        /// Numero declarado en la cabecera.
        declarados: usize,
    },

    /// Un campo de texto declara una longitud excesiva.
    #[error("un campo de {longitud} bytes excede el maximo de {TEXTO_MAXIMO}")]
    TextoExcesivo {
        /// Longitud declarada.
        longitud: usize,
    },

    /// Un campo de texto no es UTF-8.
    #[error("un campo de texto del asiento {posicion} no es UTF-8 valido")]
    TextoInvalido {
        /// Indice del asiento.
        posicion: usize,
    },

    /// Una clase de evento no pertenece al vocabulario cerrado.
    ///
    /// Se rechaza en lugar de omitirse: un asiento que no se puede clasificar es
    /// evidencia que no se puede interpretar, y descartarlo en silencio
    /// convertiria este analizador en una via para borrar asientos.
    #[error("la clase de evento '{identificador}' no existe")]
    ClaseDesconocida {
        /// Identificador leido.
        identificador: String,
    },

    /// El fichero de ancla no mide lo que el formato exige.
    #[error("el ancla mide {encontrada} bytes; se esperaban {LONGITUD_ANCLA}")]
    LongitudDeAncla {
        /// Bytes disponibles.
        encontrada: usize,
    },

    /// La numeracion del fichero no coincide con la reconstruida.
    ///
    /// **Es la supresion de un asiento.** Ver el encabezado del modulo.
    #[error(
        "el asiento en posicion {posicion} declara el numero {declarado}; le corresponde {esperado}"
    )]
    NumeracionAlterada {
        /// Indice dentro del fichero.
        posicion: usize,
        /// Numero que declara el fichero.
        declarado: u64,
        /// Numero que le corresponde por posicion.
        esperado: u64,
    },
}

/// Numero magico del fichero de ancla.
pub const MAGICO_ANCLA: &[u8; 8] = b"EJE-ANC1";

/// Longitud exacta del fichero de ancla: magico, version, numero y resumen.
pub const LONGITUD_ANCLA: usize = 8 + 2 + 8 + 32;

/// Extremo de la cadena, anclado **fuera** del registro.
///
/// # Que problema resuelve
///
/// La cadena se **reconstruye** al cargar (§«Los resumenes no se almacenan»), asi
/// que siempre es coherente consigo misma. La numeracion delata que se borre un
/// asiento intermedio, pero **alterar o cortar el ultimo no deja rastro dentro
/// del fichero**: lo unico que cambia es el extremo, y el fichero no lo lleva.
///
/// El ancla lo lleva. Es el mismo mecanismo que
/// [`Centinela`](../../guardian_cc/inventario/enum.Centinela.html) aplica a la
/// secuencia del inventario, y tiene **la misma limitacion**, que conviene no
/// olvidar: si vive en el mismo almacen que el atacante controla, puede
/// actualizar los dos de forma coherente y no queda rastro.
///
/// Lo que se consigue no es impedir la manipulacion. Es que **no sea
/// silenciosa**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ancla {
    /// Numero del ultimo asiento cubierto.
    pub numero: u64,
    /// Resumen propio de ese asiento.
    pub extremo: Resumen,
}

/// Serializa el ancla.
#[must_use]
pub fn serializar_ancla(ancla: &Ancla) -> Vec<u8> {
    let mut salida = Vec::with_capacity(LONGITUD_ANCLA);
    salida.extend_from_slice(MAGICO_ANCLA);
    salida.extend_from_slice(&VERSION_REGISTRO.to_be_bytes());
    salida.extend_from_slice(&ancla.numero.to_be_bytes());
    salida.extend_from_slice(ancla.extremo.bytes());
    salida
}

/// Analiza el fichero de ancla.
///
/// # Errores
///
/// [`ErrorPersistencia::MagicoAusente`], [`ErrorPersistencia::VersionDesconocida`]
/// o [`ErrorPersistencia::LongitudDeAncla`]. **No se degrada a «sin ancla»**:
/// corromper treinta bytes seria una via para desactivar la comprobacion, que es
/// exactamente el ataque que el ancla existe para detectar.
pub fn analizar_ancla(bytes: &[u8]) -> Result<Ancla, ErrorPersistencia> {
    if bytes.len() != LONGITUD_ANCLA {
        return Err(ErrorPersistencia::LongitudDeAncla {
            encontrada: bytes.len(),
        });
    }

    if &bytes[..8] != MAGICO_ANCLA {
        return Err(ErrorPersistencia::MagicoAusente);
    }

    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    if version != VERSION_REGISTRO {
        return Err(ErrorPersistencia::VersionDesconocida {
            encontrada: version,
        });
    }

    let mut numero = [0u8; 8];
    numero.copy_from_slice(&bytes[10..18]);

    let mut extremo = [0u8; 32];
    extremo.copy_from_slice(&bytes[18..]);

    Ok(Ancla {
        numero: u64::from_be_bytes(numero),
        extremo: Resumen::desde_bytes(extremo),
    })
}

/// Ancla que corresponde a un registro.
///
/// `None` para un registro vacio: no hay extremo que anclar, y fabricar uno con
/// el resumen genesis haria indistinguible «vacio» de «recien creado con un
/// asiento borrado».
#[must_use]
pub fn ancla_de(registro: &RegistroEvidencia) -> Option<Ancla> {
    registro.asientos().last().map(|asiento| Ancla {
        numero: asiento.numero,
        extremo: asiento.resumen_propio,
    })
}

/// Veredicto de comparar un registro con su ancla.
///
/// # Los tres desenlaces son distintos y ninguno se colapsa
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cotejo {
    /// El prefijo anclado esta intacto y el registro no lo excede.
    Conforme,

    /// El registro tiene asientos **posteriores** al ancla, y el prefijo cuadra.
    ///
    /// Es lo que deja un corte de energia entre escribir el registro y escribir
    /// el ancla. No es manipulacion: la evidencia esta, solo que la cola no
    /// quedo cubierta.
    ///
    /// Se distingue a proposito. Colapsarlo en violacion haria que cada apagon
    /// en el momento justo pareciera un ataque.
    SinAnclar {
        /// Asientos que quedaron fuera del ancla.
        posteriores: usize,
    },

    /// El registro es **mas corto** de lo que el ancla cubre.
    ///
    /// Alguien corto la cola del registro. Es el ataque que PA-57 cierra.
    Truncado {
        /// Numero que el ancla cubre.
        anclado: u64,
        /// Asientos que quedan.
        ultimo_presente: u64,
    },

    /// El asiento anclado esta y **su resumen no es el que el ancla dice**.
    ///
    /// Alguien altero un asiento dentro del tramo cubierto.
    Alterado {
        /// Numero del asiento que no cuadra.
        numero: u64,
    },
}

/// Coteja un registro cargado con su ancla.
#[must_use]
pub fn cotejar(registro: &RegistroEvidencia, ancla: &Ancla) -> Cotejo {
    let ultimo = registro.ultimo_numero();

    // Un segmento recien rotado esta vacio y su `ultimo_numero` es `base - 1`:
    // el ultimo asiento del segmento anterior, que es justo lo que el ancla
    // describe. Por eso rotar no reescribe el ancla (RPT-040 §1). El extremo del
    // segmento vacio es su genesis, que es ese mismo resumen.
    if ancla.numero == registro.base().saturating_sub(1) && registro.vacio() {
        return if registro.genesis() == ancla.extremo {
            Cotejo::Conforme
        } else {
            Cotejo::Alterado {
                numero: ancla.numero,
            }
        };
    }

    let Some(asiento) = registro.asiento(ancla.numero) else {
        return Cotejo::Truncado {
            anclado: ancla.numero,
            ultimo_presente: ultimo,
        };
    };

    if asiento.resumen_propio != ancla.extremo {
        return Cotejo::Alterado {
            numero: ancla.numero,
        };
    }

    if ultimo > ancla.numero {
        return Cotejo::SinAnclar {
            posteriores: (ultimo - ancla.numero) as usize,
        };
    }

    Cotejo::Conforme
}

/// Lector de campos con comprobacion de limites.
///
/// Existe para que ninguna lectura pueda salirse del corte sin decirlo. Cada
/// metodo devuelve `None` cuando no queda bastante, y quien llama lo traduce a
/// [`ErrorPersistencia::Truncado`] con la posicion, que es lo que el operador
/// necesita saber.
struct Lector<'a> {
    bytes: &'a [u8],
    desplazamiento: usize,
}

impl<'a> Lector<'a> {
    const fn nuevo(bytes: &'a [u8], desplazamiento: usize) -> Self {
        Self {
            bytes,
            desplazamiento,
        }
    }

    fn tomar(&mut self, cuantos: usize) -> Option<&'a [u8]> {
        let fin = self.desplazamiento.checked_add(cuantos)?;
        let trozo = self.bytes.get(self.desplazamiento..fin)?;
        self.desplazamiento = fin;
        Some(trozo)
    }

    fn entero_u64(&mut self) -> Option<u64> {
        let bruto: [u8; 8] = self.tomar(8)?.try_into().ok()?;
        Some(u64::from_be_bytes(bruto))
    }

    fn entero_i64(&mut self) -> Option<i64> {
        let bruto: [u8; 8] = self.tomar(8)?.try_into().ok()?;
        Some(i64::from_be_bytes(bruto))
    }

    fn entero_u16(&mut self) -> Option<u16> {
        let bruto: [u8; 2] = self.tomar(2)?.try_into().ok()?;
        Some(u16::from_be_bytes(bruto))
    }

    fn entero_u32(&mut self) -> Option<u32> {
        let bruto: [u8; 4] = self.tomar(4)?.try_into().ok()?;
        Some(u32::from_be_bytes(bruto))
    }
}

/// Escribe un campo de texto con su prefijo de longitud de dos bytes.
fn escribir_texto_corto(salida: &mut Vec<u8>, texto: &str) {
    let bytes = texto.as_bytes();
    let longitud = u16::try_from(bytes.len().min(TEXTO_MAXIMO)).unwrap_or(u16::MAX);
    salida.extend_from_slice(&longitud.to_be_bytes());
    salida.extend_from_slice(&bytes[..longitud as usize]);
}

/// Escribe un campo de texto con su prefijo de longitud de cuatro bytes.
fn escribir_texto_largo(salida: &mut Vec<u8>, texto: &str) {
    let bytes = texto.as_bytes();
    let longitud = bytes.len().min(TEXTO_MAXIMO);
    let declarada = u32::try_from(longitud).unwrap_or(u32::MAX);
    salida.extend_from_slice(&declarada.to_be_bytes());
    salida.extend_from_slice(&bytes[..longitud]);
}

/// Serializa el registro completo.
///
/// # Por que se reescribe entero y no se anexa
///
/// Anexar seria mas barato y abre una ventana: un corte de energia a mitad de la
/// escritura deja un asiento parcial en la cola de un fichero por lo demas
/// valido. Con reescritura atomica —escribir un temporal y renombrar, como hace
/// `disco::escribir_atomico`— el fichero es siempre el de antes o el de despues.
///
/// El coste es lineal en el tamano del registro y se paga en cada alerta. Es
/// asumible mientras las alertas sean lo que son —raras y graves— y deja de
/// serlo si algun dia se anexa cada trama; queda escrito para ese dia.
#[must_use]
pub fn serializar(registro: &RegistroEvidencia) -> Vec<u8> {
    let asientos = registro.asientos();
    let mut salida = Vec::with_capacity(LONGITUD_CABECERA + asientos.len() * 128);

    salida.extend_from_slice(MAGICO_REGISTRO);
    salida.extend_from_slice(&VERSION_REGISTRO.to_be_bytes());
    salida.extend_from_slice(&registro.base().to_be_bytes());
    salida.extend_from_slice(registro.genesis().bytes());
    salida.extend_from_slice(&(asientos.len() as u32).to_be_bytes());

    for asiento in asientos {
        salida.extend_from_slice(&asiento.numero.to_be_bytes());
        salida.extend_from_slice(&asiento.instante_utc.to_be_bytes());
        escribir_texto_corto(&mut salida, asiento.clase.identificador());
        escribir_texto_corto(&mut salida, &asiento.nodo);
        escribir_texto_largo(&mut salida, &asiento.detalle);
    }

    salida
}

/// Analiza un fichero de registro y reconstruye la cadena.
///
/// # Orden de comprobaciones
///
/// Cota global, magico, version, numero de asientos, y solo despues cualquier
/// cosa que dependa de datos del fichero. Nada se reserva en funcion de un valor
/// sin validar.
///
/// # Errores
///
/// Una variante de [`ErrorPersistencia`] por defecto detectado. Se distinguen a
/// proposito: un fichero truncado es un corte de energia y una numeracion
/// alterada es alguien borrando evidencia.
pub fn analizar(bytes: &[u8]) -> Result<RegistroEvidencia, ErrorPersistencia> {
    if bytes.len() > LONGITUD_MAXIMA {
        return Err(ErrorPersistencia::FicheroExcesivo {
            longitud: bytes.len(),
        });
    }

    if bytes.len() < LONGITUD_CABECERA_V1 {
        return Err(ErrorPersistencia::Truncado { posicion: 0 });
    }

    if &bytes[..8] != MAGICO_REGISTRO {
        return Err(ErrorPersistencia::MagicoAusente);
    }

    // La v1 se lee como lo que es —un segmento con base 1 y genesis absoluto— y
    // no como un formato ajeno. Una version POSTERIOR si se rechaza: interpretar
    // lo que no se entiende es peor que declararlo (RPT-022).
    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    let (base, genesis, cabecera) = match version {
        VERSION_SIN_SEGMENTOS => (1u64, Resumen::GENESIS, LONGITUD_CABECERA_V1),
        VERSION_REGISTRO => {
            if bytes.len() < LONGITUD_CABECERA {
                return Err(ErrorPersistencia::Truncado { posicion: 0 });
            }
            let mut numero = [0u8; 8];
            numero.copy_from_slice(&bytes[10..18]);
            let mut resumen = [0u8; 32];
            resumen.copy_from_slice(&bytes[18..50]);
            (
                u64::from_be_bytes(numero),
                Resumen::desde_bytes(resumen),
                LONGITUD_CABECERA,
            )
        }
        _ => {
            return Err(ErrorPersistencia::VersionDesconocida {
                encontrada: version,
            });
        }
    };

    // Un segmento empieza en el asiento 1 como pronto. La base cero haria que
    // `ultimo_numero` de un segmento vacio se calculara sobre `0 - 1`, y ese es
    // el tipo de aritmetica que conviene rechazar en la puerta.
    if base == 0 {
        return Err(ErrorPersistencia::BaseInvalida);
    }

    let inicio_conteo = cabecera - 4;
    let declarados = u32::from_be_bytes([
        bytes[inicio_conteo],
        bytes[inicio_conteo + 1],
        bytes[inicio_conteo + 2],
        bytes[inicio_conteo + 3],
    ]) as usize;
    if declarados > ASIENTOS_MAXIMOS {
        return Err(ErrorPersistencia::DemasiadosAsientos { declarados });
    }

    let mut lector = Lector::nuevo(bytes, cabecera);
    let mut registro = RegistroEvidencia::continuando(base, genesis);

    for posicion in 0..declarados {
        let truncado = || ErrorPersistencia::Truncado { posicion };

        let numero = lector.entero_u64().ok_or_else(truncado)?;
        let instante_utc = lector.entero_i64().ok_or_else(truncado)?;

        let identificador = leer_texto_corto(&mut lector, posicion)?;
        let nodo = leer_texto_corto(&mut lector, posicion)?;
        let detalle = leer_texto_largo(&mut lector, posicion)?;

        let clase = ClaseEvento::desde_identificador(&identificador)
            .ok_or(ErrorPersistencia::ClaseDesconocida { identificador })?;

        // La cadena se reconstruye anexando, igual que hizo el agente. Lo que se
        // compara es el numero que `anexar` asigna con el que el fichero
        // declara: es lo que delata un asiento borrado.
        // El maximo ya se comprobo sobre `declarados` antes de este bucle, asi
        // que `anexar` no puede saturar aqui. Se propaga igualmente en lugar de
        // afirmarlo con un `expect`: si algun dia las dos cotas dejan de
        // coincidir, esto devuelve un error en vez de entrar en panico.
        let asignado = registro
            .anexar(instante_utc, clase, &nodo, &detalle)
            .map_err(|_| ErrorPersistencia::DemasiadosAsientos {
                declarados: ASIENTOS_MAXIMOS + 1,
            })?
            .numero;

        if asignado != numero {
            return Err(ErrorPersistencia::NumeracionAlterada {
                posicion,
                declarado: numero,
                esperado: asignado,
            });
        }
    }

    let sobrantes = bytes.len() - lector.desplazamiento;
    if sobrantes > 0 {
        return Err(ErrorPersistencia::BytesSobrantes { sobrantes });
    }

    Ok(registro)
}

/// Lee un campo con prefijo de dos bytes.
fn leer_texto_corto(lector: &mut Lector<'_>, posicion: usize) -> Result<String, ErrorPersistencia> {
    let longitud = lector
        .entero_u16()
        .ok_or(ErrorPersistencia::Truncado { posicion })? as usize;

    let bruto = lector
        .tomar(longitud)
        .ok_or(ErrorPersistencia::Truncado { posicion })?;

    String::from_utf8(bruto.to_vec()).map_err(|_| ErrorPersistencia::TextoInvalido { posicion })
}

/// Lee un campo con prefijo de cuatro bytes.
fn leer_texto_largo(lector: &mut Lector<'_>, posicion: usize) -> Result<String, ErrorPersistencia> {
    let longitud = lector
        .entero_u32()
        .ok_or(ErrorPersistencia::Truncado { posicion })? as usize;

    // Se acota ANTES de intentar tomar: un prefijo que declare cuatro mil
    // millones no debe llegar siquiera a la comprobacion de limites.
    if longitud > TEXTO_MAXIMO {
        return Err(ErrorPersistencia::TextoExcesivo { longitud });
    }

    let bruto = lector
        .tomar(longitud)
        .ok_or(ErrorPersistencia::Truncado { posicion })?;

    String::from_utf8(bruto.to_vec()).map_err(|_| ErrorPersistencia::TextoInvalido { posicion })
}
