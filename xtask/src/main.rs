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

mod exclusion;
mod guardian;
mod vectores;

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
    println!("  cargo xtask ayuda              Muestra esta ayuda");
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
