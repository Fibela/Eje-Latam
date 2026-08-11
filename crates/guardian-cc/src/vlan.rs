//! Declaracion firmada de la naturaleza de cada segmento.
//!
//! RPT-022, PA-45.
//!
//! # El hueco que cierra
//!
//! `DeclaracionSegmento` existe desde RPT-009 y `clasificar` la consume: es la
//! **unica fuente declarativa que no exige marcar dispositivo por dispositivo**,
//! y por tanto la unica que hace tratable un parque de miles de equipos.
//!
//! Pero nadie la producia. `eje-agente` traia esto:
//!
//! ```text
//! match vlan {
//!     None => DeclaracionSegmento::NoDeclarado,
//!     Some(_) => DeclaracionSegmento::PuedeAlojarCriticos,
//! }
//! ```
//!
//! Es decir: **ninguna VLAN podia declararse limpia**, luego ningun dispositivo
//! sin marcado era contenible jamas. El mecanismo estaba entero y sin cablear —el
//! mismo defecto que ya aparecio en `disco.rs`, en `ArchivoRevocaciones` y en los
//! centinelas de alerta—.
//!
//! # Por que la declaracion viaja **dentro** del manifiesto firmado
//!
//! Un fichero de configuracion aparte seria editable sin romper ninguna firma, y
//! la edicion mas util para un atacante es de una sola linea: declarar limpia la
//! VLAN clinica. Eso convierte a todo equipo sin marcado de ese segmento en
//! contenible.
//!
//! Por eso la tabla se resume y **su resumen entra en el mismo mensaje que firma
//! el administrador**, junto a la raiz Merkle y la secuencia. Alterar un solo
//! registro cambia el resumen y la firma de la raiz deja de verificar. Es el
//! mismo argumento por el que RPT-010 §4 firma la raiz y no cada entrada.
//!
//! # La ausencia de registro **es** la ausencia de declaracion
//!
//! [`NaturalezaSegmento`] tiene dos variantes y no tres. `NoDeclarado` no se
//! codifica: una VLAN sin registro no esta declarada, y punto.
//!
//! Si existiera un codigo para «no declarado», el mismo estado tendria dos
//! representaciones —registro ausente y registro presente con ese codigo— y
//! volveria la ambiguedad que el rechazo de bytes sobrantes y
//! `deny_unknown_fields` cierran en el resto del proyecto.
//!
//! # Lo que este mecanismo **no** resuelve
//!
//! La etiqueta VLAN de una trama es la palabra del conmutador solo si el agente
//! observa un espejo de puertos de acceso: alli el equipo reescribe la etiqueta y
//! el emisor no la elige. Sobre un espejo de **troncal**, quien esta en el cable
//! puede etiquetar lo que quiera.
//!
//! La asimetria es la misma que la de la direccion de enlace en
//! [`ProveedorInventario`](crate::proveedores::ProveedorInventario), y conviene
//! tenerla escrita porque su direccion sorprende:
//!
//! - Fingir estar en una VLAN **limpia** te vuelve contenible. Nadie lo hace.
//! - Fingir estar en una VLAN **critica** te vuelve ambiguo, y por tanto
//!   incontenible sin un humano. **Esa** es la jugada.
//!
//! Luego la declaracion de segmento no es un vector para contener de mas: es un
//! vector de **evasion**, igual que la suplantacion de MAC. No se corrige aqui —no
//! puede—; se mitiga con espejo de acceso y con que ninguna prohibicion sea
//! silenciosa.

use eje_almacen::resumen::{Absorbedor, Resumen};

use crate::clasificacion::DeclaracionSegmento;
use crate::inventario::RaizVerificada;

/// Dominio del resumen de la tabla de segmentos.
///
/// Separado del de marcados y del de raiz: sin etiqueta propia, un resumen de
/// tabla podria presentarse donde se espera otro.
const DOMINIO_TABLA_VLAN: &[u8] = b"eje-latam/agt-01/tabla-vlan/v1";

/// Primer identificador de VLAN declarable.
///
/// El 0 queda fuera a proposito. En 802.1Q el VID 0 significa «trama etiquetada
/// solo por prioridad, sin pertenencia a VLAN», y `eje-captura` lo entrega como
/// `Some(0)` porque enmascara los doce bits bajos. Admitir su declaracion
/// permitiria escribir «el segmento 0 esta limpio» y con ello conceder
/// contencion automatica a cualquiera que emita tramas con prioridad y sin VLAN,
/// que es una condicion trivial de cumplir.
pub const VLAN_MINIMA: u16 = 1;

/// Ultimo identificador de VLAN declarable.
///
/// El 4095 esta reservado por la norma para uso de implementacion.
pub const VLAN_MAXIMA: u16 = 4094;

/// Numero maximo de declaraciones en una tabla.
///
/// Es el tamano del espacio declarable. Con orden estricto y sin duplicados no
/// se puede superar; se declara igualmente para acotar la reserva **antes** de
/// leer el bloque.
pub const VLANS_MAXIMAS: usize = (VLAN_MAXIMA - VLAN_MINIMA + 1) as usize;

/// Naturaleza declarada de un segmento.
///
/// Dos variantes, no tres: ver el encabezado del modulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalezaSegmento {
    /// El administrador declara que el segmento no aloja equipos criticos.
    SinDispositivosCriticos,
    /// Segmento clinico, de planta o similar.
    PuedeAlojarCriticos,
}

impl NaturalezaSegmento {
    /// Todas las naturalezas declarables.
    pub const TODAS: [Self; 2] = [Self::SinDispositivosCriticos, Self::PuedeAlojarCriticos];

    /// Identificador estable, tal como figura en `contrato-contencion.toml`.
    #[must_use]
    pub const fn identificador(self) -> &'static str {
        match self {
            Self::SinDispositivosCriticos => "SinDispositivosCriticos",
            Self::PuedeAlojarCriticos => "PuedeAlojarCriticos",
        }
    }

    /// Codigo escalar en disco.
    ///
    /// Empieza en 1 a proposito: con el 0 reservado, un bloque de ceros —relleno,
    /// fichero recien creado, sector sin escribir— no se analiza como una tabla
    /// de declaraciones validas.
    #[must_use]
    pub const fn codigo(self) -> u8 {
        match self {
            Self::SinDispositivosCriticos => 1,
            Self::PuedeAlojarCriticos => 2,
        }
    }

    /// Naturaleza a partir de su codigo escalar.
    #[must_use]
    pub const fn desde_codigo(codigo: u8) -> Option<Self> {
        match codigo {
            1 => Some(Self::SinDispositivosCriticos),
            2 => Some(Self::PuedeAlojarCriticos),
            _ => None,
        }
    }

    /// Traduce a la declaracion que consume [`clasificar`](crate::clasificacion::clasificar).
    #[must_use]
    pub const fn a_declaracion(self) -> DeclaracionSegmento {
        match self {
            Self::SinDispositivosCriticos => DeclaracionSegmento::SinDispositivosCriticos,
            Self::PuedeAlojarCriticos => DeclaracionSegmento::PuedeAlojarCriticos,
        }
    }
}

/// Defectos de una tabla de declaraciones.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorVlan {
    /// El identificador queda fuera del rango declarable.
    #[error("la vlan {vlan} esta fuera del rango declarable {VLAN_MINIMA}..={VLAN_MAXIMA}")]
    VlanFueraDeRango {
        /// Identificador rechazado.
        vlan: u16,
    },

    /// La misma VLAN se declara dos veces.
    ///
    /// Se rechaza en lugar de elegir una: cualquier eleccion es arbitraria y una
    /// de ellas favorece a quien anade un segundo registro «limpio».
    #[error("la vlan {vlan} se declara dos veces")]
    VlanDuplicada {
        /// Identificador repetido.
        vlan: u16,
    },

    /// La tabla no es la que el manifiesto firmado ancla.
    ///
    /// Es el eslabon que ata la tabla a la firma. Sin el, la tabla seria un
    /// fichero de configuracion cualquiera.
    #[error("la tabla de segmentos no corresponde al resumen anclado y firmado")]
    TablaAjenaAlManifiesto,
}

/// Declaracion de un segmento, **sin verificar**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaracionVlan {
    /// Identificador de VLAN, en `VLAN_MINIMA..=VLAN_MAXIMA`.
    pub vlan: u16,
    /// Naturaleza declarada.
    pub naturaleza: NaturalezaSegmento,
    /// Instante de emision, en segundos desde la epoca.
    pub emitido_en: u64,
    /// Vigencia declarada, en dias.
    pub vigencia_dias: u32,
}

impl DeclaracionVlan {
    /// Indica si la declaracion sigue vigente en el instante dado.
    ///
    /// # Por que caduca
    ///
    /// La declaracion peligrosa es «este segmento esta limpio»: es la que
    /// concede contencion automatica. Y es justo la que envejece mal, porque el
    /// dia que alguien conecte un carro de telemedicina a la VLAN administrativa
    /// nadie va a volver a emitir el manifiesto para corregirla.
    ///
    /// La politica de reloj es la de
    /// [`MarcadoVerificado::vigente_en`](crate::inventario::MarcadoVerificado::vigente_en),
    /// deliberadamente identica: ante duda, **caducada**. Un `ahora` anterior a
    /// la emision —reloj atrasado o fecha futura— tambien cuenta como caducada.
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

/// Tabla de declaraciones en orden canonico, **sin verificar**.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TablaVlan {
    declaraciones: Vec<DeclaracionVlan>,
}

impl TablaVlan {
    /// Tabla sin ninguna declaracion.
    ///
    /// Es un estado legitimo y frecuente: un cliente que aun no ha declarado sus
    /// segmentos. Su resumen esta definido y es distinto del de cualquier tabla
    /// con contenido, asi que «sin declarar» tambien queda firmado.
    #[must_use]
    pub const fn vacia() -> Self {
        Self {
            declaraciones: Vec::new(),
        }
    }

    /// Construye la tabla en orden canonico.
    ///
    /// Ordena por identificador ascendente y rechaza duplicados y fuera de rango.
    ///
    /// # Errores
    ///
    /// [`ErrorVlan::VlanFueraDeRango`] o [`ErrorVlan::VlanDuplicada`].
    pub fn construir(mut declaraciones: Vec<DeclaracionVlan>) -> Result<Self, ErrorVlan> {
        for declaracion in &declaraciones {
            if declaracion.vlan < VLAN_MINIMA || declaracion.vlan > VLAN_MAXIMA {
                return Err(ErrorVlan::VlanFueraDeRango {
                    vlan: declaracion.vlan,
                });
            }
        }

        declaraciones.sort_unstable_by_key(|declaracion| declaracion.vlan);

        for par in declaraciones.windows(2) {
            if par[0].vlan == par[1].vlan {
                return Err(ErrorVlan::VlanDuplicada { vlan: par[0].vlan });
            }
        }

        Ok(Self { declaraciones })
    }

    /// Declaraciones en orden canonico.
    #[must_use]
    pub fn declaraciones(&self) -> &[DeclaracionVlan] {
        &self.declaraciones
    }

    /// Resumen canonico de la tabla.
    ///
    /// El numero de declaraciones se absorbe **antes** que las declaraciones. Sin
    /// el, una tabla vacia y la ausencia de tabla producirian el mismo resumen y
    /// borrar el bloque entero no romperia la firma.
    #[must_use]
    pub fn resumen(&self) -> Resumen {
        let mut absorbedor = Absorbedor::nuevo(DOMINIO_TABLA_VLAN);
        absorbedor.entero(self.declaraciones.len() as u64);

        for declaracion in &self.declaraciones {
            absorbedor
                .entero(u64::from(declaracion.vlan))
                .entero(u64::from(declaracion.naturaleza.codigo()))
                .entero(declaracion.emitido_en)
                .entero(u64::from(declaracion.vigencia_dias));
        }

        absorbedor.finalizar()
    }

    /// Declaracion de una VLAN concreta, si figura.
    #[must_use]
    pub fn declaracion_de(&self, vlan: u16) -> Option<&DeclaracionVlan> {
        self.declaraciones
            .binary_search_by_key(&vlan, |declaracion| declaracion.vlan)
            .ok()
            .map(|posicion| &self.declaraciones[posicion])
    }
}

/// Tabla cuyo resumen coincide con el que el administrador firmo.
///
/// # Invariante
///
/// El campo es privado y no hay constructor publico salvo
/// [`Self::verificar_e_instanciar`]. Un valor de este tipo **es** una tabla
/// anclada a una [`RaizVerificada`], del mismo modo que un
/// [`MarcadoVerificado`](crate::inventario::MarcadoVerificado) lo esta a su
/// prueba de inclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TablaVlanVerificada {
    tabla: TablaVlan,
}

impl TablaVlanVerificada {
    /// Unica via de construccion.
    ///
    /// # Errores
    ///
    /// [`ErrorVlan::TablaAjenaAlManifiesto`] si el resumen recalculado no es el
    /// que la raiz verificada ancla.
    pub fn verificar_e_instanciar(
        tabla: TablaVlan,
        raiz: &RaizVerificada,
    ) -> Result<Self, ErrorVlan> {
        if tabla.resumen() != raiz.vlans() {
            return Err(ErrorVlan::TablaAjenaAlManifiesto);
        }

        Ok(Self { tabla })
    }

    /// Tabla verificada subyacente.
    #[must_use]
    pub const fn tabla(&self) -> &TablaVlan {
        &self.tabla
    }

    /// Declaracion efectiva para la etiqueta observada.
    ///
    /// # Los tres caminos a `NoDeclarado`
    ///
    /// 1. **Trama sin etiqueta.** Un espejo sin marcar no dice de donde viene.
    /// 2. **VLAN sin registro.** El administrador no declaro ese segmento.
    /// 3. **Declaracion caducada.** Ver [`DeclaracionVlan::vigente_en`].
    ///
    /// Los tres degradan hacia el mismo sitio y en la direccion segura:
    /// `NoDeclarado` admite criticos (RPT-009 §5), asi que ninguno de ellos
    /// concede contencion. Solo una declaracion **presente, en rango y vigente**
    /// puede declarar limpio un segmento.
    #[must_use]
    pub fn declaracion_para(&self, vlan: Option<u16>, ahora: u64) -> DeclaracionSegmento {
        let Some(vlan) = vlan else {
            return DeclaracionSegmento::NoDeclarado;
        };

        let Some(declaracion) = self.tabla.declaracion_de(vlan) else {
            return DeclaracionSegmento::NoDeclarado;
        };

        if !declaracion.vigente_en(ahora) {
            return DeclaracionSegmento::NoDeclarado;
        }

        declaracion.naturaleza.a_declaracion()
    }
}
