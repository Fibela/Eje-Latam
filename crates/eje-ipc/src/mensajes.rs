//! Cargas útiles de los canales del puente.
//!
//! RPT-006, PA-21. Las formas se declaran en `contrato-ipc.toml`; aquí se
//! implementan, y `pruebas.rs` comprueba que ambas coincidan.
//!
//! # `deny_unknown_fields` no es opcional
//!
//! Sin él, un mensaje con campos sobrantes se acepta en silencio. Eso convierte
//! un cambio de contrato no coordinado —o un renderer comprometido enviando
//! basura extra— en algo que el deserializador aprueba.
//!
//! Es la mitad de la asimetría documentada en el manifiesto: **el rigor de Rust
//! vive en el deserializador**. TypeScript no tiene equivalente natural, y por
//! eso su rigor vive en la prueba de paridad.
//!
//! # Nombres en el cable
//!
//! camelCase, porque el extremo TypeScript los consume tal cual. El mapeo se
//! hace con `rename_all`, no renombrando campos en Rust: los identificadores
//! siguen la convención del lenguaje.

use serde::{Deserialize, Serialize};

/// Perfil del segmento vigilado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PerfilSegmento {
    /// Red corporativa de propósito general.
    Corporativo,
    /// Red industrial u hospitalaria.
    Ot,
}

/// Clasificación de un dispositivo descubierto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaseDispositivo {
    /// Controlador lógico programable.
    Plc,
    /// Cámara de red.
    Camara,
    /// Equipamiento médico.
    Medico,
    /// Estación de trabajo.
    Estacion,
    /// No fue posible clasificarlo.
    Desconocido,
}

/// Postura de confianza cero evaluada para un nodo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Postura {
    /// Comportamiento dentro de perfil.
    Conforme,
    /// Comportamiento anómalo detectado.
    Anomalo,
    /// Nodo actualmente contenido.
    Contenido,
}

/// Estado resumido del demonio local. Respuesta de `obtener-estado-agente`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EstadoAgente {
    /// Versión del agente en ejecución.
    pub version: String,
    /// Perfil del segmento vigilado.
    pub perfil: PerfilSegmento,
    /// Si la respuesta automática está habilitada según vigencia de reglas.
    pub respuesta_automatica: bool,
}

/// Dispositivo IoT/OT descubierto. Elemento de `obtener-inventario`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodoInventario {
    /// Identificador estable del nodo.
    pub identificador: String,
    /// Dirección de capa de enlace observada.
    pub direccion_enlace: String,
    /// Clasificación del dispositivo.
    pub clase: ClaseDispositivo,
    /// Postura evaluada.
    pub postura: Postura,
}

/// Ocupación de la Bóveda Aislada. Respuesta de `obtener-estado-boveda`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EstadoBoveda {
    /// Bytes ocupados por la cola de eventos pendientes.
    pub usado_bytes: u64,
    /// Límite configurado en bytes.
    pub limite_bytes: u64,
    /// Eventos pendientes de reconciliación.
    pub eventos_pendientes: u64,
}

/// Consulta dirigida al sandbox del analista. Petición de `consultar-sandbox`.
///
/// Opera **solo contra ALM-02**. El registro de evidencia ALM-01 no es
/// alcanzable desde la interfaz (RPT-002 §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeticionConsulta {
    /// Sentencia SQL a ejecutar contra ALM-02.
    pub sentencia: String,
}

/// Filas devueltas por el sandbox. Respuesta de `consultar-sandbox`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultadoConsulta {
    /// Nombres de columna devueltos.
    pub columnas: Vec<String>,
    /// Filas devueltas, en el orden de `columnas`.
    pub filas: Vec<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Descripción declarativa, para la prueba de paridad
// ---------------------------------------------------------------------------
//
// Estas constantes NO son una tercera declaración independiente: `pruebas.rs`
// las ata a los structs mediante desestructuración exhaustiva, que deja de
// compilar si se añade o se quita un campo. La cadena queda:
//
//   contrato-ipc.toml  <-- prueba de paridad -->  CAMPOS_*
//   CAMPOS_*           <-- compilador        -->  struct

/// Campos de [`EstadoAgente`], en orden y con su tipo del manifiesto.
pub const CAMPOS_ESTADO_AGENTE: [(&str, &str); 3] = [
    ("version", "texto"),
    ("perfil", "enumerado"),
    ("respuestaAutomatica", "booleano"),
];

/// Campos de [`NodoInventario`].
pub const CAMPOS_NODO_INVENTARIO: [(&str, &str); 4] = [
    ("identificador", "texto"),
    ("direccionEnlace", "texto"),
    ("clase", "enumerado"),
    ("postura", "enumerado"),
];

/// Campos de [`EstadoBoveda`].
pub const CAMPOS_ESTADO_BOVEDA: [(&str, &str); 3] = [
    ("usadoBytes", "entero"),
    ("limiteBytes", "entero"),
    ("eventosPendientes", "entero"),
];

/// Campos de [`PeticionConsulta`].
pub const CAMPOS_PETICION_CONSULTA: [(&str, &str); 1] = [("sentencia", "texto")];

/// Campos de [`ResultadoConsulta`].
pub const CAMPOS_RESULTADO_CONSULTA: [(&str, &str); 2] = [
    ("columnas", "lista<texto>"),
    ("filas", "lista<lista<texto>>"),
];

// ---------------------------------------------------------------------------
// Alertas — RPT-019
// ---------------------------------------------------------------------------

/// Clase de suceso de alerta.
///
/// # Por que hay una sola variante
///
/// De los tres centinelas de RPT-019 §1, **solo uno es un suceso**: los otros
/// dos son condiciones y viajan en [`Condiciones`]. Anadir aqui variantes para
/// ellos convertiria un estado persistente en una lluvia de sucesos repetidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClaseAlerta {
    /// Se detecto una amenaza sobre un dispositivo que no puede contenerse.
    ///
    /// Lo mas urgente que este producto puede comunicar: no existe accion
    /// automatica posible y la unica respuesta es humana (RPT-010 §6.1).
    AmenazaIncontenible,
}

/// Punto del registro desde el que se piden las alertas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeticionAlertas {
    /// Numero de asiento desde el que continuar, exclusivo.
    pub desde_asiento: u64,
}

/// Suceso de alerta ya anexado a ALM-01.
///
/// Lleva su numero de asiento para que quien consulte pueda continuar desde
/// donde lo dejo sin recibir lo mismo dos veces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SucesoAlerta {
    /// Asiento de ALM-01 que la contiene.
    pub asiento: u64,
    /// Que ocurrio.
    pub clase: ClaseAlerta,
    /// Dispositivo implicado.
    pub dispositivo: String,
    /// Contexto para el operador.
    pub detalle: String,
}

/// Estados degradados vigentes.
///
/// # Condiciones, no sucesos
///
/// Son verdaderas hasta que alguien interviene, asi que no se anexan al
/// registro: se consultan. Anotarlas repetidamente inundaria ALM-01 con la misma
/// noticia (RPT-019 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Condiciones {
    /// Habia inventario y ya no esta (RPT-017 §2).
    pub inventario_suprimido: bool,
    /// El inventario esta presente y no supera la verificacion.
    pub inventario_no_verifica: bool,
    /// La mitad pegajosa del almacen se lleno (RPT-018 §6).
    pub observacion_saturada: bool,
    /// La captura perdio tramas y la vista de la red esta incompleta.
    pub captura_con_perdida: bool,
}

impl Condiciones {
    /// Indica si alguna condicion degradada esta vigente.
    #[must_use]
    pub const fn hay_degradacion(&self) -> bool {
        self.inventario_suprimido
            || self.inventario_no_verifica
            || self.observacion_saturada
            || self.captura_con_perdida
    }
}

/// Campos de [`PeticionAlertas`].
pub const CAMPOS_PETICION_ALERTAS: [(&str, &str); 1] = [("desdeAsiento", "entero")];

/// Campos de [`SucesoAlerta`].
pub const CAMPOS_SUCESO_ALERTA: [(&str, &str); 4] = [
    ("asiento", "entero"),
    ("clase", "enumerado"),
    ("dispositivo", "texto"),
    ("detalle", "texto"),
];

/// Campos de [`Condiciones`].
pub const CAMPOS_CONDICIONES: [(&str, &str); 4] = [
    ("inventarioSuprimido", "booleano"),
    ("inventarioNoVerifica", "booleano"),
    ("observacionSaturada", "booleano"),
    ("capturaConPerdida", "booleano"),
];
