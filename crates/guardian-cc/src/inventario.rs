//! Inventario firmado de marcados administrativos.
//!
//! RPT-011, PA-24.
//!
//! # La cadena de cuatro eslabones
//!
//! Un marcado solo vale si los cuatro se cierran. Cada uno existe porque su
//! ausencia deja pasar un ataque concreto:
//!
//! | Eslabon | Que ata | Ataque que impide |
//! |---|---|---|
//! | 1. El resumen del marcado coincide con el de la prueba | prueba ↔ **este** marcado | presentar una prueba valida de **otra** entrada |
//! | 2. La prueba de inclusion verifica contra la raiz | marcado ↔ inventario | inventar una entrada que nunca estuvo |
//! | 3. La firma hibrida verifica sobre la raiz | inventario ↔ administrador | sustituir el inventario entero |
//! | 4. La clave pertenece al dominio del cliente | administrador ↔ custodia | firmar marcados con la clave de PremosCorp |
//!
//! El eslabon 1 es el que se olvida. `verificar_inclusion` comprueba que la
//! prueba es internamente consistente con la raiz, pero **nada en ella la ata al
//! marcado que se esta verificando**: quien presente una prueba legitima de otra
//! entrada del inventario pasaria el eslabon 2 sin el 1.
//!
//! # Por que la firma cubre la raiz y no cada entrada
//!
//! RPT-010 §4. Firmar entrada por entrada no protege contra la **supresion**:
//! borrar «esta bomba es soporte vital» no rompe ninguna firma de las que
//! quedan. Firmar la raiz Merkle hace que la ausencia de una hoja cambie la raiz
//! y, con ella, invalide la firma.
//!
//! # Codificacion canonica
//!
//! Se reutiliza [`Absorbedor`] de `eje-almacen`, que ya prefija en longitud la
//! etiqueta de dominio y cada campo. Escribir una codificacion nueva para esto
//! habria significado mantener dos mecanismos equivalentes y auditar ambos.

use eje_almacen::merkle::{PruebaInclusion, verificar_inclusion};
use eje_almacen::resumen::{Absorbedor, Resumen};
use motor_pqc::firma_hibrida::{ClaveVerificacionHibrida, FirmaHibrida, verificar};

use crate::ClaseExcluida;
use crate::proveedores::DireccionEnlace;

/// Dominio del resumen de un marcado individual.
const DOMINIO_MARCADO: &[u8] = b"eje-latam/agt-01/marcado-inventario/v1";

/// Dominio del mensaje firmado sobre la raiz del inventario.
///
/// Separado del anterior: sin esta etiqueta, una firma sobre 32 bytes
/// cualesquiera podria reutilizarse como firma de raiz.
const DOMINIO_RAIZ: &[u8] = b"eje-latam/agt-01/raiz-inventario/v1";

/// Dominio de custodia de una clave de verificacion.
///
/// # Por que es un tipo y no un comentario
///
/// La clave con la que PremosCorp firma binarios y la clave con la que el
/// administrador del cliente firma su inventario de equipos son **distintas, con
/// custodios distintos**, y confundirlas es grave en ambas direcciones:
///
/// - PremosCorp no debe poder declarar que equipos del cliente son criticos.
/// - El cliente no debe poder firmar nada que el agente cargue como codigo.
///
/// Reutilizar la infraestructura de firma de releases por comodidad es el error
/// que este tipo impide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DominioClave {
    /// Administrador local del cliente. Firma marcados y declaraciones de VLAN.
    Cliente,
    /// PremosCorp. Firma binarios, reglas e imagenes de release (PA-14).
    PremosCorp,
}

/// Errores de verificacion del inventario.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorInventario {
    /// La prueba no corresponde al marcado presentado.
    #[error("la prueba de inclusion pertenece a otra entrada del inventario")]
    PruebaAjenaAlMarcado,

    /// La prueba no verifica contra la raiz.
    #[error("la prueba de inclusion no verifica contra la raiz anclada")]
    InclusionNoVerifica,

    /// La firma de la raiz no verifica.
    #[error("la firma de la raiz del inventario no verifica")]
    FirmaDeRaizInvalida,

    /// Se intento verificar un inventario con una clave de otro dominio.
    #[error("clave del dominio {encontrado:?}; el inventario exige {esperado:?}")]
    DominioDeClaveIncorrecto {
        /// Dominio de la clave presentada.
        encontrado: DominioClave,
        /// Dominio exigido.
        esperado: DominioClave,
    },
}

/// Clave de verificacion con su dominio de custodia declarado.
pub struct ClaveInventario {
    clave: ClaveVerificacionHibrida,
    dominio: DominioClave,
}

impl ClaveInventario {
    /// Aprovisiona una clave declarando su dominio de custodia.
    ///
    /// El dominio se declara al aprovisionar, no al usar: una clave que llega
    /// sin dominio no puede adquirirlo mas tarde por conveniencia.
    #[must_use]
    pub const fn nueva(clave: ClaveVerificacionHibrida, dominio: DominioClave) -> Self {
        Self { clave, dominio }
    }

    /// Dominio de custodia declarado.
    #[must_use]
    pub const fn dominio(&self) -> DominioClave {
        self.dominio
    }
}

/// Marcado tal como llega, **sin verificar**.
///
/// El nombre lo dice: cualquiera puede construir uno. Su unico destino legitimo
/// es [`MarcadoVerificado::verificar_e_instanciar`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarcadoBruto {
    /// Dispositivo al que se refiere.
    pub mac: DireccionEnlace,
    /// Clase declarada. `None` significa «declarado no critico».
    pub clase: Option<ClaseExcluida>,
    /// Instante de emision, en segundos desde la epoca.
    pub emitido_en: u64,
    /// Vigencia declarada, en dias.
    pub vigencia_dias: u32,
}

impl MarcadoBruto {
    /// Resumen canonico de este marcado.
    ///
    /// Es el valor que debe figurar como hoja del arbol del inventario. La clase
    /// se codifica como escalar cerrado y no como cadena libre: dos textos
    /// distintos para la misma clase produirian hojas distintas.
    #[must_use]
    pub fn resumen(&self) -> Resumen {
        let mut absorbedor = Absorbedor::nuevo(DOMINIO_MARCADO);
        absorbedor
            .campo(&self.mac)
            .entero(u64::from(codigo_de_clase(self.clase)))
            .entero(self.emitido_en)
            .entero(u64::from(self.vigencia_dias));
        absorbedor.finalizar()
    }
}

/// Codigo escalar de una clase excluida. `0` significa «no critico».
const fn codigo_de_clase(clase: Option<ClaseExcluida>) -> u8 {
    match clase {
        None => 0,
        Some(ClaseExcluida::SoporteVital) => 1,
        Some(ClaseExcluida::SeguridadFuncional) => 2,
        Some(ClaseExcluida::CaminoDeGestion) => 3,
    }
}

/// Mensaje canonico que el administrador firma sobre la raiz del inventario.
#[must_use]
pub fn mensaje_de_raiz(raiz: &Resumen) -> Vec<u8> {
    let mut absorbedor = Absorbedor::nuevo(DOMINIO_RAIZ);
    absorbedor.resumen(raiz);
    absorbedor.finalizar().bytes().to_vec()
}

/// Marcado administrativo cuya cadena de verificacion se cerro por completo.
///
/// # Invariante
///
/// Los campos son privados y no existe constructor publico salvo
/// [`Self::verificar_e_instanciar`]. Un valor de este tipo **es**, por
/// construccion, un marcado que pertenece a un inventario firmado por el
/// administrador del cliente.
///
/// Con campos publicos el nombre habria mentido: cualquiera podria fabricar un
/// «verificado» sin verificar nada, y la garantia criptografica se degradaria a
/// una convencion de estilo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarcadoVerificado {
    mac: DireccionEnlace,
    clase: Option<ClaseExcluida>,
    emitido_en: u64,
    vigencia_dias: u32,
}

impl MarcadoVerificado {
    /// Unica via de construccion.
    ///
    /// Cierra los cuatro eslabones descritos en la documentacion del modulo, en
    /// ese orden.
    ///
    /// # Errores
    ///
    /// Un error distinto por eslabon: el diagnostico importa tanto como el
    /// rechazo. [`ErrorInventario::PruebaAjenaAlMarcado`] y
    /// [`ErrorInventario::InclusionNoVerifica`] describen ataques distintos y
    /// colapsarlos ocultaria cual se intento.
    pub fn verificar_e_instanciar(
        bruto: MarcadoBruto,
        prueba: &PruebaInclusion,
        raiz: &Resumen,
        firma: &FirmaHibrida,
        clave: &ClaveInventario,
    ) -> Result<Self, ErrorInventario> {
        // Eslabon 4 primero: una clave del dominio equivocado no debe llegar a
        // tocar dato alguno.
        if clave.dominio != DominioClave::Cliente {
            return Err(ErrorInventario::DominioDeClaveIncorrecto {
                encontrado: clave.dominio,
                esperado: DominioClave::Cliente,
            });
        }

        // Eslabon 1: la prueba habla de ESTE marcado y no de otro.
        if prueba.resumen_asiento != bruto.resumen() {
            return Err(ErrorInventario::PruebaAjenaAlMarcado);
        }

        // Eslabon 2: el marcado pertenece al inventario.
        if !verificar_inclusion(prueba, raiz) {
            return Err(ErrorInventario::InclusionNoVerifica);
        }

        // Eslabon 3: el inventario lo firmo el administrador.
        verificar(&clave.clave, &mensaje_de_raiz(raiz), firma)
            .map_err(|_| ErrorInventario::FirmaDeRaizInvalida)?;

        Ok(Self {
            mac: bruto.mac,
            clase: bruto.clase,
            emitido_en: bruto.emitido_en,
            vigencia_dias: bruto.vigencia_dias,
        })
    }

    /// Dispositivo al que se refiere.
    #[must_use]
    pub const fn mac(&self) -> &DireccionEnlace {
        &self.mac
    }

    /// Clase declarada. `None` significa «declarado no critico».
    #[must_use]
    pub const fn clase(&self) -> Option<ClaseExcluida> {
        self.clase
    }

    /// Indica si el marcado sigue vigente en el instante dado.
    ///
    /// # Politica de reloj
    ///
    /// Un agente Local-First puede tener el reloj desviado. Ante duda se declara
    /// **caducado**, no vigente: un marcado caducado degrada a ambiguo y escala
    /// a un humano, mientras que uno indebidamente vigente permitiria contener
    /// un equipo critico.
    ///
    /// Por eso un `ahora` anterior a la emision —reloj atrasado, o marcado con
    /// fecha futura— tambien cuenta como caducado.
    #[must_use]
    pub const fn vigente_en(&self, ahora: u64) -> bool {
        if ahora < self.emitido_en {
            return false;
        }

        let transcurrido = ahora - self.emitido_en;
        let vigencia_segundos = (self.vigencia_dias as u64).saturating_mul(86_400);

        transcurrido <= vigencia_segundos
    }
}
