//! Inventario firmado de marcados administrativos.
//!
//! RPT-011, PA-24.
//!
//! # La cadena de cinco eslabones
//!
//! Un marcado solo vale si los cinco se cierran. Cada uno existe porque su
//! ausencia deja pasar un ataque concreto:
//!
//! | Eslabon | Que ata | Ataque que impide |
//! |---|---|---|
//! | 1. El resumen del marcado coincide con el de la prueba | prueba ↔ **este** marcado | presentar una prueba valida de **otra** entrada |
//! | 2. La prueba de inclusion verifica contra la raiz | marcado ↔ inventario | inventar una entrada que nunca estuvo |
//! | 3. La firma hibrida verifica sobre raiz **y** secuencia | inventario ↔ administrador | sustituir el inventario entero |
//! | 4. La clave pertenece al dominio del cliente | administrador ↔ custodia | firmar marcados con la clave de PremosCorp |
//! | 5. La secuencia no retrocede | inventario ↔ **momento** | restaurar un inventario anterior, legitimamente firmado |
//!
//! Los eslabones 3, 4 y 5 se cierran una vez en [`RaizVerificada`]; los 1 y 2 se
//! comprueban por marcado. Un [`MarcadoVerificado`] no puede existir sin los
//! cinco: no hay forma de construir uno sin una [`RaizVerificada`] previa.
//!
//! El eslabon 1 es el que se olvida. `verificar_inclusion` comprueba que la
//! prueba es internamente consistente con la raiz, pero **nada en ella la ata al
//! marcado que se esta verificando**: quien presente una prueba legitima de otra
//! entrada del inventario pasaria el eslabon 2 sin el 1.
//!
//! El eslabon 5 es PA-27. La firma de un inventario de la semana pasada es
//! perfectamente valida; lo que no es valido es que describa un estado del parque
//! ya superado. Ver [`Centinela`] para lo que este mecanismo **no** cubre.
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

use eje_almacen::merkle::{PruebaInclusion, raiz, verificar_inclusion};
use eje_almacen::resumen::{Absorbedor, Resumen};
use motor_pqc::firma_hibrida::{ClaveVerificacionHibrida, FirmaHibrida, verificar};

use crate::ClaseExcluida;
use crate::proveedores::DireccionEnlace;
use crate::revocacion::{CertificadoVerificado, IdentificadorClave, RegistroRevocaciones};

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
    /// Clave de recuperacion del cliente, fuera de linea.
    ///
    /// Firma **solo** certificados de revocacion (RPT-015 §4). Separada de la
    /// operativa a proposito: si fueran la misma, quien roba la operativa podria
    /// revocarse a si mismo a una secuencia de corte alta, que es lo contrario de
    /// una revocacion.
    ClienteRecuperacion,
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

    /// Se presento un inventario anterior al ya aceptado.
    ///
    /// PA-27. La firma es legitima —es un inventario que el administrador emitio
    /// de verdad— pero describe un estado del parque ya superado.
    #[error("reversion: se acepto la secuencia {aceptada} y se presenta {presentada}")]
    ReversionDetectada {
        /// Secuencia mas alta ya aceptada.
        aceptada: u64,
        /// Secuencia del inventario presentado.
        presentada: u64,
    },

    /// No hay marca de agua contra la que comparar.
    ///
    /// Solo es legitimo durante el aprovisionamiento inicial. Fuera de el,
    /// significa que alguien borro el centinela, que es exactamente lo que haria
    /// quien pretende revertir el inventario.
    #[error("no hay centinela de frescura; el inventario no puede fecharse")]
    FrescuraNoEstablecida,

    /// La clave que firma esta revocada para esta secuencia.
    ///
    /// RPT-015. La firma es valida y la clave existio; lo que ya no vale es que
    /// firme por encima de su corte.
    #[error("clave revocada: firma la secuencia {presentada} y su corte es {corte}")]
    ClaveRevocada {
        /// Secuencia del inventario presentado.
        presentada: u64,
        /// Corte anotado en el registro de revocaciones.
        corte: u64,
    },

    /// El inventario declara dos veces el mismo dispositivo.
    ///
    /// Un lector indulgente tomaria la primera o la ultima; ambas elecciones son
    /// arbitrarias y una de ellas favorece al atacante que anade una segunda
    /// entrada «no critico».
    #[error("el inventario declara dos veces la direccion {mac:02x?}")]
    DispositivoDuplicado {
        /// Direccion repetida.
        mac: DireccionEnlace,
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

    /// Clave de verificacion subyacente.
    #[must_use]
    pub const fn verificacion(&self) -> &ClaveVerificacionHibrida {
        &self.clave
    }

    /// Identificador estable de esta clave.
    #[must_use]
    pub fn identificador(&self) -> IdentificadorClave {
        IdentificadorClave::de(&self.clave)
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

/// Raiz de un inventario junto al numero de secuencia que lo fecha.
///
/// Ambos viajan dentro del **mismo** mensaje firmado. Firmar la raiz por un lado
/// y la secuencia por otro permitiria recombinar la raiz vieja con la secuencia
/// nueva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaizAnclada {
    /// Raiz Merkle del inventario.
    pub raiz: Resumen,
    /// Numero de secuencia, estrictamente creciente entre emisiones.
    pub secuencia: u64,
}

/// Mensaje canonico que el administrador firma sobre la raiz del inventario.
#[must_use]
pub fn mensaje_de_raiz(anclada: &RaizAnclada) -> Vec<u8> {
    let mut absorbedor = Absorbedor::nuevo(DOMINIO_RAIZ);
    absorbedor.resumen(&anclada.raiz).entero(anclada.secuencia);
    absorbedor.finalizar().bytes().to_vec()
}

/// Marca de agua de la secuencia mas alta ya aceptada.
///
/// # Lo que este mecanismo puede y no puede hacer
///
/// La secuencia firmada detecta el **reemplazo por un inventario anterior**, que
/// es el ataque de PA-27: quien compromete el almacen local no puede falsificar
/// la firma del administrador, pero si puede restaurar el fichero legitimo de la
/// semana pasada, emitido antes de que la bomba se marcara como soporte vital.
///
/// Pero la comparacion solo vale si la marca de agua **no es rebobinable por el
/// mismo atacante**. Si el centinela vive en el almacen que el atacante controla,
/// restaura ambos de forma consistente y no queda rastro. La proteccion completa
/// contra reversion exige un ancla fuera del almacen escribible —contador
/// monotono en TPM o elemento seguro—, y eso no esta disponible en todos los
/// destinos.
///
/// Lo que si se consigue aqui: que la reversion **no sea silenciosa**. Un
/// centinela ausente no se lee como «primera vez», sino como
/// [`ErrorInventario::FrescuraNoEstablecida`]. Borrarlo es tan detectable como
/// rebobinarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Centinela {
    /// Secuencia mas alta aceptada hasta ahora.
    Establecido(u64),
    /// Aun no se aprovisiono ninguna.
    ///
    /// Legitimo **solo** durante el aprovisionamiento inicial, con un humano
    /// presente. Despues, su ausencia significa que alguien lo borro.
    SinEstablecer,
}

impl Centinela {
    /// Secuencia aceptada, si la hay.
    #[must_use]
    pub const fn secuencia(self) -> Option<u64> {
        match self {
            Self::Establecido(secuencia) => Some(secuencia),
            Self::SinEstablecer => None,
        }
    }

    /// Avanza la marca de agua tras aceptar un inventario.
    ///
    /// Nunca retrocede: si la secuencia presentada fuera menor, este metodo
    /// conserva la mayor. La comprobacion de reversion vive en
    /// [`RaizVerificada::verificar`]; esto es la segunda linea.
    #[must_use]
    pub fn avanzar(self, secuencia: u64) -> Self {
        match self {
            Self::Establecido(previa) => Self::Establecido(previa.max(secuencia)),
            Self::SinEstablecer => Self::Establecido(secuencia),
        }
    }

    /// Reinicia la marca de agua a la secuencia de corte de un certificado.
    ///
    /// # La unica via que baja el centinela
    ///
    /// RPT-015 §6.1, PA-33. Sin esta operacion, un atacante con la clave
    /// operativa emite un inventario con secuencia `u64::MAX`, el agente lo
    /// acepta —la firma es valida— y **ningun inventario legitimo puede ya
    /// superarlo**: el inventario queda congelado para siempre, con un solo
    /// mensaje, y revocar no lo arregla porque el centinela sigue arriba.
    ///
    /// Es segura porque exige un [`CertificadoVerificado`], que solo se
    /// construye con la clave de recuperacion fuera de linea. Quien tuviera esa
    /// clave no necesitaria este camino para nada.
    #[must_use]
    pub const fn reiniciar_por(self, certificado: &CertificadoVerificado) -> Self {
        Self::Establecido(certificado.hasta_secuencia())
    }
}

/// Inventario en orden canonico.
///
/// # Por que el orden vive aqui y no en `eje-almacen`
///
/// La propuesta recibida pedia que el arbol Merkle de `eje-almacen` ordenase sus
/// hojas por direccion. Seria un error de capa: ese arbol sirve al registro
/// forense ALM-01, donde el orden es **cronologico y significativo** —hay una
/// prueba, `reordenar_asientos_rompe_la_cadena`, que existe precisamente para
/// impedir que se reordene—. Ordenar alli corromperia la evidencia.
///
/// El orden canonico es una propiedad **del inventario**, no del arbol. Vive
/// aqui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventario {
    marcados: Vec<MarcadoBruto>,
}

impl Inventario {
    /// Construye el inventario en orden canonico.
    ///
    /// Ordena por direccion ascendente y rechaza duplicados. Sin orden fijo, dos
    /// herramientas administrativas producirian raices distintas para el mismo
    /// contenido; sin rechazo de duplicados, un lector indulgente elegiria entre
    /// dos entradas contradictorias.
    ///
    /// # Errores
    ///
    /// Devuelve [`ErrorInventario::DispositivoDuplicado`] si una direccion
    /// aparece dos veces.
    pub fn construir(mut marcados: Vec<MarcadoBruto>) -> Result<Self, ErrorInventario> {
        marcados.sort_unstable_by_key(|marcado| marcado.mac);

        for par in marcados.windows(2) {
            if par[0].mac == par[1].mac {
                return Err(ErrorInventario::DispositivoDuplicado { mac: par[0].mac });
            }
        }

        Ok(Self { marcados })
    }

    /// Marcados en orden canonico.
    #[must_use]
    pub fn marcados(&self) -> &[MarcadoBruto] {
        &self.marcados
    }

    /// Resumenes de las hojas, en orden canonico.
    #[must_use]
    pub fn resumenes(&self) -> Vec<Resumen> {
        self.marcados.iter().map(MarcadoBruto::resumen).collect()
    }

    /// Raiz Merkle del inventario. `None` si esta vacio.
    #[must_use]
    pub fn raiz(&self) -> Option<Resumen> {
        raiz(&self.resumenes())
    }

    /// Posicion canonica de una direccion, si figura.
    #[must_use]
    pub fn posicion_de(&self, mac: &DireccionEnlace) -> Option<usize> {
        self.marcados
            .binary_search_by_key(mac, |marcado| marcado.mac)
            .ok()
    }
}

/// Raiz de inventario cuya firma, dominio de clave y frescura ya se comprobaron.
///
/// Se verifica **una vez** y sirve para validar muchos marcados. Los campos son
/// privados: existir es la prueba de que las tres comprobaciones se cerraron.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaizVerificada {
    raiz: Resumen,
    secuencia: u64,
}

impl RaizVerificada {
    /// Verifica el dominio de la clave, la frescura y la firma de la raiz.
    ///
    /// # Orden
    ///
    /// Dominio de clave, frescura y firma, en ese orden. La frescura se comprueba
    /// **antes** que la firma a proposito: un inventario revertido tiene firma
    /// valida, asi que verificar primero la firma y despues la secuencia daria un
    /// «firma correcta» enganoso en el registro antes del rechazo real.
    ///
    /// # Errores
    ///
    /// [`ErrorInventario::DominioDeClaveIncorrecto`],
    /// [`ErrorInventario::FrescuraNoEstablecida`],
    /// [`ErrorInventario::ReversionDetectada`] o
    /// [`ErrorInventario::FirmaDeRaizInvalida`].
    pub fn verificar(
        anclada: RaizAnclada,
        firma: &FirmaHibrida,
        clave: &ClaveInventario,
        centinela: Centinela,
        revocaciones: &RegistroRevocaciones,
    ) -> Result<Self, ErrorInventario> {
        if clave.dominio != DominioClave::Cliente {
            return Err(ErrorInventario::DominioDeClaveIncorrecto {
                encontrado: clave.dominio,
                esperado: DominioClave::Cliente,
            });
        }

        // Eslabon 6 (RPT-015). Va antes que la frescura y que la firma por el
        // mismo motivo que el dominio: una clave revocada no debe llegar a gastar
        // ciclos criptograficos.
        let identificador = clave.identificador();
        if !revocaciones.admite(&identificador, anclada.secuencia) {
            return Err(ErrorInventario::ClaveRevocada {
                presentada: anclada.secuencia,
                corte: revocaciones
                    .corte_de(&identificador)
                    .unwrap_or(anclada.secuencia),
            });
        }

        let Some(aceptada) = centinela.secuencia() else {
            return Err(ErrorInventario::FrescuraNoEstablecida);
        };

        if anclada.secuencia < aceptada {
            return Err(ErrorInventario::ReversionDetectada {
                aceptada,
                presentada: anclada.secuencia,
            });
        }

        verificar(&clave.clave, &mensaje_de_raiz(&anclada), firma)
            .map_err(|_| ErrorInventario::FirmaDeRaizInvalida)?;

        Ok(Self {
            raiz: anclada.raiz,
            secuencia: anclada.secuencia,
        })
    }

    /// Aprovisiona la primera raiz, estableciendo el centinela.
    ///
    /// Solo debe invocarse con un humano presente durante la instalacion. Fuera
    /// de ese momento, la ausencia de centinela es un indicio de manipulacion y
    /// [`Self::verificar`] la trata como tal.
    ///
    /// # Errores
    ///
    /// Las mismas que [`Self::verificar`] salvo las de frescura.
    pub fn aprovisionar(
        anclada: RaizAnclada,
        firma: &FirmaHibrida,
        clave: &ClaveInventario,
    ) -> Result<(Self, Centinela), ErrorInventario> {
        let verificada = Self::verificar(
            anclada,
            firma,
            clave,
            Centinela::Establecido(anclada.secuencia),
            &RegistroRevocaciones::nuevo(),
        )?;
        Ok((
            verificada,
            Centinela::SinEstablecer.avanzar(anclada.secuencia),
        ))
    }

    /// Secuencia del inventario verificado.
    #[must_use]
    pub const fn secuencia(&self) -> u64 {
        self.secuencia
    }
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
        raiz: &RaizVerificada,
    ) -> Result<Self, ErrorInventario> {
        // Los eslabones 3, 4 y 5 se cerraron al construir `RaizVerificada`: no
        // existe forma de llegar aqui con una raiz sin firmar, de otro dominio o
        // revertida. Quedan los dos que dependen de ESTE marcado.

        // Eslabon 1: la prueba habla de este marcado y no de otro.
        if prueba.resumen_asiento != bruto.resumen() {
            return Err(ErrorInventario::PruebaAjenaAlMarcado);
        }

        // Eslabon 2: el marcado pertenece al inventario.
        if !verificar_inclusion(prueba, &raiz.raiz) {
            return Err(ErrorInventario::InclusionNoVerifica);
        }

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
