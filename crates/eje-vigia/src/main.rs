//! Colector de referencia: escucha latidos y avisa de los que faltan.
//!
//! RPT-057, PA-105.
//!
//! # Este binario es fino a proposito
//!
//! Toda la decision vive en la biblioteca y se prueba sin socket. Aqui solo hay
//! transporte y reloj, que es lo que no se puede probar sin red y lo que falla de
//! forma ruidosa. Misma disciplina que `eje-agente`.
//!
//! # Por que escucha un puerto TCP y el agente no
//!
//! RPT-002 §9.3 prohibe puertos locales **en el sensor**, porque un servicio
//! alcanzable desde otra maquina es otro modelo de amenaza para un equipo que
//! esta en un armario de planta. Esto es el otro lado: un colector de syslog es
//! por definicion un servicio de red, y correrlo en la sala es lo que hace que la
//! ausencia sea observable.
//!
//! # Lo que NO es
//!
//! No es el SIEM del cliente ni pretende sustituirlo. Es la implementacion mas
//! pequena que permite apagar un sensor y comprobar que alguien se entera, y una
//! especificacion ejecutable para quien lo implemente en su herramienta.

use std::io::Read as _;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use eje_vigia::sellos::{Cotejo, Testigo};
use eje_vigia::{Acogida, Identidad, Vigia, Vigilancia, analizar, sellos};

/// Cada cuanto se revisa quien falta.
const CADENCIA_REVISION: Duration = Duration::from_secs(5);

/// Techo de una linea de syslog, para no crecer sin limite con basura.
const LINEA_MAXIMA: usize = 64 * 1024;

fn ahora_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |desde| {
            i64::try_from(desde.as_millis()).unwrap_or(i64::MAX)
        })
}

/// Linea de uso del vigia.
///
/// `--escuchar` va **sin corchetes**: es obligatoria. RPT-075, PA-128.
fn uso() {
    eprintln!("uso: eje-vigia --escuchar DIR:PUERTO [--esperar MAQUINA[/INTERFAZ]]...");
    eprintln!();
    // Sin direccion de ejemplo, y no por el guardian: RPT-054 §4.1 ya decidio
    // que un valor de ejemplo es peor que ninguno, porque el ejemplo acaba
    // siendo el despliegue. Se dice que decidir, no que teclear.
    eprintln!("  --escuchar  donde se expone el colector. Una direccion de bucle local");
    eprintln!("              para pruebas; una alcanzable para una sala de verdad.");
    eprintln!("              No hay valor por omision: exponerse es una decision.");
    eprintln!("  --esperar   sensor que se espera ver. Repetible. Sin censo solo se");
    eprintln!("              detecta «se apago», nunca «nunca arranco».");
}

fn main() -> std::process::ExitCode {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();

    // RPT-075, PA-128. **No hay valor por omision, y no es por el linter.**
    //
    // Esta cadena decide en que interfaz escucha un servicio de red. Una
    // direccion de bucle local por omision funciona en la maquina de quien la
    // escribio, y se convierte en «todas las interfaces» el dia que alguien
    // quiere que le llegue el trafico de otro equipo — y entonces el colector de
    // la sala queda expuesto a toda la red del cliente sin que nadie lo haya
    // decidido.
    //
    // Es exactamente el mismo descuido que la regla de reenvio de puertos con la
    // IP anfitriona vacia (RPT-069 §7), y se cierra igual que `EJE_INTERFAZ` en
    // el agente: obligando a declararlo.
    let mut escucha_en: Option<String> = None;
    let mut censo: Vec<String> = Vec::new();

    let mut resto = argumentos.iter();
    while let Some(bandera) = resto.next() {
        match bandera.as_str() {
            "--escuchar" => {
                if let Some(valor) = resto.next() {
                    escucha_en = Some(valor.clone());
                }
            }
            // Sin censo solo se cubre «se apago». Con censo se cubre tambien
            // «nunca arranco», que es el caso que no se puede deducir de lo oido.
            "--esperar" => {
                if let Some(valor) = resto.next() {
                    censo.push(valor.clone());
                }
            }
            _ => {
                uso();
                return std::process::ExitCode::from(2);
            }
        }
    }

    let Some(escucha_en) = escucha_en else {
        eprintln!("falta --escuchar: un colector no puede elegir por su cuenta en que");
        eprintln!("interfaz se expone. Declara la direccion de forma explicita.");
        eprintln!();
        uso();
        return std::process::ExitCode::from(2);
    };

    let vigia = Arc::new(Mutex::new(Vigia::nuevo()));
    // RPT-061, PA-115. El testigo lleva la serie de extremos por **identidad**:
    // dos agentes de un mismo servidor perimetral no comparten registro y no
    // pueden compartir serie, o el cotejo los acusa de un recorte que no existe.
    let testigo = Arc::new(Mutex::new(Testigo::nuevo()));
    if let Ok(mut guardia) = vigia.lock() {
        for maquina in &censo {
            guardia.esperar(maquina);
        }
    }

    let servidor = match TcpListener::bind(&escucha_en) {
        Ok(servidor) => servidor,
        Err(error) => {
            eprintln!("no se pudo escuchar en {escucha_en}: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };

    println!("eje-vigia escuchando en {escucha_en}");
    if censo.is_empty() {
        println!("AVISO: sin censo. Se detectara el sensor que SE APAGA, no el que NUNCA ARRANCO.");
    } else {
        println!("Censo: {}", censo.join(", "));
    }

    // Una entrada del censo sin interfaz solo casa con un agente que **tampoco**
    // la declare. Es correcto —si `maquina` casara con cualquier interfaz, un
    // sensor vivo cubriria la muerte de su companero, que es justo el colapso que
    // PA-113 elimino— y es facil de escribir mal: `--esperar sensor` en lugar de
    // `--esperar sensor/eth0` produce una entrada que nunca se resuelve y se lee
    // como «ese sensor no ha hablado nunca».
    //
    // Se avisa aqui, al arrancar, porque el sintoma llega mucho despues y
    // disfrazado de otra cosa. Es la leccion de RPT-058 §2.
    for entrada in censo.iter().filter(|entrada| !entrada.contains('/')) {
        println!(
            "  AVISO: '{entrada}' no nombra interfaz. Solo casara con un agente \
             anterior a RPT-059 que no la declare."
        );
        println!("         Para un agente actual: --esperar {entrada}/<interfaz>");
    }

    let compartido = Arc::clone(&vigia);
    let compartido_testigo = Arc::clone(&testigo);
    std::thread::spawn(move || {
        for conexion in servidor.incoming() {
            let Ok(flujo) = conexion else { continue };
            let suyo = Arc::clone(&compartido);
            let suyo_testigo = Arc::clone(&compartido_testigo);
            std::thread::spawn(move || atender(flujo, &suyo, &suyo_testigo));
        }
    });

    // El bucle de revision es el producto entero: sin el, esto es un visor de
    // registros. Lo que cierra PA-104 es que alguien pregunte «¿quien falta?» sin
    // que nadie se lo pida.
    // Dos listas y no una. La primera version las junto, y el resultado fue que
    // un sensor del censo que hablaba por primera vez salia como
    // «VUELVE: vuelve a latir» — dandole al operador un pasado que no tuvo.
    //
    // Volver de una ausencia y aparecer por primera vez tienen la misma forma
    // —un sensor que estaba en la lista de los que faltan y ya no esta— y son
    // dos noticias distintas: una dice que algo se recupero, la otra que una
    // instalacion termino. Es el mismo colapso de estados que este proyecto
    // lleva persiguiendo, aqui en el texto que lee la sala.
    let mut ausentes_anunciados: Vec<Identidad> = Vec::new();
    let mut nunca_vistos_anunciados: Vec<Identidad> = Vec::new();

    loop {
        std::thread::sleep(CADENCIA_REVISION);

        let Ok(guardia) = vigia.lock() else { continue };
        let estados = guardia.revisar(ahora_ms());
        drop(guardia);

        let mut ahora_ausentes: Vec<Identidad> = Vec::new();
        let mut ahora_nunca_vistos: Vec<Identidad> = Vec::new();

        for estado in &estados {
            match estado {
                Vigilancia::Ausente {
                    identidad,
                    hace_ms,
                    ventana_ms,
                } => {
                    ahora_ausentes.push(identidad.clone());
                    if !ausentes_anunciados.contains(identidad) {
                        println!(
                            "AUSENTE  {identidad}: sin latir desde hace {hace_ms} ms \
                             (se le permitian {ventana_ms})"
                        );
                    }
                }
                Vigilancia::NuncaVisto { identidad } => {
                    ahora_nunca_vistos.push(identidad.clone());
                    if !nunca_vistos_anunciados.contains(identidad) {
                        println!(
                            "NUNCA VISTO  {identidad}: esta en el censo y no ha dicho nada. \
                             Quiza no arranco, quiza no se instalo."
                        );
                    }
                }
                Vigilancia::Vivo { identidad, .. } => {
                    if ausentes_anunciados.contains(identidad) {
                        println!("VUELVE  {identidad}: vuelve a latir tras la ausencia.");
                    } else if nunca_vistos_anunciados.contains(identidad) {
                        println!(
                            "APARECE  {identidad}: informa por primera vez. \
                             Ya no falta del censo."
                        );
                    }
                }
            }
        }

        // Se anuncia la transicion y no el estado, por lo mismo que el agente
        // emite transiciones y no condiciones: repetir la misma noticia cada cinco
        // segundos es como se ensena a ignorarla (RPT-032 §3).
        ausentes_anunciados = ahora_ausentes;
        nunca_vistos_anunciados = ahora_nunca_vistos;
    }
}

/// Lee marcos de syslog de una conexion y los incorpora.
///
/// El marcado es el de RFC 6587 por conteo de octetos, que es el que emite
/// `eje-agente`: `LONGITUD ESPACIO MENSAJE`.
fn atender(mut flujo: TcpStream, vigia: &Arc<Mutex<Vigia>>, testigo: &Arc<Mutex<Testigo>>) {
    let mut acumulado: Vec<u8> = Vec::new();
    let mut trozo = [0_u8; 4096];

    loop {
        let leidos = match flujo.read(&mut trozo) {
            Ok(0) | Err(_) => return,
            Ok(cantidad) => cantidad,
        };
        acumulado.extend_from_slice(&trozo[..leidos]);

        while let Some(espacio) = acumulado.iter().position(|byte| *byte == b' ') {
            let Ok(cabecera) = std::str::from_utf8(&acumulado[..espacio]) else {
                return;
            };
            let Ok(longitud) = cabecera.parse::<usize>() else {
                // No es un marco contado. Se cierra en lugar de adivinar: un
                // colector que interpreta a medias es peor que uno que no lee.
                return;
            };
            if longitud > LINEA_MAXIMA {
                return;
            }
            if acumulado.len() < espacio + 1 + longitud {
                break;
            }

            let cuerpo = acumulado[espacio + 1..espacio + 1 + longitud].to_vec();
            acumulado.drain(..espacio + 1 + longitud);

            let Ok(texto) = String::from_utf8(cuerpo) else {
                continue;
            };
            if let Some(sello) = sellos::analizar(&texto) {
                anotar_sello(&sello, testigo);
                continue;
            }

            let Some(latido) = analizar(&texto) else {
                continue;
            };

            let Ok(mut guardia) = vigia.lock() else {
                return;
            };
            let acogida = guardia.observar(&latido, ahora_ms());
            drop(guardia);

            match acogida {
                Acogida::LineaBase => println!(
                    "LINEA BASE  {}: primer latido (numero {}). Nada que afirmar todavia.",
                    latido.identidad(),
                    latido.contador
                ),
                Acogida::Continua => {}
                Acogida::HuecoEnLaSerie { perdidos } => println!(
                    "HUECO  {}: faltan {perdidos} latidos por el camino. El sensor esta vivo.",
                    latido.identidad()
                ),
                Acogida::ReinicioORepeticion { visto, recibido } => println!(
                    "REVISAR  {}: el contador fue de {visto} a {recibido}. \
                     Es un reinicio del agente O una repeticion de un latido capturado. \
                     Desde aqui no se puede distinguir.",
                    latido.identidad()
                ),
            }
        }
    }
}

/// Cotej a un sello y dice en voz alta lo que sale.
///
/// RPT-061, PA-115. Las dos acusaciones se imprimen distintas porque mandan a
/// buscar cosas distintas: un recorte quita asientos, una reescritura los
/// cambia.
fn anotar_sello(sello: &sellos::SelloRecibido, testigo: &Arc<Mutex<Testigo>>) {
    let Ok(mut guardia) = testigo.lock() else {
        return;
    };
    let cotejo = guardia.cotejar(sello);
    drop(guardia);

    match cotejo {
        Cotejo::LineaBase => println!(
            "SELLO BASE  {}: extremo anotado en el asiento {}. Nada que cotejar todavia.",
            sello.identidad(),
            sello.asiento
        ),
        // El registro que crece y el sensor en calma son el funcionamiento
        // normal: anunciarlos cada vez seria la fatiga de alertas de RPT-032 §3.
        Cotejo::Avanza { .. } | Cotejo::SinCambios => {}
        Cotejo::Retroceso { visto, recibido } => {
            println!(
                "!! RECORTE  {}: el registro tenia {visto} asientos y ahora declara {recibido}.",
                sello.identidad()
            );
            println!("   El ancla local no ve esto: quien recorta puede recalcularla (RPT-038).");
        }
        Cotejo::ExtremoDistinto {
            asiento,
            visto,
            recibido,
        } => {
            println!(
                "!! REESCRITURA  {}: el asiento {asiento} tenia extremo {visto} y ahora {recibido}.",
                sello.identidad()
            );
            println!("   Misma longitud, contenido distinto.");
        }
    }
}
