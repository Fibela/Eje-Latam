//! Pruebas del contrato IPC.
//!
//! La prueba central es la de **paridad con el manifiesto**: si `Canal` y
//! `contrato-ipc.toml` divergen, esta suite falla. Sin ella, el manifiesto sería
//! documentación decorativa y volveríamos al punto de partida, con cada extremo
//! declarando sus canales por su cuenta.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::{
    Canal, ErrorIpc, LONGITUD_MAXIMA_MARCO, PREFIJO_LONGITUD, autorizar, desenmarcar, enmarcar,
};

/// Lee el manifiesto desde la raíz del workspace.
fn manifiesto() -> String {
    let ruta = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("contrato-ipc.toml");

    std::fs::read_to_string(&ruta).unwrap_or_else(|error| {
        panic!(
            "no se pudo leer el manifiesto {}: {error}.\n\
             contrato-ipc.toml es la fuente de verdad del puente y debe estar versionado.",
            ruta.display()
        )
    })
}

/// Extrae los valores de `nombre = "..."` que siguen a una cabecera de tabla.
fn nombres_bajo(contenido: &str, cabecera: &str) -> Vec<String> {
    let mut nombres = Vec::new();
    let mut dentro = false;

    for linea in contenido.lines() {
        let limpia = linea.trim();

        if limpia.starts_with('[') {
            dentro = limpia == cabecera;
            continue;
        }
        if limpia.starts_with('#') || !dentro {
            continue;
        }
        if let Some(resto) = limpia.strip_prefix("nombre = \"") {
            if let Some(fin) = resto.find('"') {
                nombres.push(resto[..fin].to_owned());
            }
        }
    }

    nombres
}

// ---------------------------------------------------------------------------
// Paridad con el manifiesto
// ---------------------------------------------------------------------------

#[test]
fn los_canales_coinciden_con_el_manifiesto() {
    let declarados = nombres_bajo(&manifiesto(), "[[canal]]");
    let implementados: Vec<String> = Canal::TODOS
        .iter()
        .map(|canal| canal.identificador().to_owned())
        .collect();

    assert_eq!(
        declarados, implementados,
        "el enum Canal y contrato-ipc.toml divergen.\n  \
         manifiesto: {declarados:?}\n  \
         codigo    : {implementados:?}\n  \
         Anadir un canal exige tocar el manifiesto, este crate y el puente de \
         TypeScript. Esa friccion es deliberada: un canal amplia la superficie de \
         ataque del proceso privilegiado."
    );
}

#[test]
fn ningun_canal_prohibido_es_alcanzable() {
    let prohibidos = nombres_bajo(&manifiesto(), "[[prohibido]]");

    assert!(
        !prohibidos.is_empty(),
        "el manifiesto debe declarar canales prohibidos como prueba de regresion"
    );

    for nombre in prohibidos {
        assert!(
            Canal::desde_identificador(&nombre).is_none(),
            "el canal '{nombre}' esta declarado como prohibido pero es alcanzable"
        );
    }
}

#[test]
fn el_limite_de_marco_coincide_con_el_manifiesto() {
    let contenido = manifiesto();
    let esperado = format!("longitud_maxima = {LONGITUD_MAXIMA_MARCO}");

    assert!(
        contenido.contains(&esperado),
        "el manifiesto debe declarar '{esperado}'; \
         un limite distinto en cada extremo permite que un lado acepte lo que el otro rechaza"
    );
}

// ---------------------------------------------------------------------------
// Autorización
// ---------------------------------------------------------------------------

#[test]
fn los_canales_permitidos_se_admiten() {
    for canal in Canal::TODOS {
        assert_eq!(autorizar(canal.identificador(), 128), Ok(canal));
    }
}

#[test]
fn un_canal_desconocido_se_rechaza() {
    assert_eq!(
        autorizar("canal-inventado", 10),
        Err(ErrorIpc::CanalNoPermitido)
    );
}

#[test]
fn no_existe_pasamanos_generico() {
    for nombre in ["invocar", "ejecutar-comando", "ordenar-contencion"] {
        assert_eq!(autorizar(nombre, 10), Err(ErrorIpc::CanalNoPermitido));
    }
}

#[test]
fn una_carga_excesiva_se_rechaza_aunque_el_canal_sea_valido() {
    let resultado = autorizar("obtener-inventario", LONGITUD_MAXIMA_MARCO + 1);
    assert_eq!(
        resultado,
        Err(ErrorIpc::CargaExcesiva {
            longitud: LONGITUD_MAXIMA_MARCO + 1
        })
    );
}

#[test]
fn el_identificador_es_reversible_y_unico() {
    let mut vistos: Vec<&str> = Vec::new();
    for canal in Canal::TODOS {
        let identificador = canal.identificador();
        assert!(
            !vistos.contains(&identificador),
            "identificador duplicado: {identificador}"
        );
        vistos.push(identificador);
        assert_eq!(Canal::desde_identificador(identificador), Some(canal));
    }
}

// ---------------------------------------------------------------------------
// Marcos
// ---------------------------------------------------------------------------

#[test]
fn el_marco_es_reversible() {
    let carga = b"{\"sentencia\":\"SELECT 1\"}";
    let marco = enmarcar(carga).expect("una carga pequena debe enmarcarse");

    assert_eq!(marco.len(), PREFIJO_LONGITUD + carga.len());
    assert_eq!(desenmarcar(&marco), Ok(&carga[..]));
}

#[test]
fn una_carga_vacia_es_un_marco_valido() {
    let marco = enmarcar(b"").expect("la carga vacia es legitima");
    assert_eq!(marco.len(), PREFIJO_LONGITUD);
    assert_eq!(desenmarcar(&marco), Ok(&b""[..]));
}

#[test]
fn un_prefijo_truncado_se_detecta() {
    assert_eq!(desenmarcar(&[0, 0]), Err(ErrorIpc::PrefijoTruncado));
    assert_eq!(desenmarcar(&[]), Err(ErrorIpc::PrefijoTruncado));
}

#[test]
fn un_marco_incompleto_se_detecta() {
    // Declara 10 bytes y solo aporta 3.
    let marco = [0, 0, 0, 10, 1, 2, 3];
    assert_eq!(
        desenmarcar(&marco),
        Err(ErrorIpc::MarcoIncompleto {
            declarados: 10,
            disponibles: 3
        })
    );
}

#[test]
fn un_prefijo_malicioso_no_provoca_reserva() {
    // Declara cerca de cuatro gigabytes. Se rechaza por el limite ANTES de tocar
    // memoria: validar despues de reservar seria una denegacion de servicio de un
    // solo paquete.
    let marco = [0xFF, 0xFF, 0xFF, 0xFF];
    assert_eq!(
        desenmarcar(&marco),
        Err(ErrorIpc::CargaExcesiva {
            longitud: u32::MAX as usize
        })
    );
}

#[test]
fn un_marco_con_cola_sobrante_devuelve_solo_lo_declarado() {
    // El transporte puede entregar varios marcos en una lectura. Devolver la cola
    // mezclaria dos mensajes.
    let mut marco = enmarcar(b"uno").expect("debe enmarcarse");
    marco.extend_from_slice(b"basura posterior");

    assert_eq!(desenmarcar(&marco), Ok(&b"uno"[..]));
}
