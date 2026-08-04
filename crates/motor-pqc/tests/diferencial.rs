//! Contraste diferencial entre dos implementaciones independientes.
//!
//! RPT-005 §7.3. **Ninguna implementación PQC en Rust tiene auditoría
//! independiente hoy.** El contraste entre dos implementaciones distintas es el
//! sustituto asequible: si ante la misma entrada producen salidas distintas, una
//! de las dos está mal, y el fallo se revela sin necesidad de auditar ninguna.
//!
//! Implementación por defecto: **RustCrypto** (`ml-kem`, `ml-dsa`).
//! Oráculo: **libcrux** (`libcrux-ml-kem`, `libcrux-ml-dsa`), formalmente
//! verificado con hax/F*.
//!
//! # Por qué estas pruebas no son redundantes con los vectores
//!
//! Los vectores ACVP y Wycheproof comprueban cada implementación contra una
//! referencia fija. El contraste comprueba las dos **entre sí** sobre entradas
//! que ningún conjunto de vectores cubre, y detecta divergencias de
//! interpretación de la especificación —como el `context` de FIPS 204— que
//! ambas podrían pasar por alto por separado.
//!
//! libcrux se declara **solo como dependencia de desarrollo**: no entra en el
//! árbol de producción ni afecta a la frontera de licencia.

// Las restricciones del workspace sobre `panic!` y `expect` protegen la ruta de
// producción. En una prueba de conformidad, abortar con un mensaje que incluya
// la semilla es el comportamiento deseado: la divergencia debe ser reproducible.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use ml_dsa::{
    B32, KeyInit as _, Keypair as _, MlDsa65, Signer as _, SigningKey as PrivadaRustCrypto,
    Verifier as _,
};
use ml_kem::{Decapsulate as _, FromSeed as _, KeyExport as _, MlKem768};
use rand::{Rng as _, SeedableRng, rngs::StdRng};

/// Semillas fijas. El contraste debe ser reproducible: una divergencia que solo
/// aparece con ciertas entradas debe poder reproducirse en el informe de fallo.
const SEMILLAS: [u64; 8] = [1, 7, 42, 1_000, 65_537, 2_026, 0xDEAD_BEEF, u64::MAX];

fn generador(semilla: u64) -> StdRng {
    StdRng::seed_from_u64(semilla)
}

// ---------------------------------------------------------------------------
// ML-KEM-768
// ---------------------------------------------------------------------------

#[test]
fn ml_kem_la_misma_semilla_produce_la_misma_clave_publica() {
    // Ambas implementaciones derivan la clave desde una semilla de 64 bytes.
    // Si las claves públicas difieren, la expansión de semilla de una de las dos
    // no sigue FIPS 203.
    for semilla in SEMILLAS {
        let mut generador = generador(semilla);
        let bruta: [u8; 64] = generador.random();

        let (_, publica_rc) = MlKem768::from_seed(&bruta.into());
        let par_lc = libcrux_ml_kem::mlkem768::generate_key_pair(bruta);

        assert_eq!(
            publica_rc.to_bytes().as_slice(),
            par_lc.public_key().as_slice(),
            "divergencia de clave pública con la semilla {semilla}"
        );
    }
}

#[test]
fn ml_kem_el_encapsulado_determinista_coincide() {
    // Con la misma aleatoriedad de encapsulado, texto cifrado y secreto
    // compartido deben coincidir byte a byte entre ambas implementaciones.
    for semilla in SEMILLAS {
        let mut generador = generador(semilla);
        let bruta: [u8; 64] = generador.random();
        let mensaje: [u8; 32] = generador.random();

        let (_, publica_rc) = MlKem768::from_seed(&bruta.into());
        let par_lc = libcrux_ml_kem::mlkem768::generate_key_pair(bruta);

        let m: B32 = mensaje.into();
        let (cifrado_rc, secreto_rc) = publica_rc.encapsulate_deterministic(&m);
        let (cifrado_lc, secreto_lc) =
            libcrux_ml_kem::mlkem768::encapsulate(par_lc.public_key(), mensaje);

        assert_eq!(
            cifrado_rc.as_slice(),
            cifrado_lc.as_slice(),
            "divergencia de texto cifrado con la semilla {semilla}"
        );
        assert_eq!(
            secreto_rc.as_slice(),
            secreto_lc.as_slice(),
            "divergencia de secreto compartido con la semilla {semilla}"
        );
    }
}

#[test]
fn ml_kem_interopera_en_ambos_sentidos() {
    // La prueba más fuerte: cada implementación desencapsula lo que produjo la
    // otra. Una divergencia aquí rompe la interoperabilidad real, no solo la
    // igualdad de bytes.
    for semilla in SEMILLAS {
        let mut generador = generador(semilla);
        let bruta: [u8; 64] = generador.random();
        let mensaje: [u8; 32] = generador.random();

        let (privada_rc, publica_rc) = MlKem768::from_seed(&bruta.into());
        let par_lc = libcrux_ml_kem::mlkem768::generate_key_pair(bruta);

        // Encapsula libcrux, desencapsula RustCrypto.
        let (cifrado_lc, secreto_lc) =
            libcrux_ml_kem::mlkem768::encapsulate(par_lc.public_key(), mensaje);
        let recuperado_rc = privada_rc
            .decapsulate_slice(cifrado_lc.as_slice())
            .expect("el texto cifrado de libcrux debe tener la longitud correcta");

        assert_eq!(
            recuperado_rc.as_slice(),
            secreto_lc.as_slice(),
            "RustCrypto no recuperó el secreto de libcrux con la semilla {semilla}"
        );

        // Encapsula RustCrypto, desencapsula libcrux.
        let m: B32 = mensaje.into();
        let (cifrado_rc, secreto_rc) = publica_rc.encapsulate_deterministic(&m);
        let mut bruto = [0u8; 1088];
        bruto.copy_from_slice(cifrado_rc.as_slice());
        let cifrado_para_lc = libcrux_ml_kem::MlKemCiphertext::from(bruto);
        let recuperado_lc =
            libcrux_ml_kem::mlkem768::decapsulate(par_lc.private_key(), &cifrado_para_lc);

        assert_eq!(
            recuperado_lc.as_slice(),
            secreto_rc.as_slice(),
            "libcrux no recuperó el secreto de RustCrypto con la semilla {semilla}"
        );
    }
}

// ---------------------------------------------------------------------------
// ML-DSA-65
// ---------------------------------------------------------------------------

#[test]
fn ml_dsa_la_misma_semilla_produce_la_misma_clave_de_verificacion() {
    for semilla in SEMILLAS {
        let mut generador = generador(semilla);
        let bruta: [u8; 32] = generador.random();

        let privada_rc = PrivadaRustCrypto::<MlDsa65>::new(&bruta.into());
        let par_lc = libcrux_ml_dsa::ml_dsa_65::generate_key_pair(bruta);

        assert_eq!(
            privada_rc.verifying_key().encode().as_slice(),
            par_lc.verification_key.as_ref(),
            "divergencia de clave de verificación con la semilla {semilla}"
        );
    }
}

#[test]
fn ml_dsa_una_firma_de_rustcrypto_la_verifica_libcrux() {
    // Interoperabilidad real. Si divergen en la interpretación del `context` de
    // FIPS 204 —que RustCrypto toma vacío por omisión— esta prueba lo revela;
    // ningún conjunto de vectores lo haría, porque cada implementación pasaría
    // sus propios vectores por separado.
    let mensaje = b"orden de contencion sobre plc-linea-3";

    for semilla in SEMILLAS {
        let mut generador = generador(semilla);
        let bruta: [u8; 32] = generador.random();

        let privada_rc = PrivadaRustCrypto::<MlDsa65>::new(&bruta.into());
        let par_lc = libcrux_ml_dsa::ml_dsa_65::generate_key_pair(bruta);

        let firma_rc = privada_rc.sign(mensaje);
        let firma_para_lc =
            libcrux_ml_dsa::ml_dsa_65::MLDSA65Signature::new(firma_rc.encode().into());

        libcrux_ml_dsa::ml_dsa_65::verify(&par_lc.verification_key, mensaje, b"", &firma_para_lc)
            .unwrap_or_else(|error| {
                panic!(
                    "libcrux rechazó una firma de RustCrypto con la semilla {semilla}: {error:?}"
                )
            });
    }
}

#[test]
fn ml_dsa_una_firma_de_libcrux_la_verifica_rustcrypto() {
    let mensaje = b"orden de contencion sobre plc-linea-3";

    for semilla in SEMILLAS {
        let mut generador = generador(semilla);
        let bruta: [u8; 32] = generador.random();
        let aleatoriedad_firma: [u8; 32] = generador.random();

        let privada_rc = PrivadaRustCrypto::<MlDsa65>::new(&bruta.into());
        let par_lc = libcrux_ml_dsa::ml_dsa_65::generate_key_pair(bruta);

        let firma_lc =
            libcrux_ml_dsa::ml_dsa_65::sign(&par_lc.signing_key, mensaje, b"", aleatoriedad_firma)
                .expect("libcrux debe poder firmar");

        let firma_para_rc = ml_dsa::Signature::<MlDsa65>::decode(firma_lc.as_ref().into())
            .expect("la firma de libcrux debe decodificarse");

        privada_rc
            .verifying_key()
            .verify(mensaje, &firma_para_rc)
            .unwrap_or_else(|error| {
                panic!(
                    "RustCrypto rechazó una firma de libcrux con la semilla {semilla}: {error:?}"
                )
            });
    }
}

#[test]
fn ml_dsa_ambas_rechazan_un_mensaje_alterado() {
    // El acuerdo debe darse también en el rechazo. Dos implementaciones que
    // coinciden al aceptar pero divergen al rechazar son un problema peor: es
    // justo donde estaba CVE-2026-24850.
    let mut generador = generador(2_026);
    let bruta: [u8; 32] = generador.random();

    let privada_rc = PrivadaRustCrypto::<MlDsa65>::new(&bruta.into());
    let par_lc = libcrux_ml_dsa::ml_dsa_65::generate_key_pair(bruta);

    let firma_rc = privada_rc.sign(b"aislar plc-3");
    let firma_para_lc = libcrux_ml_dsa::ml_dsa_65::MLDSA65Signature::new(firma_rc.encode().into());

    assert!(
        privada_rc
            .verifying_key()
            .verify(b"aislar plc-9", &firma_rc)
            .is_err(),
        "RustCrypto debe rechazar el mensaje alterado"
    );
    assert!(
        libcrux_ml_dsa::ml_dsa_65::verify(
            &par_lc.verification_key,
            b"aislar plc-9",
            b"",
            &firma_para_lc,
        )
        .is_err(),
        "libcrux debe rechazar el mensaje alterado"
    );
}
