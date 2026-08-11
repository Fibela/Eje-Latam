//! Pruebas de las invariantes forenses de ALM-01.
//!
//! Cada prueba corresponde a una propiedad que debe sostenerse ante un auditor o
//! un tribunal. Si alguna falla, lo que se rompe no es una funcion: es el valor
//! probatorio del registro.

// Las restricciones del workspace sobre `panic!` e indexado existen para
// proteger la ruta de produccion. En una prueba, abortar ante un invariante roto
// es el comportamiento deseado.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use crate::ErrorAlmacen;
use crate::cadena::{Asiento, RegistroEvidencia};
use crate::esquema::ClaseEvento;
use crate::merkle::{
    PasoPrueba, PruebaInclusion, hoja, nodo, prueba_inclusion, raiz, verificar_inclusion,
};
use crate::resumen::{Absorbedor, Resumen};
use crate::{BaseDestino, ClaseOperacion, autorizar};

fn registro_de_ejemplo(cantidad: u64) -> RegistroEvidencia {
    let mut registro = RegistroEvidencia::nuevo();
    for indice in 0..cantidad {
        let _ = registro.anexar(
            1_754_000_000_000 + indice as i64,
            ClaseEvento::DeteccionAnomalia,
            &format!("plc-linea-{indice}"),
            "trafico fuera de perfil",
        );
    }
    registro
}

fn resumenes(registro: &RegistroEvidencia) -> Vec<Resumen> {
    registro
        .asientos()
        .iter()
        .map(|asiento| asiento.resumen_propio)
        .collect()
}

// ---------------------------------------------------------------------------
// Codificacion canonica
// ---------------------------------------------------------------------------

#[test]
fn los_prefijos_de_longitud_impiden_la_ambiguedad() {
    // Sin prefijos de longitud, ("ab","c") y ("a","bc") producirian la misma
    // secuencia de bytes y por tanto el mismo resumen. En un registro de
    // evidencia eso permitiria construir dos asientos indistinguibles.
    let mut primero = Absorbedor::nuevo(b"prueba");
    primero.campo(b"ab").campo(b"c");

    let mut segundo = Absorbedor::nuevo(b"prueba");
    segundo.campo(b"a").campo(b"bc");

    assert_ne!(primero.finalizar(), segundo.finalizar());
}

#[test]
fn el_dominio_separa_resumenes_de_datos_identicos() {
    let mut uno = Absorbedor::nuevo(b"dominio-a");
    uno.campo(b"mismo dato");

    let mut otro = Absorbedor::nuevo(b"dominio-b");
    otro.campo(b"mismo dato");

    assert_ne!(uno.finalizar(), otro.finalizar());
}

#[test]
fn el_hexadecimal_es_reversible() {
    let registro = registro_de_ejemplo(1);
    let original = registro.extremo();
    let texto = original.hexadecimal();

    assert_eq!(texto.len(), 64);
    assert_eq!(Resumen::desde_hexadecimal(&texto), Some(original));
    assert_eq!(Resumen::desde_hexadecimal("corto"), None);
    assert_eq!(Resumen::desde_hexadecimal(&"z".repeat(64)), None);
}

// ---------------------------------------------------------------------------
// Cadena de custodia
// ---------------------------------------------------------------------------

#[test]
fn una_cadena_intacta_se_verifica() {
    let registro = registro_de_ejemplo(10);
    assert!(registro.verificar_cadena().is_ok());
}

#[test]
fn el_registro_vacio_es_valido() {
    let registro = RegistroEvidencia::nuevo();
    assert!(registro.vacio());
    assert!(registro.verificar_cadena().is_ok());
    assert_eq!(registro.extremo(), Resumen::GENESIS);
}

/// Funcion que altera un campo de un asiento, para las pruebas de manipulacion.
type Alteracion = fn(&mut crate::cadena::Asiento);

#[test]
fn alterar_cualquier_campo_rompe_la_cadena() {
    // Se altera un campo distinto en cada iteracion. Ninguno puede modificarse
    // sin que la verificacion lo detecte.
    let alteraciones: [(&str, Alteracion); 5] = [
        ("instante", |asiento| asiento.instante_utc += 1),
        ("clase", |asiento| {
            asiento.clase = ClaseEvento::CambioConfiguracion;
        }),
        ("nodo", |asiento| {
            asiento.nodo = "nodo-sustituido".to_owned();
        }),
        ("detalle", |asiento| {
            asiento.detalle = "todo en orden".to_owned();
        }),
        ("numero", |asiento| asiento.numero += 100),
    ];

    for (campo, alterar) in alteraciones {
        let registro = registro_de_ejemplo(5);
        // Se manipula el asiento intermedio directamente, como haria quien edita
        // la base de datos por debajo del agente.
        let mut asientos: Vec<_> = registro.asientos().to_vec();
        alterar(&mut asientos[2]);

        let mut manipulado = RegistroEvidencia::nuevo();
        for asiento in asientos {
            manipulado.anexar_crudo_para_pruebas(asiento);
        }

        assert!(
            manipulado.verificar_cadena().is_err(),
            "alterar '{campo}' debe romper la cadena"
        );
    }
}

#[test]
fn eliminar_un_asiento_intermedio_rompe_la_cadena() {
    let registro = registro_de_ejemplo(5);
    let mut asientos = registro.asientos().to_vec();
    asientos.remove(2);

    let mut manipulado = RegistroEvidencia::nuevo();
    for asiento in asientos {
        manipulado.anexar_crudo_para_pruebas(asiento);
    }

    assert!(manipulado.verificar_cadena().is_err());
}

#[test]
fn reordenar_asientos_rompe_la_cadena() {
    let registro = registro_de_ejemplo(5);
    let mut asientos = registro.asientos().to_vec();
    asientos.swap(1, 3);

    let mut manipulado = RegistroEvidencia::nuevo();
    for asiento in asientos {
        manipulado.anexar_crudo_para_pruebas(asiento);
    }

    assert!(manipulado.verificar_cadena().is_err());
}

#[test]
fn cada_asiento_enlaza_con_el_anterior() {
    let registro = registro_de_ejemplo(4);
    let asientos = registro.asientos();

    assert_eq!(asientos[0].resumen_anterior, Resumen::GENESIS);
    for indice in 1..asientos.len() {
        assert_eq!(
            asientos[indice].resumen_anterior,
            asientos[indice - 1].resumen_propio
        );
    }
}

// ---------------------------------------------------------------------------
// Sellos Merkle y divulgacion selectiva
// ---------------------------------------------------------------------------

#[test]
fn un_registro_vacio_no_produce_raiz() {
    assert_eq!(raiz(&[]), None);
}

#[test]
fn toda_prueba_de_inclusion_verifica_contra_la_raiz() {
    // Se prueba con cantidades pares e impares: el nodo impar se promueve sin
    // volver a resumirse y es donde fallan las implementaciones descuidadas.
    for cantidad in 1_u64..=9 {
        let registro = registro_de_ejemplo(cantidad);
        let lista = resumenes(&registro);
        let Some(raiz_calculada) = raiz(&lista) else {
            panic!("un registro no vacio debe producir raiz");
        };

        for posicion in 0..lista.len() {
            let Some(prueba) = prueba_inclusion(&lista, posicion, posicion as u64 + 1) else {
                panic!("debe existir prueba para toda posicion valida");
            };
            assert!(
                verificar_inclusion(&prueba, &raiz_calculada),
                "fallo con {cantidad} asientos en la posicion {posicion}"
            );
        }
    }
}

#[test]
fn una_prueba_con_asiento_sustituido_no_verifica() {
    let registro = registro_de_ejemplo(8);
    let lista = resumenes(&registro);
    let Some(raiz_calculada) = raiz(&lista) else {
        panic!("debe haber raiz");
    };
    let Some(mut prueba) = prueba_inclusion(&lista, 3, 4) else {
        panic!("debe haber prueba");
    };

    prueba.resumen_asiento = lista[5];
    assert!(!verificar_inclusion(&prueba, &raiz_calculada));
}

#[test]
fn una_prueba_con_camino_alterado_no_verifica() {
    let registro = registro_de_ejemplo(8);
    let lista = resumenes(&registro);
    let Some(raiz_calculada) = raiz(&lista) else {
        panic!("debe haber raiz");
    };
    let Some(mut prueba) = prueba_inclusion(&lista, 2, 3) else {
        panic!("debe haber prueba");
    };

    prueba.camino[0].hermano = Resumen::GENESIS;
    assert!(!verificar_inclusion(&prueba, &raiz_calculada));
}

#[test]
fn invertir_la_orientacion_de_un_paso_invalida_la_prueba() {
    let registro = registro_de_ejemplo(4);
    let lista = resumenes(&registro);
    let Some(raiz_calculada) = raiz(&lista) else {
        panic!("debe haber raiz");
    };
    let Some(mut prueba) = prueba_inclusion(&lista, 1, 2) else {
        panic!("debe haber prueba");
    };

    prueba.camino[0].hermano_a_la_derecha = !prueba.camino[0].hermano_a_la_derecha;
    assert!(!verificar_inclusion(&prueba, &raiz_calculada));
}

#[test]
fn un_asiento_ajeno_al_registro_no_puede_probarse() {
    let registro = registro_de_ejemplo(6);
    let lista = resumenes(&registro);
    let Some(raiz_calculada) = raiz(&lista) else {
        panic!("debe haber raiz");
    };
    let Some(prueba_legitima) = prueba_inclusion(&lista, 0, 1) else {
        panic!("debe haber prueba");
    };

    let inventada = PruebaInclusion {
        numero: 99,
        resumen_asiento: Resumen::desde_bytes([7u8; 32]),
        camino: prueba_legitima.camino,
    };
    assert!(!verificar_inclusion(&inventada, &raiz_calculada));
}

#[test]
fn la_separacion_de_dominio_impide_pasar_un_nodo_interno_por_hoja() {
    // Ataque de segunda preimagen sobre arboles Merkle: sin dominios distintos,
    // el resumen de un nodo interno podria presentarse como hoja y fabricar la
    // inclusion de un asiento inexistente.
    let uno = Resumen::desde_bytes([1u8; 32]);
    let dos = Resumen::desde_bytes([2u8; 32]);

    let interno = nodo(&uno, &dos);
    let como_hoja = hoja(&interno);

    assert_ne!(interno, como_hoja);
}

#[test]
fn la_raiz_cambia_si_cambia_cualquier_asiento() {
    let original = registro_de_ejemplo(7);
    let lista_original = resumenes(&original);

    let mut distinto = RegistroEvidencia::nuevo();
    for indice in 0..7_u64 {
        let detalle = if indice == 4 {
            "trafico dentro de perfil"
        } else {
            "trafico fuera de perfil"
        };
        let _ = distinto.anexar(
            1_754_000_000_000 + indice as i64,
            ClaseEvento::DeteccionAnomalia,
            &format!("plc-linea-{indice}"),
            detalle,
        );
    }

    assert_ne!(raiz(&lista_original), raiz(&resumenes(&distinto)));
}

#[test]
fn la_prueba_no_revela_los_demas_asientos() {
    // El camino contiene resumenes de nodos, no los datos de otros asientos.
    // Esta es la propiedad que permite aportar un evento a un tribunal sin
    // exportar el trafico de red de terceros (RPT-002 §6).
    let registro = registro_de_ejemplo(8);
    let lista = resumenes(&registro);
    let Some(prueba) = prueba_inclusion(&lista, 0, 1) else {
        panic!("debe haber prueba");
    };

    let tamano_prueba = prueba.camino.len();
    assert!(
        tamano_prueba <= 3,
        "para 8 asientos el camino debe tener a lo sumo 3 pasos, tiene {tamano_prueba}"
    );

    for paso in &prueba.camino {
        assert!(
            !lista.contains(&paso.hermano),
            "el camino no debe exponer resumenes de asiento en claro"
        );
    }
}

#[test]
fn el_paso_de_prueba_es_comparable() {
    let paso = PasoPrueba {
        hermano: Resumen::GENESIS,
        hermano_a_la_derecha: true,
    };
    assert_eq!(paso, paso);
}

// ---------------------------------------------------------------------------
// Esquema y autorizacion
// ---------------------------------------------------------------------------

#[test]
fn los_identificadores_de_clase_son_reversibles_y_unicos() {
    const TODAS: [ClaseEvento; 10] = [
        ClaseEvento::ArranqueAgente,
        ClaseEvento::DeteccionAnomalia,
        ClaseEvento::OrdenContencion,
        ClaseEvento::RechazoSimulacion,
        ClaseEvento::CambioConfiguracion,
        ClaseEvento::ActualizacionAplicada,
        ClaseEvento::FirmaModuloRechazada,
        ClaseEvento::BovedaDesbordada,
        ClaseEvento::UsoEnGracia,
        ClaseEvento::SelloEmitido,
    ];

    let mut vistos = Vec::new();
    for clase in TODAS {
        let identificador = clase.identificador();
        assert!(
            !vistos.contains(&identificador),
            "identificador duplicado: {identificador}"
        );
        vistos.push(identificador);
        assert_eq!(ClaseEvento::desde_identificador(identificador), Some(clase));
    }

    assert_eq!(ClaseEvento::desde_identificador("inventado"), None);
}

#[test]
fn el_ddl_de_evidencia_no_concede_modificacion() {
    let ddl = crate::esquema::DDL_EVIDENCIA.to_uppercase();
    assert!(!ddl.contains("ON DELETE CASCADE"));
    assert!(ddl.contains("STRICT"), "las tablas deben ser STRICT");
}

#[test]
fn evidencia_admite_consulta_y_anexado() {
    assert!(autorizar(BaseDestino::RegistroEvidencia, ClaseOperacion::Consulta).is_ok());
    assert!(autorizar(BaseDestino::RegistroEvidencia, ClaseOperacion::Anexado).is_ok());
}

#[test]
fn evidencia_rechaza_modificacion_y_ddl() {
    let evidencia = BaseDestino::RegistroEvidencia;
    assert!(autorizar(evidencia, ClaseOperacion::Modificacion).is_err());
    assert!(autorizar(evidencia, ClaseOperacion::DefinicionEsquema).is_err());
}

#[test]
fn sandbox_admite_todo() {
    for operacion in [
        ClaseOperacion::Consulta,
        ClaseOperacion::Anexado,
        ClaseOperacion::Modificacion,
        ClaseOperacion::DefinicionEsquema,
    ] {
        assert!(autorizar(BaseDestino::SandboxAnalista, operacion).is_ok());
    }
}

#[test]
fn la_autorizacion_es_independiente_del_encadenamiento() {
    // Regresion: la logica de encadenamiento no sustituye a la autorizacion.
    // Son capas independientes y ambas deben sostenerse por separado.
    assert!(autorizar(BaseDestino::RegistroEvidencia, ClaseOperacion::Modificacion).is_err());
    assert!(autorizar(BaseDestino::RegistroEvidencia, ClaseOperacion::Anexado).is_ok());
}

// ---------------------------------------------------------------------------
// Techo del formato — RPT-039, PA-72
// ---------------------------------------------------------------------------

#[test]
fn el_registro_se_niega_a_pasar_del_maximo_en_lugar_de_volverse_ilegible() {
    // El defecto que PA-72 cierra. `ASIENTOS_MAXIMOS` solo se comprobaba al
    // LEER, asi que un agente que superaba el techo seguia anexando, escribia el
    // fichero entero, y el arranque siguiente no podia releerlo. Un registro
    // ilegible se lee como violacion, de modo que el agente apartaba el fichero
    // acusando de manipulacion a nadie: solo habia trabajado demasiado tiempo.
    //
    // Se rellena con asientos crudos a proposito: construirlos por `anexar`
    // costaria medio millon de SHA-256 y esta prueba tardaria mas que toda la
    // suite. Lo que se comprueba aqui es la cota, no el encadenado, que ya tiene
    // las suyas.
    let mut registro = RegistroEvidencia::nuevo();
    for numero in 1..=crate::persistencia::ASIENTOS_MAXIMOS as u64 {
        registro.anexar_crudo_para_pruebas(Asiento {
            numero,
            instante_utc: 0,
            clase: ClaseEvento::ArranqueAgente,
            nodo: String::new(),
            detalle: String::new(),
            resumen_anterior: Resumen::GENESIS,
            resumen_propio: Resumen::GENESIS,
        });
    }

    assert!(registro.saturado());
    assert!(
        matches!(
            registro.anexar(0, ClaseEvento::DeteccionAnomalia, "n", "d"),
            Err(ErrorAlmacen::CapacidadExcedida { .. })
        ),
        "por encima del techo hay que negarse, no seguir y volverse ilegible"
    );
}

#[test]
fn un_registro_normal_no_esta_saturado() {
    let mut registro = RegistroEvidencia::nuevo();
    assert!(!registro.saturado());
    assert!(
        registro
            .anexar(0, ClaseEvento::ArranqueAgente, "n", "d")
            .is_ok()
    );
    assert!(!registro.saturado());
}

// ---------------------------------------------------------------------------
// Segmentacion — RPT-040, PA-59
// ---------------------------------------------------------------------------

use crate::persistencia::{ErrorPersistencia, analizar, serializar};

/// Registro con `cuantas` anotaciones, empezando donde se diga.
fn segmento(base: u64, genesis: Resumen, cuantas: u64) -> RegistroEvidencia {
    let mut registro = RegistroEvidencia::continuando(base, genesis);
    for indice in 0..cuantas {
        registro
            .anexar(
                1_000 + indice as i64,
                ClaseEvento::DeteccionAnomalia,
                "nodo",
                "detalle",
            )
            .expect("cabe");
    }
    registro
}

#[test]
fn un_segmento_que_no_empieza_en_uno_numera_desde_su_base() {
    // La numeracion deja de ser posicional. Si `anexar` siguiera derivandola de
    // la longitud, el segundo segmento reutilizaria los numeros del primero y
    // borrar evidencia volveria a salir gratis (RPT-029 §4).
    let primero = segmento(1, Resumen::GENESIS, 3);
    let segundo = segmento(4, primero.extremo(), 2);

    assert_eq!(segundo.asientos()[0].numero, 4);
    assert_eq!(segundo.asientos()[1].numero, 5);
    assert_eq!(segundo.ultimo_numero(), 5);
    assert!(segundo.verificar_cadena().is_ok());
}

#[test]
fn un_segmento_vacio_declara_el_ultimo_del_anterior() {
    // Es la propiedad de la que cuelga toda la rotacion de RPT-040 §1: un
    // segmento recien abierto no dice «no hay nada», dice «lo ultimo que consta
    // es el asiento N», y por eso el ancla del anterior lo valida sin cambiar.
    let primero = segmento(1, Resumen::GENESIS, 4);
    let vacio = RegistroEvidencia::continuando(5, primero.extremo());

    assert!(vacio.vacio());
    assert_eq!(vacio.ultimo_numero(), 4);
    assert_eq!(vacio.extremo(), primero.extremo());
}

#[test]
fn la_cadena_de_un_segmento_arranca_en_su_genesis_y_no_en_el_absoluto() {
    // Comparar contra `Resumen::GENESIS` haria que TODO segmento rotado se
    // leyera como cadena rota, es decir: rotar produciria una acusacion de
    // manipulacion en cada arranque posterior.
    let primero = segmento(1, Resumen::GENESIS, 3);
    let segundo = segmento(4, primero.extremo(), 3);

    assert!(segundo.verificar_cadena().is_ok());
    assert_eq!(segundo.asientos()[0].resumen_anterior, primero.extremo());
}

#[test]
fn alterar_un_segmento_archivado_rompe_su_enlace_con_el_siguiente() {
    // Lo que sustituye al ancla en los archivados. Cada segmento sigue
    // verificando consigo mismo —la cadena se reconstruye al cargar— pero la
    // frontera deja de cuadrar, y eso es lo que nadie puede falsificar sin tener
    // tambien el segmento siguiente.
    let primero = segmento(1, Resumen::GENESIS, 3);
    let segundo = segmento(4, primero.extremo(), 2);
    assert!(segundo.continua_a(&primero));

    // El atacante rehace el segmento archivado con un detalle distinto.
    let mut alterado = RegistroEvidencia::nuevo();
    for indice in 0..3u64 {
        alterado
            .anexar(
                1_000 + indice as i64,
                ClaseEvento::DeteccionAnomalia,
                "nodo",
                if indice == 2 { "otra cosa" } else { "detalle" },
            )
            .expect("cabe");
    }

    assert!(
        alterado.verificar_cadena().is_ok(),
        "consigo mismo cuadra: la cadena se reconstruye"
    );
    assert!(
        !segundo.continua_a(&alterado),
        "pero la frontera con el siguiente no"
    );
}

#[test]
fn un_segmento_va_y_vuelve_del_disco_con_su_base_y_su_genesis() {
    let primero = segmento(1, Resumen::GENESIS, 2);
    let segundo = segmento(3, primero.extremo(), 2);

    let recuperado = analizar(&serializar(&segundo)).expect("verifica");

    assert_eq!(recuperado.base(), 3);
    assert_eq!(recuperado.genesis(), primero.extremo());
    assert_eq!(recuperado.extremo(), segundo.extremo());
    assert!(recuperado.continua_a(&primero));
}

#[test]
fn un_fichero_de_la_version_anterior_se_lee_como_el_primer_segmento() {
    // Es evidencia real de un cliente. Rechazarlo por «version desconocida» lo
    // convertiria en ViolacionDetectada y el agente acusaria de manipulacion a
    // quien solo actualizo el ejecutable (RPT-040 §2).
    let registro = segmento(1, Resumen::GENESIS, 3);
    let v2 = serializar(&registro);

    // La v1 es la misma carga sin los campos de base y genesis.
    let mut v1 = Vec::new();
    v1.extend_from_slice(&v2[..8]);
    v1.extend_from_slice(&1u16.to_be_bytes());
    v1.extend_from_slice(&v2[50..]);

    let recuperado = analizar(&v1).expect("la version 1 se interpreta, no se rechaza");

    assert_eq!(recuperado.base(), 1);
    assert_eq!(recuperado.genesis(), Resumen::GENESIS);
    assert_eq!(recuperado.longitud(), 3);
    assert_eq!(recuperado.extremo(), registro.extremo());
}

#[test]
fn una_base_cero_se_rechaza_en_la_puerta() {
    // Con base cero, el `ultimo_numero` de un segmento vacio se calcularia sobre
    // `0 - 1`. Es aritmetica que conviene no dejar entrar en lugar de acotarla
    // treinta llamadas mas adentro.
    let mut bytes = serializar(&segmento(1, Resumen::GENESIS, 1));
    bytes[10..18].copy_from_slice(&0u64.to_be_bytes());

    assert!(matches!(
        analizar(&bytes),
        Err(ErrorPersistencia::BaseInvalida)
    ));
}

#[test]
fn una_version_posterior_a_la_conocida_se_sigue_rechazando() {
    // Leer la v1 es interpretar algo que se entiende. Leer una v3 seria adivinar,
    // y adivinar sobre evidencia es peor que declarar que no se puede.
    let mut bytes = serializar(&segmento(1, Resumen::GENESIS, 1));
    bytes[8..10].copy_from_slice(&3u16.to_be_bytes());

    assert!(matches!(
        analizar(&bytes),
        Err(ErrorPersistencia::VersionDesconocida { encontrada: 3 })
    ));
}
