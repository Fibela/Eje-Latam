//! Composicion del inventario para el cable. RPT-090, PA-138b.
//!
//! # Por que esto es un modulo y no cuatro lineas en el manejador
//!
//! Aqui vive la unica traduccion del proyecto entre un calculo del dominio y un
//! valor que sale por el socket. RPT-089 conto lo que pasa cuando esa traduccion
//! se escribe de memoria: el cable acabo declarando clases inferidas que
//! `clasificar` nunca produce.
//!
//! El `match` de [`clase_en_el_cable`] **no tiene brazo comodin**. Si manana el
//! dominio anade un motivo de ambiguedad, esto deja de compilar y obliga a
//! decidir que se le dice al operador — en lugar de mandarle en silencio el
//! valor mas parecido.

use eje_ipc::mensajes::{ClaseConocida, DeclaracionSegmento as SegmentoEnCable, NodoInventario};
use guardian_cc::ClaseExcluida;
use guardian_cc::clasificacion::{
    Clasificacion, DeclaracionSegmento, FuenteEvidencia, MotivoAmbiguedad,
};

/// Traduce el veredicto del dominio al valor que viaja.
///
/// # Los dos casos que no son obvios
///
/// `Clasificado { clase: None, MarcadoAdministrativo }` es **«no critico, y hay
/// un humano que lo firma»**: el unico estado que permite contener sin
/// intervencion. No es «no se sabe nada», que es justo lo contrario.
///
/// `Clasificado { clase: None, HuellaPasiva | OuiFabricante }` **no lo produce
/// `clasificar`**: una fuente inferida no puede declarar ausencia de criticidad
/// (RPT-009 §3). Si llegara, se trata como indeterminado en lugar de leerse como
/// una absolucion — fallar cerrado.
#[must_use]
pub const fn clase_en_el_cable(clasificacion: Clasificacion) -> ClaseConocida {
    match clasificacion {
        Clasificacion::Clasificado {
            clase: Some(ClaseExcluida::SoporteVital),
            ..
        } => ClaseConocida::DeclaradaSoporteVital,

        Clasificacion::Clasificado {
            clase: Some(ClaseExcluida::SeguridadFuncional),
            ..
        } => ClaseConocida::DeclaradaSeguridadFuncional,

        Clasificacion::Clasificado {
            clase: Some(ClaseExcluida::CaminoDeGestion),
            ..
        } => ClaseConocida::DeclaradaCaminoDeGestion,

        Clasificacion::Clasificado {
            clase: None,
            fuente: FuenteEvidencia::MarcadoAdministrativo,
        } => ClaseConocida::DeclaradaNoCritica,

        Clasificacion::Clasificado {
            clase: None,
            fuente: FuenteEvidencia::DeclaracionDeSegmento,
        } => ClaseConocida::SegmentoDeclaradoSinCriticos,

        // Ninguna fuente inferida puede declarar ausencia de criticidad, asi que
        // esto no ocurre. Si ocurriera: ni se absuelve —seria mentir— ni se
        // acusa de manipulacion —seria peor—. Se dice que no hay veredicto.
        Clasificacion::Clasificado {
            clase: None,
            fuente: FuenteEvidencia::HuellaPasiva | FuenteEvidencia::OuiFabricante,
        } => ClaseConocida::Indeterminada,

        Clasificacion::Ambiguo {
            motivo: MotivoAmbiguedad::MarcadoCaducado,
        } => ClaseConocida::AmbiguaMarcadoCaducado,

        Clasificacion::Ambiguo {
            motivo: MotivoAmbiguedad::ConflictoEntreFuentes,
        } => ClaseConocida::AmbiguaConflictoEntreFuentes,

        Clasificacion::Ambiguo {
            motivo: MotivoAmbiguedad::InferenciaSugiereCriticidad,
        } => ClaseConocida::AmbiguaInferenciaSugiereCriticidad,

        Clasificacion::Ambiguo {
            motivo: MotivoAmbiguedad::SegmentoPuedeAlojarCriticos,
        } => ClaseConocida::AmbiguaSegmentoPuedeAlojarCriticos,

        // La mas grave de las cinco: una firma invalida o una inclusion no
        // probada indican manipulacion del inventario, no ausencia de marcado
        // (RPT-010). Mandarla como «no se sabe» borraria la acusacion.
        Clasificacion::Ambiguo {
            motivo: MotivoAmbiguedad::EvidenciaNoVerificable,
        } => ClaseConocida::AmbiguaEvidenciaNoVerificable,

        // Deliberadamente inalcanzable desde `clasificar` (guardian-cc lo dice en
        // su propio comentario). Existe para que nadie asuma que la evidencia
        // siempre llega, y en el cable se dice como lo que es: sin veredicto.
        Clasificacion::NoClasificado => ClaseConocida::Indeterminada,
    }
}

/// Traduce la declaracion de segmento al cable.
///
/// Mismo motivo que `perfil_en_el_cable` (RPT-081): `eje-ipc` depende solo de
/// `thiserror` y `serde`, asi que el tipo esta duplicado y la traduccion vive
/// aqui. Un `match` exhaustivo, no un `as`.
#[must_use]
pub const fn segmento_en_el_cable(declaracion: DeclaracionSegmento) -> SegmentoEnCable {
    match declaracion {
        DeclaracionSegmento::SinDispositivosCriticos => SegmentoEnCable::SinDispositivosCriticos,
        DeclaracionSegmento::PuedeAlojarCriticos => SegmentoEnCable::PuedeAlojarCriticos,
        DeclaracionSegmento::NoDeclarado => SegmentoEnCable::NoDeclarado,
    }
}

/// La direccion, en la notacion que ya usa el resto del proyecto.
#[must_use]
pub fn direccion_en_texto(direccion: &[u8; 6]) -> String {
    direccion
        .iter()
        .map(|octeto| format!("{octeto:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Compone un nodo del inventario a partir de lo observado y su clasificacion.
#[must_use]
pub fn nodo_en_el_cable(
    vista: &guardian_cc::observacion::VistaNodo,
    clasificacion: Clasificacion,
) -> NodoInventario {
    NodoInventario {
        direccion_enlace: direccion_en_texto(&vista.direccion),
        clase: clase_en_el_cable(clasificacion),
        declaracion_segmento: segmento_en_el_cable(vista.segmento),
        visto_en_segmento_critico: vista.pegajoso,
        protocolos_observados: vista
            .protocolos
            .iter()
            .map(|protocolo| format!("{protocolo:?}").to_lowercase())
            .collect(),
    }
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use guardian_cc::observacion::{Protocolo, VistaNodo};

    /// Todo resultado alcanzable de `clasificar` tiene destino, y **ninguno se
    /// repite salvo donde esta escrito por que**.
    ///
    /// RPT-089 se abrio porque el cable declaraba clases inferidas que el motor
    /// no produce. Esta prueba mira el otro sentido: que ningun resultado del
    /// motor se quede sin valor propio.
    #[test]
    fn cada_veredicto_del_dominio_tiene_su_valor_en_el_cable() {
        let casos = [
            (
                Clasificacion::Clasificado {
                    clase: Some(ClaseExcluida::SoporteVital),
                    fuente: FuenteEvidencia::MarcadoAdministrativo,
                },
                ClaseConocida::DeclaradaSoporteVital,
            ),
            (
                Clasificacion::Clasificado {
                    clase: Some(ClaseExcluida::CaminoDeGestion),
                    fuente: FuenteEvidencia::MarcadoAdministrativo,
                },
                ClaseConocida::DeclaradaCaminoDeGestion,
            ),
            (
                Clasificacion::Clasificado {
                    clase: None,
                    fuente: FuenteEvidencia::MarcadoAdministrativo,
                },
                ClaseConocida::DeclaradaNoCritica,
            ),
            (
                Clasificacion::Clasificado {
                    clase: None,
                    fuente: FuenteEvidencia::DeclaracionDeSegmento,
                },
                ClaseConocida::SegmentoDeclaradoSinCriticos,
            ),
            (
                Clasificacion::Ambiguo {
                    motivo: MotivoAmbiguedad::MarcadoCaducado,
                },
                ClaseConocida::AmbiguaMarcadoCaducado,
            ),
            (
                Clasificacion::Ambiguo {
                    motivo: MotivoAmbiguedad::ConflictoEntreFuentes,
                },
                ClaseConocida::AmbiguaConflictoEntreFuentes,
            ),
            (
                Clasificacion::Ambiguo {
                    motivo: MotivoAmbiguedad::InferenciaSugiereCriticidad,
                },
                ClaseConocida::AmbiguaInferenciaSugiereCriticidad,
            ),
            (
                Clasificacion::Ambiguo {
                    motivo: MotivoAmbiguedad::SegmentoPuedeAlojarCriticos,
                },
                ClaseConocida::AmbiguaSegmentoPuedeAlojarCriticos,
            ),
            (
                Clasificacion::Ambiguo {
                    motivo: MotivoAmbiguedad::EvidenciaNoVerificable,
                },
                ClaseConocida::AmbiguaEvidenciaNoVerificable,
            ),
        ];

        assert_eq!(
            casos.len(),
            MotivoAmbiguedad::TODOS.len() + 4,
            "los cinco motivos mas las cuatro formas de Clasificado que se prueban"
        );

        for (veredicto, esperado) in casos {
            assert_eq!(clase_en_el_cable(veredicto), esperado, "{veredicto:?}");
        }
    }

    /// «No critico firmado» y «no se sabe» no pueden colapsar.
    ///
    /// Es la distincion que RPT-089 encontro ausente. Uno permite contener sin
    /// intervencion humana; el otro significa que nada apunta a nada.
    #[test]
    fn lo_declarado_no_critico_no_se_confunde_con_no_saber() {
        let firmado = clase_en_el_cable(Clasificacion::Clasificado {
            clase: None,
            fuente: FuenteEvidencia::MarcadoAdministrativo,
        });

        assert_eq!(firmado, ClaseConocida::DeclaradaNoCritica);
        assert_ne!(firmado, ClaseConocida::Indeterminada);
        assert_ne!(firmado, ClaseConocida::SegmentoDeclaradoSinCriticos);
    }

    /// Una fuente inferida nunca sale como absolucion.
    ///
    /// `clasificar` no produce este caso, pero si alguien lo construyera, el
    /// cable dice «no se sabe» y no «no es critico». Fallar cerrado.
    #[test]
    fn una_absolucion_inferida_se_declara_indeterminada_y_no_limpia() {
        for fuente in [
            FuenteEvidencia::HuellaPasiva,
            FuenteEvidencia::OuiFabricante,
        ] {
            assert_eq!(
                clase_en_el_cable(Clasificacion::Clasificado {
                    clase: None,
                    fuente
                }),
                ClaseConocida::Indeterminada,
                "{fuente:?} no puede absolver a nadie"
            );

            assert_ne!(
                clase_en_el_cable(Clasificacion::Clasificado {
                    clase: None,
                    fuente
                }),
                ClaseConocida::AmbiguaEvidenciaNoVerificable,
                "tampoco se acusa de manipulacion a quien solo carece de marcado"
            );
        }
    }

    #[test]
    fn la_direccion_viaja_como_mac_y_no_como_vector() {
        assert_eq!(
            direccion_en_texto(&[0x00, 0x1B, 0x21, 0x00, 0x00, 0x01]),
            "00:1b:21:00:00:01"
        );
    }

    #[test]
    fn el_nodo_lleva_lo_observado_y_nada_mas() {
        let vista = VistaNodo {
            direccion: [0x00, 0x1B, 0x21, 0x00, 0x00, 0x01],
            protocolos: vec![Protocolo::Hl7, Protocolo::Modbus],
            segmento: DeclaracionSegmento::PuedeAlojarCriticos,
            visto_en: 7,
            pegajoso: true,
        };

        let nodo = nodo_en_el_cable(
            &vista,
            Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::SegmentoPuedeAlojarCriticos,
            },
        );

        assert_eq!(nodo.direccion_enlace, "00:1b:21:00:00:01");
        assert_eq!(nodo.protocolos_observados, vec!["hl7", "modbus"]);
        assert_eq!(
            nodo.declaracion_segmento,
            SegmentoEnCable::PuedeAlojarCriticos
        );
        assert!(
            nodo.visto_en_segmento_critico,
            "la marca viaja, y significa segmento critico y NO contencion"
        );
    }
}
