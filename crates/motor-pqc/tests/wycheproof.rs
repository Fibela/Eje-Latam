//! Ejecución de los vectores adversarios de Wycheproof.
//!
//! RPT-005 §4.3. **Este fichero es el que convierte PA-17 de custodia en
//! conformidad.** `vectores.rs` comprueba que los ficheros están presentes y
//! anclados; aquí se ejecutan contra la implementación.
//!
//! # Por qué Wycheproof y no solo ACVP
//!
//! ACVP comprueba que se **calcula** lo correcto. Wycheproof comprueba que se
//! **rechaza** lo incorrecto. De los 210 casos de ML-DSA-65, **131 son inválidos**:
//! condiciones de frontera, violaciones de norma infinita, claves públicas nulas,
//! codificaciones no canónicas.
//!
//! CVE-2026-24850 —maleabilidad de firma en el verificador de ML-DSA— pasaba
//! ACVP al completo y solo se detectó con un caso de esta familia.
//!
//! # Un caso invalidado que se acepta es un fallo, no una discrepancia
//!
//! Estas pruebas no toleran «casi todos pasan». Un verificador que acepta una
//! firma que Wycheproof marca `invalid` tiene un defecto explotable.

// Las restricciones del workspace protegen la ruta de producción; en una suite de
// conformidad, abortar con el identificador del caso es lo deseado.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use ml_dsa::{KeyInit as _, MlDsa65, VerifyingKey};
use ml_kem::{Decapsulate as _, FromSeed as _, MlKem768};
use serde_json::Value;

fn cargar(nombre: &str) -> Value {
    let ruta = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("vectores")
        .join(nombre);

    let bruto = std::fs::read_to_string(&ruta).unwrap_or_else(|error| {
        panic!(
            "no se pudo leer {}: {error}.\nEjecute `cargo xtask vectores`.",
            ruta.display()
        )
    });

    serde_json::from_str(&bruto)
        .unwrap_or_else(|error| panic!("{} no es JSON valido: {error}", ruta.display()))
}

fn hexadecimal(texto: &str) -> Vec<u8> {
    (0..texto.len())
        .step_by(2)
        .map(|indice| {
            u8::from_str_radix(&texto[indice..indice + 2], 16)
                .unwrap_or_else(|error| panic!("hexadecimal invalido en '{texto}': {error}"))
        })
        .collect()
}

fn campo<'a>(valor: &'a Value, nombre: &str) -> &'a str {
    valor[nombre]
        .as_str()
        .unwrap_or_else(|| panic!("falta el campo '{nombre}' en el vector"))
}

/// Recuento de un recorrido de vectores.
struct Recuento {
    validos: usize,
    invalidos: usize,
}

// ---------------------------------------------------------------------------
// ML-DSA-65
// ---------------------------------------------------------------------------

#[test]
fn wycheproof_ml_dsa_65_verificacion() {
    let vectores = cargar("wycheproof_mldsa_65_verify_test.json");
    let grupos = vectores["testGroups"]
        .as_array()
        .expect("testGroups debe ser una lista");

    let mut recuento = Recuento {
        validos: 0,
        invalidos: 0,
    };
    let mut fallos: Vec<String> = Vec::new();

    for grupo in grupos {
        let clave_bruta = hexadecimal(campo(grupo, "publicKey"));

        // Una clave pública de longitud incorrecta debe rechazarse en la
        // decodificación, antes de llegar al verificador. Wycheproof incluye
        // grupos así, marcados `IncorrectPublicKeyLength`, y todos sus casos
        // esperan `invalid`. Que no decodifique es el comportamiento correcto:
        // el fallo sería que algún caso de ese grupo esperase `valid`.
        let clave = match <[u8; 1952]>::try_from(clave_bruta.as_slice()) {
            Ok(arreglo) => Some(VerifyingKey::<MlDsa65>::new(&arreglo.into())),
            Err(_) => None,
        };

        for caso in grupo["tests"].as_array().expect("tests debe ser una lista") {
            let identificador = caso["tcId"].as_u64().unwrap_or_default();
            let esperado_valido = campo(caso, "result") == "valid";
            let mensaje = hexadecimal(campo(caso, "msg"));
            let firma_bruta = hexadecimal(campo(caso, "sig"));
            let comentario = caso["comment"].as_str().unwrap_or("");

            // El contexto de FIPS 204 es opcional y algunos casos lo traen.
            // Ignorarlo hacía fallar los casos `non-empty context` y
            // `longest context` — un defecto del ejecutor, no del verificador.
            let contexto = caso["ctx"].as_str().map_or_else(Vec::new, hexadecimal);

            if esperado_valido {
                recuento.validos += 1;
            } else {
                recuento.invalidos += 1;
            }

            let obtenido_valido = clave.as_ref().is_some_and(|clave| {
                <[u8; 3309]>::try_from(firma_bruta.as_slice())
                    .ok()
                    .and_then(|bytes| ml_dsa::Signature::<MlDsa65>::decode(&bytes.into()))
                    .is_some_and(|firma| clave.verify_with_context(&mensaje, &contexto, &firma))
            });

            if obtenido_valido != esperado_valido {
                fallos.push(format!(
                    "tcId {identificador} ({comentario}): se esperaba {} y se obtuvo {}",
                    if esperado_valido { "valid" } else { "invalid" },
                    if obtenido_valido { "valid" } else { "invalid" },
                ));
            }
        }
    }

    assert!(
        recuento.invalidos >= 100,
        "se esperaban al menos 100 casos invalidos; se recorrieron {}. \
         Un conjunto de vectores sin casos adversarios no comprueba nada.",
        recuento.invalidos
    );

    assert!(
        fallos.is_empty(),
        "{} de {} casos divergen ({} validos, {} invalidos):\n  {}",
        fallos.len(),
        recuento.validos + recuento.invalidos,
        recuento.validos,
        recuento.invalidos,
        fallos.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// ML-KEM-768
// ---------------------------------------------------------------------------

#[test]
fn wycheproof_ml_kem_768_desencapsulado() {
    let vectores = cargar("wycheproof_mlkem_768_test.json");
    let grupos = vectores["testGroups"]
        .as_array()
        .expect("testGroups debe ser una lista");

    let mut recuento = Recuento {
        validos: 0,
        invalidos: 0,
    };
    let mut fallos: Vec<String> = Vec::new();

    for grupo in grupos {
        for caso in grupo["tests"].as_array().expect("tests debe ser una lista") {
            let identificador = caso["tcId"].as_u64().unwrap_or_default();
            let esperado_valido = campo(caso, "result") == "valid";
            let comentario = caso["flags"].to_string();

            let semilla = hexadecimal(campo(caso, "seed"));
            let cifrado = hexadecimal(campo(caso, "c"));
            let compartido_esperado = hexadecimal(campo(caso, "K"));

            if esperado_valido {
                recuento.validos += 1;
            } else {
                recuento.invalidos += 1;
            }

            let Ok(semilla_fija) = <[u8; 64]>::try_from(semilla.as_slice()) else {
                if esperado_valido {
                    fallos.push(format!(
                        "tcId {identificador}: semilla de longitud invalida"
                    ));
                }
                continue;
            };

            let (privada, publica) = MlKem768::from_seed(&semilla_fija.into());

            // La clave de encapsulado declarada debe coincidir con la derivada.
            let esperada = hexadecimal(campo(caso, "ek"));
            if ml_kem::KeyExport::to_bytes(&publica).as_slice() != esperada.as_slice()
                && esperado_valido
            {
                fallos.push(format!(
                    "tcId {identificador}: la clave derivada de la semilla no coincide con 'ek'"
                ));
                continue;
            }

            // ML-KEM no falla al desencapsular: ante un texto cifrado invalido
            // devuelve un secreto implicito distinto (rechazo implicito de
            // FIPS 203). Por eso se compara el secreto, no un Result.
            let obtenido = privada.decapsulate_slice(&cifrado).ok();
            let coincide = obtenido
                .as_ref()
                .is_some_and(|secreto| secreto.as_slice() == compartido_esperado.as_slice());

            if coincide != esperado_valido {
                fallos.push(format!(
                    "tcId {identificador} ({comentario}): se esperaba {} y se obtuvo {}",
                    if esperado_valido { "valid" } else { "invalid" },
                    if coincide { "valid" } else { "invalid" },
                ));
            }
        }
    }

    assert!(
        recuento.invalidos >= 20,
        "se esperaban al menos 20 casos invalidos; se recorrieron {}",
        recuento.invalidos
    );

    assert!(
        fallos.is_empty(),
        "{} de {} casos divergen ({} validos, {} invalidos):\n  {}",
        fallos.len(),
        recuento.validos + recuento.invalidos,
        recuento.validos,
        recuento.invalidos,
        fallos.join("\n  ")
    );
}
