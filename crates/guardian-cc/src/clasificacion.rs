//! Clasificacion de dispositivo para la exclusion permanente.
//!
//! RPT-009, PA-23.
//!
//! # El hueco que cierra este modulo
//!
//! RPT-008 dejo `ClaseExcluida` y la regla de que ningun humano puede levantar
//! una exclusion. Lo que no dejo es **como se determina que un dispositivo
//! pertenece a esas clases**. Una lista de exclusion con clasificacion no fiable
//! protege sobre el papel: `evaluar` recibia la clase como parametro y nadie la
//! calculaba.
//!
//! # Por que no hay puntuacion ni umbral
//!
//! Una confianza numerica con umbral configurable es un mando que alguien bajara
//! para reducir falsos positivos, y el dia que lo baje la proteccion desaparece
//! sin que ninguna prueba falle. Ademas, una suma ponderada permite que tres
//! senales debiles y **correlacionadas** —OUI, protocolo y VLAN, las tres
//! derivadas de «es un PLC de Siemens»— superen a una senal fuerte.
//!
//! Las fuentes se ordenan por autoridad y las reglas son discretas.
//!
//! # La asimetria de la inferencia
//!
//! Una huella pasiva puede **sugerir** que un dispositivo es critico. No puede
//! demostrar que **no** lo es: una bomba de infusion y una impresora de red
//! hablan HTTP y DHCP, y muchos equipos medicos usan modulos de red comerciales,
//! con lo que el OUI apunta al fabricante del modulo.
//!
//! De ahi las tres reglas que gobiernan este modulo:
//!
//! 1. la inferencia solo mueve la clasificacion **hacia** la exclusion,
//! 2. solo un marcado humano firmado declara que un dispositivo es contenible,
//! 3. la inferencia **nunca** produce prohibicion permanente, sino
//!    [`Clasificacion::Ambiguo`], que un humano resuelve.
//!
//! La tercera importa: un falso positivo permanente e irrevocable seria un modo
//! de fallo tan malo como el que se quiere evitar.

use crate::ClaseExcluida;

/// Fuente de la que procede una evidencia de clasificacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuenteEvidencia {
    /// Marcado explicito y firmado por el administrador del cliente.
    ///
    /// Unica fuente con un humano responsable detras, y por eso la unica que
    /// puede declarar **ausencia** de criticidad.
    MarcadoAdministrativo,
    /// Naturaleza declarada de un segmento o VLAN.
    ///
    /// Es lo que hace tratable la clasificacion: decenas de segmentos en lugar
    /// de miles de equipos.
    DeclaracionDeSegmento,
    /// Protocolo o comportamiento observado pasivamente.
    HuellaPasiva,
    /// Prefijo OUI de la direccion de capa de enlace.
    OuiFabricante,
}

impl FuenteEvidencia {
    /// Todas las fuentes, en orden de autoridad decreciente.
    pub const TODAS: [Self; 4] = [
        Self::MarcadoAdministrativo,
        Self::DeclaracionDeSegmento,
        Self::HuellaPasiva,
        Self::OuiFabricante,
    ];

    /// Identificador estable, tal como figura en `contrato-contencion.toml`.
    #[must_use]
    pub const fn identificador(self) -> &'static str {
        match self {
            Self::MarcadoAdministrativo => "MarcadoAdministrativo",
            Self::DeclaracionDeSegmento => "DeclaracionDeSegmento",
            Self::HuellaPasiva => "HuellaPasiva",
            Self::OuiFabricante => "OuiFabricante",
        }
    }

    /// Indica si la fuente puede declarar que un dispositivo **no** es critico.
    ///
    /// Solo las declarativas. Ver la asimetria en la documentacion del modulo.
    #[must_use]
    pub const fn puede_declarar_no_critico(self) -> bool {
        matches!(
            self,
            Self::MarcadoAdministrativo | Self::DeclaracionDeSegmento
        )
    }
}

/// Motivo por el que una clasificacion quedo ambigua.
///
/// Se registra para que el operador sepa **que mirar**, no solo que algo falta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotivoAmbiguedad {
    /// El marcado dice una cosa y la huella observada dice otra.
    ///
    /// O el marcado esta obsoleto o el equipo fue sustituido. Ambas exigen mirar.
    ConflictoEntreFuentes,
    /// Existio un marcado y su vigencia expiro.
    ///
    /// Un marcado vencido se degrada a ausencia; no se conserva como valido.
    MarcadoCaducado,
    /// Sin marcado, en un segmento que admite equipos criticos o sin declarar.
    SegmentoPuedeAlojarCriticos,
    /// La inferencia apunta a un equipo critico sin marcado que lo confirme.
    InferenciaSugiereCriticidad,
}

impl MotivoAmbiguedad {
    /// Todos los motivos.
    pub const TODOS: [Self; 4] = [
        Self::ConflictoEntreFuentes,
        Self::MarcadoCaducado,
        Self::SegmentoPuedeAlojarCriticos,
        Self::InferenciaSugiereCriticidad,
    ];

    /// Identificador estable, tal como figura en `contrato-contencion.toml`.
    #[must_use]
    pub const fn identificador(self) -> &'static str {
        match self {
            Self::ConflictoEntreFuentes => "ConflictoEntreFuentes",
            Self::MarcadoCaducado => "MarcadoCaducado",
            Self::SegmentoPuedeAlojarCriticos => "SegmentoPuedeAlojarCriticos",
            Self::InferenciaSugiereCriticidad => "InferenciaSugiereCriticidad",
        }
    }
}

/// Naturaleza declarada de un segmento de red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaracionSegmento {
    /// El administrador declara que el segmento no aloja equipos criticos.
    ///
    /// Traslada la responsabilidad humana al nivel de segmento, donde es
    /// tratable. Sin esta figura, todo equipo sin marcar quedaria ambiguo y el
    /// producto no contendria nada nunca — teatro en la direccion contraria.
    SinDispositivosCriticos,
    /// Segmento clinico, de planta o similar.
    PuedeAlojarCriticos,
    /// Nadie declaro nada. Se trata como [`Self::PuedeAlojarCriticos`].
    NoDeclarado,
}

impl DeclaracionSegmento {
    /// Indica si el segmento puede alojar equipos criticos.
    ///
    /// La ausencia de declaracion se resuelve hacia el lado seguro.
    #[must_use]
    pub const fn admite_criticos(self) -> bool {
        !matches!(self, Self::SinDispositivosCriticos)
    }
}

/// Marcado explicito de un dispositivo por el administrador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarcadoDispositivo {
    /// Clase declarada. `None` significa «declarado no critico».
    pub clase: Option<ClaseExcluida>,
    /// Si el marcado sigue vigente.
    ///
    /// La vigencia se calcula fuera de este modulo: aqui solo se consume el
    /// veredicto, para que la logica sea probable sin reloj.
    pub vigente: bool,
}

/// Evidencia disponible sobre un dispositivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evidencia {
    /// Marcado explicito, si existe.
    pub marcado: Option<MarcadoDispositivo>,
    /// Naturaleza declarada del segmento donde se observo.
    pub segmento: DeclaracionSegmento,
    /// Clase que la inferencia pasiva sugiere, si sugiere alguna.
    ///
    /// Nunca puede valer para descartar criticidad: ver la asimetria.
    pub inferencia: Option<ClaseExcluida>,
}

/// Resultado de clasificar un dispositivo. RPT-009.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clasificacion {
    /// Se determino la clase con evidencia declarativa.
    ///
    /// `clase: None` significa «no critico, y hay un humano que lo firma».
    Clasificado {
        /// Clase excluida, o `None` si el dispositivo es contenible.
        clase: Option<ClaseExcluida>,
        /// Fuente que sostiene la clasificacion.
        fuente: FuenteEvidencia,
    },
    /// No hay evidencia de ninguna clase.
    ///
    /// Estado deliberadamente inalcanzable desde [`clasificar`]: sin evidencia,
    /// el segmento siempre aporta algo, aunque sea su ausencia de declaracion.
    /// Existe para que quien consuma este tipo no pueda asumir que la evidencia
    /// siempre llega.
    NoClasificado,
    /// La evidencia es insuficiente o se contradice.
    Ambiguo {
        /// Que mirar.
        motivo: MotivoAmbiguedad,
    },
}

impl Clasificacion {
    /// Indica si la clasificacion permite actuar sin intervencion humana.
    #[must_use]
    pub const fn permite_accion_automatica(self) -> bool {
        matches!(self, Self::Clasificado { clase: None, .. })
    }
}

/// Clasifica un dispositivo a partir de la evidencia disponible.
///
/// # Orden de resolucion
///
/// 1. Marcado vigente que declara **critico** → clasificado, aunque la
///    inferencia diga otra cosa: el humano manda para prohibir.
/// 2. Marcado vigente que declara **no critico** y la inferencia lo contradice →
///    [`MotivoAmbiguedad::ConflictoEntreFuentes`]. El humano **no** manda para
///    permitir: o el marcado esta obsoleto o el equipo fue sustituido.
/// 3. Marcado caducado → [`MotivoAmbiguedad::MarcadoCaducado`].
/// 4. Sin marcado y con inferencia que sugiere criticidad →
///    [`MotivoAmbiguedad::InferenciaSugiereCriticidad`]. Nunca prohibicion
///    permanente.
/// 5. Sin marcado, sin inferencia, en segmento que admite criticos →
///    [`MotivoAmbiguedad::SegmentoPuedeAlojarCriticos`].
/// 6. Sin marcado, sin inferencia, en segmento declarado sin criticos →
///    contenible.
///
/// La asimetria del paso 1 frente al paso 2 es el nucleo del diseno: un humano
/// puede **anadir** una prohibicion con su sola firma, pero no puede
/// **levantarla** contra la evidencia observada.
#[must_use]
pub fn clasificar(evidencia: &Evidencia) -> Clasificacion {
    if let Some(marcado) = evidencia.marcado {
        if !marcado.vigente {
            return Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::MarcadoCaducado,
            };
        }

        if marcado.clase.is_some() {
            return Clasificacion::Clasificado {
                clase: marcado.clase,
                fuente: FuenteEvidencia::MarcadoAdministrativo,
            };
        }

        // Marcado vigente que declara no critico.
        if evidencia.inferencia.is_some() {
            return Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::ConflictoEntreFuentes,
            };
        }

        return Clasificacion::Clasificado {
            clase: None,
            fuente: FuenteEvidencia::MarcadoAdministrativo,
        };
    }

    if evidencia.inferencia.is_some() {
        return Clasificacion::Ambiguo {
            motivo: MotivoAmbiguedad::InferenciaSugiereCriticidad,
        };
    }

    if evidencia.segmento.admite_criticos() {
        return Clasificacion::Ambiguo {
            motivo: MotivoAmbiguedad::SegmentoPuedeAlojarCriticos,
        };
    }

    Clasificacion::Clasificado {
        clase: None,
        fuente: FuenteEvidencia::DeclaracionDeSegmento,
    }
}
