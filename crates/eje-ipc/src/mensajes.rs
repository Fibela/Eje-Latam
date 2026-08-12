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
    /// **No hay captura en absoluto**: este sensor no esta observando.
    ///
    /// RPT-047, PA-81.
    ///
    /// # Por que no es `captura_con_perdida`
    ///
    /// Aquella dice «observo y se me escapan tramas»; la vista es **incompleta**.
    /// Esta dice que no hay vista. Colapsarlas seria afirmar que se ve mal
    /// cuando no se ve nada, y es como un operador concluye que en ese segmento
    /// no paso nada (RPT-036 §6).
    ///
    /// # Por que el motivo no viaja aqui
    ///
    /// Privilegios, interfaz inexistente e interfaz desaparecida en marcha son
    /// tres remedios distintos, y el motivo importa. Pero `Condiciones` describe
    /// **lo que es verdad ahora**, con una forma uniforme que seis sitios
    /// comprueban; un texto que carece de sentido mientras la condicion es falsa
    /// no pertenece a esa forma.
    ///
    /// El motivo viaja donde se diagnostica: en el asiento de ALM-01 que anota
    /// la transicion y en la linea de syslog. VIS-04 muestra que no se observa;
    /// el porque esta a un clic, en el registro.
    ///
    /// # Y por que esto no puede parecer un agente sano
    ///
    /// Un proceso muerto lo reinicia el supervisor y alguien se entera. Un
    /// agente vivo que no observa puede pasar por bueno durante meses. Por eso
    /// esta condicion **se emite tambien por syslog con la gravedad mas alta**:
    /// que el sensor deje de mirar es un incidente, no un aviso.
    pub captura_no_disponible: bool,
    /// El almacen exige una accion del administrador, sin indicio de ataque.
    ///
    /// # Por que no cabe en las otras cuatro
    ///
    /// Cubre `FormatoObsoleto` (RPT-022) y `SinClaveAprovisionada` (RPT-024):
    /// dos estados que **exigen alerta y no son manipulacion**. Su remedio es
    /// reemitir o aprovisionar, no responder a un incidente.
    ///
    /// Mezclarlos con la supresion o la firma rota produciria exactamente la
    /// fatiga de alertas que la Fase 1 de PA-45 existia para evitar: un operador
    /// que aprendio a ignorar esta aviso la ignorara el dia que sea un ataque.
    pub accion_administrativa: bool,
    /// El colector de syslog no responde y la alerta no sale del equipo.
    ///
    /// # La unica condicion que no se puede emitir
    ///
    /// Las otras cinco viajan tambien por syslog hacia el SIEM del cliente. Esta
    /// no: emitirla exigiria el canal que acaba de fallar. Llega solo por este
    /// puente, que es donde VIS-04 la consulta.
    ///
    /// Existe porque un canal de alertas caido y uno silencioso son
    /// indistinguibles desde fuera, y RPT-006 §4 obliga a distinguirlos.
    pub salida_no_disponible: bool,
    /// El registro de evidencia esta lleno y **ya no admite alertas**.
    ///
    /// RPT-039 §1, PA-72.
    ///
    /// # Por que es la mas grave de las siete
    ///
    /// Las otras seis dicen que algo se ve peor. Esta dice que **este sensor ha
    /// dejado de registrar amenazas**: la alerta se calcula, no cabe en ALM-01 y
    /// se pierde. Un vigilante que no toma nota no es un vigilante degradado, es
    /// uno que no esta.
    ///
    /// No es manipulacion: nadie toco nada, el agente trabajo demasiado tiempo.
    /// Antes de PA-72 se disfrazaba de manipulacion, que era lo peor de los dos
    /// mundos — mandaba al operador a investigar un ataque inexistente mientras
    /// el sensor seguia sin registrar.
    pub registro_saturado: bool,
    /// Hay alertas anexadas que **solo viven en memoria**.
    ///
    /// RPT-044, PA-69. La escritura a disco fallo y el registro en memoria va por
    /// delante del que hay en el fichero. Las alertas existen y estan completas;
    /// lo que no esta es su durabilidad.
    ///
    /// # Se apaga sola, y por eso no basta
    ///
    /// Es una condicion: deja de ser cierta en cuanto el disco vuelve y el
    /// reintento escribe. Un fallo de dos segundos puede no verse en ninguna
    /// consulta. La constancia duradera de que hubo un tramo en riesgo es el
    /// asiento `persistencia-restablecida`, que se anexa al recuperar.
    pub evidencia_en_riesgo: bool,
}

impl Condiciones {
    /// Indica si alguna condicion degradada esta vigente.
    #[must_use]
    pub const fn hay_degradacion(&self) -> bool {
        self.inventario_suprimido
            || self.inventario_no_verifica
            || self.observacion_saturada
            || self.captura_con_perdida
            || self.captura_no_disponible
            || self.accion_administrativa
            || self.salida_no_disponible
            || self.registro_saturado
            || self.evidencia_en_riesgo
    }

    /// Indica si alguna condicion sugiere que **alguien toco el almacen**.
    ///
    /// Se separa de [`Self::hay_degradacion`] por la misma razon que
    /// `EstadoArranque::es_manipulacion` se separo de `exige_alerta`: quien
    /// consuma esto debe poder presentar de forma distinta «hay que reemitir el
    /// inventario» y «alguien borro el inventario».
    #[must_use]
    pub const fn hay_manipulacion(&self) -> bool {
        self.inventario_suprimido || self.inventario_no_verifica
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

/// Respuesta de `consultar-alertas`.
///
/// RPT-041, PA-74.
///
/// # Por que no es un array desnudo
///
/// Lo era, y tras la segmentacion de PA-59 se volvio una **vista parcial con
/// apariencia de exhaustividad**: quien pedia `desdeAsiento: 0` recibia las
/// alertas del segmento activo y no tenia forma de saber que habia diez mil
/// asientos archivados antes.
///
/// «No hay nada» y «esto no empieza aqui» no son lo mismo, y colapsarlos es como
/// un operador concluye que un incidente no ocurrio (RPT-006 §4).
///
/// # Por que no es un error
///
/// Pedir desde el cero es una peticion legitima y la respuesta es **correcta**:
/// las alertas devueltas existen y son exactas. Lo que faltaba no era validez,
/// era contexto. Devolver `AsientoFueraDeRango` habria dejado al cliente sin las
/// alertas vivas y adivinando desplazamientos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RespuestaAlertas {
    /// Numero de asiento mas antiguo que **sobrevive en disco**.
    ///
    /// No es «lo que este canal alcanza»: es lo que hay. Un cliente que pidio
    /// desde el cero y recibe 10001 sabe que esos diez mil existieron y estan
    /// archivados — no perdidos, no inexistentes.
    pub primer_disponible: u64,
    /// Alertas que caben en esta respuesta, desde `desdeAsiento` exclusive.
    pub sucesos: Vec<SucesoAlerta>,
}

/// Campos de [`RespuestaAlertas`].
pub const CAMPOS_RESPUESTA_ALERTAS: [(&str, &str); 2] =
    [("primerDisponible", "entero"), ("sucesos", "lista")];

/// Campos de [`Condiciones`].
pub const CAMPOS_CONDICIONES: [(&str, &str); 9] = [
    ("inventarioSuprimido", "booleano"),
    ("inventarioNoVerifica", "booleano"),
    ("observacionSaturada", "booleano"),
    ("capturaConPerdida", "booleano"),
    ("capturaNoDisponible", "booleano"),
    ("accionAdministrativa", "booleano"),
    ("salidaNoDisponible", "booleano"),
    ("registroSaturado", "booleano"),
    ("evidenciaEnRiesgo", "booleano"),
];
