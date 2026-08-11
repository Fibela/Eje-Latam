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
mod exclusion;
mod guardian;
mod tablero;
mod vectores;
mod vectores_ipc;

#[cfg(test)]
mod pruebas;

use std::path::Path;
use std::process::ExitCode;

const ROJO: &str = "\u{1b}[0;31m";
const VERDE: &str = "\u{1b}[0;32m";
const GRIS: &str = "\u{1b}[0;90m";
const FIN: &str = "\u{1b}[0m";

fn main() -> ExitCode {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let orden = argumentos.first().map(String::as_str).unwrap_or("ayuda");

    match orden {
        "verificar" => {
            let ruta = argumentos.get(1).map(String::as_str).unwrap_or("crates");
            ejecutar_guardian(Path::new(ruta))
        }
        "vectores" => {
            let actualizar = argumentos.iter().any(|a| a == "--actualizar");
            ejecutar_vectores(actualizar)
        }
        "tablero" => ejecutar_tablero(),
        "cobertura" => ejecutar_cobertura(),
        "vectores-ipc" => ejecutar_vectores_ipc(),
        "ayuda" | "--help" | "-h" => {
            ayuda();
            ExitCode::SUCCESS
        }
        desconocido => {
            eprintln!("{ROJO}Orden desconocida: '{desconocido}'{FIN}\n");
            ayuda();
            ExitCode::FAILURE
        }
    }
}

fn ayuda() {
    println!("Herramientas de desarrollo de Eje-Latam\n");
    println!("  cargo xtask verificar [ruta]   Guardian de inconclusos (por defecto: crates)");
    println!("  cargo xtask vectores           Descarga y ancla los vectores ACVP y Wycheproof");
    println!(
        "    --actualizar                 Reescribe el anclaje (usar solo tras cambiar FUENTES.toml)"
    );
    println!("  cargo xtask tablero            Recuento de puntos abiertos leido de RPT-002");
    println!(
        "  cargo xtask cobertura          Comprueba que toda prueba escrita se ejecuta (PA-73)"
    );
    println!(
        "  cargo xtask vectores-ipc       Regenera los vectores del formato de cable (RPT-045)"
    );
    println!("  cargo xtask ayuda              Muestra esta ayuda");
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

    ExitCode::SUCCESS
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
