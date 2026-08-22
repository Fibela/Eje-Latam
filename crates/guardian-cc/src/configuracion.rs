//! Configuracion firmada del sensor.
//!
//! RPT-074, PA-79.
//!
//! # Que compra esto, dicho sin exagerar
//!
//! El fichero esta en `0640 root:root`: solo root puede editarlo, y root ya
//! puede sustituir el binario o parar el servicio. Esto **no le cierra la puerta
//! a root**.
//!
//! Lo que cambia es que ya no puede hacerlo **en silencio**. Antes una linea
//! editada en el `EnvironmentFile` —el intervalo de latido a una hora— alargaba
//! la ventana de silencio que la sala vigila y todo seguia pareciendo sano. Con
//! la firma, el agente no la acepta y la averia es visible.
//!
//! Y desde RPT-077 la otra via esta cerrada tambien: la unidad ya no pasa estos
//! parametros por `ExecStart`, y el agente **se niega a arrancar** si alguien los
//! pasa teniendo configuracion firmada. Mientras existieron las dos vias, la
//! firma no valia nada: bastaba una linea en la unidad para ganarle.
//!
//! # Este es el primer frente, otra vez
//!
//! El analizador corre **antes** de que ninguna firma se verifique, sobre un
//! fichero que el modelo de amenazas asume manipulable. Vale aqui todo lo que
//! [`crate::formato`] dice de si mismo: no caerse, no reservar memoria a peticion
//! del atacante y no admitir dos lecturas del mismo fichero.
//!
//! # Por que binario y no TOML
//!
//! Un analizador de texto en el camino que decide si el agente confia en su
//! propia configuracion es superficie nueva en el peor sitio posible: espacios,
//! codificaciones, escapes y comentarios son ambiguedad, y la ambiguedad aqui la
//! resuelve el atacante. El formato con prefijos de longitud es el que este
//! proyecto ya sabe defender.
//!
//! El coste esta asumido: el administrador no lo lee con `cat`. A cambio el
//! agente imprime la configuracion vigente al arrancar, que es donde alguien la
//! mira de verdad.
//!
//! # Disposicion
//!
//! ```text
//! +--------------------------------------------------+
//! | magico        8 bytes  "EJE-CFG1"                 |
//! | version       u16 BE                              |
//! | secuencia     u64 BE                              |
//! | perfil        u8    escalar cerrado, 0 invalido   |
//! | intervalo_ms  u64 BE  > 0                         |
//! | grupo_puesto  u8    0 o 1                         |
//! | grupo_ipc     u32 BE                              |
//! +--------------------------------------------------+
//! | cuatro campos de texto, en orden fijo, cada uno:  |
//! |   longitud    u16 BE  <= MAXIMO_CAMPO             |
//! |   bytes       UTF-8                               |
//! |                                                   |
//! |   interfaz, colector, nombre, maquina_esperada    |
//! +--------------------------------------------------+
//! | firma         longitud fija, ML-DSA-65 + Ed25519  |
//! +--------------------------------------------------+
//! ```
//!
//! # Que cubre la firma
//!
//! Todo lo anterior, por el mensaje canonico de [`mensaje_de_configuracion`], que
//! absorbe cada campo con separacion de dominio y prefijo de longitud. No se
//! firman los bytes del fichero: se firma su **significado**, para que dos
//! codificaciones del mismo contenido no den firmas distintas.
//!
//! # Los dos campos que no son configuracion sino defensa
//!
//! [`Valores::maquina_esperada`] impide el traslado lateral: sin el, root copia
//! la configuracion de un sensor tranquilo sobre uno ruidoso y **las dos firmas
//! son legitimas**.
//!
//! [`Valores::secuencia`] impide la reversion: sin ella, root reinstala una
//! configuracion **antigua y correctamente firmada** —la que tenia el intervalo
//! largo— y la firma la avala. Es el ataque de PA-27 aplicado aqui, y se apoya en
//! el mismo centinela: desde RPT-078 el fichero de centinela lleva **dos** marcas
//! de agua, una por serie, y [`analizar`] exige que le pasen la de configuracion.
//! No se puede leer una configuracion sin decir contra que se fecha, que es la
//! unica forma conocida de que un mecanismo no se quede sin cablear.
//!
//! # Lo que esta configuracion NO dice, y por que
//!
//! **No dice donde vive el almacen ni donde nace el socket.** Lo dijo durante un
//! dia, y el circulo aparecio al cablearlo (RPT-077, PA-79): la clave con la que
//! esta configuracion se verifica es `<almacen>/clave-cliente.pub`, asi que una
//! configuracion que moviera el almacen estaria eligiendo **donde se busca la
//! clave que decide si creerla**. Basta apuntarlo a un directorio propio, dejar
//! ahi una clave propia, y la firma pasa a decir lo que uno quiera.
//!
//! No es un descuido de diseno: es la diferencia entre **politica** —a que
//! segmento mira este sensor, cada cuanto late, a quien informa, que es lo que el
//! cliente firma— e **instalacion** —donde guarda sus ficheros esta maquina, que
//! lo decide quien la instala y vive en la unidad—. Mezclarlas fue lo que produjo
//! el circulo.

use eje_almacen::resumen::Absorbedor;
use motor_pqc::firma_hibrida::{FirmaHibrida, verificar};

use crate::PerfilSegmento;
use crate::formato::TECHO_SECUENCIA;
use crate::inventario::{Centinela, ClaveInventario, DominioClave};

/// Numero magico que abre toda configuracion firmada.
pub const MAGICO_CONFIGURACION: &[u8; 8] = b"EJE-CFG1";

/// Version del formato de configuracion.
pub const VERSION_CONFIGURACION: u16 = 1;

/// Donde vive la configuracion firmada en un sensor instalado.
///
/// RPT-074 §8. **Fija en el binario y no derivada de un argumento**: si la ruta
/// saliera de la linea de ordenes, quien controle el arranque apuntaria el agente
/// a un fichero firmado distinto —uno antiguo, o el de otro sensor— y la firma lo
/// avalaria. La unidad de `systemd` no pasa configuracion por eso mismo.
pub const RUTA_CONFIGURACION: &str = "/etc/eje-latam/agente.conf.firmado";

/// Separacion de dominio del mensaje firmado.
///
/// Distinto del de la raiz del inventario y del de los certificados: una firma
/// emitida sobre un inventario no puede valer como configuracion aunque la clave
/// sea la misma.
const DOMINIO_CONFIGURACION: &[u8] = b"eje-latam/agt-01/configuracion-sensor/v1";

/// Cabecera: magico, version, secuencia, perfil, intervalo y grupo.
const LONGITUD_CABECERA: usize = 8 + 2 + 8 + 1 + 8 + 1 + 4;

/// Cuantos campos de texto lleva, en orden fijo.
const CAMPOS_DE_TEXTO: usize = 4;

/// Techo de cada campo de texto.
///
/// Una ruta larga cabe de sobra. El techo existe para que un prefijo de longitud
/// absurdo no provoque una reserva absurda: se comprueba **antes** de cortar
/// nada, que es la leccion de `eje-ipc`.
pub const MAXIMO_CAMPO: usize = 4096;

/// Codigo del perfil corporativo en el fichero.
const CODIGO_CORPORATIVO: u8 = 1;

/// Codigo del perfil OT en el fichero.
const CODIGO_OT: u8 = 2;

/// Fallos al leer una configuracion firmada.
///
/// Cada variante dice **que** estaba mal, no solo que algo lo estaba: el operador
/// que recibe «no verifica» y el que recibe «esta configuracion es de otra
/// maquina» tienen que hacer cosas distintas.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorConfiguracion {
    /// No empieza por el numero magico.
    #[error("el fichero no es una configuracion de Eje-Latam")]
    MagicoAusente,

    /// Version distinta de la conocida.
    ///
    /// No se interpreta «como si fuera» la conocida: una version futura puede
    /// significar campos nuevos, y leerla a medias es inventarse el contenido.
    #[error("version de configuracion desconocida: {encontrada}")]
    VersionDesconocida {
        /// La que traia el fichero.
        encontrada: u16,
    },

    /// Longitud imposible para este formato.
    #[error("longitud de configuracion incorrecta: {encontrada} bytes")]
    LongitudIncorrecta {
        /// Bytes que trae el fichero.
        encontrada: usize,
    },

    /// El perfil no es uno de los declarados.
    #[error("codigo de perfil desconocido: {codigo}")]
    PerfilDesconocido {
        /// El codigo que traia.
        codigo: u8,
    },

    /// Un campo de texto declara mas de lo que cabe o de lo que se admite.
    #[error("campo de texto de longitud imposible en la posicion {indice}")]
    CampoImposible {
        /// Cual de los cuatro.
        indice: usize,
    },

    /// Un campo de texto no es UTF-8.
    #[error("campo de texto que no es UTF-8 en la posicion {indice}")]
    CampoNoEsTexto {
        /// Cual de los cuatro.
        indice: usize,
    },

    /// El intervalo de latido no es positivo.
    ///
    /// Cero convertiria cada vuelta en un latido e inundaria el colector; un
    /// valor negativo no cabe en el formato. No se corrige a un valor cercano:
    /// eso seria obedecer a medias.
    #[error("el intervalo de latido tiene que ser positivo, y es {encontrado}")]
    IntervaloImposible {
        /// El que traia.
        encontrado: u64,
    },

    /// La firma no se puede interpretar.
    #[error("la firma de la configuracion esta malformada")]
    FirmaMalformada,

    /// La firma no verifica con la clave dada.
    #[error("la configuracion NO verifica: alguien la toco o la firmo otra clave")]
    FirmaInvalida,

    /// La clave presentada no es la del administrador del cliente.
    ///
    /// La configuracion del sensor la firma **quien opera la instalacion**, no
    /// PremosCorp. Sin esta comprobacion, la clave con la que se firman binarios
    /// de release podria decidir a que segmento apunta un sensor, que es
    /// exactamente la confusion que `DominioClave` existe para impedir
    /// (RPT-011 §4).
    #[error(
        "la configuracion la firma el administrador del cliente, y esta clave es de {encontrado:?}"
    )]
    DominioInesperado {
        /// El dominio que traia la clave presentada.
        encontrado: DominioClave,
    },

    /// Esta configuracion se emitio para otra maquina.
    #[error("configuracion emitida para '{esperada}' y este equipo es '{encontrada}'")]
    MaquinaAjena {
        /// La que dice el fichero.
        esperada: String,
        /// La de este equipo.
        encontrada: String,
    },

    /// La secuencia alcanza o supera [`TECHO_SECUENCIA`].
    ///
    /// RPT-078, y es PA-33 aplicado aqui. Sin techo, quien tenga la clave
    /// operativa emite **una** configuracion con `secuencia = u64::MAX`, el
    /// agente la acepta —la firma es valida— y **ninguna configuracion legitima
    /// puede ya superarla**: el sensor queda congelado con lo que diga esa, para
    /// siempre, y revocar la clave no lo arregla porque la marca sigue arriba.
    ///
    /// Se rechaza como malformacion y no como politica: un fichero asi no puede
    /// venir de un uso legitimo.
    #[error("secuencia de configuracion {declarada} en el techo o por encima ({TECHO_SECUENCIA})")]
    SecuenciaFueraDeRango {
        /// La que traia el fichero.
        declarada: u64,
    },

    /// Esta configuracion es anterior a la ultima que este sensor acepto.
    ///
    /// RPT-078, PA-79 paso 5. La firma es **legitima**: la emitio quien debia. Lo
    /// que no es legitimo es reponer la de la semana pasada —la del intervalo de
    /// latido largo, la que apuntaba a otro colector— sobre un sensor que ya vio
    /// una posterior.
    ///
    /// Es el mismo ataque que [`crate::inventario::Centinela`] resuelve para el
    /// inventario, con el mismo mecanismo y en el mismo fichero.
    #[error(
        "configuracion revertida: trae la secuencia {encontrada} y este sensor ya acepto la {aceptada}"
    )]
    SecuenciaRevertida {
        /// La que trae el fichero.
        encontrada: u64,
        /// La mas alta que este sensor acepto.
        aceptada: u64,
    },
}

/// Lo que la configuracion firmada dice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Valores {
    /// Numero monotonico de emision. Ver [`ErrorConfiguracion`] y RPT-074 §5.
    pub secuencia: u64,
    /// Interfaz que se vigila.
    pub interfaz: String,
    /// Perfil del segmento.
    pub perfil: PerfilSegmento,
    /// Colector de syslog, `host:puerto`. **Vacio es legitimo** (RPT-054 §1) y
    /// significa que este sensor no informa a ninguna sala.
    pub colector: String,
    /// Cada cuanto late, en milisegundos.
    pub intervalo_latido_ms: u64,
    /// Grupo numerico autorizado a consultar por el socket. `None` deja el socket
    /// en `0600`.
    pub grupo_ipc: Option<u32>,
    /// Identidad del sensor en la sala.
    pub nombre: String,
    /// `hostname` del equipo donde esta configuracion es valida.
    pub maquina_esperada: String,
}

/// Mensaje canonico que se firma.
///
/// Cada campo entra con prefijo de longitud, asi que `interfaz="eth" nombre="0"`
/// y `interfaz="eth0" nombre=""` no producen el mismo mensaje. Sin los prefijos
/// serian indistinguibles, que es la ambiguedad que RPT-005 §7 cierra en todo el
/// proyecto.
#[must_use]
pub fn mensaje_de_configuracion(valores: &Valores) -> Vec<u8> {
    let mut absorbedor = Absorbedor::nuevo(DOMINIO_CONFIGURACION);

    absorbedor
        .entero(valores.secuencia)
        .entero(u64::from(codigo_de_perfil(valores.perfil)))
        .entero(valores.intervalo_latido_ms)
        // Presencia y valor por separado: sin la presencia, «sin grupo» y «grupo
        // 0» —que es root— darian el mismo mensaje, y son cosas distintas.
        .entero(u64::from(u8::from(valores.grupo_ipc.is_some())))
        .entero(u64::from(valores.grupo_ipc.unwrap_or(0)))
        .campo(valores.interfaz.as_bytes())
        .campo(valores.colector.as_bytes())
        .campo(valores.nombre.as_bytes())
        .campo(valores.maquina_esperada.as_bytes());

    absorbedor.finalizar().bytes().to_vec()
}

/// Codigo en disco de un perfil.
const fn codigo_de_perfil(perfil: PerfilSegmento) -> u8 {
    match perfil {
        PerfilSegmento::Corporativo => CODIGO_CORPORATIVO,
        PerfilSegmento::Ot => CODIGO_OT,
    }
}

/// Perfil a partir de su codigo. `None` si no es uno de los declarados.
const fn perfil_desde_codigo(codigo: u8) -> Option<PerfilSegmento> {
    match codigo {
        CODIGO_CORPORATIVO => Some(PerfilSegmento::Corporativo),
        CODIGO_OT => Some(PerfilSegmento::Ot),
        // El cero no es un perfil a proposito: un fichero de ceros no puede
        // producir una configuracion valida.
        _ => None,
    }
}

/// Serializa una configuracion con su firma.
///
/// La firma se calcula fuera —la clave privada no vive en el sensor— y entra ya
/// hecha.
#[must_use]
pub fn serializar(valores: &Valores, firma: &FirmaHibrida) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(LONGITUD_CABECERA + 512);

    bytes.extend_from_slice(MAGICO_CONFIGURACION);
    bytes.extend_from_slice(&VERSION_CONFIGURACION.to_be_bytes());
    bytes.extend_from_slice(&valores.secuencia.to_be_bytes());
    bytes.push(codigo_de_perfil(valores.perfil));
    bytes.extend_from_slice(&valores.intervalo_latido_ms.to_be_bytes());
    bytes.push(u8::from(valores.grupo_ipc.is_some()));
    bytes.extend_from_slice(&valores.grupo_ipc.unwrap_or(0).to_be_bytes());

    for texto in campos_en_orden(valores) {
        // La longitud cabe en `u16` porque `analizar` rechaza mas de
        // `MAXIMO_CAMPO`, que es menor. Un valor mas largo se trunca aqui a un
        // fichero que su propio analizador rechazara, que es preferible a
        // producir uno que se lea distinto de lo que se firmo.
        let longitud = u16::try_from(texto.len()).unwrap_or(u16::MAX);
        bytes.extend_from_slice(&longitud.to_be_bytes());
        bytes.extend_from_slice(texto.as_bytes());
    }

    bytes.extend_from_slice(&firma.a_bytes());
    bytes
}

/// Los cuatro campos de texto, en el orden del formato.
///
/// Una sola definicion del orden, usada por [`serializar`] y por [`analizar`]:
/// dos listas serian la septima de la semana, y ademas una en la que un
/// intercambio de dos campos produciria ficheros que se leen al reves de como se
/// escribieron.
fn campos_en_orden(valores: &Valores) -> [&str; CAMPOS_DE_TEXTO] {
    [
        &valores.interfaz,
        &valores.colector,
        &valores.nombre,
        &valores.maquina_esperada,
    ]
}

/// Analiza y **verifica** una configuracion firmada.
///
/// # Errores
///
/// Cualquier variante de [`ErrorConfiguracion`]. No hay lectura parcial: o sale
/// una configuracion verificada entera, o sale un motivo.
///
/// # Orden de las comprobaciones
///
/// Primero la forma, despues la firma, despues la identidad de la maquina, y por
/// ultimo la frescura. La firma no se intenta sobre bytes que no se han
/// entendido, y ni la maquina ni la secuencia se comparan antes de saber que el
/// fichero es autentico: al reves, un fichero inventado podria hacer decir al
/// agente el nombre de otra maquina o una secuencia que nadie firmo.
///
/// # La frescura vive aqui y no en quien llama
///
/// RPT-078, PA-79 paso 5. Es la leccion del proyecto entero: un mecanismo que hay
/// que acordarse de invocar acaba no invocandose. Pasando la marca por parametro,
/// **no se puede leer una configuracion sin decir contra que se fecha**.
///
/// `Centinela::SinEstablecer` acepta cualquier secuencia y es el primer
/// aprovisionamiento. La proteccion completa contra reversion exigiria un ancla
/// fuera del almacen escribible —contador monotono en TPM—, y vale aqui lo mismo
/// que [`crate::inventario::Centinela`] ya deja escrito: lo que se consigue es
/// que revertir **no sea silencioso**.
pub fn analizar(
    bytes: &[u8],
    clave: &ClaveInventario,
    maquina: &str,
    aceptada: Centinela,
) -> Result<Valores, ErrorConfiguracion> {
    // El dominio, antes que nada. Recibir la clave envuelta y no desnuda es lo
    // que impide que quien llama elija con cual verificar: una clave de
    // PremosCorp puesta donde va la del cliente se rechaza por lo que **es**, no
    // por donde estaba (RPT-024).
    if clave.dominio() != DominioClave::Cliente {
        return Err(ErrorConfiguracion::DominioInesperado {
            encontrado: clave.dominio(),
        });
    }

    if bytes.len() < LONGITUD_CABECERA {
        return Err(ErrorConfiguracion::LongitudIncorrecta {
            encontrada: bytes.len(),
        });
    }

    if &bytes[..8] != MAGICO_CONFIGURACION {
        return Err(ErrorConfiguracion::MagicoAusente);
    }

    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    if version != VERSION_CONFIGURACION {
        return Err(ErrorConfiguracion::VersionDesconocida {
            encontrada: version,
        });
    }

    let mut secuencia_bytes = [0u8; 8];
    secuencia_bytes.copy_from_slice(&bytes[10..18]);
    let secuencia = u64::from_be_bytes(secuencia_bytes);

    // El techo, antes que nada de lo que cuesta. Es malformacion, no politica:
    // ver `SecuenciaFueraDeRango`.
    if secuencia >= TECHO_SECUENCIA {
        return Err(ErrorConfiguracion::SecuenciaFueraDeRango {
            declarada: secuencia,
        });
    }

    let codigo = bytes[18];
    let Some(perfil) = perfil_desde_codigo(codigo) else {
        return Err(ErrorConfiguracion::PerfilDesconocido { codigo });
    };

    let mut intervalo_bytes = [0u8; 8];
    intervalo_bytes.copy_from_slice(&bytes[19..27]);
    let intervalo_latido_ms = u64::from_be_bytes(intervalo_bytes);
    if intervalo_latido_ms == 0 {
        return Err(ErrorConfiguracion::IntervaloImposible {
            encontrado: intervalo_latido_ms,
        });
    }

    let grupo_ipc = match bytes[27] {
        0 => None,
        _ => {
            let mut gid = [0u8; 4];
            gid.copy_from_slice(&bytes[28..32]);
            Some(u32::from_be_bytes(gid))
        }
    };

    let mut posicion = LONGITUD_CABECERA;
    let mut textos: Vec<String> = Vec::with_capacity(CAMPOS_DE_TEXTO);

    for indice in 0..CAMPOS_DE_TEXTO {
        // El prefijo antes que nada, y solo si cabe.
        if posicion.saturating_add(2) > bytes.len() {
            return Err(ErrorConfiguracion::CampoImposible { indice });
        }
        let longitud = usize::from(u16::from_be_bytes([bytes[posicion], bytes[posicion + 1]]));
        posicion += 2;

        // El techo se comprueba **antes** de cortar: un prefijo que declare mas
        // de lo que hay no debe llegar a indexar nada.
        if longitud > MAXIMO_CAMPO || posicion.saturating_add(longitud) > bytes.len() {
            return Err(ErrorConfiguracion::CampoImposible { indice });
        }

        let Ok(texto) = std::str::from_utf8(&bytes[posicion..posicion + longitud]) else {
            return Err(ErrorConfiguracion::CampoNoEsTexto { indice });
        };
        textos.push(texto.to_owned());
        posicion += longitud;
    }

    // Lo que queda tiene que ser exactamente la firma. Una cola sobrante
    // admitiria dos lecturas del mismo fichero.
    let firma = FirmaHibrida::desde_bytes(&bytes[posicion..])
        .map_err(|_| ErrorConfiguracion::FirmaMalformada)?;

    let mut campos = textos.into_iter();
    let mut siguiente = || campos.next().unwrap_or_default();

    let valores = Valores {
        secuencia,
        interfaz: siguiente(),
        perfil,
        colector: siguiente(),
        intervalo_latido_ms,
        grupo_ipc,
        nombre: siguiente(),
        maquina_esperada: siguiente(),
    };

    verificar(clave.clave(), &mensaje_de_configuracion(&valores), &firma)
        .map_err(|_| ErrorConfiguracion::FirmaInvalida)?;

    // Despues de la firma, nunca antes: comparar contra un campo no autenticado
    // dejaria que un fichero inventado decidiera que nombre se compara.
    if valores.maquina_esperada != maquina {
        return Err(ErrorConfiguracion::MaquinaAjena {
            esperada: valores.maquina_esperada,
            encontrada: maquina.to_owned(),
        });
    }

    // Y la frescura al final, sobre una secuencia que la firma ya cubre.
    //
    // `<` y no `<=`: reponer la MISMA configuracion se admite, y hace falta que
    // se admita. `aceptar_configuracion` avanza la marca antes de obedecer, asi
    // que un corte de corriente entre las dos cosas deja la marca en N con la N
    // sin aplicar. Con `<=`, ese sensor no volveria a arrancar nunca.
    if let Some(aceptada) = aceptada.secuencia() {
        if valores.secuencia < aceptada {
            return Err(ErrorConfiguracion::SecuenciaRevertida {
                encontrada: valores.secuencia,
                aceptada,
            });
        }
    }

    Ok(valores)
}
