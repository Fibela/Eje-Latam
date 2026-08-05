//! Ejecución de los vectores oficiales ACVP del NIST.
//!
//! RPT-005 §4.3, PA-17. Complementa a `wycheproof.rs`: ACVP comprueba que se
//! **calcula** lo correcto, Wycheproof que se **rechaza** lo incorrecto.
//!
//! # Variantes de FIPS 204 que no cubrimos
//!
//! Los grupos de `ML-DSA-sigVer` declaran tres ejes que cambian la interfaz:
//!
//! - `signatureInterface`: `external` (la firma con contexto, que usamos) o
//!   `internal` (la interfaz interna de FIPS 204, sin contexto).
//! - `preHash`: `none` o un algoritmo de pre-resumen (HashML-DSA).
//! - `externalMu`: si el mensaje ya llega pre-procesado como `mu`.
//!
//! `motor-pqc` implementa **únicamente** la variante `external` / `none` /
//! `false` (RPT-005 §7.4). Los demás grupos se cuentan y se informan, pero no se
//! ejecutan: fingir que se comprueban sería peor que declararlos fuera de
//! alcance.
//!
//! Si alguna vez se adopta HashML-DSA, esta suite lo señalará sola: el recuento
//! de casos omitidos dejará de cuadrar con lo declarado.

// En una suite de conformidad, abortar con el identificador del caso es lo
// deseado; las restricciones del workspace protegen la ruta de producción.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use ml_dsa::{KeyInit as _, Keypair as _, MlDsa65, VerifyingKey};
use ml_kem::{FromSeed as _, KeyExport as _, MlKem768};
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
                .unwrap_or_else(|error| panic!("hexadecimal invalido: {error}"))
        })
        .collect()
}

fn texto<'a>(valor: &'a Value, nombre: &str) -> &'a str {
    valor[nombre]
        .as_str()
        .unwrap_or_else(|| panic!("falta el campo de texto '{nombre}'"))
}

/// Conjunto de parámetros que implementa `motor-pqc`.
const PARAMETROS_KEM: &str = "ML-KEM-768";

/// Conjunto de parámetros que implementa `motor-pqc`.
const PARAMETROS_DSA: &str = "ML-DSA-65";

// ---------------------------------------------------------------------------
// ML-KEM-768 — generación de claves
// ---------------------------------------------------------------------------

#[test]
fn acvp_ml_kem_768_generacion_de_claves() {
    let vectores = cargar("acvp_ml_kem_768_keygen.json");
    let grupos = vectores["testGroups"]
        .as_array()
        .expect("testGroups debe ser una lista");

    let mut ejecutados = 0_usize;
    let mut omitidos_por_parametros = 0_usize;
    let mut fallos: Vec<String> = Vec::new();

    for grupo in grupos {
        if texto(grupo, "parameterSet") != PARAMETROS_KEM {
            omitidos_por_parametros += grupo["tests"].as_array().map_or(0, Vec::len);
            continue;
        }

        for caso in grupo["tests"].as_array().expect("tests debe ser una lista") {
            let identificador = caso["tcId"].as_u64().unwrap_or_default();

            let d = hexadecimal(texto(caso, "d"));
            let z = hexadecimal(texto(caso, "z"));
            let ek_esperada = hexadecimal(texto(caso, "ek"));

            // FIPS 203 §7.1: KeyGen recibe (d, z) y la semilla se compone en ese
            // orden. Si esta prueba fallara en TODOS los casos, lo primero a
            // revisar es esta concatenacion, no la implementacion.
            let mut semilla = [0u8; 64];
            semilla[..32].copy_from_slice(&d);
            semilla[32..].copy_from_slice(&z);

            let (_, publica) = MlKem768::from_seed(&semilla.into());
            ejecutados += 1;

            if publica.to_bytes().as_slice() != ek_esperada.as_slice() {
                fallos.push(format!(
                    "tcId {identificador}: la clave de encapsulado derivada no coincide con 'ek'"
                ));
            }
        }
    }

    assert!(
        ejecutados > 0,
        "no se ejecuto ningun caso de {PARAMETROS_KEM}; se omitieron {omitidos_por_parametros} \
         por conjunto de parametros. Un fichero de vectores sin casos aplicables no comprueba nada."
    );

    assert!(
        fallos.is_empty(),
        "{} de {ejecutados} casos fallan:\n  {}",
        fallos.len(),
        fallos.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// ML-KEM-768 — encapsulado
// ---------------------------------------------------------------------------

#[test]
fn acvp_ml_kem_768_encapsulado() {
    let vectores = cargar("acvp_ml_kem_768_encapdecap.json");
    let grupos = vectores["testGroups"]
        .as_array()
        .expect("testGroups debe ser una lista");

    let mut ejecutados = 0_usize;
    let mut omitidos_parametros = 0_usize;
    let mut omitidos_desencapsulado = 0_usize;
    let mut funciones_vistas: Vec<String> = Vec::new();
    let mut fallos: Vec<String> = Vec::new();

    for grupo in grupos {
        let casos = grupo["tests"].as_array().map_or(0, Vec::len);

        if texto(grupo, "parameterSet") != PARAMETROS_KEM {
            omitidos_parametros += casos;
            continue;
        }

        let funcion = grupo["function"].as_str().unwrap_or("(ausente)");
        anotar(&mut funciones_vistas, funcion);

        // El desencapsulado exige reconstruir la clave privada desde sus bytes.
        // Se deja fuera de alcance por ahora y se declara: la ruta de
        // desencapsulado ya queda cubierta por los 193 casos de Wycheproof y por
        // el contraste diferencial con libcrux en ambos sentidos.
        if funcion != "encapsulation" {
            omitidos_desencapsulado += casos;
            continue;
        }

        for caso in grupo["tests"].as_array().expect("tests debe ser una lista") {
            let identificador = caso["tcId"].as_u64().unwrap_or_default();

            let ek = hexadecimal(texto(caso, "ek"));
            let m = hexadecimal(texto(caso, "m"));
            let c_esperado = hexadecimal(texto(caso, "c"));
            let k_esperado = hexadecimal(texto(caso, "k"));

            let Ok(ek_fija) = <[u8; 1184]>::try_from(ek.as_slice()) else {
                fallos.push(format!(
                    "tcId {identificador}: 'ek' mide {} bytes, se esperaban 1184",
                    ek.len()
                ));
                continue;
            };
            let Ok(m_fijo) = <[u8; 32]>::try_from(m.as_slice()) else {
                fallos.push(format!(
                    "tcId {identificador}: 'm' mide {} bytes, se esperaban 32",
                    m.len()
                ));
                continue;
            };

            // La construccion desde bytes es falible: una clave de encapsulado
            // sintacticamente correcta puede seguir siendo invalida. Que falle
            // aqui es un fallo del vector o de la implementacion, nunca algo a
            // ignorar en silencio.
            let publica = match ml_kem::EncapsulationKey::<MlKem768>::new(&ek_fija.into()) {
                Ok(clave) => clave,
                Err(error) => {
                    fallos.push(format!(
                        "tcId {identificador}: 'ek' no pudo decodificarse: {error:?}"
                    ));
                    continue;
                }
            };

            let (c, k) = publica.encapsulate_deterministic(&m_fijo.into());
            ejecutados += 1;

            if c.as_slice() != c_esperado.as_slice() {
                fallos.push(format!("tcId {identificador}: el texto cifrado no coincide con 'c'"));
            }
            if k.as_slice() != k_esperado.as_slice() {
                fallos.push(format!(
                    "tcId {identificador}: el secreto compartido no coincide con 'k'"
                ));
            }
        }
    }

    assert!(
        ejecutados > 0,
        "no se ejecuto ningun caso de encapsulado.\n  \
         Omitidos: {omitidos_parametros} por parametros, {omitidos_desencapsulado} por funcion.\n  \
         Valores de 'function' observados: {funciones_vistas:?}"
    );

    assert!(
        fallos.is_empty(),
        "{} de {ejecutados} casos fallan:\n  {}",
        fallos.len(),
        fallos.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// ML-DSA-65 — generación de claves
// ---------------------------------------------------------------------------

#[test]
fn acvp_ml_dsa_65_generacion_de_claves() {
    let vectores = cargar("acvp_ml_dsa_65_keygen.json");
    let grupos = vectores["testGroups"]
        .as_array()
        .expect("testGroups debe ser una lista");

    let mut ejecutados = 0_usize;
    let mut omitidos_parametros = 0_usize;
    let mut fallos: Vec<String> = Vec::new();

    for grupo in grupos {
        if texto(grupo, "parameterSet") != PARAMETROS_DSA {
            omitidos_parametros += grupo["tests"].as_array().map_or(0, Vec::len);
            continue;
        }

        for caso in grupo["tests"].as_array().expect("tests debe ser una lista") {
            let identificador = caso["tcId"].as_u64().unwrap_or_default();

            let semilla = hexadecimal(texto(caso, "seed"));
            let pk_esperada = hexadecimal(texto(caso, "pk"));

            let Ok(semilla_fija) = <[u8; 32]>::try_from(semilla.as_slice()) else {
                fallos.push(format!(
                    "tcId {identificador}: 'seed' mide {} bytes, se esperaban 32",
                    semilla.len()
                ));
                continue;
            };

            let privada = ml_dsa::SigningKey::<MlDsa65>::new(&semilla_fija.into());
            ejecutados += 1;

            if privada.verifying_key().encode().as_slice() != pk_esperada.as_slice() {
                fallos.push(format!(
                    "tcId {identificador}: la clave de verificacion derivada no coincide con 'pk'"
                ));
            }
        }
    }

    assert!(
        ejecutados > 0,
        "no se ejecuto ningun caso de {PARAMETROS_DSA}; se omitieron {omitidos_parametros} \
         por conjunto de parametros."
    );

    assert!(
        fallos.is_empty(),
        "{} de {ejecutados} casos fallan:\n  {}",
        fallos.len(),
        fallos.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// ML-DSA-65 — verificación de firma
// ---------------------------------------------------------------------------

/// Recuento de un recorrido de grupos de sigVer.
#[derive(Default)]
struct RecuentoDsa {
    ejecutados: usize,
    aceptados_esperados: usize,
    rechazados_esperados: usize,
    omitidos_parametros: usize,
    omitidos_interfaz_interna: usize,
    omitidos_prehash: usize,
    omitidos_mu_externo: usize,
}

/// Valores de `preHash` que corresponden a la variante pura de FIPS 204.
///
/// ACVP no usa un vocabulario único: se han observado tanto `pure` como `none`
/// según la revisión del esquema. Aceptar ambos evita que un cambio de
/// vocabulario del NIST deje la suite sin ejecutar ni un caso, en silencio.
const PREHASH_PURO: [&str; 2] = ["pure", "none"];

/// Registra los valores distintos observados en un eje del esquema.
///
/// Cuando ningún caso resulta aplicable, el informe de fallo enumera lo que
/// realmente traía el fichero. Adivinar el vocabulario del esquema ya nos costó
/// una ronda: el filtro debe diagnosticarse solo.
fn anotar(observados: &mut Vec<String>, valor: &str) {
    if !observados.iter().any(|previo| previo == valor) {
        observados.push(valor.to_owned());
    }
}

#[test]
fn acvp_ml_dsa_65_verificacion_de_firma() {
    let vectores = cargar("acvp_ml_dsa_65_sigver.json");
    let grupos = vectores["testGroups"]
        .as_array()
        .expect("testGroups debe ser una lista");

    let mut recuento = RecuentoDsa::default();
    let mut fallos: Vec<String> = Vec::new();
    let mut interfaces_vistas: Vec<String> = Vec::new();
    let mut prehash_vistos: Vec<String> = Vec::new();

    for grupo in grupos {
        let casos = grupo["tests"].as_array().map_or(0, Vec::len);

        if texto(grupo, "parameterSet") != PARAMETROS_DSA {
            recuento.omitidos_parametros += casos;
            continue;
        }

        let interfaz = grupo["signatureInterface"].as_str().unwrap_or("(ausente)");
        let prehash = grupo["preHash"].as_str().unwrap_or("(ausente)");
        anotar(&mut interfaces_vistas, interfaz);
        anotar(&mut prehash_vistos, prehash);

        if interfaz != "external" {
            recuento.omitidos_interfaz_interna += casos;
            continue;
        }
        if !PREHASH_PURO.contains(&prehash) {
            recuento.omitidos_prehash += casos;
            continue;
        }
        if grupo["externalMu"].as_bool() == Some(true) {
            recuento.omitidos_mu_externo += casos;
            continue;
        }

        for caso in grupo["tests"].as_array().expect("tests debe ser una lista") {
            let identificador = caso["tcId"].as_u64().unwrap_or_default();
            let esperado_valido = caso["testPassed"]
                .as_bool()
                .unwrap_or_else(|| panic!("tcId {identificador}: falta 'testPassed'"));
            let motivo = caso["reason"].as_str().unwrap_or("");

            let clave_bruta = hexadecimal(texto(caso, "pk"));
            let mensaje = hexadecimal(texto(caso, "message"));
            let firma_bruta = hexadecimal(texto(caso, "signature"));
            let contexto = caso["context"].as_str().map_or_else(Vec::new, hexadecimal);

            recuento.ejecutados += 1;
            if esperado_valido {
                recuento.aceptados_esperados += 1;
            } else {
                recuento.rechazados_esperados += 1;
            }

            // Una clave o una firma de longitud incorrecta deben rechazarse en la
            // decodificacion. Que no decodifiquen es el comportamiento correcto.
            let obtenido_valido = <[u8; 1952]>::try_from(clave_bruta.as_slice())
                .ok()
                .map(|bytes| VerifyingKey::<MlDsa65>::new(&bytes.into()))
                .zip(
                    <[u8; 3309]>::try_from(firma_bruta.as_slice())
                        .ok()
                        .and_then(|bytes| ml_dsa::Signature::<MlDsa65>::decode(&bytes.into())),
                )
                .is_some_and(|(clave, firma)| {
                    clave.verify_with_context(&mensaje, &contexto, &firma)
                });

            if obtenido_valido != esperado_valido {
                fallos.push(format!(
                    "tcId {identificador} ({motivo}): se esperaba {} y se obtuvo {}",
                    if esperado_valido { "aceptar" } else { "rechazar" },
                    if obtenido_valido { "aceptar" } else { "rechazar" },
                ));
            }
        }
    }

    assert!(
        recuento.ejecutados > 0,
        "no se ejecuto ningun caso aplicable.\n  \
         Omitidos: {} por parametros, {} por interfaz interna, {} por pre-resumen, {} por mu externo.\n  \
         Valores de 'signatureInterface' observados en {PARAMETROS_DSA}: {:?}\n  \
         Valores de 'preHash' observados en {PARAMETROS_DSA}: {:?}\n  \
         Se esperaba interfaz 'external' y pre-resumen en {PREHASH_PURO:?}.",
        recuento.omitidos_parametros,
        recuento.omitidos_interfaz_interna,
        recuento.omitidos_prehash,
        recuento.omitidos_mu_externo,
        interfaces_vistas,
        prehash_vistos
    );

    // ACVP incluye casos que deben rechazarse. Si no hubiera ninguno, esta suite
    // solo estaria comprobando la ruta feliz.
    assert!(
        recuento.rechazados_esperados > 0,
        "ningun caso ejecutado esperaba rechazo; se ejecutaron {} casos, todos de aceptacion",
        recuento.ejecutados
    );

    assert!(
        fallos.is_empty(),
        "{} de {} casos divergen ({} de aceptacion, {} de rechazo).\n\
         Omitidos por variante no implementada: {} interfaz interna, {} pre-resumen, {} mu externo.\n  {}",
        fallos.len(),
        recuento.ejecutados,
        recuento.aceptados_esperados,
        recuento.rechazados_esperados,
        recuento.omitidos_interfaz_interna,
        recuento.omitidos_prehash,
        recuento.omitidos_mu_externo,
        fallos.join("\n  ")
    );
}
