//! Emisor de manifiestos. **No se despliega en el sensor.**
//!
//! RPT-026, PA-48.
//!
//! ```text
//! eje-manifiesto generar --semilla clave.sem --almacen datos-eje
//! eje-manifiesto emitir  --semilla clave.sem --entrada parque.toml \
//!                        --salida datos-eje/inventario.inv \
//!                        [--anterior datos-eje/inventario.inv]
//! ```
//!
//! # La frase de paso se lee de la entrada estandar y **se ve al teclearla**
//!
//! Ocultarla exige una dependencia mas para manejar el terminal, y traerla en el
//! mismo paso que Argon2id, TOML y la aleatoriedad del sistema habria mezclado
//! cuatro APIs sin verificar. Se anota como PA-53 y se avisa por pantalla, que es
//! mejor que ocultarlo.
//!
//! Se lee de la entrada estandar y **no de una variable de entorno**: en varios
//! sistemas el entorno de un proceso es legible por otros usuarios, y en casi
//! todos acaba en el historial del intérprete.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use eje_manifiesto::entrada::{ConfiguracionEntrada, Entrada};
use eje_manifiesto::fragmento::{
    analizar as analizar_fragmento, huella_de, reunir_verificando,
    serializar as serializar_fragmento,
};
use eje_manifiesto::reposo_semilla::{LONGITUD_SAL, abrir, sellar};
use eje_manifiesto::{Emisor, ErrorEmision};
use guardian_cc::arranque::{RutasAlmacen, aprovisionar_clave};
use guardian_cc::clave::analizar as analizar_clave;
use guardian_cc::configuracion;
use guardian_cc::inventario::DominioClave;
use guardian_cc::revocacion::{
    Anotacion, ArchivoRevocaciones, CertificadoRevocacion, mensaje_de_certificado,
};
use motor_pqc::firma_hibrida::firmar;
use motor_pqc::reparto::{CUSTODIOS, UMBRAL, repartir};
use motor_pqc::reposo::LONGITUD_NONCE;
use motor_pqc::secreto::Secreto;
use motor_pqc::semilla::{LONGITUD_SEMILLA, SemillaFirma, derivar_par, derivar_verificacion};

/// Fallos de la herramienta.
#[derive(Debug, thiserror::Error)]
enum ErrorHerramienta {
    /// Argumentos incorrectos.
    #[error(
        "uso:\n  \
         eje-manifiesto generar      --semilla <fichero> --almacen <directorio>\n  \
         eje-manifiesto emitir       --semilla <fichero> --entrada <toml> --salida <inv> \
         [--anterior <inv>]\n  \
         eje-manifiesto configurar   --semilla <fichero> --entrada <toml> --salida <cfg> \
         [--anterior <cfg>]\n  \
         eje-manifiesto recuperacion --fragmentos <prefijo> --almacen <directorio>\n  \
         eje-manifiesto revocar      --fragmento-uno <frg> --fragmento-dos <frg> \
         --almacen <directorio> --sucesora <pub> --corte <n>"
    )]
    Uso,

    /// Un fragmento de la clave de recuperacion no es valido.
    #[error(transparent)]
    Fragmento(#[from] eje_manifiesto::fragmento::ErrorFragmento),

    /// Fallo de entrada/salida.
    #[error("{ruta}: {fuente}")]
    Fichero {
        /// Ruta implicada.
        ruta: String,
        /// Causa.
        fuente: std::io::Error,
    },

    /// El sistema no pudo entregar aleatoriedad.
    ///
    /// **No hay respaldo.** Un generador de reserva escrito por nosotros seria
    /// peor que fallar, porque produciria claves con la apariencia de buenas.
    #[error("el sistema no entrego aleatoriedad; no se genera ninguna clave")]
    SinAleatoriedad,

    /// La semilla no se pudo abrir o sellar.
    #[error(transparent)]
    Semilla(#[from] eje_manifiesto::reposo_semilla::ErrorSemilla),

    /// La entrada del administrador no es valida.
    #[error(transparent)]
    Entrada(#[from] eje_manifiesto::entrada::ErrorEntrada),

    /// La emision fallo.
    #[error(transparent)]
    Emision(#[from] ErrorEmision),

    /// La configuracion anterior no se pudo leer o no verifica. RPT-074, PA-79.
    ///
    /// Se propaga en lugar de tratarse como «no hay anterior»: caer a la serie
    /// desde uno ante un fichero ilegible dejaria que borrarlo bastara para
    /// rebobinar la secuencia, que es justo lo que la secuencia impide.
    #[error("la configuracion anterior no verifica: {0}")]
    ConfiguracionAnterior(#[from] guardian_cc::configuracion::ErrorConfiguracion),

    /// El aprovisionamiento de la clave publica fallo.
    #[error("no se pudo escribir la clave de verificacion: {detalle}")]
    Aprovisionamiento {
        /// Motivo.
        detalle: String,
    },
}

/// Lee bytes aleatorios del sistema.
fn aleatorio<const N: usize>() -> Result<[u8; N], ErrorHerramienta> {
    let mut bruto = [0u8; N];
    getrandom::fill(&mut bruto).map_err(|_| ErrorHerramienta::SinAleatoriedad)?;
    Ok(bruto)
}

fn leer(ruta: &Path) -> Result<Vec<u8>, ErrorHerramienta> {
    std::fs::read(ruta).map_err(|fuente| ErrorHerramienta::Fichero {
        ruta: ruta.display().to_string(),
        fuente,
    })
}

fn escribir(ruta: &Path, bytes: &[u8]) -> Result<(), ErrorHerramienta> {
    std::fs::write(ruta, bytes).map_err(|fuente| ErrorHerramienta::Fichero {
        ruta: ruta.display().to_string(),
        fuente,
    })
}

/// Lee las dos entradas de `emitir` y `configurar` **sin conocer secreto alguno**.
///
/// # PA-144, y por que esto es una funcion y no dos lineas movidas
///
/// `emitir` y `configurar` pedian la frase de paso ANTES de abrir la semilla.
/// Con una ruta equivocada, la herramienta imprimia el aviso de PA-53, se
/// tecleaba la frase en claro sobre la pantalla, y solo despues fallaba por un
/// fichero que no existia. **El secreto se quemo en una corrida que no podia
/// terminar bien.** Paso el 31 de agosto de 2026 aprovisionando la VM de PA-78 y
/// costo una frase de paso, que hubo que dar por comprometida.
///
/// `generar` ya lo hacia al reves —comprueba el fichero y luego pregunta—, asi
/// que el mismo programa tenia los dos ordenes a la vez.
///
/// Existiendo esta funcion, quien anada un tercer comando que abra una semilla
/// tiene delante el orden correcto ya escrito. Mover dos lineas habria arreglado
/// los dos sitios de hoy y ninguno de mañana.
///
/// # Errores
///
/// [`ErrorHerramienta::Fichero`] con la ruta que fallo, para que se vea cual de
/// las dos es.
fn entradas_sin_secreto(
    ruta_semilla: &Path,
    ruta_entrada: &Path,
) -> Result<(Vec<u8>, String), ErrorHerramienta> {
    let sellada = leer(ruta_semilla)?;
    let texto = String::from_utf8_lossy(&leer(ruta_entrada)?).into_owned();
    Ok((sellada, texto))
}

/// Pide la frase de paso por la entrada estandar.
fn pedir_frase(motivo: &str) -> Result<Vec<u8>, ErrorHerramienta> {
    eprintln!("Frase de paso ({motivo}), y Enter al terminar.");
    eprintln!("AVISO: se vera al teclearla; no la use delante de nadie (PA-53).");
    // RPT-082, PA-134. «Y Enter al terminar» es la mitad del arreglo que se ve.
    // El aviso decia como se VE la frase y no como se TERMINA, y con
    // `read_to_string` no terminaba nunca. Decir que hacer no cuesta nada; que
    // alguien lo averigue a la tercera, si.

    leer_frase(&mut std::io::stdin().lock())
}

/// Lee **una linea** de la entrada y la toma por frase de paso.
///
/// RPT-082, PA-134.
///
/// # El defecto que esto corrige
///
/// Usaba `read_to_string`, que lee **hasta el fin de la entrada** y no hasta el
/// salto de linea. Pulsar Enter no terminaba nada: la herramienta se quedaba
/// esperando para siempre, sin decir por que, y hacia falta un Ctrl-D que el
/// aviso no menciona. Costo tres intentos aprovisionando la VM de PA-78, y
/// durante dos parecio culpa de quien lo tecleaba.
///
/// Y tenia un segundo filo, peor: **cualquier linea pegada despues entraba en la
/// frase**. Pegar las dos ordenes del aprovisionamiento de golpe —`generar` y
/// `configurar`— habria cifrado la semilla con el texto de un comando, y no se
/// habria sabido hasta que `configurar` fallara.
///
/// # Por que una linea, y no lo que haya
///
/// Una frase de paso con un salto de linea dentro no se puede teclear, asi que
/// admitirla no compraba nada y era justo la puerta por la que entraba el texto
/// pegado. Se cortan `\r` y `\n` del final —y **solo** esos—: recortar espacios
/// alteraria en silencio un secreto que alguien eligio con ellos.
///
/// Sigue funcionando por tuberia: `printf '%s' 'frase' | eje-manifiesto ...`
/// entrega una linea sin salto final, y el fin de entrada la cierra igual.
///
/// # Errores
///
/// [`ErrorHerramienta::Fichero`] si la entrada no se puede leer, y tambien si se
/// cierra **sin entregar nada**. Cero bytes no es una frase vacia: es que nadie
/// llego a escribir. Colapsarlas dejaria que un guion mal encadenado sellara una
/// semilla sin que nadie hubiera decidido con que.
fn leer_frase(entrada: &mut impl std::io::BufRead) -> Result<Vec<u8>, ErrorHerramienta> {
    let mut texto = String::new();

    let leidos = entrada
        .read_line(&mut texto)
        .map_err(|fuente| ErrorHerramienta::Fichero {
            ruta: "<entrada estandar>".to_owned(),
            fuente,
        })?;

    if leidos == 0 {
        return Err(ErrorHerramienta::Fichero {
            ruta: "<entrada estandar>".to_owned(),
            fuente: std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "la entrada se cerro sin frase de paso",
            ),
        });
    }

    Ok(texto.trim_end_matches(['\r', '\n']).as_bytes().to_vec())
}

/// Instante actual, en segundos desde la epoca.
fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |transcurrido| transcurrido.as_secs())
}

/// Opciones leidas de la linea de ordenes.
#[derive(Default)]
struct Opciones {
    semilla: Option<PathBuf>,
    almacen: Option<PathBuf>,
    entrada: Option<PathBuf>,
    salida: Option<PathBuf>,
    anterior: Option<PathBuf>,
    fragmentos: Option<PathBuf>,
    fragmento_uno: Option<PathBuf>,
    fragmento_dos: Option<PathBuf>,
    sucesora: Option<PathBuf>,
    corte: u64,
}

fn leer_opciones(argumentos: &[String]) -> Result<Opciones, ErrorHerramienta> {
    let mut opciones = Opciones::default();
    let mut indice = 0;

    while indice < argumentos.len() {
        let Some(valor) = argumentos.get(indice + 1) else {
            return Err(ErrorHerramienta::Uso);
        };
        let ruta = Some(PathBuf::from(valor));

        match argumentos[indice].as_str() {
            "--semilla" => opciones.semilla = ruta,
            "--almacen" => opciones.almacen = ruta,
            "--entrada" => opciones.entrada = ruta,
            "--salida" => opciones.salida = ruta,
            "--anterior" => opciones.anterior = ruta,
            "--fragmentos" => opciones.fragmentos = ruta,
            "--fragmento-uno" => opciones.fragmento_uno = ruta,
            "--fragmento-dos" => opciones.fragmento_dos = ruta,
            "--sucesora" => opciones.sucesora = ruta,
            "--corte" => opciones.corte = valor.parse().map_err(|_| ErrorHerramienta::Uso)?,
            _ => return Err(ErrorHerramienta::Uso),
        }

        indice += 2;
    }

    Ok(opciones)
}

/// Crea una semilla nueva y aprovisiona la clave publica en el almacen.
fn generar(opciones: &Opciones) -> Result<(), ErrorHerramienta> {
    let (Some(ruta_semilla), Some(almacen)) = (&opciones.semilla, &opciones.almacen) else {
        return Err(ErrorHerramienta::Uso);
    };

    // Negarse a sobrescribir. Una semilla pisada deja huerfano todo lo firmado
    // con la anterior, y el agente lo leera como firma invalida: manipulacion.
    if ruta_semilla.exists() {
        return Err(ErrorHerramienta::Fichero {
            ruta: ruta_semilla.display().to_string(),
            fuente: std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "ya existe; sobrescribirla dejaria huerfano todo lo firmado",
            ),
        });
    }

    let frase = pedir_frase("nueva, para cifrar la semilla")?;

    let semilla: SemillaFirma = Secreto::nuevo(aleatorio::<LONGITUD_SEMILLA>()?);
    let emisor = Emisor::desde_semilla(Secreto::nuevo(*semilla.exponer()));

    let sellada = sellar(
        &semilla,
        &frase,
        aleatorio::<LONGITUD_SAL>()?,
        aleatorio::<LONGITUD_NONCE>()?,
    )?;

    escribir(ruta_semilla, &sellada)?;

    let rutas = RutasAlmacen::nuevo(almacen.clone());
    std::fs::create_dir_all(rutas.directorio()).map_err(|fuente| ErrorHerramienta::Fichero {
        ruta: rutas.directorio().display().to_string(),
        fuente,
    })?;

    aprovisionar_clave(
        &rutas.clave_operativa(),
        emisor.verificacion(),
        DominioClave::Cliente,
    )
    .map_err(|error| ErrorHerramienta::Aprovisionamiento {
        detalle: error.to_string(),
    })?;

    println!("Semilla cifrada en   : {}", ruta_semilla.display());
    println!(
        "Clave aprovisionada  : {}",
        rutas.clave_operativa().display()
    );
    println!();
    println!("Falta la clave de recuperacion (RPT-015 §4). Sin ella no se pueden");
    println!("leer certificados de revocacion, que es el unico remedio si esta");
    println!("semilla se compromete.");

    Ok(())
}

/// Emite un manifiesto firmado.
fn emitir(opciones: &Opciones) -> Result<(), ErrorHerramienta> {
    let (Some(ruta_semilla), Some(ruta_entrada), Some(salida)) =
        (&opciones.semilla, &opciones.entrada, &opciones.salida)
    else {
        return Err(ErrorHerramienta::Uso);
    };

    // PA-144. Todo lo que puede fallar sin secretos, antes del secreto.
    let (semilla_sellada, texto) = entradas_sin_secreto(ruta_semilla, ruta_entrada)?;
    let entrada = Entrada::analizar(&texto)?;

    let frase = pedir_frase("la de esta semilla")?;
    let emisor = Emisor::desde_semilla(abrir(&semilla_sellada, &frase)?);

    let instante = ahora();
    let marcados = entrada.marcados(instante)?;
    let segmentos = entrada.segmentos(instante)?;

    // El anterior se pasa si existe. `secuencia_siguiente` lo VERIFICA antes de
    // creerse su numero: un fichero editado no decide que se emite despues.
    let anterior = match &opciones.anterior {
        Some(ruta) if ruta.exists() => Some(leer(ruta)?),
        _ => None,
    };
    let secuencia = emisor.secuencia_siguiente(anterior.as_deref())?;

    let bytes = emisor.emitir(marcados, segmentos, secuencia)?;
    escribir(salida, &bytes)?;

    println!("Manifiesto emitido   : {}", salida.display());
    println!("Secuencia            : {secuencia}");
    println!("Marcados             : {}", entrada.marcado.len());
    println!("Segmentos declarados : {}", entrada.segmento.len());
    println!();
    println!("El agente lo aceptara solo si su centinela esta por debajo de {secuencia}.");

    Ok(())
}

/// Crea la clave de recuperacion y la reparte entre tres custodios.
///
/// # Por que es un comando aparte de `generar`
///
/// RPT-015 §4 separa las dos claves para que quien roba la operativa no pueda
/// revocar. Producirlas en el mismo comando las dejaria juntas en la misma
/// maquina y en el mismo instante, y **la separacion criptografica no sobrevive
/// a una separacion operativa que nadie hace**.
///
/// El secreto nunca se escribe entero: solo salen los tres fragmentos.
fn recuperacion(opciones: &Opciones) -> Result<(), ErrorHerramienta> {
    let (Some(prefijo), Some(almacen)) = (&opciones.fragmentos, &opciones.almacen) else {
        return Err(ErrorHerramienta::Uso);
    };

    let semilla: SemillaFirma = Secreto::nuevo(aleatorio::<LONGITUD_SEMILLA>()?);
    let huella = huella_de(Secreto::nuevo(*semilla.exponer()));
    let verificacion = derivar_verificacion(Secreto::nuevo(*semilla.exponer()));

    let partes = repartir(&semilla, &aleatorio::<LONGITUD_SEMILLA>()?);

    for parte in &partes {
        let ruta = prefijo.with_extension(format!("{}.frg", parte.indice));

        if ruta.exists() {
            return Err(ErrorHerramienta::Fichero {
                ruta: ruta.display().to_string(),
                fuente: std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "ya existe; sobrescribir un fragmento inutiliza el reparto entero",
                ),
            });
        }

        escribir(&ruta, &serializar_fragmento(parte, &huella))?;
        println!(
            "Fragmento {} de {CUSTODIOS} : {}",
            parte.indice,
            ruta.display()
        );
    }

    let rutas = RutasAlmacen::nuevo(almacen.clone());
    std::fs::create_dir_all(rutas.directorio()).map_err(|fuente| ErrorHerramienta::Fichero {
        ruta: rutas.directorio().display().to_string(),
        fuente,
    })?;

    aprovisionar_clave(
        &rutas.clave_recuperacion(),
        &verificacion,
        DominioClave::ClienteRecuperacion,
    )
    .map_err(|error| ErrorHerramienta::Aprovisionamiento {
        detalle: error.to_string(),
    })?;

    println!(
        "Clave aprovisionada  : {}",
        rutas.clave_recuperacion().display()
    );
    println!();
    println!("Hacen falta {UMBRAL} de {CUSTODIOS} fragmentos para reconstruir. Reparta los");
    println!("tres AHORA y borre los tres de esta maquina: juntos son la clave.");
    println!();
    println!("RPT-015 §8.1: si los tres custodios son de la misma organizacion, el");
    println!("umbral real frente a un interno es menor que {UMBRAL}-de-{CUSTODIOS}.");

    Ok(())
}

/// Emite un certificado de revocacion firmado con la clave de recuperacion.
fn revocar(opciones: &Opciones) -> Result<(), ErrorHerramienta> {
    let (Some(uno), Some(otro), Some(almacen), Some(sucesora)) = (
        &opciones.fragmento_uno,
        &opciones.fragmento_dos,
        &opciones.almacen,
        &opciones.sucesora,
    ) else {
        return Err(ErrorHerramienta::Uso);
    };

    let semilla = reunir_verificando(
        &analizar_fragmento(&leer(uno)?)?,
        &analizar_fragmento(&leer(otro)?)?,
    )?;

    let rutas = RutasAlmacen::nuevo(almacen.clone());
    let revocada = analizar_clave(&leer(&rutas.clave_operativa())?, DominioClave::Cliente)
        .map_err(|error| ErrorHerramienta::Aprovisionamiento {
            detalle: error.to_string(),
        })?;
    let nueva = analizar_clave(&leer(sucesora)?, DominioClave::Cliente).map_err(|error| {
        ErrorHerramienta::Aprovisionamiento {
            detalle: error.to_string(),
        }
    })?;

    let certificado = CertificadoRevocacion {
        revocada: revocada.identificador(),
        hasta_secuencia: opciones.corte,
        sucesora: nueva.identificador(),
        emitido_en: ahora(),
    };

    let (firmante, verificacion) = derivar_par(semilla);
    let firma = firmar(&firmante, &mensaje_de_certificado(&certificado));

    // Se anexa al registro existente en lugar de sustituirlo. Reescribirlo
    // borraria revocaciones anteriores, y una revocacion que desaparece es
    // exactamente lo que RPT-015 impide que ocurra en silencio.
    let clave_lectora = guardian_cc::inventario::ClaveInventario::nueva(
        verificacion,
        DominioClave::ClienteRecuperacion,
    );

    let mut archivo = match std::fs::read(rutas.revocaciones()) {
        Ok(bytes) => ArchivoRevocaciones::analizar(&bytes, &clave_lectora).map_err(|error| {
            ErrorHerramienta::Aprovisionamiento {
                detalle: format!("el registro existente no verifica: {error}"),
            }
        })?,
        Err(_) => ArchivoRevocaciones::nuevo(),
    };

    archivo.anotar(Anotacion { certificado, firma });
    escribir(&rutas.revocaciones(), &archivo.serializar())?;

    println!("Certificado emitido  : {}", rutas.revocaciones().display());
    println!("Corte de secuencia   : {}", opciones.corte);
    println!("Anotaciones en total : {}", archivo.anotaciones().len());
    println!();
    println!("El agente dejara de aceptar lo que la clave anterior firme POR ENCIMA");
    println!(
        "de {}. Lo de por debajo sigue valiendo (RPT-015 §3).",
        opciones.corte
    );

    Ok(())
}

/// Emite la configuracion firmada de un sensor. RPT-074, PA-79.
///
/// # La secuencia no se escribe: se deduce de la anterior **verificada**
///
/// Un numero puesto a mano en el TOML permitiria retroceder la serie —volver a
/// emitir la configuracion de la semana pasada, la del intervalo largo— sin que
/// nada lo notara. Se lee la anterior con la clave de este mismo emisor y se
/// suma uno; si no hay anterior, la serie empieza en uno.
///
/// # Y la anterior se verifica antes de creerle el numero
///
/// Es lo mismo que `secuencia_siguiente` hace con el inventario. Un fichero
/// editado no decide que se emite despues.
fn configurar(opciones: &Opciones) -> Result<(), ErrorHerramienta> {
    let (Some(ruta_semilla), Some(ruta_entrada), Some(salida)) =
        (&opciones.semilla, &opciones.entrada, &opciones.salida)
    else {
        return Err(ErrorHerramienta::Uso);
    };

    // PA-144. Todo lo que puede fallar sin secretos, antes del secreto.
    let (semilla_sellada, texto) = entradas_sin_secreto(ruta_semilla, ruta_entrada)?;
    let entrada = ConfiguracionEntrada::analizar(&texto)?;

    let frase = pedir_frase("la de esta semilla")?;
    let emisor = Emisor::desde_semilla(abrir(&semilla_sellada, &frase)?);

    // La secuencia de la anterior, si existe. Se verifica contra **esta misma
    // maquina**: una configuracion de otro sensor no puede continuar esta serie,
    // y `analizar` ya lo comprueba.
    let anterior = match &opciones.anterior {
        Some(ruta) if ruta.exists() => {
            let previa = configuracion::analizar(
                &leer(ruta)?,
                &emisor.como_clave_de_cliente(),
                &entrada.maquina,
                // Sin marca: la frescura es del SENSOR, y aqui no hay ninguno.
                // El emisor no la puede consultar —vive en el almacen del sensor,
                // a un pais de distancia— y comparar contra una inventada seria
                // peor que no comparar. Lo que hace este comando es **avanzar** la
                // serie desde la anterior; quien la comprueba es el agente
                // (RPT-078, RPT-074 §5).
                guardian_cc::inventario::Centinela::SinEstablecer,
            )?;
            Some(previa.secuencia)
        }
        _ => None,
    };

    let secuencia = match anterior {
        Some(previa) => previa.checked_add(1).ok_or(ErrorHerramienta::Uso)?,
        None => 1,
    };

    let valores = entrada.valores(secuencia)?;
    let firma = emisor.firmar_configuracion(&valores);
    escribir(salida, &configuracion::serializar(&valores, &firma))?;

    println!("Configuracion firmada : {}", salida.display());
    println!("Valida solo en        : {}", valores.maquina_esperada);
    println!("Identidad en la sala  : {}", valores.nombre);
    println!("Interfaz              : {}", valores.interfaz);
    println!("Intervalo de latido   : {} ms", valores.intervalo_latido_ms);
    println!("Secuencia             : {secuencia}");
    println!();

    if valores.colector.is_empty() {
        println!("  !! SIN COLECTOR. Este sensor no informara a ninguna sala, y");
        println!("     nadie fuera notara si se apaga (RPT-054 §4.1).");
        println!();
    }

    println!(
        "El agente la aceptara solo en '{}'.",
        valores.maquina_esperada
    );

    Ok(())
}

fn main() -> Result<(), ErrorHerramienta> {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let Some((orden, resto)) = argumentos.split_first() else {
        return Err(ErrorHerramienta::Uso);
    };

    let opciones = leer_opciones(resto)?;

    match orden.as_str() {
        "generar" => generar(&opciones),
        "emitir" => emitir(&opciones),
        "configurar" => configurar(&opciones),
        "recuperacion" => recuperacion(&opciones),
        "revocar" => revocar(&opciones),
        _ => Err(ErrorHerramienta::Uso),
    }
}

#[cfg(test)]
mod pruebas_frase {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::leer_frase;

    /// La frase termina en el **salto de linea**, no al cerrar la entrada.
    ///
    /// RPT-082, PA-134. Es el defecto entero: con `read_to_string`, pulsar Enter
    /// no terminaba nada y la herramienta se colgaba sin decir por que.
    #[test]
    fn una_linea_termina_la_frase() {
        let mut entrada = &b"prueba-pa78-eje\nesto ya no es la frase\n"[..];

        assert_eq!(
            leer_frase(&mut entrada).expect("una linea es una frase"),
            b"prueba-pa78-eje"
        );
    }

    /// Y lo que venga detras **no entra**.
    ///
    /// El segundo filo del defecto, y el peor: pegar las dos ordenes del
    /// aprovisionamiento de golpe habria cifrado la semilla con el texto de un
    /// comando. Esta prueba lo reproduce con la orden exacta que se pego aquel
    /// dia.
    #[test]
    fn una_orden_pegada_detras_no_se_convierte_en_la_frase() {
        let pegado = b"mi-frase\n/usr/local/bin/eje-manifiesto configurar --semilla cliente.sem\n";

        let frase = leer_frase(&mut &pegado[..]).expect("la primera linea es la frase");

        assert_eq!(frase, b"mi-frase");
        assert!(
            !String::from_utf8_lossy(&frase).contains("eje-manifiesto"),
            "la orden pegada entro en la frase de paso"
        );
    }

    /// Por tuberia, sin salto final, sigue funcionando.
    ///
    /// `printf '%s' 'frase' | ...` es como se desbloqueo el aprovisionamiento de
    /// la VM, y tiene que seguir valiendo: es la forma reproducible, la que no
    /// depende de que nadie recuerde pulsar Ctrl-D.
    #[test]
    fn una_tuberia_sin_salto_final_sigue_valiendo() {
        assert_eq!(
            leer_frase(&mut &b"sin-salto-final"[..]).expect("el fin de entrada cierra la linea"),
            b"sin-salto-final"
        );
    }

    /// Se cortan `\r` y `\n`, y **solo** esos.
    ///
    /// Recortar espacios alteraria en silencio un secreto que alguien eligio con
    /// ellos, y el fallo aparecería mucho despues, al no poder abrir la semilla.
    #[test]
    fn los_espacios_de_la_frase_se_respetan() {
        assert_eq!(
            leer_frase(&mut &b"con espacios al final   \r\n"[..]).expect("valida"),
            b"con espacios al final   "
        );
    }

    /// Una entrada cerrada sin nada **no es una frase vacia**.
    ///
    /// Cero bytes significa que nadie llego a escribir. Colapsarlo con la frase
    /// vacia dejaria que un guion mal encadenado sellara una semilla sin que
    /// nadie hubiera decidido con que (RPT-006 §4, una vez mas).
    #[test]
    fn una_entrada_cerrada_sin_nada_no_es_una_frase_vacia() {
        assert!(leer_frase(&mut &b""[..]).is_err());
        assert!(
            leer_frase(&mut &b"\n"[..]).is_ok(),
            "una linea vacia SI se leyo; que este vacia lo rechaza el sellado"
        );
    }
}
