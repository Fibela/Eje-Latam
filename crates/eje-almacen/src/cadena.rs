//! Cadena de resumenes del registro de evidencia ALM-01.
//!
//! # Cadena de resumenes frente a arbol Merkle
//!
//! RPT-002 §5 describe la integridad de ALM-01 como "encadenamiento de hashes
//! (arbol de Merkle)". Son dos construcciones distintas y ambas hacen falta,
//! por razones diferentes:
//!
//! - La **cadena** (este modulo) hace evidente cualquier mutacion: alterar un
//!   asiento invalida el suyo y el de todos los posteriores. Verificarla cuesta
//!   O(n) y exige el registro completo.
//! - El **arbol Merkle** ([`crate::merkle`]) permite demostrar que un asiento
//!   concreto pertenece al registro **sin revelar los demas**.
//!
//! Esa segunda propiedad no es un lujo. Para aportar un evento a un proceso
//! judicial habria que exportar el registro entero de un hospital, que contiene
//! trafico de red de otros pacientes y sistemas: un problema de proteccion de
//! datos bajo LGPD y equivalentes (RPT-002 §6). La divulgacion selectiva evita
//! elegir entre probar el hecho y proteger a terceros.

use crate::ErrorAlmacen;
use crate::esquema::ClaseEvento;
use crate::resumen::{Absorbedor, Resumen};

/// Dominio de separacion para el resumen de un asiento.
const DOMINIO_ASIENTO: &[u8] = b"eje-latam/alm-01/asiento/v1";

/// Asiento inmutable del registro de evidencia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asiento {
    /// Numero de asiento, monotono y sin huecos, comenzando en 1.
    pub numero: u64,
    /// Instante en milisegundos desde el epoch UTC.
    pub instante_utc: i64,
    /// Clase del evento.
    pub clase: ClaseEvento,
    /// Nodo al que se refiere el evento.
    pub nodo: String,
    /// Detalle del evento, ya seudonimizado (RPT-003 §6 y RPT-002 §6).
    pub detalle: String,
    /// Resumen del asiento anterior, o [`Resumen::GENESIS`] para el primero.
    pub resumen_anterior: Resumen,
    /// Resumen propio, calculado sobre todos los campos anteriores.
    pub resumen_propio: Resumen,
}

impl Asiento {
    /// Recalcula el resumen que corresponde a los campos de este asiento.
    ///
    /// Comparar el resultado con [`Asiento::resumen_propio`] detecta cualquier
    /// alteracion de los datos.
    #[must_use]
    pub fn resumen_calculado(&self) -> Resumen {
        let mut absorbedor = Absorbedor::nuevo(DOMINIO_ASIENTO);
        absorbedor
            .entero(self.numero)
            .entero_con_signo(self.instante_utc)
            .campo(self.clase.identificador().as_bytes())
            .campo(self.nodo.as_bytes())
            .campo(self.detalle.as_bytes())
            .resumen(&self.resumen_anterior);
        absorbedor.finalizar()
    }
}

/// Registro de evidencia en memoria.
///
/// Implementacion de referencia sobre la que se validan las invariantes. La
/// persistencia en libSQL se apoya en esta misma logica de encadenamiento: la
/// base almacena, no decide.
#[derive(Debug, Default)]
pub struct RegistroEvidencia {
    asientos: Vec<Asiento>,
}

impl RegistroEvidencia {
    /// Crea un registro vacio.
    #[must_use]
    pub const fn nuevo() -> Self {
        Self {
            asientos: Vec::new(),
        }
    }

    /// Numero de asientos registrados.
    #[must_use]
    pub fn longitud(&self) -> usize {
        self.asientos.len()
    }

    /// Indica si el registro esta vacio.
    #[must_use]
    pub fn vacio(&self) -> bool {
        self.asientos.is_empty()
    }

    /// Asientos registrados, en orden.
    #[must_use]
    pub fn asientos(&self) -> &[Asiento] {
        &self.asientos
    }

    /// Resumen del ultimo asiento, o genesis si el registro esta vacio.
    #[must_use]
    pub fn extremo(&self) -> Resumen {
        self.asientos
            .last()
            .map_or(Resumen::GENESIS, |asiento| asiento.resumen_propio)
    }

    /// Anexa un evento al registro.
    ///
    /// Es la unica operacion de escritura. No existe metodo alguno para
    /// modificar ni eliminar un asiento: la inmutabilidad es una propiedad del
    /// tipo, no una convencion.
    pub fn anexar(
        &mut self,
        instante_utc: i64,
        clase: ClaseEvento,
        nodo: &str,
        detalle: &str,
    ) -> &Asiento {
        let numero = self.asientos.len() as u64 + 1;
        let resumen_anterior = self.extremo();

        let mut asiento = Asiento {
            numero,
            instante_utc,
            clase,
            nodo: nodo.to_owned(),
            detalle: detalle.to_owned(),
            resumen_anterior,
            resumen_propio: Resumen::GENESIS,
        };
        asiento.resumen_propio = asiento.resumen_calculado();

        self.asientos.push(asiento);
        // El indice existe: se acaba de insertar.
        &self.asientos[numero as usize - 1]
    }

    /// Verifica la integridad de toda la cadena.
    ///
    /// Comprueba numeracion consecutiva, enlace con el asiento anterior y
    /// coincidencia del resumen propio con los datos.
    ///
    /// # Errores
    ///
    /// Devuelve [`ErrorAlmacen::CadenaRota`] indicando el primer asiento donde
    /// se detecto la discontinuidad.
    pub fn verificar_cadena(&self) -> Result<(), ErrorAlmacen> {
        let mut esperado_anterior = Resumen::GENESIS;

        for (indice, asiento) in self.asientos.iter().enumerate() {
            let numero_esperado = indice as u64 + 1;

            if asiento.numero != numero_esperado {
                return Err(ErrorAlmacen::CadenaRota {
                    asiento: numero_esperado,
                });
            }
            if asiento.resumen_anterior != esperado_anterior {
                return Err(ErrorAlmacen::CadenaRota {
                    asiento: asiento.numero,
                });
            }
            if asiento.resumen_propio != asiento.resumen_calculado() {
                return Err(ErrorAlmacen::CadenaRota {
                    asiento: asiento.numero,
                });
            }

            esperado_anterior = asiento.resumen_propio;
        }

        Ok(())
    }

    /// Inserta un asiento ya construido, sin recalcular su resumen.
    ///
    /// Existe unicamente para que las pruebas puedan reconstruir un registro
    /// manipulado —como haria quien edita la base de datos por debajo del
    /// agente— y comprobar que [`RegistroEvidencia::verificar_cadena`] lo
    /// detecta. No forma parte de la API publica.
    #[cfg(test)]
    pub(crate) fn anexar_crudo_para_pruebas(&mut self, asiento: Asiento) {
        self.asientos.push(asiento);
    }

    /// Recupera un asiento por su numero.
    #[must_use]
    pub fn asiento(&self, numero: u64) -> Option<&Asiento> {
        if numero == 0 {
            return None;
        }
        self.asientos.get(numero as usize - 1)
    }
}
