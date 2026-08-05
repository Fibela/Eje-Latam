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
