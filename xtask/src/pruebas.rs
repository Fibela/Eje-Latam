//! Pruebas del guardian.
//!
//! Cada caso corresponde a un modo de fallo observado o previsto. El primer
//! bloque reproduce el defecto exacto que tenia la version PowerShell.

use std::path::Path;

use crate::exclusion::lineas_de_prueba;
use crate::guardian::{Comprobacion, Hallazgo, analizar, comprobaciones};

/// Los patrones del guardian, con la garantia de que hay alguno.
///
/// La asercion no es ceremonia: con la lista vacia, **toda** prueba de este
/// fichero pasaria sin comprobar nada. Un verificador que no puede verificar no
/// se lee como conforme (RPT-006 §4).
fn patrones() -> Vec<Comprobacion> {
    let patrones = comprobaciones().unwrap_or_else(|_| Vec::new());
    assert!(
        !patrones.is_empty(),
        "sin patrones, cualquier fuente pasaria por limpia"
    );
    patrones
}

fn etiquetas(fuente: &str) -> Vec<&'static str> {
    let comprobaciones = comprobaciones().unwrap_or_else(|_| Vec::new());
    assert!(
        !comprobaciones.is_empty(),
        "las comprobaciones deben compilar"
    );
    analizar(Path::new("prueba.rs"), fuente, &comprobaciones)
        .into_iter()
        .map(|hallazgo: Hallazgo| hallazgo.etiqueta)
        .collect()
}

fn hay_hallazgos(fuente: &str) -> bool {
    !etiquetas(fuente).is_empty()
}

// ---------------------------------------------------------------------------
// El defecto de la version PowerShell
// ---------------------------------------------------------------------------

#[test]
fn detecta_violacion_posterior_a_un_modulo_de_pruebas_intermedio() {
    // La version PowerShell hacia `break` al ver `#[cfg(test)]` y abandonaba el
    // fichero completo. Esta violacion, situada DESPUES del modulo de pruebas,
    // quedaba sin revisar y el guardian informaba conformidad.
    let fuente = r#"
pub fn primera() -> u32 { 1 }

#[cfg(test)]
mod pruebas_intermedias {
    #[test]
    fn algo() {
        let mock = 1;
        assert_eq!(mock, 1);
    }
}

pub fn segunda() { todo!() }
"#;

    let encontradas = etiquetas(fuente);
    assert!(
        encontradas.contains(&"Implementaciones inconclusas"),
        "el todo!() posterior al modulo de pruebas debe detectarse; se detecto: {encontradas:?}"
    );
}

#[test]
fn no_marca_el_mock_legitimo_dentro_del_modulo_de_pruebas() {
    let fuente = r#"
pub fn produccion() -> u32 { 1 }

#[cfg(test)]
mod pruebas {
    #[test]
    fn con_mock_y_endpoint() {
        let mock = "http://localhost:8080";
        let _ = mock;
    }
}
"#;
    assert!(
        !hay_hallazgos(fuente),
        "mocks y endpoints dentro de #[cfg(test)] son legitimos"
    );
}

// ---------------------------------------------------------------------------
// Robustez del conteo de llaves
// ---------------------------------------------------------------------------

#[test]
fn una_llave_dentro_de_una_cadena_no_cierra_el_bloque_de_pruebas() {
    let fuente = r#"
#[cfg(test)]
mod pruebas {
    #[test]
    fn llave_en_cadena() {
        let s = "}";
        let mock = s;
        let _ = mock;
    }
}
"#;
    assert!(
        !hay_hallazgos(fuente),
        "una llave de cierre dentro de una cadena no debe terminar la exclusion"
    );
}

#[test]
fn una_llave_dentro_de_un_literal_de_caracter_no_cierra_el_bloque() {
    let fuente = r#"
#[cfg(test)]
mod pruebas {
    #[test]
    fn llave_en_caracter() {
        let c = '}';
        let dummy = c;
        let _ = dummy;
    }
}
"#;
    assert!(!hay_hallazgos(fuente));
}

#[test]
fn una_llave_dentro_de_un_comentario_no_cierra_el_bloque() {
    let fuente = r#"
#[cfg(test)]
mod pruebas {
    // }
    /* } */
    #[test]
    fn algo() {
        let mock = 1;
        let _ = mock;
    }
}
"#;
    assert!(!hay_hallazgos(fuente));
}

#[test]
fn una_llave_en_cadena_cruda_no_cierra_el_bloque() {
    let fuente = r###"
#[cfg(test)]
mod pruebas {
    #[test]
    fn cruda() {
        let s = r#"} "no cierra" }"#;
        let stub_ = s;
        let _ = stub_;
    }
}
"###;
    assert!(!hay_hallazgos(fuente));
}

#[test]
fn un_tiempo_de_vida_no_se_confunde_con_literal_de_caracter() {
    let fuente = r#"
pub struct Envoltorio<'a> {
    pub dato: &'a str,
}

pub fn inconcluso() { todo!() }
"#;
    assert!(
        etiquetas(fuente).contains(&"Implementaciones inconclusas"),
        "un tiempo de vida no debe desbaratar el analisis lexico"
    );
}

#[test]
fn cfg_test_sobre_un_use_no_abre_bloque() {
    // `#[cfg(test)] use ...;` no lleva llaves. Si el guardian lo tratara como
    // apertura de bloque, excluiria el resto del fichero.
    let fuente = r#"
#[cfg(test)]
use std::collections::HashMap;

pub fn inconcluso() { unimplemented!() }
"#;
    assert!(
        etiquetas(fuente).contains(&"Implementaciones inconclusas"),
        "un atributo sobre un `use` no debe excluir el resto del fichero"
    );
}

#[test]
fn modulos_de_prueba_anidados_se_excluyen_por_completo() {
    let fuente = r#"
#[cfg(test)]
mod pruebas {
    mod interno {
        fn ayuda() {
            let mock = 1;
            let _ = mock;
        }
    }
    #[test]
    fn algo() {
        let dummy = 2;
        let _ = dummy;
    }
}

pub fn despues() { todo!() }
"#;
    let encontradas = etiquetas(fuente);
    assert_eq!(
        encontradas,
        vec!["Implementaciones inconclusas"],
        "solo debe detectarse la violacion posterior al bloque anidado"
    );
}

// ---------------------------------------------------------------------------
// Cobertura de cada comprobacion
// ---------------------------------------------------------------------------

#[test]
fn detecta_cada_categoria_en_codigo_de_produccion() {
    let casos: [(&str, &str); 6] = [
        ("pub fn a() { todo!() }", "Implementaciones inconclusas"),
        (
            r#"pub fn b() { panic!("TODO: pendiente") }"#,
            "Panicos con marcador pendiente",
        ),
        ("// TODO: revisar esto", "Marcadores de trabajo pendiente"),
        ("pub enum E { NoImplementado }", "Rutas sin implementar"),
        (
            "pub fn mock_switch() -> bool { true }",
            "Datos simulados fuera de pruebas",
        ),
        (
            r#"pub const P: &str = "127.0.0.1";"#,
            "Puntos finales y credenciales de ejemplo",
        ),
    ];

    for (fuente, esperada) in casos {
        let encontradas = etiquetas(fuente);
        assert!(
            encontradas.contains(&esperada),
            "'{fuente}' deberia activar '{esperada}', se activo: {encontradas:?}"
        );
    }
}

#[test]
fn no_soportado_es_admisible_y_no_implementado_no() {
    // RPT-003 §9.5: `NoImplementado` significa pendiente y debe detectarse;
    // `NoSoportado` describe una operacion que no existe por diseno.
    assert!(hay_hallazgos("pub enum E { NoImplementado }"));
    assert!(!hay_hallazgos("pub enum E { NoSoportado }"));
}

// ---------------------------------------------------------------------------
// El guardian mira el codigo, no la prosa — RPT-076, PA-129
// ---------------------------------------------------------------------------

/// Un marcador de trabajo pendiente **sigue** cazandose, y vive en un comentario.
///
/// Es la prueba que estuvo a punto de perderse. Al quitar los comentarios para
/// arreglar los falsos positivos, esta comprobacion —cuyo patron es literalmente
/// `//\s*TODO`— habria dejado de encontrar nada, en silencio y para siempre.
///
/// Lo cazo la prueba de categorias que ya existia. De ahi que cada comprobacion
/// declare su ambito en lugar de compartir uno.
#[test]
fn un_marcador_pendiente_se_caza_aunque_este_en_un_comentario() {
    let hallazgos = analizar(
        Path::new("pendiente.rs"),
        "pub fn a() -> u8 { 1 } // TODO: revisar el techo\n",
        &patrones(),
    );

    assert_eq!(
        hallazgos.len(),
        1,
        "un marcador pendiente vive en un comentario: mirarlo en el codigo seria \
         no mirarlo nunca. {hallazgos:?}"
    );
    assert_eq!(hallazgos[0].etiqueta, "Marcadores de trabajo pendiente");
}

/// Un comentario que **explica** un patron prohibido no lo comete.
///
/// El guardian acuso dos veces en dos dias a la prosa que explica el diseño. La
/// salida facil era reescribir la prosa; esta prueba fija la otra.
#[test]
fn un_patron_citado_en_un_comentario_no_es_un_hallazgo() {
    let fuente = "\
// Este estado degradado no es un mock: se declara en lugar de fingir.
/* Y tampoco hay un 127.0.0.1 por omision, que seria peor. */
pub fn honesta() -> u8 {
    7
}
";

    let hallazgos = analizar(Path::new("prosa.rs"), fuente, &patrones());

    assert!(
        hallazgos.is_empty(),
        "el guardian acusa a la prosa que lo explica: {hallazgos:?}"
    );
}

/// Y una cadena **si** se sigue mirando.
///
/// Es la mitad que impide que el arreglo se convierta en un guardian ciego: lo
/// que no es codigo es el comentario; una cadena literal si lo es, y
/// `bind("127.0.0.1:5514")` es exactamente lo que la regla existe para cazar.
#[test]
fn un_punto_final_dentro_de_una_cadena_sigue_siendo_un_hallazgo() {
    let fuente = "\
pub fn escuchar() -> &'static str {
    \"127.0.0.1:5514\"
}
";

    let hallazgos = analizar(Path::new("cadena.rs"), fuente, &patrones());

    assert_eq!(
        hallazgos.len(),
        1,
        "una cadena es codigo y debe seguir cazandose: {hallazgos:?}"
    );
}

/// Codigo y comentario en la misma linea: se mira lo de la izquierda.
///
/// Sin esto, bastaria con poner un comentario detras para esconder cualquier
/// violacion — que es como un guardian se vuelve decorativo.
#[test]
fn el_codigo_anterior_a_un_comentario_sigue_mirandose() {
    let fuente = "\
pub fn escuchar() -> &'static str {
    \"127.0.0.1:5514\" // aqui no hay nada que ver
}
";

    let hallazgos = analizar(Path::new("mixta.rs"), fuente, &patrones());

    assert_eq!(hallazgos.len(), 1, "{hallazgos:?}");
}

#[test]
fn codigo_limpio_no_produce_hallazgos() {
    let fuente = r#"
//! Modulo de ejemplo sin nada pendiente.

/// Suma dos numeros.
pub fn sumar(a: u32, b: u32) -> u32 {
    a + b
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn suma() {
        assert_eq!(sumar(2, 2), 4);
    }
}
"#;
    assert!(!hay_hallazgos(fuente));
}

// ---------------------------------------------------------------------------
// Exclusion linea a linea
// ---------------------------------------------------------------------------

#[test]
fn la_exclusion_cubre_exactamente_el_bloque() {
    let fuente = "pub fn a() {}\n#[cfg(test)]\nmod p {\n    fn b() {}\n}\npub fn c() {}\n";
    let excluidas = lineas_de_prueba(fuente);

    assert_eq!(excluidas.len(), 6);
    assert!(!excluidas[0], "linea 1 es produccion");
    assert!(excluidas[2], "linea 3 abre el bloque de pruebas");
    assert!(excluidas[3], "linea 4 esta dentro del bloque");
    assert!(excluidas[4], "linea 5 cierra el bloque");
    assert!(!excluidas[5], "linea 6 vuelve a ser produccion");
}
