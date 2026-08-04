//! Punto de entrada del demonio Eje-Agente.

#![forbid(unsafe_code)]

use eje_agente::{ConfiguracionAgente, VERSION};
use guardian_cc::PerfilSegmento;

fn main() {
    let configuracion = ConfiguracionAgente::para_segmento(PerfilSegmento::Corporativo);
    let capa_b = configuracion.red.capa_b_autorizada;

    println!("Eje-Agente {VERSION}");
    println!("Perfil de segmento : {:?}", configuracion.perfil);
    println!("Modo de esquema    : {:?}", configuracion.modo_esquema);
    println!("Capa B autorizada  : {capa_b}");
    println!("Estado de licencia : {:?}", configuracion.licencia);
    println!();
    println!("Modulos enlazados  : guardian-cc, motor-pqc, eje-almacen, boveda, eje-red");
    println!("Licencia           : Apache-2.0 (frontera ratificada en RPT-003 §2.7)");
}
