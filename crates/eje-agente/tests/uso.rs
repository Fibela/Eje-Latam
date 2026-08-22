//! PA-85. El mensaje de uso tiene que llegar a quien lo necesita.
//!
//! # Por que esta prueba existe
//!
//! `ErrorAgente::Uso` llevaba la linea de uso completa en su `#[error(...)]` y
//! nadie la veia: `fn main() -> Result<_, _>` imprime con `Debug`, asi que la
//! salida era `Error: Uso`.
//!
//! Eso costo dos rondas de diagnostico reales (RPT-046 §11.2). Un binario sin
//! recompilar rechazaba `--grupo-ipc` y decia `Error: Uso`; con el uso impreso
//! se habria visto al instante que la opcion no figuraba en esa version.
//!
//! Es una prueba de **integracion a proposito**: ejecuta el binario de verdad.
//! Comprobar la cadena de `Display` desde una prueba unitaria no detectaria el
//! defecto, porque el defecto no estaba en el mensaje sino en quien lo imprime.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

/// Opciones que el mensaje debe nombrar **desde fuera del proceso**.
///
/// # Esta lista se recorto a proposito (RPT-071, PA-122)
///
/// Antes tenia cuatro nombres y decia cubrir «las que se anadieron tarde, que
/// son las que se olvidan». `--directorio-socket` se anadio tarde, se olvido, y
/// **no estaba en la lista**: la prueba declaraba un proposito que no cumplia y
/// paso en verde el dia entero que la opcion fue indescubrible.
///
/// Que el mensaje las nombre **todas** ya no se comprueba aqui, porque desde una
/// prueba de integracion no se puede ver `OPCIONES` y cualquier lista seria otra
/// copia a mano. Lo hace `pruebas_opciones::la_linea_de_uso_nombra_todas_las_opciones`,
/// que sale de la misma tabla que la linea.
///
/// Lo que esta suite comprueba es lo suyo, y no se puede comprobar de otro modo:
/// que el mensaje **salga del binario de verdad** y llegue a `stderr`.
const ESPERADAS: &[&str] = &["--interfaz"];

#[test]
fn sin_argumentos_el_agente_explica_como_se_usa() {
    let salida = Command::new(env!("CARGO_BIN_EXE_eje-agente"))
        .output()
        .expect("el binario de la propia caja debe poder ejecutarse");

    assert!(
        !salida.status.success(),
        "sin argumentos el agente no debe terminar con exito"
    );

    let texto = String::from_utf8_lossy(&salida.stderr);

    for opcion in ESPERADAS {
        assert!(
            texto.contains(opcion),
            "el mensaje de uso no nombra '{opcion}'.\nSalida:\n{texto}"
        );
    }
}

#[test]
fn el_mensaje_de_uso_no_es_el_nombre_de_la_variante() {
    // La regresion concreta: `Debug` imprimia `Uso` y se daba por bueno.
    let salida = Command::new(env!("CARGO_BIN_EXE_eje-agente"))
        .output()
        .expect("ejecuta");

    let texto = String::from_utf8_lossy(&salida.stderr);
    let limpio = texto.trim();

    assert_ne!(limpio, "Error: Uso", "volvio el formato de `Debug`");
    assert!(
        limpio.starts_with("uso:"),
        "el mensaje debe empezar por la linea de uso, no por el nombre del error.\n\
         Salida: {limpio}"
    );
}

#[test]
fn una_opcion_desconocida_tambien_explica_el_uso() {
    // No solo la ausencia de argumentos: equivocarse de opcion es el caso que
    // de verdad ocurrio, y es donde mas falta hace saber cuales existen.
    let salida = Command::new(env!("CARGO_BIN_EXE_eje-agente"))
        .args(["--opcion-que-no-existe", "valor"])
        .output()
        .expect("ejecuta");

    assert!(!salida.status.success());
    assert!(
        String::from_utf8_lossy(&salida.stderr).contains("--interfaz"),
        "una opcion desconocida debe listar las que si existen"
    );
}
