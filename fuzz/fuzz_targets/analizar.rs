//! Objetivo de fuzzing guiado por cobertura sobre `formato::analizar`.
//!
//! RPT-014, PA-29.
//!
//! # Por que este y no otro
//!
//! `analizar` es el unico punto del producto que interpreta **entrada no
//! autenticada**: corre antes de que se verifique ninguna firma, sobre un fichero
//! que el modelo de amenazas asume manipulable. Toda la cadena de cinco eslabones
//! se apoya en que no entre en panico ni reserve memoria a peticion de quien
//! escribio el fichero.
//!
//! # Ejecucion
//!
//! ```text
//! cargo install cargo-fuzz
//! cargo +nightly fuzz run analizar
//! ```
//!
//! Este paquete **no forma parte del workspace**: `cargo-fuzz` exige nightly y el
//! proyecto esta fijado a estable 1.85. `cargo test --workspace` no lo compila.
//!
//! # Relacion con el arnes de `guardian-cc`
//!
//! El arnes determinista de `crates/guardian-cc/src/lib.rs` corre en CI y es una
//! **red de regresion**: semilla fija, espacio pequeno, no crece entre
//! ejecuciones. Esto es lo otro: mutacion guiada por cobertura que si crece. La
//! afirmacion de que el analizador resiste entrada hostil descansa aqui, no alli.
//!
//! # Semillas
//!
//! Conviene sembrar `fuzz/corpus/analizar/` con al menos un fichero valido, que
//! se obtiene serializando un inventario de prueba. Sin semilla, el fuzzer gasta
//! mucho tiempo antes de acertar con los ocho bytes del numero magico.

#![no_main]

use libfuzzer_sys::fuzz_target;

use guardian_cc::formato::{analizar, serializar};

fuzz_target!(|datos: &[u8]| {
    // Invariante 1: no hay panico. Se cumple por volver de la llamada.
    let Ok(fichero) = analizar(datos) else {
        return;
    };

    // Invariante 2: si acepta, la codificacion estructural es canonica.
    //
    // Se excluye la firma de la comparacion: nada garantiza que reencodear una
    // firma mutada que aun decodifica devuelva los mismos bytes, y esa
    // normalizacion seria un falso positivo ajeno al analizador.
    let reserializado = serializar(
        &fichero.inventario,
        fichero.anclada.secuencia,
        &fichero.firma,
    );

    // Resta comprobada, no `-` a secas. Hoy `analizar` no puede aceptar nada mas
    // corto que la firma, pero apoyarse en esa garantia dentro de un objetivo de
    // fuzzing convierte un cambio futuro del analizador en un desbordamiento que
    // el fuzzer reportaria como hallazgo. Un falso positivo aqui cuesta una tarde
    // de depuracion.
    let longitud_firma = motor_pqc::firma_hibrida::FirmaHibrida::longitud_serializada();
    let (Some(hasta), true) = (
        datos.len().checked_sub(longitud_firma),
        reserializado.len() == datos.len(),
    ) else {
        return;
    };

    assert_eq!(
        &reserializado[..hasta],
        &datos[..hasta],
        "el analizador acepto una codificacion no canonica"
    );
});
