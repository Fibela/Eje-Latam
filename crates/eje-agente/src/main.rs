//! Punto de entrada del demonio Eje-Agente.
//!
//! RPT-020, PA-44.
//!
//! # Que hace y que no
//!
//! Recorre el camino completo por primera vez: **captura → observacion →
//! clasificacion → veredicto → condiciones**. Hasta ahora cada pieza estaba
//! verificada por separado y ninguna habia visto un paquete.
//!
//! Lo que **no** hace, y conviene decirlo antes de que alguien lo suponga:
//!
//! - **No carga inventario.** `arrancar` de RPT-017 exige dos claves y no existe
//!   aprovisionamiento que las entregue. El agente opera como primer arranque,
//!   que es el estado honesto: sin marcados, la clasificacion resuelve por
//!   segmento (RPT-009 §5).
//! - **No declara segmentos.** Sin manifiesto verificado, toda VLAN sale como
//!   `NoDeclarado` y ningun equipo sin marcado es contenible. La traduccion de
//!   etiqueta a declaracion ya no vive aqui —vivia, y por eso ninguna VLAN
//!   podia declararse limpia (RPT-022 §1)—; ahora la aporta
//!   [`EstadoArranque::declaracion_para`], que sin manifiesto no aporta nada.
//! - **No contiene nada.** Calcula el veredicto y lo imprime. La emision hacia un
//!   conmutador sigue bloqueada en PA-22.
//! - **No anexa a ALM-01.** Los manejadores de RPT-019 son PA-43.
//!
//! # Dos modos
//!
//! `--ciclos 1` —el valor por omision— es el recorrido de comprobacion de
//! siempre. `--ciclos 0` es el servicio continuo de RPT-034: observa, clasifica,
//! persiste **si algo cambio**, emite y atiende consultas, en ese orden y en
//! bucle.
//!
//! El modo por defecto no cambia. Convertir el agente en demonio por omision
//! habria alterado lo que hace un binario ya existente sin que nadie lo pidiera.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eje_agente::alertas::{CargaRegistro, apartar, cargar_desde};
use eje_agente::ciclo::{Ciclo, Observacion, Resultado};
use eje_agente::salida::{DespachoTcp, Emisor};
use eje_agente::servicio::{Escucha, Manejadores};
use eje_agente::{ConfiguracionAgente, VERSION};
use eje_captura::transporte::extraer;
use eje_captura::{DireccionEnlace, ErrorCaptura, FuentePasiva, abrir};
use guardian_cc::PerfilSegmento;
use guardian_cc::arranque::{ErrorArranque, EstadoArranque, RutasAlmacen, arrancar_con_almacen};
use guardian_cc::observacion::Protocolo;
use guardian_cc::proveedores::ProveedorHuella;

/// Plazo de espera por trama.
const PLAZO: Duration = Duration::from_millis(500);

/// Tramas a observar antes de resumir, si no se indica otra cosa.
const TRAMAS_POR_DEFECTO: u64 = 200;

/// Directorio de almacen por defecto, relativo al directorio de trabajo.
const ALMACEN_POR_DEFECTO: &str = "datos-eje";

/// Plazo para conectar y escribir al colector de syslog.
///
/// Corto a proposito: un colector que no responde no debe detener la
/// observacion. Lo que no sale queda en ALM-01 y la condicion lo dice.
const PLAZO_SYSLOG: Duration = Duration::from_secs(3);

/// Fallos del arranque del agente.
#[derive(Debug, thiserror::Error)]
enum ErrorAgente {
    /// Faltan argumentos o son incorrectos.
    #[error(
        "uso: eje-agente --interfaz <nombre> [--tramas <n>] [--perfil corporativo|ot] [--almacen <ruta>] [--syslog <host:puerto>] [--ciclos <n|0=continuo>] [--grupo-ipc <gid>]"
    )]
    Uso,

    /// La captura no pudo abrirse.
    #[error(transparent)]
    Captura(#[from] ErrorCaptura),

    /// El almacen local no se pudo leer.
    #[error(transparent)]
    Arranque(#[from] ErrorArranque),

    /// El registro de evidencia no se pudo leer.
    ///
    /// **Solo llega aqui un fallo del sistema de ficheros**, no un registro
    /// danado: eso lo resuelve [`CargaRegistro`] sin impedir el arranque. Si el
    /// disco no responde, en cambio, el agente no puede prometer que anexara
    /// nada, y arrancar fingiendo que si seria peor que no arrancar.
    #[error(transparent)]
    Evidencia(#[from] guardian_cc::disco::ErrorDisco),
}

/// Opciones de la linea de ordenes.
struct Opciones {
    interfaz: String,
    tramas: u64,
    perfil: PerfilSegmento,
    almacen: PathBuf,
    /// Colector de syslog, `host:puerto`. Sin el, la alerta no sale del equipo.
    syslog: Option<String>,
    /// Ciclos a ejecutar. `0` es servicio continuo (PA-67).
    ///
    /// Por defecto **uno**: el recorrido de comprobacion que existia antes sigue
    /// siendo el comportamiento de partida. Convertir el agente en demonio por
    /// omision habria cambiado lo que hace un binario ya existente sin que nadie
    /// lo pidiera.
    ciclos: u64,
    /// Grupo (numerico) autorizado a consultar por el socket. PA-82.
    ///
    /// Sin el, el socket queda en `0600` y solo el usuario del agente puede
    /// conectarse — lo que obliga a la consola a correr con `sudo` cuando el
    /// agente captura tramas (RPT-046 §11).
    ///
    /// Es numerico porque resolver un nombre exige `getgrnam`, fuera de la
    /// biblioteca estandar. El empaquetado conoce el grupo que crea. PA-84.
    grupo_ipc: Option<u32>,
}

/// Describe un estado de arranque para el operador.
///
/// Se escribe aqui y no en `guardian-cc` a proposito: el texto que ve una
/// persona es presentacion, y meterlo en la biblioteca invitaria a que alguna
/// decision se tomara comparando cadenas.
const fn describir(estado: &EstadoArranque) -> &'static str {
    match estado {
        EstadoArranque::Operativo(_) => "operativo, inventario verificado",
        EstadoArranque::PrimerArranque => "primer arranque, sin inventario todavia",
        EstadoArranque::SinClaveAprovisionada => "SIN CLAVE aprovisionada; nada se puede verificar",
        EstadoArranque::FormatoObsoleto { .. } => {
            "formato anterior; hay que reemitir el inventario"
        }
        EstadoArranque::Supresion { .. } => "SUPRESION: habia inventario y ya no esta",
        EstadoArranque::NoVerifica { .. } => "el inventario NO VERIFICA",
    }
}

fn leer_opciones() -> Result<Opciones, ErrorAgente> {
    let mut interfaz = None;
    let mut tramas = TRAMAS_POR_DEFECTO;
    let mut perfil = PerfilSegmento::Corporativo;
    let mut almacen = PathBuf::from(ALMACEN_POR_DEFECTO);
    let mut syslog = None;
    let mut ciclos = 1u64;
    let mut grupo_ipc = None;

    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let mut indice = 0;

    while indice < argumentos.len() {
        let clave = argumentos[indice].as_str();
        let Some(valor) = argumentos.get(indice + 1) else {
            return Err(ErrorAgente::Uso);
        };

        match clave {
            "--interfaz" => interfaz = Some(valor.clone()),
            "--tramas" => tramas = valor.parse().map_err(|_| ErrorAgente::Uso)?,
            "--perfil" => {
                perfil = match valor.as_str() {
                    "corporativo" => PerfilSegmento::Corporativo,
                    "ot" => PerfilSegmento::Ot,
                    _ => return Err(ErrorAgente::Uso),
                }
            }
            "--almacen" => almacen = PathBuf::from(valor),
            "--syslog" => syslog = Some(valor.clone()),
            "--ciclos" => ciclos = valor.parse().map_err(|_| ErrorAgente::Uso)?,
            "--grupo-ipc" => grupo_ipc = Some(valor.parse().map_err(|_| ErrorAgente::Uso)?),
            _ => return Err(ErrorAgente::Uso),
        }

        indice += 2;
    }

    Ok(Opciones {
        interfaz: interfaz.ok_or(ErrorAgente::Uso)?,
        tramas,
        perfil,
        almacen,
        syslog,
        ciclos,
        grupo_ipc,
    })
}

/// Protocolo que un puerto **sugiere**.
///
/// # Esto no es huella
///
/// Un puerto no prueba un protocolo. Modbus movido al 10502 se escapa, y
/// cualquiera puede abrir el 502 y hablar otra cosa.
///
/// Se admite porque alimenta una fuente **inferida**, y por RPT-009 §3 esas solo
/// pueden sugerir criticidad, nunca descartarla: un falso negativo deja el
/// dispositivo donde ya estaba, y un falso positivo lleva a ambiguedad y a un
/// humano. Ninguna de las dos direcciones concede permiso.
///
/// La taxonomia de verdad es trabajo de dominio (RPT-018 §8.1) y esta tabla
/// debera moverse a donde viva esa decision.
const fn protocolo_de(puerto: u16) -> Option<Protocolo> {
    match puerto {
        502 => Some(Protocolo::Modbus),
        20_000 => Some(Protocolo::Dnp3),
        2_575 => Some(Protocolo::Hl7),
        47_808 => Some(Protocolo::Bacnet),
        _ => None,
    }
}

/// Instante actual en segundos desde la epoca.
///
/// Un reloj anterior a la epoca devuelve `0`, con lo que toda vigencia se lee
/// como caducada. Es la direccion segura y la misma que aplican
/// `MarcadoVerificado::vigente_en` y `DeclaracionVlan::vigente_en`.
fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |transcurrido| transcurrido.as_secs())
}

/// Punto de entrada.
///
/// # Por que no devuelve `Result`
///
/// Devolverlo es lo comodo, y Rust imprime el error con **`Debug`**, no con
/// `Display`. El resultado era `Error: Uso` — el nombre de la variante— en lugar
/// de la linea de uso que `#[error(...)]` ya tenia escrita.
///
/// Costo dos rondas de diagnostico (RPT-046 §11.2, PA-85): un binario viejo
/// rechazaba `--grupo-ipc` y decia `Error: Uso`. Con el uso impreso se habria
/// visto de inmediato que la opcion no figuraba.
fn main() {
    if let Err(error) = ejecutar() {
        // `{error}` y no `{error:?}`: lo que se escribio para una persona debe
        // llegar a esa persona.
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn ejecutar() -> Result<(), ErrorAgente> {
    let opciones = leer_opciones()?;
    let configuracion = ConfiguracionAgente::para_segmento(opciones.perfil);

    println!("Eje-Agente {VERSION}");
    println!("Interfaz           : {}", opciones.interfaz);
    println!("Perfil de segmento : {:?}", configuracion.perfil);
    println!(
        "Capa B autorizada  : {}",
        configuracion.red.capa_b_autorizada
    );
    println!();

    // PA-49. El agente lee su propio estado del almacen: claves, centinela,
    // revocaciones e inventario. Hasta aqui construia `PrimerArranque` a mano
    // porque `arrancar` exigia dos claves que nadie le daba.
    let rutas = RutasAlmacen::nuevo(opciones.almacen.clone());
    let (estado, _centinela) = arrancar_con_almacen(&rutas)?;

    // PA-58. El registro se carga del disco ANTES de observar nada, para que las
    // alertas de esta ejecucion continuen la serie en lugar de reiniciarla.
    let carga = cargar_desde(&rutas.evidencia())?;
    let aviso_registro = describir_carga(&carga);

    if let CargaRegistro::ViolacionDetectada { .. } = carga {
        // Se aparta, no se borra. Si el renombrado falla se avisa y se sigue
        // observando: quedarse sin vigilancia por un fallo de disco seria un
        // dano cierto para evitar uno ya ocurrido.
        match apartar(&rutas.evidencia(), instante_utc()) {
            Ok(destino) => println!(
                "  !! El registro anterior se aparto en {}",
                destino.display()
            ),
            Err(error) => println!("  !! No se pudo apartar el registro danado: {error}"),
        }
    }

    let registro = carga.registro();

    // PA-42. Sin `--syslog` no hay emisor y la alerta no sale del equipo. Se
    // dice por pantalla en lugar de suponerse: un agente que no emite y no lo
    // anuncia parece uno que emite y nadie escucha.
    let emisor = opciones.syslog.as_ref().map(|destino| {
        Emisor::nuevo(
            DespachoTcp::nuevo(destino, PLAZO_SYSLOG),
            &opciones.interfaz,
        )
    });

    println!("Almacen            : {}", rutas.directorio().display());
    println!(
        "Salida de alertas  : {}",
        opciones
            .syslog
            .as_deref()
            .unwrap_or("NINGUNA; las alertas no salen del equipo")
    );
    println!("Estado de arranque : {}", describir(&estado));
    println!("Registro de alertas: {aviso_registro}");

    if estado.exige_alerta() {
        // Dos avisos distintos y no uno. Un formato obsoleto o una instalacion a
        // medias exigen accion administrativa; una supresion o una firma rota
        // exigen respuesta a incidente. Presentarlos igual es como se ensena a
        // un operador a ignorar el segundo.
        if estado.es_manipulacion() {
            println!("  !! MANIPULACION: alguien toco el almacen del agente");
        } else {
            println!("  !  Requiere accion del administrador");
        }
    }
    println!();

    // PA-66. La escucha se abre una vez, fuera del bucle: reabrirla en cada
    // ciclo dejaria una ventana en la que VIS-04 no encuentra a nadie.
    let acceso = opciones
        .grupo_ipc
        .map_or(eje_agente::servicio::Acceso::SoloPropietario, |gid| {
            eje_agente::servicio::Acceso::Grupo(gid)
        });

    let escucha = match Escucha::abrir(&rutas.socket(), acceso) {
        Ok(escucha) => {
            println!("Escucha local      : {}", escucha.ruta().display());
            Some(escucha)
        }
        Err(error) => {
            println!("Escucha local      : NO disponible ({error})");
            None
        }
    };

    // RPT-047, PA-81. `abrir` deja de propagarse con `?`.
    //
    // Antes, un fallo de captura mataba el proceso y se llevaba la escucha que
    // se acababa de abrir tres lineas mas arriba. El momento en que mas falta
    // hace la consola era justo aquel en que el agente ya no estaba.
    let mut fuente = match abrir(&opciones.interfaz) {
        Ok(fuente) => Some(fuente),
        Err(error) => {
            println!("Captura            : NO DISPONIBLE ({error})");
            println!("  !! ESTE SENSOR NO ESTA OBSERVANDO. Se reintenta cada vuelta.");
            None
        }
    };

    // PA-68. El ciclo vive en la biblioteca, donde se puede ejercitar N vueltas
    // en pruebas sin levantar un demonio. Aqui queda solo lo que exige una
    // tarjeta de red de verdad: capturar y presentar.
    let mut ciclo = Ciclo::nuevo(rutas.evidencia(), opciones.perfil, registro, emisor);
    let mut vueltas = 0u64;

    println!();

    loop {
        // El instante se toma en CADA vuelta. Calculado una sola vez antes del
        // bucle —como estaba— un demonio de dias congelaria el reloj y **ningun
        // marcado caducaria nunca** (RPT-036 §3). Ahora ademas se pasa como
        // argumento, con lo que congelarlo dejo de poder hacerse por descuido.
        let instante = ahora();

        // Reintento por vuelta. Una interfaz que aparece tarde —arranque del
        // sistema, cable reconectado— debe recuperarse sola, sin que nadie
        // reinicie el agente.
        if fuente.is_none() {
            if let Ok(recuperada) = abrir(&opciones.interfaz) {
                println!("Captura            : RESTABLECIDA");
                fuente = Some(recuperada);
            }
        }

        let mut observaciones: Vec<Observacion> = Vec::new();
        let mut ilegibles = 0u64;
        let mut perdida: Option<String> = None;
        let inicio = Instant::now();

        while (observaciones.len() as u64) < opciones.tramas {
            // La fuente puede no existir (nunca abrio) o desaparecer en marcha
            // (alguien retiro la interfaz). Ninguno de los dos casos propaga:
            // los dos se declaran y se reintentan.
            let intento = match fuente.as_mut() {
                Some(activa) => activa.siguiente(PLAZO),
                None => break,
            };

            let siguiente = match intento {
                Ok(valor) => valor,
                Err(error) => {
                    perdida = Some(error.to_string());
                    break;
                }
            };

            let Some(trama) = siguiente else {
                // Red silenciosa. No es fallo, pero tampoco conviene esperar
                // indefinidamente en un recorrido de comprobacion.
                println!("(sin tramas en {PLAZO:?}; se detiene la observacion)");
                break;
            };

            let Some(extraida) = extraer(&trama) else {
                ilegibles = ilegibles.saturating_add(1);
                continue;
            };

            let protocolo = extraida.transporte.and_then(|transporte| {
                let (origen, destino) = transporte.puertos();
                protocolo_de(destino).or_else(|| protocolo_de(origen))
            });

            observaciones.push(Observacion {
                origen: extraida.origen,
                protocolo,
                vlan: extraida.vlan,
            });
        }

        // La perdida del nucleo se traslada al almacen ANTES de concluir nada:
        // sin esto, menos protocolos vistos se leerian como ausencia de riesgo
        // (RPT-018 §4).
        //
        // Sin captura NO se fabrican estadisticas en cero. Cero descartes dice
        // «vista completa», y sin captura no hay vista completa: no hay vista.
        // La ausencia se representa como ausencia y `capturaNoDisponible` es la
        // que carga con el significado (RPT-047 §3).
        let estadisticas = match fuente.as_ref() {
            Some(activa) => Some(activa.estadisticas()?),
            None => None,
        };
        if estadisticas
            .as_ref()
            .is_some_and(eje_captura::Estadisticas::hay_perdida)
        {
            ciclo.anotar_perdida();
        }

        if let Some(detalle) = perdida {
            println!("Captura            : PERDIDA EN MARCHA ({detalle})");
            fuente = None;
        }

        // Despues de capturar y en TODAS las vueltas, no solo al cambiar.
        //
        // El ciclo no puede deducir esto del almacen: cero tramas con la
        // captura caida y cero tramas en una red tranquila son el MISMO dato.
        // Un estado que solo se fija al cambiar se queda pegado el dia que
        // alguien olvide el camino de vuelta.
        ciclo.declarar_captura(fuente.is_some());

        let mut vistos: BTreeMap<DireccionEnlace, u64> = BTreeMap::new();
        for observacion in &observaciones {
            *vistos.entry(observacion.origen).or_insert(0) += 1;
        }

        let resultado = ciclo.vuelta(&estado, &observaciones, instante, instante_utc());

        println!(
            "Tramas observadas  : {} en {:?}",
            observaciones.len(),
            inicio.elapsed()
        );
        println!("Tramas ilegibles   : {ilegibles}");
        match &estadisticas {
            Some(datos) => println!(
                "Descartes del nucleo: {} (vista {})",
                datos.descartadas,
                if datos.hay_perdida() {
                    "INCOMPLETA"
                } else {
                    "completa"
                }
            ),
            // Ni «0» ni «completa»: no se sabe, porque no se miro.
            None => println!("Descartes del nucleo: SIN CAPTURA (no hay vista)"),
        }
        println!("Dispositivos       : {}", vistos.len());
        println!();

        for (mac, cuantas) in vistos.iter().take(20) {
            let indicio = ciclo.almacen().indicio(mac).map_or_else(
                |error| format!("error: {error}"),
                |valor| format!("{valor:?}"),
            );

            println!("  {mac:02x?}  tramas={cuantas:<5}  {indicio}");
        }

        if vistos.len() > 20 {
            println!("  ... y {} mas", vistos.len() - 20);
        }

        println!();
        println!(
            "Almacen: {} volatiles, {} pegajosos, saturado={}",
            ciclo.almacen().volatiles(),
            ciclo.almacen().pegajosos(),
            ciclo.almacen().pegajoso_saturado()
        );
        println!();

        presentar(&resultado, ciclo.evidencia());

        // Al final del ciclo, sobre lo ya persistido (RPT-034 §4). Una consulta
        // nunca responde con lo que aun vive solo en memoria.
        if let Some(escucha) = &escucha {
            let atendidas = escucha.atender(&mut Manejadores {
                registro: ciclo.registro(),
                condiciones: &resultado.condiciones,
                evidencia: ciclo.evidencia(),
            });
            if atendidas > 0 {
                println!("Consultas atendidas: {atendidas}");
            }
        }

        vueltas = vueltas.saturating_add(1);
        if opciones.ciclos != 0 && vueltas >= opciones.ciclos {
            break;
        }
        println!();
    }

    Ok(())
}

/// Describe el resultado de cargar el registro, para el operador.
const fn describir_carga(carga: &CargaRegistro) -> &'static str {
    match carga {
        CargaRegistro::Conforme(_) => "verifica",
        CargaRegistro::Truncado { .. } => "TRUNCADO; se perdio la cola por un corte",
        CargaRegistro::ViolacionDetectada { .. } => "NO VERIFICA: alguien lo toco",
    }
}

/// Imprime lo que la vuelta produjo.
///
/// Presentacion y nada mas. La decision ya la tomo [`Ciclo::vuelta`]; separarlo
/// es lo que permite que el ciclo se pruebe sin capturar `stdout`.
fn presentar(resultado: &Resultado, evidencia: &std::path::Path) {
    println!("  Con marcado firmado          : {}", resultado.con_marcado);
    println!("  Contenibles sin intervencion : {}", resultado.contenibles);
    println!("  Requieren humano o prohibidos: {}", resultado.escalados);
    println!();

    println!(
        "  Alertas anexadas a ALM-01    : {}",
        resultado.anexadas.len()
    );
    for suceso in resultado.anexadas.iter().take(5) {
        println!(
            "    asiento {:<4} {}  {:?}",
            suceso.asiento, suceso.dispositivo, suceso.clase
        );
    }
    if resultado.anexadas.len() > 5 {
        println!("    ... y {} mas", resultado.anexadas.len() - 5);
    }

    if resultado.persistido {
        println!("  Registro persistido          : {}", evidencia.display());
    }
    if resultado.sellado {
        // PA-64. El extremo salio de la maquina. Es lo unico que un equipo
        // comprometido no puede deshacer despues.
        println!("  Extremo atestiguado fuera    : si");
    }
    if let Some(archivado) = &resultado.rotado {
        println!("  Segmento archivado           : {}", archivado.display());
    }
    if let Some(motivo) = &resultado.fallo_persistencia {
        println!("  !! No se pudo persistir el registro: {motivo}");
        println!("     Las alertas de esta vuelta NO sobreviven al reinicio.");
    }

    let estados = &resultado.condiciones;
    println!();
    println!("  Condiciones vigentes:");
    println!(
        "    inventario suprimido      : {}",
        estados.inventario_suprimido
    );
    println!(
        "    inventario no verifica    : {}",
        estados.inventario_no_verifica
    );
    println!(
        "    observacion saturada      : {}",
        estados.observacion_saturada
    );
    println!(
        "    captura con perdida       : {}",
        estados.captura_con_perdida
    );
    println!(
        "    accion administrativa     : {}",
        estados.accion_administrativa
    );
    println!(
        "    salida no disponible      : {}",
        estados.salida_no_disponible
    );
    println!(
        "    registro saturado         : {}",
        estados.registro_saturado
    );

    if estados.registro_saturado {
        println!();
        println!("  !! El registro esta LLENO. Este sensor ya no anota amenazas.");
        println!("     No es manipulacion: nadie toco nada. Hace falta rotar el");
        println!("     registro para que vuelva a registrar (PA-59).");
    }
    if resultado.perdidas > 0 {
        println!(
            "  !! {} amenazas detectadas en esta vuelta NO se pudieron anotar.",
            resultado.perdidas
        );
    }

    if estados.salida_no_disponible {
        println!();
        println!("  !! El SIEM del cliente NO se esta enterando de nada.");
        println!("     Las alertas siguen en ALM-01, que es para lo que existe.");
        println!("     Y el extremo del registro no esta saliendo del equipo: mientras");
        println!("     dure, un recorte del registro local no dejaria testigo (PA-64).");
    }
}

/// Instante actual en milisegundos desde la epoca, con signo.
///
/// ALM-01 lo quiere con signo y en milisegundos. Un reloj anterior a la epoca
/// produce un valor negativo, que es representable y honesto; devolver 0 seria
/// inventar una fecha.
fn instante_utc() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |transcurrido| {
            i64::try_from(transcurrido.as_millis()).unwrap_or(i64::MAX)
        })
}
