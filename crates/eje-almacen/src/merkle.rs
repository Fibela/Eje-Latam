//! Sellos Merkle y pruebas de inclusion.
//!
//! # Para que sirve
//!
//! Permite demostrar que un asiento concreto pertenece al registro **sin
//! entregar el registro completo**. Aportar un evento a un proceso judicial no
//! deberia obligar a exportar todo el trafico de red de un hospital, que
//! contiene datos de terceros (RPT-002 §6).
//!
//! # Separacion de dominio
//!
//! Las hojas se resumen con un dominio distinto al de los nodos internos, segun
//! la construccion de RFC 6962. Sin esa separacion, un atacante podria presentar
//! el resumen de un nodo interno como si fuera una hoja y fabricar una prueba de
//! inclusion para un asiento que nunca existio: es el ataque de segunda
//! preimagen sobre arboles Merkle.

use crate::resumen::{Absorbedor, Resumen};

/// Dominio de las hojas del arbol.
const DOMINIO_HOJA: &[u8] = b"eje-latam/alm-01/merkle-hoja/v1";

/// Dominio de los nodos internos del arbol.
const DOMINIO_NODO: &[u8] = b"eje-latam/alm-01/merkle-nodo/v1";

/// Resumen de una hoja a partir del resumen de un asiento.
#[must_use]
pub fn hoja(resumen_asiento: &Resumen) -> Resumen {
    let mut absorbedor = Absorbedor::nuevo(DOMINIO_HOJA);
    absorbedor.resumen(resumen_asiento);
    absorbedor.finalizar()
}

/// Resumen de un nodo interno a partir de sus dos hijos.
#[must_use]
pub fn nodo(izquierdo: &Resumen, derecho: &Resumen) -> Resumen {
    let mut absorbedor = Absorbedor::nuevo(DOMINIO_NODO);
    absorbedor.resumen(izquierdo).resumen(derecho);
    absorbedor.finalizar()
}

/// Calcula la raiz Merkle de una secuencia de resumenes de asiento.
///
/// Devuelve `None` para una secuencia vacia: un sello sin asientos no significa
/// nada y no debe poder construirse.
#[must_use]
pub fn raiz(resumenes: &[Resumen]) -> Option<Resumen> {
    if resumenes.is_empty() {
        return None;
    }

    let mut nivel: Vec<Resumen> = resumenes.iter().map(hoja).collect();

    while nivel.len() > 1 {
        let mut siguiente = Vec::with_capacity(nivel.len().div_ceil(2));
        let mut indice = 0;
        while indice < nivel.len() {
            match (nivel.get(indice), nivel.get(indice + 1)) {
                (Some(izquierdo), Some(derecho)) => siguiente.push(nodo(izquierdo, derecho)),
                // Nodo impar: se promueve al nivel siguiente sin volver a
                // resumirlo, como en RFC 6962.
                (Some(unico), None) => siguiente.push(*unico),
                _ => break,
            }
            indice += 2;
        }
        nivel = siguiente;
    }

    nivel.first().copied()
}

/// Sello emitido sobre un rango de asientos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sello {
    /// Primer numero de asiento cubierto, inclusive.
    pub desde: u64,
    /// Ultimo numero de asiento cubierto, inclusive.
    pub hasta: u64,
    /// Raiz Merkle del rango.
    pub raiz: Resumen,
}

/// Paso del camino de autenticacion de una prueba de inclusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasoPrueba {
    /// Resumen del nodo hermano.
    pub hermano: Resumen,
    /// Si el hermano ocupa la posicion derecha en el par.
    pub hermano_a_la_derecha: bool,
}

/// Prueba de que un asiento pertenece al rango sellado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruebaInclusion {
    /// Numero del asiento probado.
    pub numero: u64,
    /// Resumen del asiento probado.
    pub resumen_asiento: Resumen,
    /// Camino desde la hoja hasta la raiz.
    pub camino: Vec<PasoPrueba>,
}

/// Construye la prueba de inclusion del asiento en la posicion indicada.
///
/// `posicion` es el indice dentro de `resumenes`, comenzando en 0.
#[must_use]
pub fn prueba_inclusion(
    resumenes: &[Resumen],
    posicion: usize,
    numero_asiento: u64,
) -> Option<PruebaInclusion> {
    let resumen_asiento = *resumenes.get(posicion)?;

    let mut nivel: Vec<Resumen> = resumenes.iter().map(hoja).collect();
    let mut indice = posicion;
    let mut camino = Vec::new();

    while nivel.len() > 1 {
        let par = indice / 2;
        // En un arbol binario completo el hermano de un nodo se obtiene
        // invirtiendo el bit menos significativo de su indice.
        let es_hijo_izquierdo = indice % 2 == 0;
        let posicion_hermano = indice ^ 1;

        if let Some(hermano) = nivel.get(posicion_hermano) {
            camino.push(PasoPrueba {
                hermano: *hermano,
                hermano_a_la_derecha: es_hijo_izquierdo,
            });
        }
        // Si no hay hermano, el nodo se promueve sin paso de autenticacion.

        let mut siguiente = Vec::with_capacity(nivel.len().div_ceil(2));
        let mut cursor = 0;
        while cursor < nivel.len() {
            match (nivel.get(cursor), nivel.get(cursor + 1)) {
                (Some(a), Some(b)) => siguiente.push(nodo(a, b)),
                (Some(unico), None) => siguiente.push(*unico),
                _ => break,
            }
            cursor += 2;
        }

        nivel = siguiente;
        indice = par;
    }

    Some(PruebaInclusion {
        numero: numero_asiento,
        resumen_asiento,
        camino,
    })
}

/// Verifica una prueba de inclusion contra una raiz conocida.
///
/// Quien verifica no necesita el registro: le basta la prueba, el asiento y la
/// raiz publicada.
#[must_use]
pub fn verificar_inclusion(prueba: &PruebaInclusion, raiz_esperada: &Resumen) -> bool {
    let mut actual = hoja(&prueba.resumen_asiento);

    for paso in &prueba.camino {
        actual = if paso.hermano_a_la_derecha {
            nodo(&actual, &paso.hermano)
        } else {
            nodo(&paso.hermano, &actual)
        };
    }

    actual == *raiz_esperada
}
