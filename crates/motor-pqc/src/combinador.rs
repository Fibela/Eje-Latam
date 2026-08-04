//! Combinador de secretos para construcciones híbridas.
//!
//! # Por qué no basta concatenar los secretos
//!
//! Una construcción híbrida debe ser segura si **cualquiera** de sus dos
//! componentes lo es. Concatenar `ss_pq || ss_clasico` y usarlo como clave no
//! garantiza esa propiedad: no vincula los textos cifrados ni las claves públicas
//! que produjeron esos secretos, de modo que un atacante capaz de manipular el
//! transporte podría recombinar componentes de sesiones distintas.
//!
//! Las construcciones híbridas revisadas —X-Wing y el diseño híbrido de TLS—
//! coinciden en lo esencial: **derivar la clave final con una función de
//! derivación que absorba también los textos cifrados y las claves públicas**.
//!
//! # Por qué todo lleva prefijo de longitud
//!
//! Igual que en `eje-almacen`, absorber campos concatenados sin delimitar su
//! longitud permite que combinaciones distintas produzcan la misma entrada. Aquí
//! el efecto es peor: dos sesiones diferentes podrían derivar la misma clave.

use hkdf::Hkdf;
use sha2::Sha256;

use crate::ErrorPqc;
use crate::secreto::SecretoCompartido;

/// Etiqueta de dominio del intercambio de claves híbrido.
///
/// Incluye los algoritmos y la versión: cambiar cualquiera de los dos debe
/// producir claves distintas aunque los secretos de entrada coincidan.
pub const ETIQUETA_KEM: &[u8] = b"eje-latam/pqc/kem-hibrido/x25519+ml-kem-768/v1";

/// Etiqueta de dominio de la firma híbrida.
pub const ETIQUETA_FIRMA: &[u8] = b"eje-latam/pqc/firma-hibrida/ed25519+ml-dsa-65/v1";

/// Absorbe un campo precedido de su longitud en 8 bytes big-endian.
fn absorber(destino: &mut Vec<u8>, valor: &[u8]) {
    destino.extend_from_slice(&(valor.len() as u64).to_be_bytes());
    destino.extend_from_slice(valor);
}

/// Deriva el secreto compartido final de un intercambio híbrido.
///
/// # Argumentos
///
/// * `secreto_poscuantico` — secreto compartido producido por ML-KEM.
/// * `secreto_clasico` — secreto compartido producido por X25519.
/// * `cifrado_poscuantico` — texto cifrado de encapsulado de ML-KEM.
/// * `clave_publica_clasica` — clave pública efímera X25519 del emisor.
///
/// Los dos últimos se vinculan a la derivación para que la clave resultante
/// dependa de la transcripción completa, no solo de los secretos.
///
/// # Errores
///
/// Devuelve [`ErrorPqc::DerivacionFallida`] si la función de derivación rechaza
/// la longitud solicitada, condición que no puede darse con los parámetros
/// actuales pero que no se silencia.
pub fn derivar_secreto_hibrido(
    secreto_poscuantico: &[u8],
    secreto_clasico: &[u8],
    cifrado_poscuantico: &[u8],
    clave_publica_clasica: &[u8],
) -> Result<SecretoCompartido, ErrorPqc> {
    let mut material = Vec::with_capacity(secreto_poscuantico.len() + secreto_clasico.len() + 16);
    absorber(&mut material, secreto_poscuantico);
    absorber(&mut material, secreto_clasico);

    let mut contexto = Vec::with_capacity(ETIQUETA_KEM.len() + cifrado_poscuantico.len() + 64);
    absorber(&mut contexto, ETIQUETA_KEM);
    absorber(&mut contexto, cifrado_poscuantico);
    absorber(&mut contexto, clave_publica_clasica);

    let derivador = Hkdf::<Sha256>::new(None, &material);
    let mut salida = [0u8; 32];
    derivador
        .expand(&contexto, &mut salida)
        .map_err(|_| ErrorPqc::DerivacionFallida)?;

    Ok(SecretoCompartido::nuevo(salida))
}

/// Construye el mensaje canónico que ambos esquemas de firma firman.
///
/// Firmar el mensaje en bruto con dos algoritmos distintos permitiría, en
/// principio, reutilizar una de las dos firmas en otro contexto. Vincular la
/// etiqueta de dominio evita esa reutilización entre protocolos.
#[must_use]
pub fn mensaje_canonico_de_firma(mensaje: &[u8]) -> Vec<u8> {
    let mut canonico = Vec::with_capacity(ETIQUETA_FIRMA.len() + mensaje.len() + 16);
    absorber(&mut canonico, ETIQUETA_FIRMA);
    absorber(&mut canonico, mensaje);
    canonico
}
