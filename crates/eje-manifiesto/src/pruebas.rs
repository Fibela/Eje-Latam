//! Pruebas del emisor.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use guardian_cc::ClaseExcluida;
use guardian_cc::clave::{analizar as analizar_clave, serializar as serializar_clave};
use guardian_cc::formato::{ErrorFormato, analizar};
use guardian_cc::vlan::NaturalezaSegmento;
use motor_pqc::secreto::Secreto;

use super::*;

/// Instante fijo, para que las vigencias no dependan del reloj.
const AHORA: u64 = 1_785_888_000;

/// Direccion de la bomba de infusion del banco.
const CRITICO: [u8; 6] = [0x00, 0x1B, 0x21, 0x00, 0x00, 0x01];

fn semilla(byte: u8) -> SemillaFirma {
    Secreto::nuevo([byte; 32])
}

fn marcados() -> Vec<MarcadoBruto> {
    vec![
        MarcadoBruto {
            mac: [0x00, 0x1B, 0x21, 0x00, 0x00, 0x02],
            clase: None,
            emitido_en: AHORA,
            vigencia_dias: 365,
        },
        MarcadoBruto {
            mac: CRITICO,
            clase: Some(ClaseExcluida::SoporteVital),
            emitido_en: AHORA,
            vigencia_dias: 365,
        },
    ]
}

fn segmentos() -> Vec<DeclaracionVlan> {
    vec![DeclaracionVlan {
        vlan: 10,
        naturaleza: NaturalezaSegmento::SinDispositivosCriticos,
        emitido_en: AHORA,
        vigencia_dias: 365,
    }]
}

#[test]
fn la_misma_semilla_produce_siempre_la_misma_clave() {
    // Es lo que hace innecesario serializar la clave privada. Si esto fallara,
    // guardar la semilla no serviria de nada y habria que exponer material
    // privado, que es justo lo que RPT-021 §3 prohibe.
    let uno = Emisor::desde_semilla(semilla(7));
    let otro = Emisor::desde_semilla(semilla(7));

    assert_eq!(uno.verificacion().a_bytes(), otro.verificacion().a_bytes());
}

#[test]
fn semillas_distintas_producen_claves_distintas() {
    let uno = Emisor::desde_semilla(semilla(7));
    let otro = Emisor::desde_semilla(semilla(8));

    assert_ne!(uno.verificacion().a_bytes(), otro.verificacion().a_bytes());
}

#[test]
fn un_manifiesto_recien_emitido_lo_verifica_el_agente() {
    // El recorrido que cierra PA-48 con PA-49: lo que firma el emisor lo lee el
    // MISMO codigo que corre en el sensor.
    let emisor = Emisor::desde_semilla(semilla(1));
    let bytes = emisor.emitir(marcados(), segmentos(), 1).expect("emite");

    let local = InventarioLocal::cargar(
        &bytes,
        &emisor.como_clave_de_cliente(),
        Centinela::Establecido(1),
        &RegistroRevocaciones::default(),
    )
    .expect("el agente debe poder cargarlo");

    assert_eq!(local.entradas(), 2);
    assert_eq!(local.segmentos(), 1);
    assert_eq!(local.secuencia(), 1);

    use guardian_cc::proveedores::ProveedorInventario;
    assert_eq!(
        local
            .marcado(&CRITICO)
            .expect("verifica")
            .expect("figura")
            .clase(),
        Some(ClaseExcluida::SoporteVital)
    );
}

#[test]
fn la_clave_aprovisionada_es_la_que_firmo() {
    // El emisor produce el fichero `.pub` que consume el agente. Si las dos
    // claves no coincidieran, el aprovisionamiento seria un ritual sin efecto.
    let emisor = Emisor::desde_semilla(semilla(2));
    let fichero = serializar_clave(emisor.verificacion(), DominioClave::Cliente);
    let leida = analizar_clave(&fichero, DominioClave::Cliente).expect("analiza");

    let bytes = emisor.emitir(marcados(), segmentos(), 3).expect("emite");

    assert!(
        InventarioLocal::cargar(
            &bytes,
            &leida,
            Centinela::Establecido(3),
            &RegistroRevocaciones::default()
        )
        .is_ok(),
        "la clave del fichero .pub verifica lo que el emisor firmo"
    );
}

#[test]
fn otro_emisor_no_puede_continuar_la_serie() {
    // La secuencia sale del manifiesto anterior VERIFICADO. Un emisor con otra
    // semilla no puede leerla, y eso es lo correcto: no es su serie.
    let emisor = Emisor::desde_semilla(semilla(3));
    let bytes = emisor.emitir(marcados(), segmentos(), 5).expect("emite");

    let intruso = Emisor::desde_semilla(semilla(4));

    assert!(matches!(
        intruso.secuencia_siguiente(Some(&bytes)),
        Err(ErrorEmision::AnteriorNoVerifica { .. })
    ));
}

#[test]
fn la_secuencia_avanza_desde_el_manifiesto_anterior() {
    let emisor = Emisor::desde_semilla(semilla(5));

    assert_eq!(
        emisor.secuencia_siguiente(None).expect("primera"),
        1,
        "la primera emision es la 1, no la 0: sin manifiesto y manifiesto \
         inicial no deben compartir numero"
    );

    let bytes = emisor.emitir(marcados(), segmentos(), 41).expect("emite");

    assert_eq!(
        emisor.secuencia_siguiente(Some(&bytes)).expect("siguiente"),
        42
    );
}

#[test]
fn un_anterior_corrupto_no_reinicia_la_serie() {
    // Si el fallo devolviera 1, bastaria corromper el fichero para que el
    // siguiente manifiesto naciera revertido y el agente lo rechazara. Peor: el
    // administrador creeria haber emitido y no lo sabria hasta el incidente.
    let emisor = Emisor::desde_semilla(semilla(6));
    let mut bytes = emisor.emitir(marcados(), segmentos(), 9).expect("emite");
    let ultimo = bytes.len() - 1;
    bytes[ultimo] ^= 0xFF;

    assert!(matches!(
        emisor.secuencia_siguiente(Some(&bytes)),
        Err(ErrorEmision::AnteriorNoVerifica { .. })
    ));
}

#[test]
fn el_techo_de_secuencia_se_rechaza_al_emitir() {
    let emisor = Emisor::desde_semilla(semilla(9));

    for secuencia in [TECHO_SECUENCIA, TECHO_SECUENCIA + 1, u64::MAX] {
        assert!(
            matches!(
                emisor.emitir(marcados(), segmentos(), secuencia),
                Err(ErrorEmision::TechoAlcanzado { .. })
            ),
            "la secuencia {secuencia} no debe emitirse"
        );
    }

    assert!(
        emisor
            .emitir(marcados(), segmentos(), TECHO_SECUENCIA - 1)
            .is_ok(),
        "justo por debajo si"
    );
}

#[test]
fn el_agente_tambien_rechaza_una_secuencia_por_encima_del_techo() {
    // La comprobacion del emisor es una cortesia: quien tenga la clave no usa
    // nuestro emisor. La defensa real es la del analizador del agente, y esta
    // prueba existe porque un techo que solo viviera aqui no protegeria de nada.
    let emisor = Emisor::desde_semilla(semilla(10));
    let mut bytes = emisor
        .emitir(marcados(), segmentos(), TECHO_SECUENCIA - 1)
        .expect("emite");

    // Se sube la secuencia en el fichero ya firmado. La firma deja de cuadrar,
    // pero el techo se comprueba ANTES de la criptografia a proposito: el
    // rechazo debe nombrar la saturacion y no un fallo de firma.
    bytes[10..18].copy_from_slice(&u64::MAX.to_be_bytes());

    assert_eq!(
        analizar(&bytes).err(),
        Some(ErrorFormato::SecuenciaFueraDeRango {
            declarada: u64::MAX
        })
    );
}

#[test]
fn un_inventario_vacio_no_se_emite() {
    let emisor = Emisor::desde_semilla(semilla(11));

    assert!(matches!(
        emisor.emitir(Vec::new(), segmentos(), 1),
        Err(ErrorEmision::InventarioInvalido { .. })
    ));
}

#[test]
fn un_dispositivo_declarado_dos_veces_no_se_emite() {
    // Se rechaza al emitir y no solo al leer. Un manifiesto que el agente va a
    // rechazar no deberia llegar a existir: el administrador se enteraria en el
    // sensor, tarde y sin contexto.
    let emisor = Emisor::desde_semilla(semilla(12));
    let mut repetidos = marcados();
    let primero = repetidos[0];
    repetidos.push(primero);

    assert!(matches!(
        emisor.emitir(repetidos, segmentos(), 1),
        Err(ErrorEmision::InventarioInvalido { .. })
    ));
}

#[test]
fn una_vlan_fuera_de_rango_no_se_emite() {
    let emisor = Emisor::desde_semilla(semilla(13));
    let fuera = vec![DeclaracionVlan {
        vlan: 0,
        naturaleza: NaturalezaSegmento::SinDispositivosCriticos,
        emitido_en: AHORA,
        vigencia_dias: 365,
    }];

    assert!(matches!(
        emisor.emitir(marcados(), fuera, 1),
        Err(ErrorEmision::SegmentosInvalidos { .. })
    ));
}

#[test]
fn el_orden_de_entrada_no_cambia_los_bytes_emitidos() {
    // El orden canonico lo impone el constructor compartido con el agente. Dos
    // herramientas administrativas que enumeren en orden distinto deben producir
    // el mismo fichero.
    let emisor = Emisor::desde_semilla(semilla(14));

    let uno = emisor.emitir(marcados(), segmentos(), 2).expect("emite");

    let mut invertidos = marcados();
    invertidos.reverse();
    let otro = emisor.emitir(invertidos, segmentos(), 2).expect("emite");

    assert_eq!(uno, otro);
}

// ---------------------------------------------------------------------------
// Entrada del administrador — RPT-026
// ---------------------------------------------------------------------------

use crate::entrada::{Entrada, ErrorEntrada, analizar_mac};

const PARQUE: &str = r#"
[[marcado]]
mac = "00:1b:21:00:00:01"
clase = "soporte-vital"

[[marcado]]
mac = "00-1B-21-00-00-02"

[[segmento]]
vlan = 10
naturaleza = "SinDispositivosCriticos"
vigencia_dias = 90
"#;

#[test]
fn la_entrada_del_administrador_se_traduce_al_vocabulario() {
    let entrada = Entrada::analizar(PARQUE).expect("analiza");
    let marcados = entrada.marcados(AHORA).expect("marcados");
    let segmentos = entrada.segmentos(AHORA).expect("segmentos");

    assert_eq!(marcados.len(), 2);
    assert_eq!(marcados[0].mac, CRITICO);
    assert_eq!(marcados[0].clase, Some(ClaseExcluida::SoporteVital));
    assert_eq!(
        marcados[0].vigencia_dias, 365,
        "la vigencia por defecto se aplica sin declararla"
    );

    assert_eq!(
        marcados[1].clase, None,
        "la ausencia de clase significa «declarado no critico»"
    );
    assert_eq!(marcados[1].mac, [0x00, 0x1B, 0x21, 0x00, 0x00, 0x02]);

    assert_eq!(segmentos.len(), 1);
    assert_eq!(segmentos[0].vlan, 10);
    assert_eq!(segmentos[0].vigencia_dias, 90);
}

#[test]
fn un_campo_mal_escrito_no_se_ignora_en_silencio() {
    // Es la razon entera de traer un analizador de verdad. Sin
    // `deny_unknown_fields`, esto produce un marcado NO CRITICO de un equipo de
    // soporte vital, y el administrador no tiene forma de notarlo.
    let con_errata = r#"
[[marcado]]
mac = "00:1b:21:00:00:01"
clse = "soporte-vital"
"#;

    assert!(matches!(
        Entrada::analizar(con_errata),
        Err(ErrorEntrada::TomlInvalido { .. })
    ));
}

#[test]
fn una_clase_desconocida_no_degrada_a_no_critico() {
    let inventada = r#"
[[marcado]]
mac = "00:1b:21:00:00:01"
clase = "soporte-vitall"
"#;

    let entrada = Entrada::analizar(inventada).expect("el TOML es valido");

    assert!(matches!(
        entrada.marcados(AHORA),
        Err(ErrorEntrada::ClaseDesconocida { .. })
    ));
}

#[test]
fn una_mac_sin_separadores_se_rechaza() {
    // Doce caracteres seguidos se transponen sin notarlo, y una MAC transpuesta
    // marca el equipo equivocado.
    for texto in [
        "001b21000001",
        "00:1b:21:00:00",
        "00:1b:21:00:00:01:02",
        "00:1b:21:00:00:zz",
        "0:1b:21:00:00:01",
    ] {
        assert!(
            matches!(analizar_mac(texto), Err(ErrorEntrada::MacInvalida { .. })),
            "'{texto}' no debe analizarse"
        );
    }

    assert_eq!(analizar_mac("00:1B:21:00:00:01").expect("valida"), CRITICO);
    assert_eq!(analizar_mac("00-1b-21-00-00-01").expect("valida"), CRITICO);
}

// ---------------------------------------------------------------------------
// Semilla en reposo — RPT-026
// ---------------------------------------------------------------------------

use crate::reposo_semilla::{ErrorSemilla, LONGITUD_FICHERO, LONGITUD_SAL, abrir, sellar};
use motor_pqc::reposo::LONGITUD_NONCE;

fn sellada(frase: &[u8]) -> Vec<u8> {
    sellar(
        &semilla(42),
        frase,
        [3u8; LONGITUD_SAL],
        [4u8; LONGITUD_NONCE],
    )
    .expect("sella")
}

#[test]
fn la_semilla_sellada_se_abre_con_su_frase() {
    let bytes = sellada(b"correcta");
    assert_eq!(bytes.len(), LONGITUD_FICHERO);

    let abierta = abrir(&bytes, b"correcta").expect("abre");
    assert_eq!(abierta.exponer(), semilla(42).exponer());
}

#[test]
fn una_frase_distinta_no_abre_la_semilla() {
    assert!(matches!(
        abrir(&sellada(b"correcta"), b"incorrecta"),
        Err(ErrorSemilla::NoAbre)
    ));
}

#[test]
fn una_frase_vacia_no_sella_nada() {
    // Cifrar con frase vacia es la opcion B que RPT-023 §4 rechazo, disfrazada
    // de cifrado.
    assert!(matches!(
        sellar(
            &semilla(42),
            b"",
            [3u8; LONGITUD_SAL],
            [4u8; LONGITUD_NONCE]
        ),
        Err(ErrorSemilla::FraseVacia)
    ));
}

#[test]
fn alterar_la_sal_no_da_un_fallo_distinguible() {
    // La cabecera va autenticada. Si no lo estuviera, cambiar la sal y observar
    // como falla seria un principio de oraculo.
    let mut bytes = sellada(b"correcta");
    bytes[12] ^= 0xFF;

    assert!(matches!(
        abrir(&bytes, b"correcta"),
        Err(ErrorSemilla::NoAbre)
    ));
}

#[test]
fn una_cola_sobrante_en_la_semilla_se_rechaza() {
    let mut bytes = sellada(b"correcta");
    bytes.push(0);

    assert!(matches!(
        abrir(&bytes, b"correcta"),
        Err(ErrorSemilla::LongitudIncorrecta { .. })
    ));
}

#[test]
fn el_ciclo_completo_del_emisor_produce_algo_que_el_agente_verifica() {
    // De la frase de paso al veredicto. Es el recorrido que un administrador
    // hace de verdad, con el TOML y la semilla cifrada por medio.
    let bytes_semilla = sellada(b"la frase del cliente");
    let abierta = abrir(&bytes_semilla, b"la frase del cliente").expect("abre");
    let emisor = Emisor::desde_semilla(abierta);

    let entrada = Entrada::analizar(PARQUE).expect("analiza");
    let manifiesto = emisor
        .emitir(
            entrada.marcados(AHORA).expect("marcados"),
            entrada.segmentos(AHORA).expect("segmentos"),
            1,
        )
        .expect("emite");

    let clave = serializar_clave(emisor.verificacion(), DominioClave::Cliente);
    let aprovisionada = analizar_clave(&clave, DominioClave::Cliente).expect("clave");

    let local = InventarioLocal::cargar(
        &manifiesto,
        &aprovisionada,
        Centinela::Establecido(1),
        &RegistroRevocaciones::default(),
    )
    .expect("el agente lo verifica");

    use guardian_cc::proveedores::ProveedorInventario;
    assert_eq!(
        local
            .marcado(&CRITICO)
            .expect("verifica")
            .expect("figura")
            .clase(),
        Some(ClaseExcluida::SoporteVital),
        "el equipo que el administrador escribio en el TOML llega protegido"
    );
    assert_eq!(local.segmentos(), 1);
}

// ---------------------------------------------------------------------------
// Reparto de la clave de recuperacion — RPT-027
// ---------------------------------------------------------------------------

use crate::fragmento::{
    ErrorFragmento, analizar as analizar_fragmento, huella_de, reunir_verificando,
    serializar as serializar_fragmento,
};
use motor_pqc::reparto::{CUSTODIOS, ErrorReparto, UMBRAL, repartir, reunir};

fn reparto_de_prueba() -> (SemillaFirma, Vec<Vec<u8>>) {
    let secreto = semilla(77);
    let huella = huella_de(semilla(77));
    let partes = repartir(&secreto, &[0x5Au8; 32]);

    let ficheros = partes
        .iter()
        .map(|parte| serializar_fragmento(parte, &huella))
        .collect();

    (secreto, ficheros)
}

#[test]
fn dos_fragmentos_cualesquiera_reconstruyen_el_secreto() {
    let (secreto, ficheros) = reparto_de_prueba();

    // Las tres parejas posibles. Si solo se probara una, un error de
    // interpolacion que dependiera de los indices pasaria desapercibido.
    for (uno, otro) in [(0, 1), (0, 2), (1, 2)] {
        let reunida = reunir_verificando(
            &analizar_fragmento(&ficheros[uno]).expect("analiza"),
            &analizar_fragmento(&ficheros[otro]).expect("analiza"),
        )
        .expect("reune");

        assert_eq!(
            reunida.exponer(),
            secreto.exponer(),
            "la pareja ({uno}, {otro}) debe reconstruir"
        );
    }
}

#[test]
fn un_solo_custodio_no_reconstruye_aunque_repita_su_fragmento() {
    // Si `reunir` aceptara dos puntos con la misma abscisa, el umbral de dos
    // dejaria de ser dos: bastaria presentar el propio fragmento dos veces.
    let (_, ficheros) = reparto_de_prueba();
    let solo = analizar_fragmento(&ficheros[0]).expect("analiza");

    assert!(matches!(
        reunir(&solo.fragmento, &solo.fragmento),
        Err(ErrorReparto::CustodioRepetido { indice: 1 })
    ));
}

#[test]
fn un_fragmento_no_revela_el_secreto() {
    // Shamir es incondicionalmente seguro con un fragmento de menos, asi que lo
    // unico comprobable aqui es lo minimo: que el fragmento no ES el secreto.
    // Una implementacion que devolviera el secreto como fragmento pasaria todas
    // las demas pruebas de este bloque.
    let (secreto, ficheros) = reparto_de_prueba();

    for fichero in &ficheros {
        let leido = analizar_fragmento(fichero).expect("analiza");
        assert_ne!(&leido.fragmento.bytes, secreto.exponer());
    }
}

#[test]
fn el_indice_cero_no_es_un_fragmento() {
    // `f(0)` ES el secreto. Un fragmento con indice 0 seria el secreto entero.
    let (_, ficheros) = reparto_de_prueba();
    let mut falso = ficheros[0].clone();
    falso[10] = 0;

    let leido = analizar_fragmento(&falso).expect("el formato sigue bien");
    let otro = analizar_fragmento(&ficheros[1]).expect("analiza");

    assert!(matches!(
        reunir(&leido.fragmento, &otro.fragmento),
        Err(ErrorReparto::IndiceFueraDeRango { indice: 0 })
    ));
}

#[test]
fn un_fragmento_alterado_se_detecta_al_reunir() {
    // Lo que Shamir NO da. Sin la huella, esto produce otra clave en silencio y
    // nadie se entera hasta que el agente rechaza el certificado, en mitad del
    // incidente que motivo la reconstruccion.
    let (_, ficheros) = reparto_de_prueba();

    let mut alterado = ficheros[0].clone();
    let ultimo = alterado.len() - 1;
    alterado[ultimo] ^= 0xFF;

    assert!(matches!(
        reunir_verificando(
            &analizar_fragmento(&alterado).expect("el formato sigue bien"),
            &analizar_fragmento(&ficheros[1]).expect("analiza"),
        ),
        Err(ErrorFragmento::ReconstruccionNoCuadra)
    ));
}

#[test]
fn no_se_pueden_mezclar_fragmentos_de_dos_repartos() {
    let (_, unos) = reparto_de_prueba();

    let otra_huella = huella_de(semilla(78));
    let otras = repartir(&semilla(78), &[0x11u8; 32]);
    let ajeno = serializar_fragmento(&otras[1], &otra_huella);

    assert!(matches!(
        reunir_verificando(
            &analizar_fragmento(&unos[0]).expect("analiza"),
            &analizar_fragmento(&ajeno).expect("analiza"),
        ),
        Err(ErrorFragmento::RepartosDistintos)
    ));
}

#[test]
fn el_fragmento_declara_su_esquema_para_quien_lo_encuentre_dentro_de_anos() {
    let (_, ficheros) = reparto_de_prueba();

    assert_eq!(ficheros.len(), usize::from(CUSTODIOS));
    assert_eq!(ficheros[0][11], UMBRAL);
    assert_eq!(ficheros[0][12], CUSTODIOS);

    // Un esquema distinto se rechaza en lugar de intentarse.
    let mut otro_esquema = ficheros[0].clone();
    otro_esquema[11] = 3;

    assert!(matches!(
        analizar_fragmento(&otro_esquema),
        Err(ErrorFragmento::EsquemaDistinto {
            umbral: 3,
            custodios: 3
        })
    ));
}

#[test]
fn la_clave_reunida_firma_certificados_que_el_agente_verifica() {
    // El recorrido entero de PA-54: tres fragmentos, dos custodios, un
    // certificado de revocacion que el agente acepta.
    use guardian_cc::revocacion::{
        CertificadoRevocacion, CertificadoVerificado, mensaje_de_certificado,
    };
    use motor_pqc::firma_hibrida::firmar;
    use motor_pqc::semilla::derivar_par;

    let (_, ficheros) = reparto_de_prueba();
    let reunida = reunir_verificando(
        &analizar_fragmento(&ficheros[0]).expect("analiza"),
        &analizar_fragmento(&ficheros[2]).expect("analiza"),
    )
    .expect("reune");

    let (firmante, verificacion) = derivar_par(reunida);
    let recuperacion = guardian_cc::inventario::ClaveInventario::nueva(
        verificacion,
        DominioClave::ClienteRecuperacion,
    );

    let comprometida = Emisor::desde_semilla(semilla(90));
    let sucesora = Emisor::desde_semilla(semilla(91));

    let certificado = CertificadoRevocacion {
        revocada: comprometida.como_clave_de_cliente().identificador(),
        hasta_secuencia: 12,
        sucesora: sucesora.como_clave_de_cliente().identificador(),
        emitido_en: AHORA,
    };

    let firma = firmar(&firmante, &mensaje_de_certificado(&certificado));
    let verificado =
        CertificadoVerificado::verificar(certificado, &firma, &recuperacion).expect("verifica");

    assert_eq!(verificado.hasta_secuencia(), 12);
}

#[test]
fn el_emisor_no_entra_en_el_binario_del_agente() {
    // La regla del §«Este crate no se despliega», comprobada donde se puede
    // comprobar desde aqui: `eje-agente` no debe declarar este crate.
    //
    // No sustituye a PA-12. Que el fichero de dependencias este limpio no
    // impide que el empaquetador copie el binario del emisor al instalador; eso
    // solo lo cierra una comprobacion sobre el artefacto, y no existe todavia.
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("raiz del workspace");

    let manifiesto = std::fs::read_to_string(raiz.join("crates/eje-agente/Cargo.toml"))
        .expect("eje-agente debe tener manifiesto");

    assert!(
        !manifiesto.contains("eje-manifiesto"),
        "el agente no puede depender del emisor: un sensor comprometido no debe \
         poder firmar inventarios"
    );
}
