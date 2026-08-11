//! Manejadores de los dos canales de alerta.
//!
//! RPT-028, PA-43.
//!
//! # Por que viven aqui
//!
//! `SucesoAlerta` esta en `eje-ipc` —es el contrato de cable— y
//! `RegistroEvidencia` en `eje-almacen`. `guardian-cc` no conoce ninguno de los
//! dos: decide, no comunica.
//!
//! El agente es el unico que tiene las dos mitades, asi que la traduccion vive
//! aqui. Meterla en `guardian-cc` habria obligado a que la biblioteca de
//! decision dependiera del formato de cable, y con eso un cambio de la interfaz
//! podria arrastrar a la logica de contencion.
//!
//! # La conversion es de una sola direccion
//!
//! RPT-019 §7.3: `SucesoAlerta` lleva `asiento: u64` y **ese numero no es un
//! dato del suceso** — lo asigna ALM-01 al anexar. Aqui solo existe
//! [`suceso_desde`], que va del asiento al DTO.
//!
//! Lo que **no** se puede prometer: `SucesoAlerta` tiene campos publicos porque
//! serde los necesita, asi que cualquiera puede construir uno con un asiento
//! inventado. La garantia no es del tipo, es de este modulo: **el agente nunca
//! fabrica un asiento**. La prueba que lo sostiene comprueba que todo suceso que
//! sale de [`consultar`] corresponde a una entrada real del registro.

use std::path::{Path, PathBuf};

use eje_almacen::persistencia::{
    ASIENTOS_POR_SEGMENTO, Cotejo, ErrorPersistencia, LONGITUD_ANCLA, LONGITUD_MAXIMA, analizar,
    analizar_ancla, ancla_de, cotejar, serializar, serializar_ancla,
};
use eje_almacen::{Asiento, ClaseEvento, RegistroEvidencia};
use eje_ipc::mensajes::{ClaseAlerta, Condiciones, PeticionAlertas, SucesoAlerta};
use guardian_cc::arranque::EstadoArranque;
use guardian_cc::disco::{ErrorDisco, escribir_atomico, leer_hasta};
use guardian_cc::observacion::AlmacenObservacion;
use guardian_cc::{Veredicto, proveedores::DireccionEnlace};

/// Numero maximo de sucesos que devuelve una consulta.
///
/// Una consulta sin cota permitiria pedir el registro entero en un solo marco y
/// chocar contra el limite de `eje-ipc`, con lo que el consumidor no recibiria
/// **nada** en lugar de recibir un lote. Quien quiera mas continua desde el
/// ultimo asiento devuelto, que es justo para lo que existe `desdeAsiento`.
pub const SUCESOS_POR_CONSULTA: usize = 256;

/// Representacion textual de una direccion de enlace.
///
/// Se separa del formato de depuracion de Rust a proposito: `{:02x?}` produce
/// `[00, 1b, ...]`, que no es lo que un operador reconoce como una MAC.
#[must_use]
pub fn nombrar(mac: &DireccionEnlace) -> String {
    mac.iter()
        .map(|octeto| format!("{octeto:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Resultado de cargar el registro de evidencia del disco.
///
/// # El principio triestatico, aplicado a la evidencia
///
/// RPT-006 §4: un verificador que no distingue «no hay violacion» de «no pude
/// comprobarlo» miente. Aqui las tres respuestas son distintas y **ninguna se
/// colapsa en otra**:
///
/// | Estado | Que ocurrio | Que hace el agente |
/// |---|---|---|
/// | [`Self::Conforme`] | el fichero verifica, o no existe | continua la serie |
/// | [`Self::Truncado`] | corte de energia durante la escritura | continua, avisando |
/// | [`Self::ViolacionDetectada`] | alguien toco el fichero | **no lo toca**, empieza uno nuevo |
#[derive(Debug)]
pub enum CargaRegistro {
    /// El registro verifica. Un fichero ausente cuenta como registro vacio: es
    /// el primer arranque del agente y no hay nada que sospechar.
    Conforme(Box<RegistroEvidencia>),

    /// El fichero termina a medias. Es el defecto esperable de un corte de
    /// energia durante la escritura, no una alteracion.
    ///
    /// Se distingue a proposito: colapsarlo en violacion haria que cada corte de
    /// luz pareciera un ataque, y esa es la fatiga que la Fase 1 de PA-45
    /// existia para evitar.
    Truncado {
        /// Motivo, para el registro forense.
        detalle: String,
    },

    /// El fichero esta y no verifica. **Alguien lo toco.**
    ViolacionDetectada {
        /// Motivo.
        detalle: String,
    },
}

impl CargaRegistro {
    /// Registro utilizable, o uno vacio si no se pudo confiar en el fichero.
    ///
    /// # Por que un registro vacio y no el contenido parcial
    ///
    /// Ante una violacion, cargar los asientos que si verificaban seria peor que
    /// no cargar nada: quien borro evidencia elegiria **que se conserva**, y el
    /// operador veria un registro que parece integro.
    #[must_use]
    pub fn registro(self) -> RegistroEvidencia {
        match self {
            Self::Conforme(registro) => *registro,
            Self::Truncado { .. } | Self::ViolacionDetectada { .. } => RegistroEvidencia::nuevo(),
        }
    }
}

/// Carga el registro de evidencia, distinguiendo los tres estados.
///
/// # El fichero danado **no se borra ni se sobrescribe**
///
/// Es la decision que mas importa de este modulo. Un registro que no verifica es
/// evidencia de que alguien intervino, y **esa evidencia vale mas que la que
/// contiene**. Quien lo pisara para «arrancar limpio» destruiria la unica prueba
/// de la manipulacion.
///
/// Quien llama debe conservarlo y anexar en otro sitio. La politica esta en
/// [`ruta_apartada`].
#[must_use]
pub fn cargar_registro(bytes: Option<&[u8]>) -> CargaRegistro {
    let Some(bytes) = bytes else {
        // Ausente es el primer arranque. A diferencia del inventario, aqui no
        // hay centinela que atestigue que hubo algo antes, y no se inventa uno:
        // afirmar manipulacion sin testigo seria acusar sin pruebas.
        return CargaRegistro::Conforme(Box::new(RegistroEvidencia::nuevo()));
    };

    match analizar(bytes) {
        Ok(registro) => CargaRegistro::Conforme(Box::new(registro)),

        Err(error @ ErrorPersistencia::Truncado { .. }) => CargaRegistro::Truncado {
            detalle: error.to_string(),
        },

        Err(error) => CargaRegistro::ViolacionDetectada {
            detalle: error.to_string(),
        },
    }
}

/// Carga el registro desde una ruta concreta.
///
/// # La cota es la del registro, no la del inventario
///
/// `disco::leer` fijaba ocho megabytes —la del inventario— y un registro forense
/// crece hasta sesenta y cuatro. Reutilizarla sin mas habria rechazado ficheros
/// validos, y el rechazo habria dicho «excesivo», que un operador lee como
/// manipulacion. De ahi `leer_hasta` (RPT-030 §2).
///
/// # Errores
///
/// [`ErrorDisco`] ante fallo de lectura distinto de «no existe». El fichero
/// ausente **no** es error: es el primer arranque.
pub fn cargar_desde(ruta: &Path) -> Result<CargaRegistro, ErrorDisco> {
    let carga = leer_registro(ruta)?;

    // PA-57. El ancla vive fuera del registro, asi que cotejarla es lo unico que
    // ve una alteracion del ULTIMO asiento: dentro del fichero, la cadena se
    // reconstruye y siempre cuadra consigo misma.
    let CargaRegistro::Conforme(registro) = carga else {
        return Ok(carga);
    };

    let ruta_ancla = ruta.with_extension("anc");

    let ancla = match leer_hasta(&ruta_ancla, LONGITUD_ANCLA) {
        Ok(bytes) => match analizar_ancla(&bytes) {
            Ok(ancla) => ancla,
            // Un ancla corrupta no se degrada a «sin ancla»: corromper treinta
            // bytes seria la via para desactivar la comprobacion.
            Err(error) => {
                return Ok(CargaRegistro::ViolacionDetectada {
                    detalle: format!("el ancla de la evidencia no se puede leer: {error}"),
                });
            }
        },

        Err(ErrorDisco::NoExiste { .. }) => {
            // Ausente con registro vacio es el primer arranque. Ausente con
            // asientos dentro es que alguien la borro, que es lo que haria quien
            // pretende cortar la cola sin que se note.
            if registro.vacio() {
                return Ok(CargaRegistro::Conforme(registro));
            }
            return Ok(CargaRegistro::ViolacionDetectada {
                detalle: "hay evidencia y su ancla no esta".to_owned(),
            });
        }

        Err(error) => return Err(error),
    };

    Ok(match cotejar(&registro, &ancla) {
        Cotejo::Conforme => CargaRegistro::Conforme(registro),

        // Corte de energia entre escribir el registro y escribir el ancla. La
        // evidencia esta; solo su cola quedo sin cubrir.
        Cotejo::SinAnclar { posteriores } => CargaRegistro::Truncado {
            detalle: format!("{posteriores} asientos quedaron sin anclar"),
        },

        otro => CargaRegistro::ViolacionDetectada {
            detalle: format!("el registro no cuadra con su ancla: {otro:?}"),
        },
    })
}

/// Lee y analiza el registro, sin cotejar el ancla.
fn leer_registro(ruta: &Path) -> Result<CargaRegistro, ErrorDisco> {
    match leer_hasta(ruta, LONGITUD_MAXIMA) {
        Ok(bytes) => Ok(cargar_registro(Some(&bytes))),
        Err(ErrorDisco::NoExiste { .. }) => Ok(cargar_registro(None)),

        // Un fichero que excede la cota no se lee, y tampoco se declara
        // conforme. Es evidencia que no se puede comprobar, que es el tercer
        // estado de RPT-006 §4 y no se colapsa en ninguno de los otros dos.
        Err(error @ ErrorDisco::Excesivo { .. }) => Ok(CargaRegistro::ViolacionDetectada {
            detalle: error.to_string(),
        }),

        Err(error) => Err(error),
    }
}

/// Persiste el registro completo, de forma atomica.
///
/// # Errores
///
/// [`ErrorDisco`] si la escritura falla.
/// # El registro va **antes** que el ancla
///
/// Si muriera entre las dos escrituras, el orden decide que falsa alarma se
/// produce:
///
/// - **Ancla primero**: quedaria un ancla que cubre asientos que no estan en
///   disco, y eso se lee como `Truncado` — «alguien corto la cola de la
///   evidencia». Respuesta a incidente por un corte de luz.
/// - **Registro primero**: quedan asientos que el ancla no cubre, y eso se lee
///   como `SinAnclar`, que es un estado propio y no una acusacion.
///
/// La evidencia real pesa mas que su cobertura, y una falsa alarma de
/// manipulacion cuesta mas que una cola sin anclar.
///
/// # Errores
///
/// Nombre del fichero de un segmento archivado.
///
/// El indice se **deriva de la base** y no se guarda en ningun contador: un
/// contador aparte podria desincronizarse del contenido, y entonces dos
/// rotaciones escribirian el mismo nombre y una se comeria a la otra.
#[must_use]
pub fn ruta_de_segmento(activo: &Path, base: u64) -> PathBuf {
    let indice = base.saturating_sub(1) / ASIENTOS_POR_SEGMENTO as u64 + 1;

    let raiz = activo.file_stem().map_or_else(
        || std::ffi::OsString::from("evidencia"),
        std::ffi::OsStr::to_os_string,
    );

    let mut nombre = raiz;
    nombre.push(format!("-{indice:06}.alm"));
    activo.with_file_name(nombre)
}

/// Numero de asiento mas antiguo que sobrevive en el directorio.
///
/// RPT-041, PA-74. **Se lee del disco en cada llamada y no se cachea.**
///
/// # Por que no se cachea
///
/// Una cifra tomada al arrancar seguiria diciendo `1` despues de que alguien
/// borrara `evidencia-000001.alm` con el agente en marcha, y el agente afirmaria
/// que la evidencia esta disponible desde el asiento 1 cuando ese tramo ya no
/// existe. Es un dato que se queda obsoleto **en la direccion que oculta la
/// manipulacion**, que es la peor de las dos.
///
/// El coste es un `read_dir` sobre unas decenas de entradas, muy por debajo de
/// serializar las hasta 256 alertas de la propia respuesta.
///
/// # Si no se puede leer el directorio
///
/// Se devuelve la base del segmento activo, que es lo unico que consta con
/// certeza. Inventar un `1` afirmaria que hay histórico sin haberlo comprobado.
#[must_use]
pub fn primer_disponible(activo: &Path, registro: &RegistroEvidencia) -> u64 {
    let Some(directorio) = activo.parent() else {
        return registro.base();
    };
    let Ok(entradas) = std::fs::read_dir(directorio) else {
        return registro.base();
    };

    let raiz = activo.file_stem().unwrap_or_default().to_string_lossy();
    let prefijo = format!("{raiz}-");

    let mut minimo = registro.base();
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name();
        let nombre = nombre.to_string_lossy();
        if !nombre.starts_with(&prefijo) || !nombre.ends_with(".alm") {
            continue;
        }

        // El indice del nombre no vale: dice que segmento es, no en que asiento
        // empieza. La base se lee de la cabecera, que es la que manda.
        let Ok(bytes) = std::fs::read(entrada.path()) else {
            continue;
        };
        if let Ok(segmento) = analizar(&bytes) {
            minimo = minimo.min(segmento.base());
        }
    }

    minimo
}

/// Cierra el segmento activo si alcanzo su tamano y abre el siguiente.
///
/// RPT-040, PA-59. Devuelve la ruta del segmento archivado, o `None` si no
/// tocaba rotar.
///
/// # Son dos escrituras y el ancla no se toca
///
/// 1. Se escribe el segmento que se cierra, completo, con su nombre definitivo.
/// 2. Se sustituye el activo por uno vacio que arrastra la base y el extremo.
///
/// El ancla se queda como esta, y **es correcta en los dos momentos**: describe
/// el asiento *N* con su extremo, que es el ultimo del segmento cerrado y
/// tambien lo ultimo que consta para el nuevo, que esta vacio.
///
/// Un corte entre 1 y 2 deja el activo intacto y la rotacion se reintenta; un
/// corte despues de 2 deja un estado que el ancla vieja valida sin objecion. No
/// hay un tercer paso que pueda quedarse a medias porque no hay tercer paso.
///
/// # Errores
///
/// [`ErrorDisco`] si alguna escritura falla. Si falla la primera **no se toca el
/// activo**: perder el archivo y ademas vaciar el activo seria perder la misma
/// evidencia dos veces.
pub fn rotar_si_toca(
    ruta: &Path,
    registro: &mut RegistroEvidencia,
) -> Result<Option<PathBuf>, ErrorDisco> {
    if registro.longitud() < ASIENTOS_POR_SEGMENTO {
        return Ok(None);
    }

    let destino = ruta_de_segmento(ruta, registro.base());
    escribir_atomico(&destino, &serializar(registro))?;

    let siguiente =
        RegistroEvidencia::continuando(registro.ultimo_numero() + 1, registro.extremo());
    escribir_atomico(ruta, &serializar(&siguiente))?;
    *registro = siguiente;

    Ok(Some(destino))
}

/// [`ErrorDisco`] si alguna escritura falla.
pub fn persistir(ruta: &Path, registro: &RegistroEvidencia) -> Result<(), ErrorDisco> {
    escribir_atomico(ruta, &serializar(registro))?;

    let ruta_ancla = ruta.with_extension("anc");

    match ancla_de(registro) {
        Some(ancla) => escribir_atomico(&ruta_ancla, &serializar_ancla(&ancla)),

        // Un registro vacio no tiene extremo que anclar. Se retira el ancla
        // anterior si la habia: dejarla apuntando a un asiento que ya no existe
        // seria un truncamiento permanente e imaginario.
        None => {
            let _ = std::fs::remove_file(&ruta_ancla);
            Ok(())
        }
    }
}

/// Aparta un registro que no verifica y devuelve donde quedo.
///
/// # No se borra
///
/// Un registro que no verifica es evidencia de que alguien intervino, y esa
/// evidencia vale mas que la que contiene. Renombrar en lugar de sobrescribir
/// deja el agente listo para anexar de nuevo **sin destruir la prueba**.
///
/// # Errores
///
/// [`ErrorDisco`] si el renombrado falla. Que falle no debe impedir arrancar:
/// quien llama decide, y lo razonable es avisar y seguir observando.
pub fn apartar(ruta: &Path, instante_utc: i64) -> Result<PathBuf, ErrorDisco> {
    let destino = ruta_apartada(ruta, instante_utc);

    std::fs::rename(ruta, &destino).map_err(|error| ErrorDisco::Entrada {
        ruta: ruta.display().to_string(),
        detalle: error.to_string(),
    })?;

    // El ancla se aparta con el registro. Dejarla atras la volveria huerfana:
    // cubriria asientos que ya no estan en su sitio y **cada arranque
    // posterior leeria un truncamiento** que no ocurrio, para siempre.
    //
    // Se conserva junto al registro apartado porque tambien es evidencia: dice
    // cual era el extremo antes de que alguien tocara nada.
    let ancla = ruta.with_extension("anc");
    if ancla.exists() {
        let _ = std::fs::rename(&ancla, ruta_apartada(&ancla, instante_utc));
    }

    Ok(destino)
}

/// Ruta a la que se aparta un registro que no verifica.
///
/// Se aparta en lugar de borrarse, y el nombre lleva el instante para que dos
/// incidentes no se pisen entre si.
#[must_use]
pub fn ruta_apartada(original: &Path, instante_utc: i64) -> PathBuf {
    let mut nombre = original.file_name().map_or_else(
        || "registro".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );

    nombre.push_str(&format!(".violacion-{instante_utc}"));
    original.with_file_name(nombre)
}

/// Anexa una amenaza incontenible al registro de evidencia.
///
/// # Por que solo esta clase se anexa
///
/// De los tres centinelas de RPT-019 §1, **solo uno es un suceso**. Los otros
/// dos son condiciones —verdaderas hasta que alguien intervenga— y anotarlas
/// repetidamente inundaria ALM-01 con la misma noticia.
///
/// Devuelve el numero de asiento, o `None` si el registro esta lleno.
///
/// # Por que `None` y no un panico ni un cero
///
/// RPT-039 §1, PA-72. Que una alerta no quepa es grave y el tipo obliga a
/// mirarlo. Un `0` de relleno lo dejaria pasar por un asiento cualquiera, y un
/// panico apagaria el sensor entero por no poder anotar una linea.
pub fn anotar_incontenible(
    registro: &mut RegistroEvidencia,
    instante_utc: i64,
    mac: &DireccionEnlace,
    veredicto: &Veredicto,
) -> Option<u64> {
    let detalle = format!(
        "amenaza sobre dispositivo no contenible: {veredicto:?}. \
         Ninguna accion automatica es posible; la respuesta es humana"
    );

    registro
        .anexar(
            instante_utc,
            ClaseEvento::DeteccionAnomalia,
            &nombrar(mac),
            &detalle,
        )
        .ok()
        .map(|asiento| asiento.numero)
}

/// Traduce un asiento del registro a un suceso de alerta.
///
/// Devuelve `None` si el asiento no es de una clase que se comunique como
/// alerta. **No hay conversion inversa**: ver el encabezado del modulo.
#[must_use]
pub fn suceso_desde(asiento: &Asiento) -> Option<SucesoAlerta> {
    // La correspondencia es explicita y no exhaustiva a proposito: anadir una
    // clase de evento a ALM-01 no debe convertirla en alerta por omision.
    // Comunicar de mas al operador es la otra cara de la fatiga.
    let clase = match asiento.clase {
        ClaseEvento::DeteccionAnomalia => ClaseAlerta::AmenazaIncontenible,
        _ => return None,
    };

    Some(SucesoAlerta {
        asiento: asiento.numero,
        clase,
        dispositivo: asiento.nodo.clone(),
        detalle: asiento.detalle.clone(),
    })
}

/// Manejador de `consultar-alertas`.
///
/// # Por que es consulta y no empuje
///
/// El manifiesto lo declara `direccion = "consulta"`. Con empuje, un agente que
/// alerta mas rapido de lo que VIS-04 consume acumularia en una cola que alguien
/// tendria que acotar, y acotarla significa **descartar alertas**. Con consulta,
/// el registro es la cola y ya esta acotado por disco.
#[must_use]
pub fn consultar(registro: &RegistroEvidencia, peticion: &PeticionAlertas) -> Vec<SucesoAlerta> {
    registro
        .asientos()
        .iter()
        // Exclusivo: quien pide «desde el 7» ya tiene el 7.
        .filter(|asiento| asiento.numero > peticion.desde_asiento)
        .filter_map(suceso_desde)
        .take(SUCESOS_POR_CONSULTA)
        .collect()
}

/// Manejador de `obtener-condiciones`.
///
/// # Los cinco estados degradados salen de dos sitios
///
/// Tres del estado de arranque y dos del almacen de observacion. Ninguno se
/// guarda: se derivan en el momento de la consulta, porque una condicion
/// almacenada puede quedar desfasada respecto de lo que es cierto.
///
/// # `salida_no_disponible` se rellena despues
///
/// Sale en `false` a proposito. Es el **resultado** de intentar emitir, y emitir
/// necesita las condiciones: pedirlo aqui seria circular. Quien llama emite con
/// lo que devuelve esta funcion y despues fija el campo.
///
/// Que no se emita nunca por syslog hace que ese orden sea seguro: las
/// transiciones que se calculan no dependen del campo que aun no se conoce.
#[must_use]
pub fn condiciones(
    estado: &EstadoArranque,
    observacion: &AlmacenObservacion,
    registro: &RegistroEvidencia,
) -> Condiciones {
    Condiciones {
        salida_no_disponible: false,
        inventario_suprimido: matches!(estado, EstadoArranque::Supresion { .. }),
        inventario_no_verifica: matches!(estado, EstadoArranque::NoVerifica { .. }),
        observacion_saturada: observacion.pegajoso_saturado(),
        captura_con_perdida: observacion.hay_perdida(),

        // El campo que PA-43 obligo a anadir. `FormatoObsoleto` y
        // `SinClaveAprovisionada` exigen alerta y no son manipulacion, y sin
        // esto no tenian por donde llegar al operador: no son sucesos —no
        // ocurren, SON— y no cabian en las otras cuatro condiciones.
        //
        // Se deriva de los dos predicados en lugar de enumerar variantes: si
        // manana aparece un tercer estado con ese perfil, llega solo.
        accion_administrativa: estado.exige_alerta() && !estado.es_manipulacion(),

        // PA-72. Se deriva del registro y no se guarda, como las demas: una
        // condicion almacenada puede quedar desfasada respecto de lo que es
        // cierto, y esta cambia en el momento en que alguien rota el fichero.
        registro_saturado: registro.saturado(),

        // PA-69. Sale en `false` por el mismo motivo que `salida_no_disponible`:
        // es el resultado de intentar escribir, y quien llama lo rellena. Aqui no
        // se sabe si el fichero va por detras del registro en memoria.
        evidencia_en_riesgo: false,
    }
}
