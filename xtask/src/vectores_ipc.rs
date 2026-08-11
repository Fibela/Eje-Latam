//! Vectores del formato de cable, generados por el lado que manda.
//!
//! RPT-045 §3, PA-77 en camino.
//!
//! # Por que existe
//!
//! El cliente de TypeScript sera un **tercer sitio** donde vive el contrato, y
//! esta vez a nivel de bytes. Las barreras de PA-75 y PA-76 comprueban formas y
//! esquemas; ninguna mira el cable. Un prefijo escrito en little-endian pasaria
//! las pruebas de los dos lenguajes sin inmutarse y fallaria la primera vez que
//! los dos procesos se hablen de verdad.
//!
//! # Como
//!
//! Estos vectores los produce el **codificador de Rust**, que es el que manda.
//! Una prueba de Rust comprueba que regenerarlos da lo mismo —si alguien toca
//! `enmarcar` o `componer_peticion`, el fichero deja de cuadrar y hay que
//! regenerarlo a proposito— y una prueba de TypeScript exige que su codificador
//! produzca esos bytes exactos.
//!
//! Es el mismo patron que `motor-pqc` usa con ACVP y Wycheproof: nadie mantiene
//! una tabla a mano, y ninguno de los dos lados puede moverse en silencio.

use std::fmt::Write as _;

use eje_ipc::{
    CODIGO_RECHAZO, CODIGO_RESPUESTA, Canal, LONGITUD_MAXIMA_MARCO, NOMBRE_MAXIMO,
    PREFIJO_LONGITUD, PREFIJO_NOMBRE, componer_peticion, componer_rechazo, componer_respuesta,
    enmarcar,
};

/// Nombre del fichero, junto al manifiesto en la raiz del repositorio.
pub const FICHERO: &str = "vectores-ipc.json";

/// Hexadecimal en minusculas, sin separadores.
fn hex(bytes: &[u8]) -> String {
    let mut salida = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // `write!` sobre un `String` no puede fallar.
        let _ = write!(salida, "{byte:02x}");
    }
    salida
}

/// Carga determinista de la longitud pedida.
///
/// No aleatoria a proposito: un vector que cambia en cada generacion no sirve
/// como ancla.
fn carga_de(longitud: usize) -> Vec<u8> {
    (0..longitud).map(|indice| (indice % 251) as u8).collect()
}

/// Construye el contenido completo del fichero de vectores.
///
/// # Por que el JSON se escribe a mano
///
/// Todo lo que entra es ASCII: identificadores de canal y hexadecimal. No hay
/// texto libre que escapar —los motivos de rechazo viajan **en hexadecimal**,
/// precisamente para que el fichero no dependa de la codificacion con que se
/// guarde— y traer `serde` a `xtask` por esto seria mas superficie que ayuda.
#[must_use]
pub fn generar() -> String {
    let mut peticiones: Vec<(String, Canal, Vec<u8>)> = Vec::new();

    // Un caso por canal con carga vacia: cubre los seis identificadores y el
    // caso de longitud cero, que es el que mas veces se implementa mal.
    for canal in Canal::TODOS {
        peticiones.push((
            format!("carga vacia en {}", canal.identificador()),
            canal,
            Vec::new(),
        ));
    }

    peticiones.push((
        "peticion de alertas tipica".to_owned(),
        Canal::ConsultarAlertas,
        br#"{"desdeAsiento":0}"#.to_vec(),
    ));
    peticiones.push((
        "carga de 256 bytes con todos los valores".to_owned(),
        Canal::ConsultarSandbox,
        carga_de(256),
    ));
    peticiones.push((
        "carga con byte nulo y con 0xff".to_owned(),
        Canal::ConsultarAlertas,
        vec![0x00, 0xff, 0x00, 0xff],
    ));

    let mut salida = String::new();
    salida.push_str("{\n");
    salida.push_str(
        "  \"descripcion\": \"Vectores del formato de cable de eje-ipc. \
         Generados por 'cargo xtask vectores-ipc'; NO editar a mano.\",\n",
    );

    // Las cotas viajan con los vectores para que el otro lado pueda comprobar
    // tambien sus constantes, no solo sus bytes.
    salida.push_str("  \"limites\": {\n");
    let _ = writeln!(salida, "    \"marcoMaximo\": {LONGITUD_MAXIMA_MARCO},");
    let _ = writeln!(salida, "    \"prefijoLongitud\": {PREFIJO_LONGITUD},");
    let _ = writeln!(salida, "    \"prefijoNombre\": {PREFIJO_NOMBRE},");
    let _ = writeln!(salida, "    \"nombreMaximo\": {NOMBRE_MAXIMO},");
    let _ = writeln!(salida, "    \"codigoRespuesta\": {CODIGO_RESPUESTA},");
    let _ = writeln!(salida, "    \"codigoRechazo\": {CODIGO_RECHAZO}");
    salida.push_str("  },\n");

    salida.push_str("  \"peticiones\": [\n");
    for (indice, (nombre, canal, carga)) in peticiones.iter().enumerate() {
        // Si un caso no se puede componer, el vector no existe y el fichero lo
        // dice: mejor eso que anclar bytes inventados.
        let Ok(cuerpo) = componer_peticion(*canal, carga) else {
            continue;
        };
        let Ok(marco) = enmarcar(&cuerpo) else {
            continue;
        };

        let coma = if indice + 1 == peticiones.len() {
            ""
        } else {
            ","
        };
        salida.push_str("    {\n");
        let _ = writeln!(salida, "      \"nombre\": \"{nombre}\",");
        let _ = writeln!(salida, "      \"canal\": \"{}\",", canal.identificador());
        let _ = writeln!(salida, "      \"cargaHex\": \"{}\",", hex(carga));
        let _ = writeln!(salida, "      \"cuerpoHex\": \"{}\",", hex(&cuerpo));
        let _ = writeln!(salida, "      \"marcoHex\": \"{}\"", hex(&marco));
        let _ = writeln!(salida, "    }}{coma}");
    }
    salida.push_str("  ],\n");

    let respuestas: Vec<(&str, bool, Vec<u8>)> = vec![
        ("respuesta con cuerpo vacio", true, Vec::new()),
        (
            "respuesta con json",
            true,
            br#"{"primerDisponible":1,"sucesos":[]}"#.to_vec(),
        ),
        (
            "rechazo con motivo ascii",
            false,
            b"canal sin manejador".to_vec(),
        ),
        // El caso que caza el error mas probable de una reimplementacion: contar
        // caracteres en lugar de bytes. `componer_rechazo` recorta por BYTES, y
        // un `String.prototype.slice` en TypeScript recorta por unidades UTF-16.
        (
            "rechazo con motivo acentuado",
            false,
            "el inventario no verifica: revisión pendiente"
                .as_bytes()
                .to_vec(),
        ),
    ];

    salida.push_str("  \"respuestas\": [\n");
    for (indice, (nombre, es_respuesta, cuerpo)) in respuestas.iter().enumerate() {
        let payload = if *es_respuesta {
            match componer_respuesta(cuerpo) {
                Ok(payload) => payload,
                Err(_) => continue,
            }
        } else {
            componer_rechazo(&String::from_utf8_lossy(cuerpo))
        };
        let Ok(marco) = enmarcar(&payload) else {
            continue;
        };

        let coma = if indice + 1 == respuestas.len() {
            ""
        } else {
            ","
        };
        salida.push_str("    {\n");
        let _ = writeln!(salida, "      \"nombre\": \"{nombre}\",");
        let _ = writeln!(
            salida,
            "      \"clase\": \"{}\",",
            if *es_respuesta {
                "respuesta"
            } else {
                "rechazo"
            }
        );
        let _ = writeln!(salida, "      \"cuerpoHex\": \"{}\",", hex(cuerpo));
        let _ = writeln!(salida, "      \"cargaHex\": \"{}\",", hex(&payload));
        let _ = writeln!(salida, "      \"marcoHex\": \"{}\"", hex(&marco));
        let _ = writeln!(salida, "    }}{coma}");
    }
    salida.push_str("  ]\n}\n");

    salida
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Ruta del fichero anclado, en la raiz del repositorio.
    fn ruta() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask cuelga de la raiz")
            .join(FICHERO)
    }

    #[test]
    fn el_fichero_anclado_coincide_con_lo_que_produce_el_codificador() {
        // El ancla entera. Si alguien toca `enmarcar` o `componer_peticion`, esto
        // rompe y hay que regenerar **a proposito** — que es justo lo que impide
        // que el formato se mueva sin que el lado de TypeScript se entere.
        let anclado = std::fs::read_to_string(ruta()).unwrap_or_else(|error| {
            panic!(
                "no se pudo leer {}: {error}.\n\
                 Genera los vectores con 'cargo xtask vectores-ipc'.",
                ruta().display()
            )
        });

        assert_eq!(
            anclado.replace("\r\n", "\n"),
            generar(),
            "el formato de cable cambio y los vectores no.\n\
             Si el cambio es deliberado: 'cargo xtask vectores-ipc' y revisa el diff."
        );
    }

    #[test]
    fn generar_es_determinista() {
        // Un vector que cambia en cada generacion no sirve como ancla, y la
        // forma tipica de romperlo es meter aleatoriedad o una marca de tiempo.
        assert_eq!(generar(), generar());
    }

    #[test]
    fn el_hexadecimal_es_de_dos_digitos_en_minuscula() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff, 0xa5]), "000fffa5");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn el_motivo_acentuado_ocupa_mas_bytes_que_caracteres() {
        // La razon de que ese caso este en los vectores: quien reimplemente el
        // recorte con `String.prototype.slice` contara unidades UTF-16 y
        // producira otros bytes.
        let motivo = "el inventario no verifica: revisión pendiente";
        assert!(
            motivo.len() > motivo.chars().count(),
            "el caso debe llevar al menos un caracter multibyte"
        );
    }
}
