//! Pruebas del contrato IPC.
//!
//! La prueba central es la de **paridad con el manifiesto**: si `Canal` y
//! `contrato-ipc.toml` divergen, esta suite falla. Sin ella, el manifiesto sería
//! documentación decorativa y volveríamos al punto de partida, con cada extremo
//! declarando sus canales por su cuenta.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::{
    CODIGO_RECHAZO, CODIGO_RESPUESTA, Canal, ErrorIpc, LONGITUD_MAXIMA_MARCO, NOMBRE_MAXIMO,
    PREFIJO_LONGITUD, PREFIJO_NOMBRE, autorizar, componer_peticion, componer_rechazo,
    descomponer_peticion, desenmarcar, enmarcar,
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
fn la_forma_de_la_peticion_coincide_con_el_manifiesto() {
    // RPT-035. Este bloque del manifiesto faltaba: declaraba QUE canales existen
    // y QUE campos lleva cada carga, pero no COMO viaja el nombre por el cable.
    // Mientras no hubo transporte no se noto; al escribir el servicio, cada
    // extremo habria tenido que inventarlo.
    let contenido = manifiesto();

    for esperado in [
        format!("prefijo_nombre = {PREFIJO_NOMBRE}"),
        format!("nombre_maximo = {NOMBRE_MAXIMO}"),
        format!("codigo_respuesta = {CODIGO_RESPUESTA}"),
        format!("codigo_rechazo = {CODIGO_RECHAZO}"),
    ] {
        assert!(
            contenido.contains(&esperado),
            "el manifiesto debe declarar '{esperado}'; una forma distinta en cada \
             extremo permite que un lado acepte lo que el otro rechaza"
        );
    }
}

#[test]
fn la_peticion_es_reversible_y_autoriza_al_descomponer() {
    for canal in Canal::TODOS {
        let peticion = componer_peticion(canal, b"carga").expect("el canal esta permitido");
        let (leido, util) = descomponer_peticion(&peticion).expect("se descompone");

        assert_eq!(leido, canal);
        assert_eq!(util, b"carga");
    }
}

#[test]
fn un_nombre_prefijado_absurdo_no_llega_a_indexar() {
    let mut absurdo = Vec::new();
    absurdo.extend_from_slice(&u16::MAX.to_be_bytes());
    absurdo.extend_from_slice(b"corto");

    assert_eq!(
        descomponer_peticion(&absurdo),
        Err(ErrorIpc::CanalNoPermitido)
    );
    assert!(NOMBRE_MAXIMO < u16::MAX as usize);
}

#[test]
fn el_rechazo_nunca_falla_al_componerse() {
    // Quien rechaza ya esta en el camino de error. Un fallo al construir el
    // mensaje de fallo dejaria al otro extremo sin respuesta ninguna, que es lo
    // unico inaceptable.
    let enorme = "x".repeat(LONGITUD_MAXIMA_MARCO * 2);
    let rechazo = componer_rechazo(&enorme);

    assert_eq!(rechazo[0], CODIGO_RECHAZO);
    assert!(rechazo.len() <= LONGITUD_MAXIMA_MARCO);
    assert!(enmarcar(&rechazo).is_ok(), "y siempre cabe en un marco");
}

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

// ---------------------------------------------------------------------------
// Carga util — PA-21
// ---------------------------------------------------------------------------

use crate::mensajes::{
    CAMPOS_CONDICIONES, CAMPOS_ESTADO_AGENTE, CAMPOS_ESTADO_BOVEDA, CAMPOS_NODO_INVENTARIO,
    CAMPOS_PETICION_ALERTAS, CAMPOS_PETICION_CONSULTA, CAMPOS_RESPUESTA_ALERTAS,
    CAMPOS_RESULTADO_CONSULTA, CAMPOS_SUCESO_ALERTA, ClaseAlerta, ClaseDispositivo, Condiciones,
    EstadoAgente, EstadoBoveda, NodoInventario, PerfilSegmento, PeticionAlertas, PeticionConsulta,
    Postura, ResultadoConsulta, SucesoAlerta,
};

/// Campo declarado en el manifiesto.
struct CampoDeclarado {
    registro: String,
    nombre: String,
    tipo: String,
}

/// Extrae el valor entrecomillado de una línea `clave = "valor"`.
fn entrecomillado(linea: &str, prefijo: &str) -> Option<String> {
    let resto = linea.strip_prefix(prefijo)?;
    let sin_inicio = resto.strip_prefix('"')?;
    let fin = sin_inicio.find('"')?;
    Some(sin_inicio[..fin].to_owned())
}

/// Lee los bloques `[[campo]]` del manifiesto, en orden de aparición.
fn campos_declarados(contenido: &str) -> Vec<CampoDeclarado> {
    let mut salida = Vec::new();
    let mut actual: Option<(String, String, String)> = None;

    let cerrar = |actual: Option<(String, String, String)>, salida: &mut Vec<CampoDeclarado>| {
        if let Some((registro, nombre, tipo)) = actual {
            if !registro.is_empty() && !nombre.is_empty() {
                salida.push(CampoDeclarado {
                    registro,
                    nombre,
                    tipo,
                });
            }
        }
    };

    for linea in contenido.lines() {
        let limpia = linea.trim();

        if limpia.starts_with('[') {
            cerrar(actual.take(), &mut salida);
            if limpia == "[[campo]]" {
                actual = Some((String::new(), String::new(), String::new()));
            }
            continue;
        }
        if limpia.starts_with('#') {
            continue;
        }

        if let Some((registro, nombre, tipo)) = actual.as_mut() {
            if let Some(valor) = entrecomillado(limpia, "registro = ") {
                *registro = valor;
            } else if let Some(valor) = entrecomillado(limpia, "nombre = ") {
                *nombre = valor;
            } else if let Some(valor) = entrecomillado(limpia, "tipo = ") {
                *tipo = valor;
            }
        }
    }

    cerrar(actual, &mut salida);
    salida
}

/// Compara los campos declarados para un registro con los implementados.
fn comprobar_registro(registro: &str, esperados: &[(&str, &str)]) {
    let contenido = manifiesto();
    let declarados: Vec<(String, String)> = campos_declarados(&contenido)
        .into_iter()
        .filter(|campo| campo.registro == registro)
        .map(|campo| (campo.nombre, campo.tipo))
        .collect();

    let implementados: Vec<(String, String)> = esperados
        .iter()
        .map(|(nombre, tipo)| ((*nombre).to_owned(), (*tipo).to_owned()))
        .collect();

    assert_eq!(
        declarados, implementados,
        "el registro '{registro}' diverge entre contrato-ipc.toml y el codigo.\n  \
         manifiesto: {declarados:?}\n  codigo    : {implementados:?}\n  \
         El orden tambien importa: reordenar en un solo lado produce un diff que \
         parece inocuo."
    );
}

#[test]
fn los_registros_coinciden_con_el_manifiesto() {
    comprobar_registro("EstadoAgente", &CAMPOS_ESTADO_AGENTE);
    comprobar_registro("NodoInventario", &CAMPOS_NODO_INVENTARIO);
    comprobar_registro("EstadoBoveda", &CAMPOS_ESTADO_BOVEDA);
    comprobar_registro("PeticionConsulta", &CAMPOS_PETICION_CONSULTA);
    comprobar_registro("ResultadoConsulta", &CAMPOS_RESULTADO_CONSULTA);
    comprobar_registro("PeticionAlertas", &CAMPOS_PETICION_ALERTAS);
    comprobar_registro("SucesoAlerta", &CAMPOS_SUCESO_ALERTA);
    comprobar_registro("Condiciones", &CAMPOS_CONDICIONES);
    comprobar_registro("RespuestaAlertas", &CAMPOS_RESPUESTA_ALERTAS);
}

#[test]
fn las_constantes_estan_atadas_a_los_structs() {
    // Desestructuracion exhaustiva, SIN `..`. Anadir un campo a cualquiera de
    // estos structs rompe la compilacion hasta que se anada tambien a su
    // constante. Es lo que impide que CAMPOS_* sea una tercera declaracion
    // independiente que pueda divergir en silencio.
    let EstadoAgente {
        version,
        perfil,
        respuesta_automatica,
    } = EstadoAgente {
        version: "0.1.0".to_owned(),
        perfil: PerfilSegmento::Ot,
        respuesta_automatica: false,
    };
    let _ = (version, perfil, respuesta_automatica);
    assert_eq!(CAMPOS_ESTADO_AGENTE.len(), 3);

    let NodoInventario {
        identificador,
        direccion_enlace,
        clase,
        postura,
    } = NodoInventario {
        identificador: "plc-3".to_owned(),
        direccion_enlace: "00:11:22:33:44:55".to_owned(),
        clase: ClaseDispositivo::Plc,
        postura: Postura::Conforme,
    };
    let _ = (identificador, direccion_enlace, clase, postura);
    assert_eq!(CAMPOS_NODO_INVENTARIO.len(), 4);

    let EstadoBoveda {
        usado_bytes,
        limite_bytes,
        eventos_pendientes,
    } = EstadoBoveda {
        usado_bytes: 0,
        limite_bytes: 1,
        eventos_pendientes: 0,
    };
    let _ = (usado_bytes, limite_bytes, eventos_pendientes);
    assert_eq!(CAMPOS_ESTADO_BOVEDA.len(), 3);

    let PeticionConsulta { sentencia } = PeticionConsulta {
        sentencia: "SELECT 1".to_owned(),
    };
    let _ = sentencia;
    assert_eq!(CAMPOS_PETICION_CONSULTA.len(), 1);

    let ResultadoConsulta { columnas, filas } = ResultadoConsulta {
        columnas: Vec::new(),
        filas: Vec::new(),
    };
    let _ = (columnas, filas);
    assert_eq!(CAMPOS_RESULTADO_CONSULTA.len(), 2);

    let PeticionAlertas { desde_asiento } = PeticionAlertas { desde_asiento: 0 };
    let _ = desde_asiento;
    assert_eq!(CAMPOS_PETICION_ALERTAS.len(), 1);

    let SucesoAlerta {
        asiento,
        clase,
        dispositivo,
        detalle,
    } = SucesoAlerta {
        asiento: 1,
        clase: ClaseAlerta::AmenazaIncontenible,
        dispositivo: "00:11:22:33:44:55".to_owned(),
        detalle: "prueba".to_owned(),
    };
    let _ = (asiento, clase, dispositivo, detalle);
    assert_eq!(CAMPOS_SUCESO_ALERTA.len(), 4);

    let Condiciones {
        inventario_suprimido,
        inventario_no_verifica,
        observacion_saturada,
        captura_con_perdida,
        accion_administrativa,
        salida_no_disponible,
        registro_saturado,
        evidencia_en_riesgo,
    } = Condiciones {
        inventario_suprimido: false,
        inventario_no_verifica: false,
        observacion_saturada: false,
        captura_con_perdida: false,
        accion_administrativa: false,
        salida_no_disponible: false,
        registro_saturado: false,
        evidencia_en_riesgo: false,
    };
    let _ = (
        inventario_suprimido,
        inventario_no_verifica,
        observacion_saturada,
        captura_con_perdida,
        accion_administrativa,
        salida_no_disponible,
        registro_saturado,
        evidencia_en_riesgo,
    );
    assert_eq!(CAMPOS_CONDICIONES.len(), 8);
}

#[test]
fn las_condiciones_distinguen_lo_degradado_de_lo_normal() {
    let normal = Condiciones {
        inventario_suprimido: false,
        inventario_no_verifica: false,
        observacion_saturada: false,
        captura_con_perdida: false,
        accion_administrativa: false,
        salida_no_disponible: false,
        registro_saturado: false,
        evidencia_en_riesgo: false,
    };
    assert!(!normal.hay_degradacion());
    assert!(!normal.hay_manipulacion());

    // Cada condicion basta por si sola: no hay ninguna que sea «menos grave».
    for degradada in [
        Condiciones {
            inventario_suprimido: true,
            ..normal
        },
        Condiciones {
            inventario_no_verifica: true,
            ..normal
        },
        Condiciones {
            observacion_saturada: true,
            ..normal
        },
        Condiciones {
            captura_con_perdida: true,
            ..normal
        },
        Condiciones {
            accion_administrativa: true,
            ..normal
        },
        Condiciones {
            registro_saturado: true,
            ..normal
        },
        Condiciones {
            evidencia_en_riesgo: true,
            ..normal
        },
    ] {
        assert!(degradada.hay_degradacion(), "{degradada:?}");
    }
}

#[test]
fn la_manipulacion_no_se_confunde_con_la_accion_administrativa() {
    // RPT-028 §2. Un formato obsoleto o una instalacion a medias exigen
    // reemitir o aprovisionar; una supresion o una firma rota exigen respuesta
    // a incidente. Que VIS-04 las presente igual es como se ensena a un
    // operador a ignorar la segunda.
    let normal = Condiciones {
        inventario_suprimido: false,
        inventario_no_verifica: false,
        observacion_saturada: false,
        captura_con_perdida: false,
        accion_administrativa: false,
        salida_no_disponible: false,
        registro_saturado: false,
        evidencia_en_riesgo: false,
    };

    for manipulada in [
        Condiciones {
            inventario_suprimido: true,
            ..normal
        },
        Condiciones {
            inventario_no_verifica: true,
            ..normal
        },
    ] {
        assert!(manipulada.hay_manipulacion(), "{manipulada:?}");
    }

    // Las otras tres degradan sin acusar a nadie. La saturacion y la perdida
    // tampoco: son limites del propio agente, no huellas de un tercero.
    for degradada in [
        Condiciones {
            accion_administrativa: true,
            ..normal
        },
        Condiciones {
            observacion_saturada: true,
            ..normal
        },
        Condiciones {
            captura_con_perdida: true,
            ..normal
        },
    ] {
        assert!(degradada.hay_degradacion(), "{degradada:?}");
        assert!(
            !degradada.hay_manipulacion(),
            "no hay indicio de que nadie tocara nada: {degradada:?}"
        );
    }
}

#[test]
fn un_suceso_de_alerta_es_reversible() {
    let alerta = SucesoAlerta {
        asiento: 42,
        clase: ClaseAlerta::AmenazaIncontenible,
        dispositivo: "00:1B:21:00:00:01".to_owned(),
        detalle: "soporte vital con actividad anomala".to_owned(),
    };

    let bruto = serde_json::to_string(&alerta).expect("serializa");
    assert!(
        bruto.contains("\"clase\":\"amenazaIncontenible\""),
        "la clase viaja en camelCase: {bruto}"
    );
    assert_eq!(
        serde_json::from_str::<SucesoAlerta>(&bruto).expect("deserializa"),
        alerta
    );
}

#[test]
fn todo_canal_declara_su_respuesta() {
    let contenido = manifiesto();
    for canal in Canal::TODOS {
        let marca = format!("canal = \"{}\"", canal.identificador());
        assert!(
            contenido.contains(&marca),
            "el canal '{}' no declara ningun mensaje en el manifiesto",
            canal.identificador()
        );
    }
}

// --- Rigor del deserializador ----------------------------------------------
//
// El rigor de Rust vive aqui. TypeScript no tiene equivalente natural, y por eso
// su rigor vive en la prueba de paridad (RPT-006, asimetria del fallo).

#[test]
fn un_campo_sobrante_se_rechaza() {
    let bruto = r#"{"version":"0.1.0","perfil":"ot","respuestaAutomatica":false,"extra":1}"#;
    assert!(
        serde_json::from_str::<EstadoAgente>(bruto).is_err(),
        "deny_unknown_fields debe rechazar campos no declarados"
    );
}

#[test]
fn un_campo_ausente_se_rechaza() {
    let bruto = r#"{"version":"0.1.0","perfil":"ot"}"#;
    assert!(serde_json::from_str::<EstadoAgente>(bruto).is_err());
}

#[test]
fn un_valor_fuera_del_enumerado_se_rechaza() {
    let bruto = r#"{"version":"0.1.0","perfil":"militar","respuestaAutomatica":false}"#;
    assert!(serde_json::from_str::<EstadoAgente>(bruto).is_err());
}

#[test]
fn el_nombre_en_el_cable_es_camel_case() {
    // snake_case en Rust, camelCase en el cable. Si el mapeo se rompiera, el
    // extremo TypeScript recibiria `undefined` en silencio.
    let bruto = r#"{"version":"0.1.0","perfil":"ot","respuestaAutomatica":true}"#;
    let estado: EstadoAgente =
        serde_json::from_str(bruto).expect("el mensaje canonico debe deserializarse");
    assert!(estado.respuesta_automatica);

    let vuelta = serde_json::to_string(&estado).expect("debe serializarse");
    assert!(vuelta.contains("respuestaAutomatica"));
    assert!(!vuelta.contains("respuesta_automatica"));
}

#[test]
fn el_mensaje_completo_es_reversible() {
    let original = ResultadoConsulta {
        columnas: vec!["numero".to_owned(), "clase".to_owned()],
        filas: vec![vec!["1".to_owned(), "deteccion-anomalia".to_owned()]],
    };
    let texto = serde_json::to_string(&original).expect("debe serializarse");
    let recuperado: ResultadoConsulta = serde_json::from_str(&texto).expect("debe deserializarse");
    assert_eq!(original, recuperado);
}

#[test]
fn un_marco_con_cola_sobrante_devuelve_solo_lo_declarado() {
    // El transporte puede entregar varios marcos en una lectura. Devolver la cola
    // mezclaria dos mensajes.
    let mut marco = enmarcar(b"uno").expect("debe enmarcarse");
    marco.extend_from_slice(b"basura posterior");

    assert_eq!(desenmarcar(&marco), Ok(&b"uno"[..]));
}
