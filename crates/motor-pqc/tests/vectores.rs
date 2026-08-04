//! Conformidad frente a los vectores ACVP y Wycheproof.
//!
//! RPT-005 §4.3, PA-17.
//!
//! # Esta suite FALLA cuando los vectores no están, no se salta
//!
//! La tentación es marcar estas pruebas como `#[ignore]` o hacer que se salten
//! silenciosamente cuando el directorio está vacío. **Sería el mismo modo de
//! fallo que ya nos mordió dos veces en este proyecto**: el `.ps1` que abandonaba
//! el fichero al primer `#[cfg(test)]` y la configuración de dependency-cruiser
//! que excluía `dist` del grafo. Ambos daban verde con la violación presente.
//!
//! Una suite de conformidad que se salta sola informa "todo bien" cuando lo
//! cierto es "no se comprobó nada". Aquí falla con un mensaje accionable.
//!
//! # Carga en tiempo de ejecución, no `include_str!`
//!
//! Los ficheros de Wycheproof llegan a varios megabytes. Incrustarlos con
//! `include_str!` los mete en el binario de pruebas y obliga a recompilar la
//! suite cada vez que se actualiza un vector. Se leen del disco con
//! `CARGO_MANIFEST_DIR`.
//!
//! (Nótese que esto no afecta al binario de producción en ningún caso: los
//! ficheros de `tests/` nunca se compilan dentro del artefacto distribuido.)

// Igual que en `diferencial.rs`: en una suite de conformidad, abortar es el
// comportamiento deseado.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

/// Directorio de vectores, relativo al manifiesto del crate.
fn directorio_vectores() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("vectores")
}

/// Ficheros de vectores esperados, según `FUENTES.toml`.
const ESPERADOS: [&str; 7] = [
    "acvp_ml_kem_768_keygen.json",
    "acvp_ml_kem_768_encapdecap.json",
    "acvp_ml_dsa_65_keygen.json",
    "acvp_ml_dsa_65_sigver.json",
    "wycheproof_mlkem_768_test.json",
    "wycheproof_mldsa_65_verify_test.json",
    "wycheproof_mldsa_44_verify_test.json",
];

const INSTRUCCION: &str = "\
Los vectores de prueba no están en el repositorio.

    cargo xtask vectores

descarga los ficheros declarados en crates/motor-pqc/tests/vectores/FUENTES.toml
y ancla su resumen SHA-256 en FUENTES.lock.

Mientras falten, `motor-pqc` NO es apto para producción: RPT-005 §4.3 exige
vectores ACVP y Wycheproof, y `Conformidad::apto_para_produccion()` devuelve
false. Esta prueba falla a propósito para que ese estado sea visible.";

#[test]
fn los_vectores_estan_presentes() {
    let directorio = directorio_vectores();
    let mut ausentes = Vec::new();

    for nombre in ESPERADOS {
        if !directorio.join(nombre).is_file() {
            ausentes.push(nombre);
        }
    }

    assert!(
        ausentes.is_empty(),
        "faltan {} de {} ficheros de vectores: {}\n\n{INSTRUCCION}",
        ausentes.len(),
        ESPERADOS.len(),
        ausentes.join(", ")
    );
}

#[test]
fn el_anclaje_de_resumenes_existe() {
    // FUENTES.lock es lo que hace segura la exoneración de gitleaks sobre este
    // directorio (.gitleaks.toml). Sin anclaje, la exoneración es un punto ciego.
    let anclaje = directorio_vectores().join("FUENTES.lock");
    assert!(
        anclaje.is_file(),
        "falta el anclaje de resúmenes FUENTES.lock.\n\n\
         Sin él, la exoneración de gitleaks sobre tests/vectores/ deja de ser \
         segura: cualquiera podría depositar ahí una clave real sin ser detectado.\n\n{INSTRUCCION}"
    );
}

#[test]
fn la_declaracion_de_fuentes_cubre_ambos_conjuntos() {
    // Comprobación de coherencia entre la declaración y lo que esta suite espera.
    // Si alguien añade una fuente a FUENTES.toml sin añadirla a ESPERADOS, el
    // fichero se descargaría pero nunca se comprobaría.
    let fuentes = directorio_vectores().join("FUENTES.toml");
    let contenido =
        std::fs::read_to_string(&fuentes).expect("FUENTES.toml debe estar versionado en el repo");

    for nombre in ESPERADOS {
        assert!(
            contenido.contains(nombre),
            "'{nombre}' se espera en esta suite pero no está declarado en FUENTES.toml"
        );
    }

    let declarados = contenido.matches("nombre = ").count();
    assert_eq!(
        declarados,
        ESPERADOS.len(),
        "FUENTES.toml declara {declarados} ficheros y la suite espera {}. \
         Un fichero declarado y no comprobado se descarga para nada.",
        ESPERADOS.len()
    );

    assert!(
        contenido.contains("[[acvp]]"),
        "deben declararse vectores ACVP: comprueban que se calcula lo correcto"
    );
    assert!(
        contenido.contains("[[wycheproof]]"),
        "deben declararse vectores Wycheproof: comprueban que se rechaza lo incorrecto"
    );
}
