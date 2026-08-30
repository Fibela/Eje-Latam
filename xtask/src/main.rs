//! Herramientas de desarrollo de Eje-Latam.
//!
//! Punto de entrada unico para las verificaciones propias del proyecto, en
//! sustitucion de scripts de shell. Corre identico en Windows, Linux y CI, y se
//! prueba con `cargo test` (RPT-003 §9.5, PA-11).
//!
//! ```text
//! cargo xtask verificar [ruta]   Guardian de inconclusos. Ruta por defecto: crates
//! cargo xtask ayuda              Muestra esta ayuda
//! ```

mod cobertura;
mod conformidad;
mod empaquetar;
mod exclusion;
mod guardian;
mod instalador;
mod manual;
mod sembrar;
mod tablero;
mod vectores;
mod vectores_ipc;

#[cfg(test)]
mod pruebas;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const ROJO: &str = "\u{1b}[0;31m";
const VERDE: &str = "\u{1b}[0;32m";
const GRIS: &str = "\u{1b}[0;90m";
const FIN: &str = "\u{1b}[0m";

/// Una orden de `xtask`: como se teclea, para que sirve y que ejecuta.
///
/// # Por que es una tabla y no un `match` con una ayuda al lado
///
/// RPT-066, PA-119. Este listado es la **unica** fuente: de aqui sale el
/// despacho, de aqui sale `ayuda`, y contra esto se coteja `docs/Comandos.md`.
///
/// Mientras la ayuda fue una tira de `println!` aparte del `match`, nada impedia
/// que una orden existiera sin anunciarse ni que se anunciara una que ya no
/// existe. No es una posibilidad teorica: es la misma anatomia del tablero que
/// conto 76 de 115 durante dos semanas (PA-108) y de las dos pruebas que nadie
/// ejecutaba (PA-73). Tres indices escritos a mano, tres veces el mismo defecto.
struct Orden {
    /// Como se teclea.
    nombre: &'static str,
    /// Argumentos que admite, para la linea de ayuda. Vacio si no toma ninguno.
    argumentos: &'static str,
    /// Que hace, en una linea.
    resumen: &'static str,
    /// Recibe los argumentos **completos**, con el nombre de la orden en `[0]`.
    ejecutar: fn(&[String]) -> ExitCode,
}

/// Todas las ordenes que `xtask` acepta. Fuente unica.
const ORDENES: &[Orden] = &[
    Orden {
        nombre: "verificar",
        argumentos: "[ruta]",
        resumen: "Guardian de inconclusos (por defecto: crates)",
        ejecutar: orden_verificar,
    },
    Orden {
        nombre: "tablero",
        argumentos: "",
        resumen: "Recuento de puntos abiertos leido de RPT-002 §12",
        ejecutar: orden_tablero,
    },
    Orden {
        nombre: "cobertura",
        argumentos: "",
        resumen: "Comprueba que toda prueba escrita se ejecuta (PA-73)",
        ejecutar: orden_cobertura,
    },
    Orden {
        nombre: "manual",
        argumentos: "",
        resumen: "Paridad entre docs/Comandos.md y estas ordenes (PA-119)",
        ejecutar: orden_manual,
    },
    Orden {
        nombre: "empaquetar",
        argumentos: "[ruta]",
        resumen: "Artefacto headless, revisado sobre el disco (PA-107)",
        ejecutar: orden_empaquetar,
    },
    Orden {
        nombre: "probar-instalador",
        argumentos: "",
        resumen: "Caja de arena del instalador (PA-116). NO cubre PA-117",
        ejecutar: orden_probar_instalador,
    },
    Orden {
        nombre: "conformidad",
        argumentos: "",
        resumen: "Ejecuta las suites PQC y emite CONFORMIDAD.lock (PA-121)",
        ejecutar: orden_conformidad,
    },
    Orden {
        nombre: "vectores",
        argumentos: "[--actualizar]",
        resumen: "Descarga y ancla los vectores ACVP y Wycheproof",
        ejecutar: orden_vectores,
    },
    Orden {
        nombre: "vectores-ipc",
        argumentos: "",
        resumen: "Regenera los vectores del formato de cable (RPT-045)",
        ejecutar: orden_vectores_ipc,
    },
    Orden {
        nombre: "sembrar",
        argumentos: "<ruta> [cuantos] [relleno]",
        resumen: "Fabrica un registro de evidencia para pruebas",
        ejecutar: orden_sembrar,
    },
    Orden {
        nombre: "ayuda",
        argumentos: "",
        resumen: "Muestra esta ayuda",
        ejecutar: orden_ayuda,
    },
];

fn main() -> ExitCode {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let pedida = argumentos.first().map(String::as_str).unwrap_or("ayuda");

    // `--help` y `-h` son alias y no ordenes: si estuvieran en ORDENES, la
    // paridad con el manual exigiria documentarlos como si se tecleara
    // `cargo xtask --help`, que no es como se invoca esto.
    let nombre = match pedida {
        "--help" | "-h" => "ayuda",
        otro => otro,
    };

    if let Some(orden) = ORDENES.iter().find(|orden| orden.nombre == nombre) {
        return (orden.ejecutar)(&argumentos);
    }

    eprintln!("{ROJO}Orden desconocida: '{pedida}'{FIN}\n");
    ayuda();
    ExitCode::FAILURE
}

fn ayuda() {
    println!("Herramientas de desarrollo de Eje-Latam\n");
    for orden in ORDENES {
        let invocacion = format!("cargo xtask {} {}", orden.nombre, orden.argumentos);
        // El ancho sale de la invocacion mas larga, no de un numero elegido a
        // ojo: `sembrar <ruta> [cuantos] [relleno]` desbordaba una columna de 40
        // y rompia la alineacion de su propia linea.
        let ancho = ORDENES
            .iter()
            .map(|orden| orden.nombre.len() + orden.argumentos.len() + "cargo xtask  ".len())
            .max()
            .unwrap_or(40);

        println!(
            "  {:<ancho$} {}",
            invocacion.trim_end(),
            orden.resumen,
            ancho = ancho
        );
    }
}

fn orden_ayuda(_argumentos: &[String]) -> ExitCode {
    ayuda();
    ExitCode::SUCCESS
}

fn orden_verificar(argumentos: &[String]) -> ExitCode {
    let ruta = argumentos.get(1).map(String::as_str).unwrap_or("crates");
    ejecutar_guardian(Path::new(ruta))
}

fn orden_vectores(argumentos: &[String]) -> ExitCode {
    ejecutar_vectores(argumentos.iter().any(|arg| arg == "--actualizar"))
}

fn orden_tablero(_argumentos: &[String]) -> ExitCode {
    ejecutar_tablero()
}

fn orden_cobertura(_argumentos: &[String]) -> ExitCode {
    ejecutar_cobertura()
}

fn orden_conformidad(_argumentos: &[String]) -> ExitCode {
    ejecutar_conformidad()
}

fn orden_vectores_ipc(_argumentos: &[String]) -> ExitCode {
    ejecutar_vectores_ipc()
}

fn orden_probar_instalador(_argumentos: &[String]) -> ExitCode {
    ejecutar_probar_instalador()
}

fn orden_sembrar(argumentos: &[String]) -> ExitCode {
    ejecutar_sembrar(argumentos)
}

fn orden_empaquetar(argumentos: &[String]) -> ExitCode {
    ejecutar_empaquetar(argumentos)
}

/// Cotejo entre el manual de comandos y las ordenes que existen. RPT-066, PA-119.
fn orden_manual(_argumentos: &[String]) -> ExitCode {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));

    let nombres: Vec<&str> = ORDENES.iter().map(|orden| orden.nombre).collect();

    println!("{GRIS}Manual de comandos contra las ordenes que existen{FIN}");

    match manual::cotejar(raiz, &nombres) {
        Ok(discrepancias) if discrepancias.is_empty() => {
            println!(
                "  {} orden(es), todas documentadas en docs/Comandos.md",
                nombres.len()
            );
            println!();
            println!("{VERDE}El manual y el binario dicen lo mismo.{FIN}");
            ExitCode::SUCCESS
        }
        Ok(discrepancias) => {
            for discrepancia in &discrepancias {
                eprintln!("  {ROJO}{discrepancia}{FIN}");
            }
            eprintln!();
            eprintln!("Un comando documentado que ya no existe manda a teclear algo que falla,");
            eprintln!("y una orden sin documentar no la usa nadie mas que quien la escribio.");
            ExitCode::FAILURE
        }
        // Ni verde ni rojo: no se sabe. RPT-006 §4.
        Err(motivo) => {
            eprintln!("  ComprobacionImposible: {motivo}");
            ExitCode::from(3)
        }
    }
}

/// Imprime el recuento del tablero **leyendolo**, no de memoria.
///
/// El tablero se resumio a mano cuatro veces y las cuatro reintrodujo puntos ya
/// cerrados. Un recuento derivado no puede equivocarse en eso.
fn ejecutar_tablero() -> ExitCode {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));

    let puntos = match tablero::desde_raiz(raiz) {
        Ok(puntos) => puntos,
        Err(motivo) => {
            eprintln!("{ROJO}{motivo}{FIN}");
            return ExitCode::FAILURE;
        }
    };

    let recuento = tablero::contar(&puntos);

    println!("{GRIS}Tablero de RPT-002 §12{FIN}");
    println!("  Identificadores : {}", recuento.total());
    println!("  {VERDE}Cerrados{FIN}        : {}", recuento.cerrados);
    println!("  Parciales       : {}", recuento.parciales);
    println!("  Abiertos        : {}", recuento.abiertos);
    println!(
        "  Pendientes      : {} (parciales + abiertos)",
        recuento.pendientes()
    );
    println!();

    let pendientes: Vec<&str> = puntos
        .iter()
        .filter(|punto| punto.estado != tablero::Estado::Cerrado)
        .map(|punto| punto.identificador.as_str())
        .collect();

    println!("{GRIS}Pendientes: {}{FIN}", pendientes.join(", "));

    // PA-108. Sin esto, el tablero puede quedarse atras y el recuento seguir
    // pareciendo el total del proyecto. Ocurrio: se quedo en PA-76 mientras los
    // reportes acunaban treinta y nueve identificadores nuevos.
    let huerfanos = match tablero::citados_sin_fila(raiz, &puntos) {
        Ok(huerfanos) => huerfanos,
        Err(motivo) => {
            eprintln!("{ROJO}{motivo}{FIN}");
            return ExitCode::FAILURE;
        }
    };

    if huerfanos.is_empty() {
        println!();
        println!("{VERDE}Todo identificador citado en docs/ tiene fila en el tablero.{FIN}");
        return ExitCode::SUCCESS;
    }

    println!();
    eprintln!(
        "{ROJO}{} identificador(es) citados en docs/ y SIN fila en el tablero:{FIN}",
        huerfanos.len()
    );
    for huerfano in &huerfanos {
        eprintln!("  {huerfano}");
    }
    eprintln!();
    eprintln!("Un punto que solo existe en el reporte que lo acuno no esta en ningun");
    eprintln!("recuento, y desaparece en cuanto nadie recuerda haberlo escrito.");

    ExitCode::FAILURE
}

/// Construye el artefacto headless y lo revisa. RPT-062, PA-107.
fn ejecutar_empaquetar(argumentos: &[String]) -> ExitCode {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));
    let destino = argumentos
        .get(1)
        .map_or_else(|| raiz.join("target/paquete/eje-agente"), PathBuf::from);

    println!(
        "{GRIS}Empaquetando el sensor headless en {}{FIN}",
        destino.display()
    );

    match empaquetar::empaquetar(raiz, &destino) {
        Ok(ficheros) => {
            for fichero in &ficheros {
                println!("  {fichero}");
            }
            println!();
            println!("{VERDE}Artefacto revisado sobre el disco: nada prohibido.{FIN}");
            // El aviso dice lo que falta HOY, no lo que faltaba. Decia «el
            // formato sigue sin decidirse» y ese aviso acuno PA-126; al cerrar
            // el formato paso a mentir en la direccion contraria, callando lo
            // unico que de verdad falta (RPT-073 §11).
            println!(
                "{GRIS}El paquete lleva resumenes y NO firma: se comprueba que llega \
                 entero, no de donde viene (PA-14a).{FIN}"
            );
            ExitCode::SUCCESS
        }
        Err(motivo) => {
            eprintln!("{ROJO}{motivo}{FIN}");
            ExitCode::FAILURE
        }
    }
}

/// Caja de arena del instalador. RPT-063, PA-116.
fn ejecutar_probar_instalador() -> ExitCode {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));

    println!("{GRIS}Instalador contra un destino desechable{FIN}");

    match instalador::probar(raiz) {
        instalador::Resultado::Conforme(comprobado) => {
            for afirmacion in &comprobado {
                println!("  {VERDE}PASA{FIN}   {afirmacion}");
            }
            println!();
            println!("{VERDE}El instalador respeta las rutas que se le dan.{FIN}");
            println!(
                "{GRIS}Esto NO dice nada del ciclo de vida del servicio: eso es PA-117 y \
                 exige systemd como PID 1 (RPT-062 §5).{FIN}"
            );
            ExitCode::SUCCESS
        }
        instalador::Resultado::ViolacionDetectada(fallos) => {
            for fallo in &fallos {
                eprintln!("  {ROJO}FALLA{FIN}  {fallo}");
            }
            ExitCode::FAILURE
        }
        instalador::Resultado::ComprobacionImposible(motivo) => {
            // Ni verde ni rojo: no se sabe. RPT-006 §4.
            eprintln!("  ComprobacionImposible: {motivo}");
            ExitCode::from(3)
        }
    }
}

fn ejecutar_vectores(actualizar: bool) -> ExitCode {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));

    println!("{GRIS}Sincronizando vectores de prueba de motor-pqc{FIN}");
    match vectores::sincronizar(raiz, actualizar) {
        Ok(()) => {
            println!("{VERDE}Vectores sincronizados y anclados.{FIN}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{ROJO}{error}{FIN}");
            ExitCode::FAILURE
        }
    }
}

fn ejecutar_guardian(ruta: &Path) -> ExitCode {
    let objetivo = ruta.display();
    println!("{GRIS}Guardian de inconclusos sobre '{objetivo}'{FIN}");

    let hallazgos = match guardian::verificar(ruta) {
        Ok(hallazgos) => hallazgos,
        Err(error) => {
            eprintln!("{ROJO}No se pudo ejecutar el guardian: {error}{FIN}");
            return ExitCode::FAILURE;
        }
    };

    if hallazgos.is_empty() {
        println!(
            "{VERDE}Sin implementaciones inconclusas ni datos simulados en ruta de produccion.{FIN}"
        );
        return ExitCode::SUCCESS;
    }

    let comprobaciones = guardian::comprobaciones().unwrap_or_default();

    let mut etiqueta_previa = "";
    for hallazgo in &hallazgos {
        if hallazgo.etiqueta != etiqueta_previa {
            println!("\n{ROJO}FALLO — {}{FIN}", hallazgo.etiqueta);
            if let Some(comprobacion) = comprobaciones
                .iter()
                .find(|c| c.etiqueta == hallazgo.etiqueta)
            {
                println!("{GRIS}   {}{FIN}", comprobacion.motivo);
            }
            etiqueta_previa = hallazgo.etiqueta;
        }
        println!(
            "   {}:{} → {}",
            hallazgo.fichero.display(),
            hallazgo.linea,
            hallazgo.contenido
        );
    }

    println!(
        "\n{ROJO}{} hallazgo(s). El build de release queda bloqueado.{FIN}",
        hallazgos.len()
    );
    println!(
        "{GRIS}Sustituya el marcador por codigo real o abra un issue formal; no relaje el guardian (RPT-003 §9.5).{FIN}"
    );

    ExitCode::FAILURE
}

/// Compara las pruebas escritas con las que `cargo test` registra.
///
/// RPT-039 §8, PA-73. Existe porque dos pruebas de PA-72 quedaron anidadas
/// dentro de otra funcion y la suite siguio en verde con dos menos.
///
/// # La comparacion es una desigualdad
///
/// Falla solo si hay **mas** en el arbol que registradas, que es la condicion de
/// prueba fantasma. Al reves no lo es: las pruebas de documentacion se registran
/// y no llevan `#[test]` en ninguna parte.
fn ejecutar_cobertura() -> ExitCode {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));

    let recuento = match cobertura::en_el_arbol(raiz) {
        Ok(recuento) => recuento,
        Err(error) => {
            eprintln!("{ROJO}No se pudo leer el arbol de fuentes: {error}{FIN}");
            return ExitCode::FAILURE;
        }
    };

    let exigibles: usize = recuento
        .iter()
        .map(|(_, cuantas)| cuantas.incondicionales)
        .sum();
    let condicionadas: usize = recuento
        .iter()
        .map(|(_, cuantas)| cuantas.condicionadas)
        .sum();

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let salida = match std::process::Command::new(cargo)
        .current_dir(raiz)
        .args(["test", "--workspace", "--", "--list"])
        .output()
    {
        Ok(salida) => salida,
        Err(error) => {
            eprintln!("{ROJO}No se pudo ejecutar 'cargo test -- --list': {error}{FIN}");
            return ExitCode::FAILURE;
        }
    };

    if !salida.status.success() {
        // No se degrada a «sin pruebas registradas»: eso daria cero, la
        // desigualdad fallaria y el motivo real —que la compilacion esta rota—
        // quedaria enterrado bajo una acusacion de pruebas fantasma.
        eprintln!("{ROJO}'cargo test -- --list' no termino bien.{FIN}");
        eprint!("{}", String::from_utf8_lossy(&salida.stderr));
        return ExitCode::FAILURE;
    }

    let registradas = cobertura::registradas(&String::from_utf8_lossy(&salida.stdout));

    println!("Cobertura de ejecucion (PA-73)");
    println!("  Exigibles en el arbol         : {exigibles}");
    println!("  Condicionadas por #[cfg]      : {condicionadas}  (no comparables aqui)");
    println!("  Pruebas que cargo registra    : {registradas}");

    if exigibles > registradas {
        println!();
        eprintln!(
            "{ROJO}Hay {} prueba(s) escritas que NADIE ejecuta.{FIN}",
            exigibles - registradas
        );
        eprintln!("{GRIS}Causas habituales: un #[test] anidado dentro de otra funcion, un");
        eprintln!("modulo de pruebas que se dejo de declarar, o un fichero fuera del arbol.{FIN}");
        return ExitCode::FAILURE;
    }

    println!("{VERDE}Toda prueba exigible esta registrada.{FIN}");
    if condicionadas > 0 {
        // Se dice en voz alta y no se calla: son las que esta herramienta **no**
        // puede vigilar, y quien lea la salida debe saber cuantas son.
        println!(
            "{GRIS}Las {condicionadas} condicionadas quedan fuera de la comparacion: desde esta plataforma no se puede saber si deben estar (RPT-006 §4).{FIN}"
        );
    }
    ExitCode::SUCCESS
}

/// Regenera los vectores del formato de cable.
///
/// RPT-045 §3. El codificador de Rust es el que manda; el fichero es el ancla
/// que impide que el cliente de TypeScript se desvie sin que nadie lo note.
/// `cargo xtask sembrar <ruta> [cuantos]`
///
/// Fabrica un registro de evidencia para ejercitar la fragmentacion de marcos.
/// Vive en `xtask` a proposito: es una herramienta de desarrollo y no se
/// distribuye con el producto.
fn ejecutar_sembrar(argumentos: &[String]) -> ExitCode {
    let Some(ruta) = argumentos.get(1) else {
        eprintln!("uso: cargo xtask sembrar <ruta-del-registro> [cuantos] [relleno]");
        eprintln!("ejemplo: cargo xtask sembrar /tmp/eje/evidencia.alm 300 4000");
        return ExitCode::FAILURE;
    };

    let Ok(cuantos) = argumentos
        .get(2)
        .map_or(Ok(2_000), |texto| texto.parse::<u64>())
    else {
        eprintln!("el numero de asientos debe ser un entero");
        return ExitCode::FAILURE;
    };

    let Ok(relleno) = argumentos
        .get(3)
        .map_or(Ok(0), |texto| texto.parse::<usize>())
    else {
        eprintln!("el relleno debe ser un entero (bytes por asiento)");
        return ExitCode::FAILURE;
    };

    match sembrar::sembrar(std::path::Path::new(ruta), cuantos, relleno) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn ejecutar_vectores_ipc() -> ExitCode {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));
    let ruta = raiz.join(vectores_ipc::FICHERO);

    match std::fs::write(&ruta, vectores_ipc::generar()) {
        Ok(()) => {
            println!(
                "{VERDE}Vectores de cable escritos en {}{FIN}",
                ruta.display()
            );
            println!(
                "{GRIS}Si el diff no es el que esperabas, el formato de cable cambio \
sin querer.{FIN}"
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{ROJO}No se pudo escribir {}: {error}{FIN}", ruta.display());
            ExitCode::FAILURE
        }
    }
}

/// Ejecuta las tres suites poscuanticas y, solo si pasan, emite el atestado.
///
/// # Por que ejecutar y emitir van juntos y en este orden
///
/// RPT-005 §9.3: «ejecutaria las tres suites y, **solo si pasan**, emitiria el
/// fichero». Separarlo en dos ordenes dejaria emitir sin probar, y entonces el
/// fichero solo diria «alguien escribio esto», que es la constante `true` que ese
/// reporte ya descarto.
///
/// El atestado se compone **despues**, leyendo el arbol. No recoge nada de la
/// ejecucion —ni fechas, ni maquina, ni duracion— porque nada de eso es
/// reproducible, y un campo que no se puede recalcular convierte la barrera en
/// ruido.
fn ejecutar_conformidad() -> ExitCode {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));

    println!("{GRIS}Conformidad poscuantica (RPT-005 §9.3){FIN}");
    println!();

    for &suite in conformidad::SUITES {
        print!("  {suite} ... ");
        let _ = std::io::Write::flush(&mut std::io::stdout());

        let salida = std::process::Command::new(env!("CARGO"))
            .current_dir(raiz)
            .args(["test", "-p", "motor-pqc", "--test", suite])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match salida {
            Ok(estado) if estado.success() => println!("{VERDE}pasa{FIN}"),

            Ok(_) => {
                println!("{ROJO}FALLA{FIN}");
                eprintln!();
                eprintln!("{ROJO}No se emite {}.{FIN}", conformidad::FICHERO);
                eprintln!(
                    "  Un atestado emitido con una suite en rojo diria que el motor es\n  \
                     conforme cuando no lo es. Reproduce el fallo con:\n\n    \
                     cargo test -p motor-pqc --test {suite}"
                );
                return ExitCode::FAILURE;
            }

            Err(error) => {
                println!("{ROJO}NO SE PUDO EJECUTAR{FIN}");
                eprintln!("  {error}");
                eprintln!(
                    "  No se emite nada: «no se pudo comprobar» no es «pasa»."
                );
                return ExitCode::FAILURE;
            }
        }
    }

    let atestado = match conformidad::componer(raiz) {
        Ok(atestado) => atestado,
        Err(fallo) => {
            eprintln!("{ROJO}{fallo}{FIN}");
            return ExitCode::FAILURE;
        }
    };

    let destino = raiz.join(conformidad::FICHERO);

    if let Err(error) = std::fs::write(&destino, conformidad::rendir(&atestado)) {
        eprintln!("{ROJO}no se pudo escribir {}: {error}{FIN}", destino.display());
        return ExitCode::FAILURE;
    }

    println!();
    println!("  {} paquete(s) atestiguados", atestado.paquetes.len());
    println!("  canal   {}", atestado.canal);
    println!("  huella  {}", atestado.huella);
    println!();
    println!("{VERDE}{} emitido.{FIN}", conformidad::FICHERO);
    println!(
        "{GRIS}  Ata QUE se probo, no QUE se probo (RPT-005 §9.4): componer esta\n\
         \x20 huella sin ejecutar nada es posible. Cerrarlo es PA-14.{FIN}"
    );

    ExitCode::SUCCESS
}
