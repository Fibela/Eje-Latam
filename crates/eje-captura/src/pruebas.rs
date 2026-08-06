//! Pruebas de `eje-captura`.
//!
//! Lo que se puede probar sin una tarjeta de red es la **forma** del crate: que
//! el `unsafe` este donde se dijo, que las tramas cortas no desborden y que la
//! perdida sea visible. La captura real necesita una interfaz y va en PA-40.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::*;

// ---------------------------------------------------------------------------
// La frontera del `unsafe` es exigible, no una convencion
// ---------------------------------------------------------------------------

/// Ficheros fuente del crate.
fn fuentes() -> Vec<(String, String)> {
    let directorio = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");

    std::fs::read_dir(&directorio)
        .unwrap_or_else(|error| panic!("no se pudo leer {}: {error}", directorio.display()))
        .filter_map(Result::ok)
        .filter(|entrada| entrada.path().extension().is_some_and(|ext| ext == "rs"))
        .filter_map(|entrada| {
            let nombre = entrada.file_name().to_string_lossy().into_owned();
            std::fs::read_to_string(entrada.path())
                .ok()
                .map(|contenido| (nombre, contenido))
        })
        .collect()
}

/// Agujas partidas en el fuente para que el escaner no se encuentre a si mismo.
///
/// La primera version buscaba las cadenas escribiendolas enteras, y los dos
/// guardianes se detectaron a si mismos: su propio literal de busqueda estaba en
/// este fichero.
///
/// La salida facil era excluir `pruebas.rs` del escaneo. Se descarto: dejaria un
/// punto ciego donde nadie veria un `allow` de `unsafe` puesto aqui, y el codigo
/// de prueba se compila dentro del crate igual que el resto. Partir la aguja
/// mantiene este fichero bajo vigilancia.
mod aguja {
    /// `allow` de codigo inseguro.
    pub const PERMISO_INSEGURO: &str = concat!("allow(", "unsafe_code)");

    /// Vias de transmision que no deben existir.
    pub const TRANSMISION: [&str; 4] = [
        concat!("libc::", "send"),
        concat!("libc::", "sendto"),
        concat!("libc::", "sendmsg"),
        concat!("fn ", "enviar"),
    ];
}

#[test]
fn el_escaner_no_se_encuentra_a_si_mismo() {
    // Si alguien reescribiera las agujas enteras, los guardianes volverian a
    // fallar por autorreferencia y la tentacion seria excluir este fichero. Esta
    // prueba deja constancia de por que estan partidas.
    let este = fuentes()
        .into_iter()
        .find(|(nombre, _)| nombre == "pruebas.rs")
        .map(|(_, contenido)| contenido)
        .expect("pruebas.rs debe estar entre las fuentes");

    assert!(
        !este.contains(aguja::PERMISO_INSEGURO),
        "la aguja del permiso inseguro volvio a escribirse entera"
    );
    for prohibido in aguja::TRANSMISION {
        assert!(
            !este.contains(prohibido),
            "la aguja '{prohibido}' volvio a escribirse entera"
        );
    }
}

#[test]
fn solo_un_modulo_admite_unsafe() {
    // El workspace declara `unsafe_code = "warn"` y clippy corre con
    // `-D warnings`, asi que cualquier `unsafe` sin permiso explicito rompe la
    // compilacion. Esta prueba anade lo que ese lint no da: que el permiso
    // exista en UN solo sitio y siga siendo el esperado.
    //
    // Sin ella, ampliar la frontera costaria una linea y nadie se enteraria.
    let permisos: Vec<String> = fuentes()
        .into_iter()
        .filter(|(_, contenido)| contenido.contains(aguja::PERMISO_INSEGURO))
        .map(|(nombre, _)| nombre)
        .collect();

    assert_eq!(
        permisos,
        vec!["linux.rs".to_owned()],
        "el `unsafe` debe vivir solo en linux.rs; ampliar esa frontera exige \
         revision explicita (RPT-018 §2)"
    );
}

#[test]
fn no_existe_ninguna_via_de_transmision() {
    // La pasividad de RPT-002 §9.2 es por tipo: no basta con no llamar a
    // `send`, tiene que no haber `send` que llamar. Si alguien anadiera un
    // metodo de envio, esta prueba lo veria antes que la revision.
    for (nombre, contenido) in fuentes() {
        for prohibido in aguja::TRANSMISION {
            assert!(
                !contenido.contains(prohibido),
                "'{prohibido}' aparece en {nombre}: la captura debe ser de solo lectura"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tramas
// ---------------------------------------------------------------------------

fn trama_de(bytes: Vec<u8>, en_el_cable: usize) -> Trama {
    Trama {
        bytes,
        longitud_en_el_cable: en_el_cable,
    }
}

#[test]
fn una_trama_completa_entrega_sus_direcciones() {
    let mut bytes = vec![0u8; 14];
    bytes[..6].copy_from_slice(&[0xFF; 6]);
    bytes[6..12].copy_from_slice(&[0x00, 0x1B, 0x21, 0x00, 0x00, 0x01]);

    let trama = trama_de(bytes, 14);

    assert_eq!(trama.destino(), Some([0xFF; 6]));
    assert_eq!(trama.origen(), Some([0x00, 0x1B, 0x21, 0x00, 0x00, 0x01]));
    assert!(!trama.recortada());
}

#[test]
fn una_trama_corta_no_desborda() {
    // La trama llega de la red y su longitud no es una promesa. Indexar a
    // ciegas seria un panico a peticion de quien emita la trama.
    for longitud in 0..14 {
        let trama = trama_de(vec![0u8; longitud], longitud);

        if longitud < 6 {
            assert_eq!(trama.destino(), None, "longitud {longitud}");
        }
        if longitud < 12 {
            assert_eq!(trama.origen(), None, "longitud {longitud}");
        }
    }
}

#[test]
fn el_recorte_se_distingue_de_la_trama_corta() {
    // Sin esta distincion, un analizador de huella concluiria que un protocolo
    // no aparece cuando lo que ocurre es que la trama se corto antes de llegar
    // a el.
    let corta = trama_de(vec![0u8; 60], 60);
    let recortada = trama_de(vec![0u8; LONGITUD_MAXIMA_TRAMA], 9_000);

    assert!(!corta.recortada());
    assert!(recortada.recortada());
}

// ---------------------------------------------------------------------------
// Perdida
// ---------------------------------------------------------------------------

#[test]
fn la_perdida_es_visible() {
    assert!(!Estadisticas::default().hay_perdida());
    assert!(
        !Estadisticas {
            recibidas: 1_000,
            descartadas: 0
        }
        .hay_perdida()
    );
    assert!(
        Estadisticas {
            recibidas: 1_000,
            descartadas: 1
        }
        .hay_perdida(),
        "una sola trama perdida ya deja la vista incompleta"
    );
}

// ---------------------------------------------------------------------------
// Plataformas sin soporte
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "linux"))]
#[test]
fn fuera_de_linux_se_declara_no_soportado_y_no_se_finge() {
    // «No soportado» es admisible; «no implementado» no lo es (RPT-003 §9.5 y
    // el guardian de inconclusos). Devolver una captura vacia que nunca entrega
    // tramas seria peor que un error: pareceria una red silenciosa.
    match abrir("cualquiera") {
        Err(ErrorCaptura::PlataformaNoSoportada) => {}
        Err(otro) => panic!("se esperaba PlataformaNoSoportada y llego {otro}"),
        Ok(_) => panic!("no debe haber captura fuera de Linux"),
    }
}

#[cfg(target_os = "linux")]
#[test]
fn una_interfaz_inexistente_se_distingue_de_la_falta_de_privilegios() {
    // Los dos fallos tienen remedios distintos —uno es un nombre mal escrito y
    // el otro una capacidad que falta— y colapsarlos obligaria al operador a
    // adivinar cual de los dos tiene delante.
    match abrir("interfaz-que-no-existe-0") {
        Err(ErrorCaptura::InterfazNoDisponible { interfaz }) => {
            assert_eq!(interfaz, "interfaz-que-no-existe-0");
        }
        Err(ErrorCaptura::PrivilegiosInsuficientes { .. }) => {
            // Sin CAP_NET_RAW el socket no llega a abrirse y el nombre no se
            // resuelve. Es un resultado legitimo de esta prueba en CI sin
            // privilegios, y distinguirlo es justo lo que se comprueba.
        }
        Err(otro) => panic!("fallo inesperado: {otro}"),
        Ok(_) => panic!("una interfaz inexistente no debe abrirse"),
    }
}
