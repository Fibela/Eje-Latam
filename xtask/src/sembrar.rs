//! Siembra un registro de evidencia con alertas de mentira.
//!
//! RPT-046 §11.1, RPT-048 §4.
//!
//! # ADVERTENCIA
//!
//! Esta herramienta **fabrica evidencia**. En un producto forense eso es
//! peligroso: lo que produce tiene la misma forma, la misma cadena de resumenes
//! y la misma ancla que un registro real, porque usa el mismo codigo.
//!
//! Por eso cada asiento lleva un marcador visible en su detalle y el nodo es
//! `SEMBRADO`. Un registro sembrado se reconoce leyendolo, no por dónde este.
//!
//! No forma parte del binario del agente ni del instalador: vive en `xtask`,
//! que es una herramienta de desarrollo y no se distribuye.
//!
//! # Para que existe
//!
//! Dos deudas que sólo se cierran con volumen:
//!
//! - **La fragmentacion de marcos** (RPT-046 §11.1). Todo lo que ha cruzado el
//!   cable cupo en un trozo. El acumulador de RPT-045 —la pieza que mas
//!   preocupaba— no ha acumulado nunca nada. Hace falta una respuesta que no
//!   quepa, y `consultar-alertas` con cientos de asientos es la unica que puede
//!   darla.
//! - **`primerDisponible > 1`** (RPT-048 §4). El panel debe distinguir «no ha
//!   pasado nada» de «lo anterior esta archivado», y hasta ahora ese caso solo
//!   existe en pruebas.

use std::path::Path;

use eje_almacen::cadena::RegistroEvidencia;
use eje_almacen::esquema::ClaseEvento;
use eje_almacen::persistencia::{ancla_de, serializar, serializar_ancla};

/// Marcador que hace inconfundible un asiento sembrado.
pub const MARCA: &str = "[SEMBRADO-NO-ES-EVIDENCIA-REAL]";

/// Nodo al que se atribuyen los asientos sembrados.
const NODO: &str = "SEMBRADO";

/// Construye un registro con `cuantos` asientos de deteccion.
///
/// # Errores
///
/// Propaga el fallo de `anexar` si se supera el techo del formato, que es
/// exactamente lo que PA-72 puso ahi para que no ocurra en silencio.
pub fn construir(cuantos: u64, relleno: usize) -> Result<RegistroEvidencia, String> {
    let mut registro = RegistroEvidencia::nuevo();

    // El relleno existe para una sola cosa: que la respuesta no quepa en un
    // trozo del socket. Con 256 asientos —el tope por respuesta— y detalles
    // cortos, el marco ronda los 45 KB y el nucleo lo entrega entero; el
    // acumulador de RPT-045 nunca llega a acumular.
    //
    // Es texto de mentira dentro de un asiento de mentira: no pretende
    // parecerse a un detalle real, y por eso se ve que es paja.
    let paja = "x".repeat(relleno);

    for indice in 0..cuantos {
        // Instante determinista: un registro que cambia en cada siembra no sirve
        // para comparar dos ejecuciones.
        let instante = 1_754_000_000_000_i64 + (indice as i64) * 1_000;

        registro
            .anexar(
                instante,
                ClaseEvento::DeteccionAnomalia,
                NODO,
                &format!(
                    "{MARCA} alerta de prueba numero {} generada por 'cargo xtask sembrar'{}{paja}",
                    indice + 1,
                    if paja.is_empty() { "" } else { " relleno:" }
                ),
            )
            .map_err(|error| format!("no se pudo anexar el asiento {}: {error}", indice + 1))?;
    }

    Ok(registro)
}

/// Escribe el registro sembrado en `ruta`, con su ancla.
///
/// # Por que tambien el ancla
///
/// La primera version no la escribia, para que nadie creyera que este fichero
/// paso por el ciclo real. El agente lo aparto al arrancar como
/// `evidencia.alm.violacion-...` y declaro «NO VERIFICA: alguien lo toco».
///
/// **Tenia razon.** El ancla que habia en el directorio describia un registro
/// vacio, y RPT-033 esta ahi para detectar exactamente eso. La deteccion de
/// manipulacion funciono sobre un caso real por primera vez, aunque el
/// manipulador fuera esta herramienta.
///
/// Asi que se escribe el par completo. Un registro sembrado se sigue
/// reconociendo por la marca de cada asiento, que es lo unico que sobrevive a
/// copiar el fichero a otro sitio.
///
/// # Errores
///
/// Fallo del sistema de ficheros.
pub fn sembrar(ruta: &Path, cuantos: u64, relleno: usize) -> Result<(), String> {
    let registro = construir(cuantos, relleno)?;
    let bytes = serializar(&registro);

    std::fs::write(ruta, &bytes).map_err(|error| format!("{}: {error}", ruta.display()))?;

    let ruta_ancla = ruta.with_extension("anc");
    match ancla_de(&registro) {
        Some(ancla) => std::fs::write(&ruta_ancla, serializar_ancla(&ancla))
            .map_err(|error| format!("{}: {error}", ruta_ancla.display()))?,
        // Un registro vacio no tiene extremo que anclar, y dejar el ancla
        // anterior apuntando a un asiento que ya no existe seria un
        // truncamiento imaginario y permanente.
        None => {
            let _ = std::fs::remove_file(&ruta_ancla);
        }
    }

    println!("Sembrados {cuantos} asientos en {}", ruta.display());
    println!("Tamano del registro : {} bytes", bytes.len());
    println!(
        "Ancla               : {}",
        ruta.with_extension("anc").display()
    );
    println!();
    println!("ESTO NO ES EVIDENCIA REAL. Cada asiento lleva '{MARCA}'.");
    println!("Arranca el agente con --almacen sobre ese directorio y consulta");
    println!("'consultar-alertas' para ver si la respuesta parte el marco.");

    Ok(())
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn cada_asiento_sembrado_se_reconoce_por_su_detalle() {
        // La unica defensa contra confundir esto con evidencia real es que lo
        // diga cada asiento, no el nombre del fichero ni el directorio.
        let registro = construir(5, 0).expect("siembra");

        assert_eq!(registro.asientos().len(), 5);
        for asiento in registro.asientos() {
            assert!(
                asiento.detalle.contains(MARCA),
                "un asiento sembrado sin marca es indistinguible de uno real"
            );
        }
    }

    #[test]
    fn el_registro_sembrado_verifica_como_cualquier_otro() {
        // Usa el mismo `anexar`, asi que la cadena tiene que cuadrar. Si no
        // cuadrara, lo sembrado no serviria para probar el camino real.
        let registro = construir(64, 0).expect("siembra");
        assert!(registro.verificar_cadena().is_ok());
    }

    #[test]
    fn la_siembra_es_determinista() {
        // Dos ejecuciones deben dar los mismos bytes: un registro que cambia
        // solo no sirve para comparar dos pruebas del cable.
        assert_eq!(
            serializar(&construir(16, 0).expect("una")),
            serializar(&construir(16, 0).expect("otra"))
        );
    }

    #[test]
    fn el_relleno_engorda_el_asiento_sin_quitarle_la_marca() {
        // Si el relleno tapara la marca, un registro grande dejaria de ser
        // reconocible como sembrado justo cuando mas cuesta leerlo entero.
        let registro = construir(2, 4_000).expect("siembra");
        let primero = &registro.asientos()[0];

        assert!(primero.detalle.len() > 4_000);
        assert!(primero.detalle.contains(MARCA));
    }

    #[test]
    fn sembrar_cero_asientos_no_es_un_error_sino_un_registro_vacio() {
        assert_eq!(construir(0, 0).expect("vacio").asientos().len(), 0);
    }
}
