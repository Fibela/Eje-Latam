//! Paridad entre `docs/Comandos.md` y las ordenes que `xtask` acepta.
//!
//! RPT-066, PA-119.
//!
//! # Por que existe
//!
//! El manual de comandos es una lista escrita a mano de cosas que viven en el
//! codigo. Es la tercera de la misma familia en una semana:
//!
//! | Indice | Se quedo atras | Lector que lo deriva |
//! |---|---|---|
//! | Puntos abiertos | Conto 76 de 115 durante dos semanas | `cargo xtask tablero` (PA-108) |
//! | Pruebas escritas | Dos quedaron anidadas y nadie las ejecutaba | `cargo xtask cobertura` (PA-73) |
//! | Comandos | *aun no se habia quedado atras* | esto (PA-119) |
//!
//! La tercera fila se escribio **antes** de que ocurriera, que es la unica vez
//! de las tres que ha pasado.
//!
//! # La comprobacion va en las dos direcciones, y no son igual de graves
//!
//! Una orden **sin documentar** deja una herramienta que solo usa quien la
//! escribio. Molesto, y se descubre solo.
//!
//! Un comando **documentado que ya no existe** manda a alguien a teclear algo
//! que falla, y lo hara en la sesion en la que menos tiempo hay para averiguar
//! por que. Esa es la direccion que justifica el modulo, y por eso se barre
//! `docs/` entero y no solo el manual: un reporte que cita una orden retirada
//! miente igual.

use std::path::Path;

/// Ficheros que se barren buscando citas de `cargo xtask …`.
const EXTENSION: &str = "md";

/// El manual, que ademas debe **anunciar** todas las ordenes.
const MANUAL: &str = "docs/Comandos.md";

/// Compara las ordenes reales con lo que dice la documentacion.
///
/// Devuelve la lista de discrepancias, vacia si no hay ninguna.
///
/// # Errores
///
/// `Err` si `docs/` o el manual no se pueden leer. No se degrada a «ninguna
/// discrepancia»: eso daria verde por no haber mirado, que es justo lo que
/// RPT-006 §4 prohibe.
pub fn cotejar(raiz: &Path, ordenes: &[&str]) -> Result<Vec<String>, String> {
    let ruta_manual = raiz.join(MANUAL);
    let manual = std::fs::read_to_string(&ruta_manual)
        .map_err(|error| format!("no se pudo leer {}: {error}", ruta_manual.display()))?;

    let mut discrepancias = Vec::new();

    // Direccion 1: toda orden que existe se anuncia en el manual.
    let anunciadas = citadas(&manual);
    for orden in ordenes {
        if !anunciadas.iter().any(|anunciada| anunciada == orden) {
            discrepancias.push(format!(
                "la orden '{orden}' existe y NO aparece en {MANUAL}: no la usara nadie mas que quien la escribio"
            ));
        }
    }

    // Direccion 2: toda orden citada en cualquier documento existe de verdad.
    let mut documentos = Vec::new();
    recorrer(&raiz.join("docs"), &mut documentos)?;
    documentos.sort();

    for documento in &documentos {
        let texto = std::fs::read_to_string(documento)
            .map_err(|error| format!("no se pudo leer {}: {error}", documento.display()))?;

        let relativo = documento
            .strip_prefix(raiz)
            .unwrap_or(documento)
            .display()
            .to_string()
            .replace('\\', "/");

        for citada in citadas(&texto) {
            if !ordenes.contains(&citada.as_str()) {
                discrepancias.push(format!(
                    "{relativo} manda teclear 'cargo xtask {citada}', y esa orden NO existe"
                ));
            }
        }
    }

    Ok(discrepancias)
}

/// Lo que un documento debe decir, en la misma linea, para citar una orden que
/// todavia no existe.
///
/// # Por que la salida es esta y no una lista de excepciones
///
/// RPT-005 §9.3 diseña `cargo xtask conformidad` con todo detalle y esa orden
/// nunca se construyo. El documento no esta mal —es un diseño, y los diseños se
/// escriben antes— pero **se lee como una instruccion**, y quien lo copie
/// tecleara algo que falla.
///
/// Una lista de excepciones dentro de esta barrera lo habria callado sin
/// arreglar nada: el lector del reporte seguiria copiando el comando. Exigir el
/// aviso **en la linea de la cita** pone la advertencia donde esta el daño. La
/// escapatoria mejora el documento en lugar de debilitar la comprobacion.
const MARCADOR: &str = "NO EXISTE TODAVIA";

/// Nombres de orden citados como `cargo xtask <nombre>` en un texto.
///
/// # Que se descarta a proposito
///
/// Un marcador como `cargo xtask <orden>` no nombra ninguna orden: es la forma
/// de referirse a todas. Se descarta porque el nombre no empieza por letra
/// minuscula, y sin eso la barrera acusaria al reporte que la explica.
///
/// Y se descarta la linea entera si lleva [`MARCADOR`].
fn citadas(texto: &str) -> Vec<String> {
    const PREFIJO: &str = "cargo xtask ";

    let mut encontradas = Vec::new();

    for linea in texto.lines() {
        if linea.contains(MARCADOR) {
            continue;
        }

        let mut resto = linea;
        while let Some(posicion) = resto.find(PREFIJO) {
            resto = &resto[posicion + PREFIJO.len()..];

            let nombre: String = resto
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                .collect();

            // Un guion al final viene de una linea partida, no del nombre.
            let nombre = nombre.trim_end_matches('-').to_owned();

            if !nombre.is_empty() && !encontradas.contains(&nombre) {
                encontradas.push(nombre);
            }
        }
    }

    encontradas
}

/// Acumula los `.md` de un arbol, sin seguir enlaces.
fn recorrer(directorio: &Path, acumulado: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let entradas = std::fs::read_dir(directorio)
        .map_err(|error| format!("no se pudo leer {}: {error}", directorio.display()))?;

    for entrada in entradas {
        let entrada = entrada
            .map_err(|error| format!("entrada ilegible en {}: {error}", directorio.display()))?;
        let ruta = entrada.path();

        if ruta.is_dir() {
            recorrer(&ruta, acumulado)?;
        } else if ruta.extension().is_some_and(|ext| ext == EXTENSION) {
            acumulado.push(ruta);
        }
    }

    Ok(())
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn se_reconoce_una_orden_citada_en_un_bloque_de_codigo() {
        let texto = "Antes de nada:\n\n```bash\ncargo xtask tablero\n```\n";
        assert_eq!(citadas(texto), vec!["tablero".to_owned()]);
    }

    #[test]
    fn se_reconocen_varias_y_sin_repetir() {
        let texto = "cargo xtask verificar crates\ncargo xtask tablero\ncargo xtask tablero\n";
        assert_eq!(
            citadas(texto),
            vec!["verificar".to_owned(), "tablero".to_owned()]
        );
    }

    #[test]
    fn el_guion_del_nombre_se_conserva() {
        assert_eq!(
            citadas("cargo xtask probar-instalador"),
            vec!["probar-instalador".to_owned()]
        );
        assert_eq!(
            citadas("cargo xtask vectores-ipc"),
            vec!["vectores-ipc".to_owned()]
        );
    }

    /// El marcador que significa «cualquier orden» no es una orden.
    ///
    /// Sin esto, RPT-065 §6 —el reporte que **acuna** esta barrera, y que
    /// escribe `cargo xtask <orden>` para hablar de todas— seria la primera
    /// acusada por ella.
    #[test]
    fn un_marcador_no_es_una_orden() {
        assert!(citadas("todo `cargo xtask <orden>` citado en docs/").is_empty());
        assert!(citadas("cargo xtask ").is_empty());
        assert!(citadas("cargo xtask").is_empty());
    }

    /// Una linea partida deja un guion colgando que no es parte del nombre.
    #[test]
    fn un_guion_de_corte_de_linea_no_entra_en_el_nombre() {
        assert_eq!(
            citadas("cargo xtask probar-\ninstalador"),
            vec!["probar".to_owned()]
        );
    }

    /// Una orden **diseñada y no construida** se puede citar si el documento lo
    /// dice en la misma linea. Es el caso de RPT-005 §9.3.
    #[test]
    fn una_orden_declarada_inexistente_no_se_acusa() {
        let texto = "- `cargo xtask conformidad` (NO EXISTE TODAVIA: PA-121) emite el fichero.";
        assert!(citadas(texto).is_empty());
    }

    /// Y el aviso vale para su linea, no para el documento entero.
    ///
    /// Sin esto, un solo `NO EXISTE TODAVIA` en la cabecera de un reporte
    /// desactivaria la barrera en las cuarenta lineas siguientes, que es como
    /// mueren las comprobaciones: no se apagan, se les amplia el alcance.
    #[test]
    fn el_aviso_no_se_derrama_a_las_lineas_siguientes() {
        let texto = "`cargo xtask conformidad` (NO EXISTE TODAVIA)\ncargo xtask fantasma\n";
        assert_eq!(citadas(texto), vec!["fantasma".to_owned()]);
    }

    /// La direccion que justifica el modulo: documentado y ya no existe.
    #[test]
    fn una_orden_retirada_del_binario_se_acusa() {
        let citadas_en_doc = citadas("```bash\ncargo xtask fantasma\n```");
        let existentes = ["tablero", "manual"];
        let huerfanas: Vec<&String> = citadas_en_doc
            .iter()
            .filter(|nombre| !existentes.contains(&nombre.as_str()))
            .collect();

        assert_eq!(huerfanas.len(), 1, "una orden retirada debe salir a la luz");
    }

    /// Prueba de fuego sobre el arbol de verdad, no sobre un texto inventado.
    ///
    /// Es la unica que puede fallar cuando alguien anada una orden y olvide el
    /// manual, que es exactamente el dia para el que existe todo esto.
    ///
    /// # La lista sale de `ORDENES`, no se reescribe aqui
    ///
    /// Copiar los nombres en la prueba habria creado un **cuarto** indice
    /// escrito a mano, y esta vez dentro de la barrera que existe para cazar
    /// esos. La prueba habria seguido en verde con una orden nueva sin
    /// documentar, que es literalmente el caso que comprueba.
    #[test]
    fn el_manual_y_el_binario_dicen_lo_mismo_hoy() {
        let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask cuelga de la raiz del repositorio");

        let ordenes: Vec<&str> = crate::ORDENES.iter().map(|orden| orden.nombre).collect();

        match cotejar(raiz, &ordenes) {
            Ok(discrepancias) => assert!(
                discrepancias.is_empty(),
                "el manual y el binario se han separado:\n{}",
                discrepancias.join("\n")
            ),
            // Sin `docs/` no se puede afirmar nada, y fingir que si es el
            // fallo que esta barrera existe para no cometer.
            Err(motivo) => panic!("no se pudo cotejar: {motivo}"),
        }
    }
}
