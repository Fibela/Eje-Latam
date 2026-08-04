//! Pruebas del motor poscuántico.
//!
//! Estas pruebas cubren **nuestra construcción híbrida**, no las primitivas.
//! La conformidad de ML-KEM y ML-DSA se establece con vectores ACVP y Wycheproof
//! y con el contraste diferencial, que son artefactos externos (RPT-005 §7.3 y
//! PA-17).

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use rand_core::{Infallible, TryCryptoRng, TryRng};

use crate::combinador::{
    ETIQUETA_FIRMA, ETIQUETA_KEM, derivar_secreto_hibrido, mensaje_canonico_de_firma,
};
use crate::conformidad::Conformidad;
use crate::reposo::{ClaveSimetrica, LONGITUD_NONCE, cifrar, descifrar};
use crate::secreto::Secreto;
use crate::{firma_hibrida, kem_hibrido};

/// Generador determinista para pruebas reproducibles.
///
/// No es criptográficamente seguro y **nunca sale de este módulo**: vive dentro
/// de `#[cfg(test)]` precisamente para que el guardián de inconclusos no tenga
/// que distinguir entre un generador de pruebas y uno de producción.
struct GeneradorDeterminista {
    estado: u64,
}

impl GeneradorDeterminista {
    const fn nuevo(semilla: u64) -> Self {
        Self { estado: semilla }
    }

    /// xorshift64*, suficiente para reproducibilidad en pruebas.
    fn siguiente(&mut self) -> u64 {
        self.estado ^= self.estado >> 12;
        self.estado ^= self.estado << 25;
        self.estado ^= self.estado >> 27;
        self.estado.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

impl TryRng for GeneradorDeterminista {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok((self.siguiente() >> 32) as u32)
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.siguiente())
    }

    fn try_fill_bytes(&mut self, destino: &mut [u8]) -> Result<(), Self::Error> {
        for trozo in destino.chunks_mut(8) {
            let valor = self.siguiente().to_le_bytes();
            let longitud = trozo.len();
            trozo.copy_from_slice(&valor[..longitud]);
        }
        Ok(())
    }
}

// `CryptoRng` se obtiene por implementacion generica a partir de `TryCryptoRng`
// con `Error = Infallible`; implementarlo aqui seria un conflicto.
impl TryCryptoRng for GeneradorDeterminista {}

// ---------------------------------------------------------------------------
// Combinador híbrido
// ---------------------------------------------------------------------------

#[test]
fn el_combinador_es_determinista() {
    let uno = derivar_secreto_hibrido(b"pq", b"clasico", b"cifrado", b"publica").unwrap();
    let otro = derivar_secreto_hibrido(b"pq", b"clasico", b"cifrado", b"publica").unwrap();
    assert_eq!(uno, otro);
}

#[test]
fn cambiar_cualquier_entrada_cambia_el_secreto() {
    let base = derivar_secreto_hibrido(b"pq", b"clasico", b"cifrado", b"publica").unwrap();

    let variantes = [
        derivar_secreto_hibrido(b"PQ", b"clasico", b"cifrado", b"publica").unwrap(),
        derivar_secreto_hibrido(b"pq", b"CLASICO", b"cifrado", b"publica").unwrap(),
        derivar_secreto_hibrido(b"pq", b"clasico", b"CIFRADO", b"publica").unwrap(),
        derivar_secreto_hibrido(b"pq", b"clasico", b"cifrado", b"PUBLICA").unwrap(),
    ];

    for (indice, variante) in variantes.iter().enumerate() {
        assert_ne!(base, *variante, "la variante {indice} no alteró el secreto");
    }
}

#[test]
fn los_prefijos_de_longitud_impiden_la_ambiguedad() {
    // Sin prefijos, ("ab","c") y ("a","bc") producirían el mismo material de
    // entrada y dos sesiones distintas derivarían la misma clave.
    let uno = derivar_secreto_hibrido(b"ab", b"c", b"ct", b"pk").unwrap();
    let otro = derivar_secreto_hibrido(b"a", b"bc", b"ct", b"pk").unwrap();
    assert_ne!(uno, otro);

    let tercero = derivar_secreto_hibrido(b"pq", b"cl", b"ct", b"pk").unwrap();
    let cuarto = derivar_secreto_hibrido(b"pq", b"cl", b"c", b"tpk").unwrap();
    assert_ne!(tercero, cuarto);
}

#[test]
fn la_etiqueta_de_dominio_nombra_los_algoritmos_y_la_version() {
    let kem = core::str::from_utf8(ETIQUETA_KEM).unwrap();
    assert!(kem.contains("x25519"));
    assert!(kem.contains("ml-kem-768"));
    assert!(kem.ends_with("/v1"));

    let firma = core::str::from_utf8(ETIQUETA_FIRMA).unwrap();
    assert!(firma.contains("ed25519"));
    assert!(firma.contains("ml-dsa-65"));
}

#[test]
fn el_mensaje_canonico_vincula_el_dominio() {
    let canonico = mensaje_canonico_de_firma(b"hola");
    assert!(
        canonico
            .windows(ETIQUETA_FIRMA.len())
            .any(|ventana| ventana == ETIQUETA_FIRMA),
        "el mensaje canónico debe incorporar la etiqueta de dominio"
    );
    assert_ne!(canonico, b"hola".to_vec());
}

// ---------------------------------------------------------------------------
// Intercambio de claves híbrido
// ---------------------------------------------------------------------------

#[test]
fn el_intercambio_hibrido_converge() {
    let mut generador = GeneradorDeterminista::nuevo(0xE1E_1A7A);
    let (privada, publica) = kem_hibrido::generar_par(&mut generador);

    let (encapsulado, secreto_emisor) = kem_hibrido::encapsular(&publica, &mut generador).unwrap();
    let secreto_receptor = kem_hibrido::desencapsular(&privada, &encapsulado).unwrap();

    assert_eq!(secreto_emisor, secreto_receptor);
}

#[test]
fn dos_intercambios_producen_secretos_distintos() {
    let mut generador = GeneradorDeterminista::nuevo(7);
    let (_, publica) = kem_hibrido::generar_par(&mut generador);

    let (_, primero) = kem_hibrido::encapsular(&publica, &mut generador).unwrap();
    let (_, segundo) = kem_hibrido::encapsular(&publica, &mut generador).unwrap();

    assert_ne!(
        primero, segundo,
        "cada sesión debe derivar su propio secreto"
    );
}

#[test]
fn otra_clave_privada_no_recupera_el_secreto() {
    let mut generador = GeneradorDeterminista::nuevo(11);
    let (_, publica) = kem_hibrido::generar_par(&mut generador);
    let (ajena, _) = kem_hibrido::generar_par(&mut generador);

    let (encapsulado, secreto_emisor) = kem_hibrido::encapsular(&publica, &mut generador).unwrap();
    let intento = kem_hibrido::desencapsular(&ajena, &encapsulado).unwrap();

    assert_ne!(secreto_emisor, intento);
}

#[test]
fn alterar_la_publica_clasica_del_encapsulado_rompe_la_convergencia() {
    // La componente clásica está vinculada a la derivación: manipularla en
    // tránsito debe impedir que ambos extremos coincidan.
    let mut generador = GeneradorDeterminista::nuevo(23);
    let (privada, publica) = kem_hibrido::generar_par(&mut generador);
    let (mut encapsulado, secreto_emisor) =
        kem_hibrido::encapsular(&publica, &mut generador).unwrap();

    let (_, otra) = kem_hibrido::generar_par(&mut generador);
    encapsulado.publica_clasica = otra.clasica;

    let intento = kem_hibrido::desencapsular(&privada, &encapsulado).unwrap();
    assert_ne!(secreto_emisor, intento);
}

// ---------------------------------------------------------------------------
// Firma híbrida
// ---------------------------------------------------------------------------

#[test]
fn una_firma_hibrida_valida_verifica() {
    let mut generador = GeneradorDeterminista::nuevo(101);
    let (firmante, verificador) = firma_hibrida::generar_par(&mut generador);

    let firma = firma_hibrida::firmar(&firmante, b"orden de contencion");
    assert!(firma_hibrida::verificar(&verificador, b"orden de contencion", &firma).is_ok());
}

#[test]
fn un_mensaje_alterado_no_verifica() {
    let mut generador = GeneradorDeterminista::nuevo(102);
    let (firmante, verificador) = firma_hibrida::generar_par(&mut generador);

    let firma = firma_hibrida::firmar(&firmante, b"aislar plc-3");
    assert!(firma_hibrida::verificar(&verificador, b"aislar plc-9", &firma).is_err());
}

#[test]
fn romper_solo_la_componente_clasica_invalida_la_firma() {
    // La verificación es conjunción, no disyunción. Si bastara con que una
    // componente verificara, la seguridad del conjunto sería la de la más débil.
    let mut generador = GeneradorDeterminista::nuevo(103);
    let (firmante, verificador) = firma_hibrida::generar_par(&mut generador);
    let (otro_firmante, _) = firma_hibrida::generar_par(&mut generador);

    let mut firma = firma_hibrida::firmar(&firmante, b"mensaje");
    let firma_ajena = firma_hibrida::firmar(&otro_firmante, b"mensaje");
    firma.clasica = firma_ajena.clasica;

    assert!(
        firma_hibrida::verificar(&verificador, b"mensaje", &firma).is_err(),
        "una componente clásica ajena debe invalidar toda la firma"
    );
}

#[test]
fn romper_solo_la_componente_poscuantica_invalida_la_firma() {
    let mut generador = GeneradorDeterminista::nuevo(104);
    let (firmante, verificador) = firma_hibrida::generar_par(&mut generador);
    let (otro_firmante, _) = firma_hibrida::generar_par(&mut generador);

    let mut firma = firma_hibrida::firmar(&firmante, b"mensaje");
    let firma_ajena = firma_hibrida::firmar(&otro_firmante, b"mensaje");
    firma.poscuantica = firma_ajena.poscuantica;

    assert!(firma_hibrida::verificar(&verificador, b"mensaje", &firma).is_err());
}

#[test]
fn la_firma_serializada_contiene_ambas_componentes() {
    let mut generador = GeneradorDeterminista::nuevo(105);
    let (firmante, _) = firma_hibrida::generar_par(&mut generador);
    let firma = firma_hibrida::firmar(&firmante, b"mensaje");

    let bytes = firma.a_bytes();
    assert!(
        bytes.len() > firma_hibrida::LONGITUD_FIRMA_CLASICA,
        "la serialización debe incluir la componente poscuántica"
    );
}

// ---------------------------------------------------------------------------
// Cifrado en reposo
// ---------------------------------------------------------------------------

fn clave_de_prueba() -> ClaveSimetrica {
    Secreto::nuevo([0x2Au8; 32])
}

#[test]
fn el_cifrado_en_reposo_es_reversible() {
    let clave = clave_de_prueba();
    let nonce = [1u8; LONGITUD_NONCE];

    let cifrado = cifrar(&clave, &nonce, b"evidencia sensible", b"alm-01").unwrap();
    let recuperado = descifrar(&clave, &nonce, &cifrado, b"alm-01").unwrap();

    assert_eq!(recuperado, b"evidencia sensible");
}

#[test]
fn alterar_el_texto_cifrado_se_detecta() {
    let clave = clave_de_prueba();
    let nonce = [2u8; LONGITUD_NONCE];
    let mut cifrado = cifrar(&clave, &nonce, b"evidencia", b"alm-01").unwrap();

    cifrado[0] ^= 0x01;
    assert!(descifrar(&clave, &nonce, &cifrado, b"alm-01").is_err());
}

#[test]
fn cambiar_los_datos_asociados_se_detecta() {
    // Los datos asociados vinculan el cifrado a su contexto: un bloque de ALM-01
    // no puede trasplantarse a ALM-02.
    let clave = clave_de_prueba();
    let nonce = [3u8; LONGITUD_NONCE];
    let cifrado = cifrar(&clave, &nonce, b"evidencia", b"alm-01").unwrap();

    assert!(descifrar(&clave, &nonce, &cifrado, b"alm-02").is_err());
}

#[test]
fn un_nonce_de_longitud_incorrecta_se_rechaza() {
    let clave = clave_de_prueba();
    assert!(cifrar(&clave, &[0u8; 8], b"x", b"").is_err());
    assert!(descifrar(&clave, &[0u8; 8], b"xxxxxxxxxxxxxxxx", b"").is_err());
}

// ---------------------------------------------------------------------------
// Secretos y conformidad
// ---------------------------------------------------------------------------

#[test]
fn el_secreto_no_revela_su_contenido_al_depurar() {
    let secreto = Secreto::nuevo([0xFFu8; 32]);
    let texto = format!("{secreto:?}");
    assert!(texto.contains("oculto"));
    assert!(!texto.contains("255"));
}

#[test]
fn la_conformidad_exige_las_tres_comprobaciones() {
    assert!(Conformidad::COMPLETA.apto_para_produccion());

    let parciales = [
        Conformidad {
            acvp: true,
            wycheproof: true,
            contraste_diferencial: false,
        },
        Conformidad {
            acvp: true,
            wycheproof: false,
            contraste_diferencial: true,
        },
        Conformidad {
            acvp: false,
            wycheproof: true,
            contraste_diferencial: true,
        },
    ];

    for parcial in parciales {
        assert!(
            !parcial.apto_para_produccion(),
            "ninguna comprobación sustituye a otra: {parcial:?}"
        );
    }
}

#[test]
fn solo_acvp_no_basta() {
    // RPT-005 §4.3. CVE-2026-24850 pasaba ACVP y fue detectada por Wycheproof.
    let solo_acvp = Conformidad {
        acvp: true,
        ..Conformidad::default()
    };
    assert!(!solo_acvp.apto_para_produccion());
    assert_eq!(solo_acvp.pendientes().len(), 2);
}
