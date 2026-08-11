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
use crate::persistencia::ASIENTOS_MAXIMOS;
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
#[derive(Debug)]
pub struct RegistroEvidencia {
    asientos: Vec<Asiento>,
    /// Numero del primer asiento que este segmento contiene.
    ///
    /// RPT-040 §2, PA-59. Un registro sin segmentar tiene `base = 1`, que es lo
    /// que un fichero de la version 1 significa sin necesidad de migrarlo.
    base: u64,
    /// Resumen con el que enlaza el primer asiento del segmento.
    ///
    /// Para el primer segmento es [`Resumen::GENESIS`]; para los siguientes, el
    /// extremo del segmento anterior. Es lo que **encadena los segmentos entre
    /// si**: alterar un segmento archivado rompe este enlace en el siguiente
    /// (RPT-040 §4), y por eso los archivados no necesitan ancla propia.
    genesis: Resumen,
}

impl Default for RegistroEvidencia {
    /// Delega en [`RegistroEvidencia::nuevo`].
    ///
    /// # Por que no se deriva
    ///
    /// RPT-040, PA-59. `Default` derivado daria `base: 0` y
    /// `genesis: Default::default()`, y `base == 0` es justo el estado que
    /// `analizar` rechaza en la puerta: con base cero, el `ultimo_numero` de un
    /// segmento vacio se calcula sobre `0 - 1`.
    ///
    /// Derivarlo habria construido en memoria, sin ruido, el mismo registro
    /// invalido que el analizador se niega a leer del disco.
    fn default() -> Self {
        Self::nuevo()
    }
}

impl RegistroEvidencia {
    /// Crea un registro vacio que empieza en el asiento 1.
    #[must_use]
    pub const fn nuevo() -> Self {
        Self {
            asientos: Vec::new(),
            base: 1,
            genesis: Resumen::GENESIS,
        }
    }

    /// Crea un segmento vacio que continua a otro.
    ///
    /// RPT-040 §1. El segmento nuevo arrastra el extremo del anterior como
    /// genesis, y de ahi sale la propiedad que hace la rotacion segura: el ancla
    /// que describe el final del segmento cerrado describe **tambien** el estado
    /// inicial de este, con el mismo numero y el mismo extremo. Rotar no toca el
    /// ancla, asi que no hay estado intermedio que un corte de energia pueda
    /// dejar a medias.
    #[must_use]
    pub const fn continuando(base: u64, genesis: Resumen) -> Self {
        Self {
            asientos: Vec::new(),
            base,
            genesis,
        }
    }

    /// Este segmento continua exactamente al dado.
    ///
    /// RPT-040 §4, PA-59. **Es lo que sustituye al ancla en los segmentos
    /// archivados.** Alterar un asiento de un segmento cerrado cambia su
    /// extremo, y entonces deja de coincidir con el genesis del siguiente: la
    /// cadena se rompe en la frontera aunque cada segmento verifique consigo
    /// mismo sin objecion.
    ///
    /// Sin esta comprobacion la propiedad seguiria siendo cierta y no la
    /// ejecutaria nadie, que es la unica forma que tiene una garantia de no
    /// existir.
    #[must_use]
    pub fn continua_a(&self, anterior: &Self) -> bool {
        self.base == anterior.ultimo_numero() + 1 && self.genesis == anterior.extremo()
    }

    /// Numero del primer asiento que este segmento contiene.
    #[must_use]
    pub const fn base(&self) -> u64 {
        self.base
    }

    /// Resumen con el que enlaza el primer asiento.
    #[must_use]
    pub const fn genesis(&self) -> Resumen {
        self.genesis
    }

    /// Numero del ultimo asiento **conocido**, este o no en este segmento.
    ///
    /// Para un segmento vacio es `base - 1`: el ultimo del anterior. Es la cifra
    /// que el ancla describe, y por eso un segmento recien abierto la satisface
    /// sin que nadie reescriba nada.
    #[must_use]
    pub fn ultimo_numero(&self) -> u64 {
        self.asientos
            .last()
            .map_or(self.base.saturating_sub(1), |asiento| asiento.numero)
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

    /// Resumen del ultimo asiento, o el genesis del segmento si esta vacio.
    #[must_use]
    pub fn extremo(&self) -> Resumen {
        self.asientos
            .last()
            .map_or(self.genesis, |asiento| asiento.resumen_propio)
    }

    /// Anexa un evento al registro.
    ///
    /// Es la unica operacion de escritura. No existe metodo alguno para
    /// modificar ni eliminar un asiento: la inmutabilidad es una propiedad del
    /// tipo, no una convencion.
    /// # Errores
    ///
    /// [`ErrorAlmacen::CapacidadExcedida`] cuando el registro ya contiene
    /// [`ASIENTOS_MAXIMOS`] asientos. **Negarse es lo correcto**: seguir
    /// anexando produce un fichero que el arranque siguiente no puede leer y que
    /// por tanto se lee como manipulacion (RPT-039 §1).
    ///
    /// Perder una alerta es grave. Perderla **y ademas** acusar de manipulacion
    /// a quien no toco nada lo es mas, porque manda al operador a investigar un
    /// ataque que no existio mientras el sensor sigue sin registrar.
    pub fn anexar(
        &mut self,
        instante_utc: i64,
        clase: ClaseEvento,
        nodo: &str,
        detalle: &str,
    ) -> Result<&Asiento, ErrorAlmacen> {
        if self.asientos.len() >= ASIENTOS_MAXIMOS {
            return Err(ErrorAlmacen::CapacidadExcedida {
                maximo: ASIENTOS_MAXIMOS,
            });
        }

        let numero = self.base + self.asientos.len() as u64;
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
        Ok(&self.asientos[self.asientos.len() - 1])
    }

    /// El registro no admite mas asientos.
    ///
    /// Es una condicion, no un suceso: sigue siendo cierta hasta que alguien
    /// intervenga (RPT-019 §2). Mientras dure, **este sensor no registra
    /// amenazas**.
    #[must_use]
    pub fn saturado(&self) -> bool {
        self.asientos.len() >= ASIENTOS_MAXIMOS
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
        // El primer asiento del segmento enlaza con el extremo del segmento
        // anterior, no con el genesis absoluto. Comparar contra `GENESIS` aqui
        // haria que todo segmento rotado se leyera como cadena rota.
        let mut esperado_anterior = self.genesis;

        for (indice, asiento) in self.asientos.iter().enumerate() {
            let numero_esperado = self.base + indice as u64;

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
        // Un numero anterior a la base no esta en este segmento. **No es que no
        // exista**: esta en un segmento archivado, y quien pregunte debe poder
        // distinguirlo (PA-74).
        let posicion = numero.checked_sub(self.base)?;
        self.asientos.get(usize::try_from(posicion).ok()?)
    }
}
