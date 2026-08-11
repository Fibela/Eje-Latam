//! Reparto de secreto 2-de-3 sobre GF(2^8).
//!
//! RPT-027, PA-54.
//!
//! # Por que existe
//!
//! RPT-015 §8.1 ratifico que la clave de recuperacion del cliente vive repartida
//! entre tres custodios y se reconstruye con dos. Hasta ahora era una decision
//! escrita sin nada que la implementara.
//!
//! # Por que no se trae una dependencia
//!
//! El reparto de Shamir es de los pocos esquemas donde escribirlo es defendible:
//! **es incondicionalmente seguro** si los coeficientes son aleatorios, y no
//! depende de suposiciones sutiles de implementacion como las que hacen
//! peligroso escribir una curva o un AEAD a mano.
//!
//! La multiplicacion va **sin tablas** de logaritmos. Con tablas seria mas corta
//! y filtraria por cache; el bucle de aqui es de tiempo constante en el valor de
//! los operandos, que es lo que corresponde en una operacion que toca material de
//! clave.
//!
//! # Lo que Shamir **no** da, y hay que decirlo
//!
//! **No hay integridad.** Un custodio que entregue un fragmento alterado no hace
//! fallar la reconstruccion: produce **otro secreto**, silenciosamente. Shamir
//! reparte, no autentica.
//!
//! Este modulo no lo resuelve porque no puede: la comprobacion exige conocer algo
//! derivado del secreto original, y eso es una decision de formato de fichero.
//! [`eje-manifiesto`](../../eje_manifiesto/fragmento/index.html) la cierra
//! guardando la huella de la clave publica en cada fragmento y comparandola tras
//! reunir. Aqui queda escrito para que nadie use [`reunir`] creyendo que valida.

use crate::secreto::Secreto;

/// Numero de custodios.
pub const CUSTODIOS: u8 = 3;

/// Fragmentos necesarios para reconstruir.
pub const UMBRAL: u8 = 2;

/// Longitud del secreto repartido, en bytes.
pub const LONGITUD_SECRETO: usize = 32;

/// Fallos del reparto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ErrorReparto {
    /// Se presentaron dos fragmentos del mismo custodio.
    ///
    /// Dos puntos con la misma abscisa no determinan una recta. Aceptarlo
    /// significaria dividir por cero, y **el umbral de dos dejaria de ser dos**:
    /// un custodio podria reconstruir presentando su fragmento dos veces.
    #[error("los dos fragmentos son del mismo custodio ({indice})")]
    CustodioRepetido {
        /// Indice repetido.
        indice: u8,
    },

    /// Un indice de custodio esta fuera de rango.
    ///
    /// El cero queda excluido a proposito: `f(0)` **es el secreto**, asi que un
    /// fragmento con indice 0 seria el secreto entero.
    #[error("el indice de custodio {indice} esta fuera de 1..={CUSTODIOS}")]
    IndiceFueraDeRango {
        /// Indice rechazado.
        indice: u8,
    },
}

/// Fragmento en poder de un custodio.
///
/// No deriva `Debug`: es material de clave, y la mitad de los registros de
/// depuracion acaban en un fichero.
#[derive(Clone)]
pub struct Fragmento {
    /// Abscisa, en `1..=CUSTODIOS`.
    pub indice: u8,
    /// Ordenada, byte a byte.
    pub bytes: [u8; LONGITUD_SECRETO],
}

/// Multiplicacion en GF(2^8) con el polinomio de AES (`x^8+x^4+x^3+x+1`).
///
/// Sin ramas y sin tablas: la mascara `0u8.wrapping_sub(bit)` vale `0xFF` cuando
/// el bit es 1 y `0x00` cuando es 0, lo que sustituye al `if` sin introducir un
/// salto que dependa del operando.
const fn multiplicar(mut uno: u8, mut otro: u8) -> u8 {
    let mut producto = 0u8;
    let mut vuelta = 0;

    while vuelta < 8 {
        producto ^= uno & 0u8.wrapping_sub(otro & 1);

        let desborda = (uno >> 7) & 1;
        uno <<= 1;
        uno ^= 0x1B & 0u8.wrapping_sub(desborda);

        otro >>= 1;
        vuelta += 1;
    }

    producto
}

/// Inverso multiplicativo en GF(2^8), por exponenciacion a 254.
///
/// `a^254 = a^-1` porque el grupo multiplicativo tiene 255 elementos. El caso
/// `a == 0` devuelve 0, que no es un inverso; no se alcanza porque quien llama
/// garantiza que el divisor es la diferencia de dos indices distintos.
const fn invertir(valor: u8) -> u8 {
    let mut resultado = 1u8;
    let mut base = valor;
    let mut exponente = 254u32;

    while exponente > 0 {
        if exponente & 1 == 1 {
            resultado = multiplicar(resultado, base);
        }
        base = multiplicar(base, base);
        exponente >>= 1;
    }

    resultado
}

/// Reparte un secreto entre [`CUSTODIOS`] fragmentos.
///
/// `coeficientes` son los `a1` de las rectas `f(x) = secreto + a1·x`, uno por
/// byte. Se **reciben** en lugar de generarse aqui por la misma razon que la sal
/// y el nonce del sellado: este modulo no debe decidir de donde sale la
/// aleatoriedad, y recibirlos permite que la prueba fije valores sin que exista
/// un camino de produccion con valores fijos.
///
/// # La calidad de `coeficientes` es la seguridad del esquema
///
/// Shamir es incondicionalmente seguro **solo** si son uniformes e
/// independientes. Con coeficientes predecibles, un fragmento basta para
/// recuperar el secreto.
#[must_use]
pub fn repartir(
    secreto: &Secreto<LONGITUD_SECRETO>,
    coeficientes: &[u8; LONGITUD_SECRETO],
) -> Vec<Fragmento> {
    let plano = secreto.exponer();

    (1..=CUSTODIOS)
        .map(|indice| {
            let mut bytes = [0u8; LONGITUD_SECRETO];

            for (posicion, destino) in bytes.iter_mut().enumerate() {
                *destino = plano[posicion] ^ multiplicar(coeficientes[posicion], indice);
            }

            Fragmento { indice, bytes }
        })
        .collect()
}

/// Reconstruye el secreto a partir de dos fragmentos.
///
/// # Esto **no** valida nada
///
/// Un fragmento alterado produce otro secreto, no un error. Ver el encabezado del
/// modulo: la comprobacion vive en el formato de fichero, no aqui.
///
/// # Errores
///
/// [`ErrorReparto::IndiceFueraDeRango`] o [`ErrorReparto::CustodioRepetido`].
pub fn reunir(
    uno: &Fragmento,
    otro: &Fragmento,
) -> Result<Secreto<LONGITUD_SECRETO>, ErrorReparto> {
    for fragmento in [uno, otro] {
        if fragmento.indice == 0 || fragmento.indice > CUSTODIOS {
            return Err(ErrorReparto::IndiceFueraDeRango {
                indice: fragmento.indice,
            });
        }
    }

    if uno.indice == otro.indice {
        return Err(ErrorReparto::CustodioRepetido { indice: uno.indice });
    }

    // Interpolacion de Lagrange en x = 0 con dos puntos. En GF(2^n) la resta es
    // la misma operacion que la suma, asi que `x1 - x2` es `x1 ^ x2`.
    let diferencia = invertir(uno.indice ^ otro.indice);
    let peso_uno = multiplicar(otro.indice, diferencia);
    let peso_otro = multiplicar(uno.indice, diferencia);

    let mut plano = [0u8; LONGITUD_SECRETO];
    for (posicion, destino) in plano.iter_mut().enumerate() {
        *destino = multiplicar(uno.bytes[posicion], peso_uno)
            ^ multiplicar(otro.bytes[posicion], peso_otro);
    }

    Ok(Secreto::nuevo(plano))
}
