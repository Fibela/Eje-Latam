//! Esquema del registro de evidencia ALM-01.
//!
//! Las clases de evento son cerradas y estables: un registro forense cuyo
//! vocabulario cambie sin control pierde comparabilidad entre despliegues y
//! entre versiones.

/// Clase de evento registrable en ALM-01.
///
/// Cada variante corresponde a una accion que un reporte canonico declara
/// registrable. Anadir una variante es un cambio de esquema y exige enmienda
/// documentada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClaseEvento {
    /// El agente arranco. Delimita periodos de observacion continua.
    ArranqueAgente,
    /// `guardian-cc` observo un comportamiento anomalo en un nodo IoT/OT.
    DeteccionAnomalia,
    /// Se ejecuto una orden de contencion sobre un nodo.
    OrdenContencion,
    /// Se rechazo una orden por proceder de un simulacro (RPT-003 §8.1).
    RechazoSimulacion,
    /// Cambio de configuracion del agente o de la capa de red.
    CambioConfiguracion,
    /// Se aplico una actualizacion firmada.
    ActualizacionAplicada,
    /// Se rechazo un modulo por firma invalida (RPT-004 §5.2).
    FirmaModuloRechazada,
    /// La Boveda Aislada alcanzo su limite y descarto eventos.
    BovedaDesbordada,
    /// Uso de funciones empresariales en periodo de gracia (RPT-003 §3.4).
    UsoEnGracia,
    /// Se sello un rango de asientos con una raiz Merkle.
    SelloEmitido,
    /// La escritura a disco fallo durante un tramo y se restablecio.
    ///
    /// RPT-044, PA-69. **Se anexa al recuperar, no al fallar**, y esa es toda la
    /// diferencia: un evento anexado durante el fallo iria al registro que no se
    /// puede escribir, y moriria con el proceso igual que las alertas que
    /// pretende explicar. Anadir bytes a un disco lleno ademas empeora el
    /// siguiente intento.
    ///
    /// Al recuperar, en cambio, el disco funciona por definicion, y el asiento
    /// puede describir el tramo entero: desde cuando, cuantas vueltas, cuantos
    /// asientos estuvieron solo en memoria.
    PersistenciaRestablecida,
}

impl ClaseEvento {
    /// Identificador estable de la clase, usado en el resumen y en la base.
    ///
    /// Estos literales forman parte del formato de evidencia: cambiarlos
    /// invalida los resumenes de todo registro existente.
    #[must_use]
    pub const fn identificador(self) -> &'static str {
        match self {
            Self::ArranqueAgente => "arranque-agente",
            Self::DeteccionAnomalia => "deteccion-anomalia",
            Self::OrdenContencion => "orden-contencion",
            Self::RechazoSimulacion => "rechazo-simulacion",
            Self::CambioConfiguracion => "cambio-configuracion",
            Self::ActualizacionAplicada => "actualizacion-aplicada",
            Self::FirmaModuloRechazada => "firma-modulo-rechazada",
            Self::BovedaDesbordada => "boveda-desbordada",
            Self::UsoEnGracia => "uso-en-gracia",
            Self::SelloEmitido => "sello-emitido",
            Self::PersistenciaRestablecida => "persistencia-restablecida",
        }
    }

    /// Recupera la clase a partir de su identificador estable.
    #[must_use]
    pub fn desde_identificador(texto: &str) -> Option<Self> {
        const TODAS: [ClaseEvento; 11] = [
            ClaseEvento::ArranqueAgente,
            ClaseEvento::DeteccionAnomalia,
            ClaseEvento::OrdenContencion,
            ClaseEvento::RechazoSimulacion,
            ClaseEvento::CambioConfiguracion,
            ClaseEvento::ActualizacionAplicada,
            ClaseEvento::FirmaModuloRechazada,
            ClaseEvento::BovedaDesbordada,
            ClaseEvento::UsoEnGracia,
            ClaseEvento::SelloEmitido,
            ClaseEvento::PersistenciaRestablecida,
        ];
        TODAS
            .into_iter()
            .find(|clase| clase.identificador() == texto)
    }
}

/// Definicion SQL de ALM-01.
///
/// El registro es de solo anexado. La ausencia de `UPDATE` y `DELETE` en la
/// interfaz no basta: la autorizacion se aplica ademas en [`crate::autorizar`], y
/// la integridad se verifica con la cadena de resumenes. Tres capas, porque la
/// evidencia solo vale si ninguna de ellas puede saltarse en solitario.
pub const DDL_EVIDENCIA: &str = "\
CREATE TABLE IF NOT EXISTS evidencia (
    numero            INTEGER PRIMARY KEY,
    instante_utc      INTEGER NOT NULL,
    clase             TEXT    NOT NULL,
    nodo              TEXT    NOT NULL,
    detalle           TEXT    NOT NULL,
    resumen_anterior  BLOB    NOT NULL,
    resumen_propio    BLOB    NOT NULL UNIQUE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_evidencia_instante ON evidencia (instante_utc);
CREATE INDEX IF NOT EXISTS idx_evidencia_clase    ON evidencia (clase);
CREATE INDEX IF NOT EXISTS idx_evidencia_nodo     ON evidencia (nodo);

CREATE TABLE IF NOT EXISTS sellos (
    desde  INTEGER NOT NULL,
    hasta  INTEGER NOT NULL,
    raiz   BLOB    NOT NULL,
    PRIMARY KEY (desde, hasta)
) STRICT;
";

/// Definicion SQL de ALM-02, el sandbox del analista.
///
/// Se inicializa con una vista de solo lectura sobre la evidencia. El analista
/// puede crear y destruir sus propias tablas sin tocar el registro.
pub const DDL_SANDBOX: &str = "\
CREATE TABLE IF NOT EXISTS evidencia_copia (
    numero        INTEGER PRIMARY KEY,
    instante_utc  INTEGER NOT NULL,
    clase         TEXT    NOT NULL,
    nodo          TEXT    NOT NULL,
    detalle       TEXT    NOT NULL
) STRICT;
";
