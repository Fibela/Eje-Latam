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
//! - **No contiene nada.** Calcula el veredicto y lo imprime. La emision hacia un
//!   conmutador sigue bloqueada en PA-22.
//! - **No anexa a ALM-01.** Los manejadores de RPT-019 son PA-43.
//!
//! Es un recorrido observable, no un servicio.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use eje_agente::{ConfiguracionAgente, VERSION};
use eje_captura::transporte::extraer;
use eje_captura::{DireccionEnlace, ErrorCaptura, FuentePasiva, abrir};
use guardian_cc::clasificacion::DeclaracionSegmento;
use guardian_cc::observacion::{AlmacenObservacion, Protocolo};
use guardian_cc::proveedores::{Indicio, ProveedorHuella};
use guardian_cc::{PerfilSegmento, Veredicto};

/// Plazo de espera por trama.
const PLAZO: Duration = Duration::from_millis(500);

/// Tramas a observar antes de resumir, si no se indica otra cosa.
const TRAMAS_POR_DEFECTO: u64 = 200;

/// Fallos del arranque del agente.
#[derive(Debug, thiserror::Error)]
enum ErrorAgente {
    /// Faltan argumentos o son incorrectos.
    #[error("uso: eje-agente --interfaz <nombre> [--tramas <n>] [--perfil corporativo|ot]")]
    Uso,

    /// La captura no pudo abrirse.
    #[error(transparent)]
    Captura(#[from] ErrorCaptura),
}

/// Opciones de la linea de ordenes.
struct Opciones {
    interfaz: String,
    tramas: u64,
    perfil: PerfilSegmento,
}

fn leer_opciones() -> Result<Opciones, ErrorAgente> {
    let mut interfaz = None;
    let mut tramas = TRAMAS_POR_DEFECTO;
    let mut perfil = PerfilSegmento::Corporativo;

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
            _ => return Err(ErrorAgente::Uso),
        }

        indice += 2;
    }

    Ok(Opciones {
        interfaz: interfaz.ok_or(ErrorAgente::Uso)?,
        tramas,
        perfil,
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

/// Segmento derivado de la etiqueta VLAN.
///
/// Sin etiqueta —puerto espejo sin marcar— no se puede saber, y `NoDeclarado`
/// es la respuesta segura: RPT-009 §5 lo trata como si pudiera alojar criticos.
/// Inventar «limpio» por comodidad convertiria un puerto sin etiquetar en
/// permiso para contener.
const fn segmento_de(vlan: Option<u16>) -> DeclaracionSegmento {
    match vlan {
        None => DeclaracionSegmento::NoDeclarado,
        Some(_) => DeclaracionSegmento::PuedeAlojarCriticos,
    }
}

fn main() -> Result<(), ErrorAgente> {
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

    let mut fuente = abrir(&opciones.interfaz)?;
    let mut almacen = AlmacenObservacion::nuevo();
    let mut vistos: BTreeMap<DireccionEnlace, u64> = BTreeMap::new();
    let mut observadas = 0u64;
    let mut ilegibles = 0u64;
    let inicio = Instant::now();

    while observadas < opciones.tramas {
        let Some(trama) = fuente.siguiente(PLAZO)? else {
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

        almacen.observar(extraida.origen, protocolo, segmento_de(extraida.vlan));
        *vistos.entry(extraida.origen).or_insert(0) += 1;
        observadas = observadas.saturating_add(1);
    }

    // La perdida del nucleo se traslada al almacen ANTES de concluir nada: sin
    // esto, menos protocolos vistos se leerian como ausencia de riesgo
    // (RPT-018 §4).
    let estadisticas = fuente.estadisticas()?;
    if estadisticas.hay_perdida() {
        almacen.anotar_perdida();
    }

    println!(
        "Tramas observadas  : {observadas} en {:?}",
        inicio.elapsed()
    );
    println!("Tramas ilegibles   : {ilegibles}");
    println!(
        "Descartes del nucleo: {} (vista {})",
        estadisticas.descartadas,
        if estadisticas.hay_perdida() {
            "INCOMPLETA"
        } else {
            "completa"
        }
    );
    println!("Dispositivos       : {}", vistos.len());
    println!();

    for (mac, cuantas) in vistos.iter().take(20) {
        let indicio = almacen.indicio(mac).map_or_else(
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
        almacen.volatiles(),
        almacen.pegajosos(),
        almacen.pegajoso_saturado()
    );
    println!();
    println!("Sin inventario aprovisionado, ningun dispositivo tiene marcado.");
    println!("El veredicto que sigue es el de un primer arranque (RPT-017 §3).");

    resumir_veredictos(&almacen, &vistos, opciones.perfil);

    Ok(())
}

/// Imprime el veredicto de cada dispositivo observado.
fn resumir_veredictos(
    almacen: &AlmacenObservacion,
    vistos: &BTreeMap<DireccionEnlace, u64>,
    perfil: PerfilSegmento,
) {
    // `Evidencia` vive en `clasificacion`; `proveedores` solo la usa. Importarla
    // desde alli fallaba porque ese `use` es privado.
    use guardian_cc::clasificacion::{Evidencia, clasificar};
    use guardian_cc::evaluar;
    use guardian_cc::proveedores::ProveedorSegmento;

    let mut ejecutables = 0u64;
    let mut escalados = 0u64;

    for mac in vistos.keys() {
        let Ok(historial) = almacen.historial(mac) else {
            escalados = escalados.saturating_add(1);
            continue;
        };

        let inferencia = almacen
            .indicio(mac)
            .unwrap_or(Indicio::Indeterminado)
            .clase();

        let evidencia = Evidencia {
            // Sin inventario no hay marcado. No es una simplificacion del
            // recorrido: es lo que el agente ve hoy.
            marcado: None,
            segmento: historial.declaracion_efectiva(),
            inferencia,
        };

        match evaluar(clasificar(&evidencia), perfil) {
            Veredicto::Ejecutar => ejecutables = ejecutables.saturating_add(1),
            _ => escalados = escalados.saturating_add(1),
        }
    }

    println!("  Contenibles sin intervencion : {ejecutables}");
    println!("  Requieren humano o prohibidos: {escalados}");
}
