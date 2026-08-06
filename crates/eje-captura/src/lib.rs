//! # Eje-Captura — captura pasiva de trafico
//!
//! RPT-018, PA-37.
//!
//! ## Por que este crate existe aparte
//!
//! `guardian-cc` declara `#![forbid(unsafe_code)]` y debe seguir declarandolo:
//! es lo primero que lee un auditor, y su garantia de seguridad de memoria no
//! puede depender de revisar un modulo de sockets.
//!
//! AF_PACKET exige FFI. Luego el `unsafe` vive aqui, confinado a un solo modulo,
//! y este crate **no depende de `guardian-cc`**: emite observaciones, no
//! clasifica.
//!
//! Son dos posturas de auditoria distintas. `guardian-cc` se audita leyendo
//! logica; `eje-captura`, leyendo llamadas al nucleo.
//!
//! ## Pasivo por tipo
//!
//! RPT-002 §9.2 prohibe emitir trafico en perfil OT. Aqui eso no es una regla
//! que alguien deba recordar: **[`FuentePasiva`] no expone forma alguna de
//! transmitir**. No es que no llamemos a `send`; es que no hay `send` que
//! llamar.
//!
//! Un socket que puede transmitir y no transmite depende de que nadie escriba la
//! linea. Uno que no puede, no.
//!
//! ## El descarte tiene que ser visible
//!
//! Una captura sobre red cargada descarta tramas. Eso es normal. Lo que no puede
//! ser es descartarlas en silencio: el clasificador veria menos protocolos,
//! inferiria menos criticidad, y la ausencia de indicio se leeria como ausencia
//! de riesgo — un guardian que informa verde porque no miro (RPT-006 §4).
//!
//! Por eso [`Estadisticas`] forma parte de la interfaz y no es telemetria
//! opcional.

#![deny(unsafe_code)]

#[cfg(target_os = "linux")]
mod linux;

pub mod transporte;

use std::time::Duration;

/// Direccion de capa de enlace.
pub type DireccionEnlace = [u8; 6];

/// Longitud maxima de trama que se conserva.
///
/// Se corta a la MTU habitual mas la cabecera: la huella pasiva vive en las
/// cabeceras, y conservar tramas enteras multiplicaria el consumo sin comprar
/// informacion.
pub const LONGITUD_MAXIMA_TRAMA: usize = 1_600;

/// Fallos de la captura.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorCaptura {
    /// La plataforma no ofrece captura pasiva.
    ///
    /// Windows exige Npcap, cuya redistribucion requiere licencia OEM
    /// (RPT-003 §5.4). macOS usa BPF. Ninguna de las dos esta soportada todavia.
    #[error("captura pasiva no soportada en esta plataforma")]
    PlataformaNoSoportada,

    /// Faltan privilegios para abrir el socket.
    ///
    /// En Linux, `CAP_NET_RAW`. Se distingue del resto de fallos porque tiene
    /// remedio administrativo y el agente puede informarlo con precision en
    /// lugar de decir «no pude».
    #[error("privilegios insuficientes para captura en '{interfaz}'")]
    PrivilegiosInsuficientes {
        /// Interfaz sobre la que se intento.
        interfaz: String,
    },

    /// La interfaz no existe o no admite captura.
    #[error("interfaz '{interfaz}' no disponible")]
    InterfazNoDisponible {
        /// Nombre solicitado.
        interfaz: String,
    },

    /// Fallo del sistema al operar sobre el socket.
    #[error("fallo del sistema en captura: {detalle}")]
    Sistema {
        /// Descripcion del errno o equivalente.
        detalle: String,
    },
}

/// Trama observada, recortada a [`LONGITUD_MAXIMA_TRAMA`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trama {
    /// Bytes conservados, desde la cabecera de enlace.
    pub bytes: Vec<u8>,
    /// Longitud real en el cable, que puede exceder la conservada.
    ///
    /// Se guarda para poder distinguir «trama corta» de «trama recortada». Sin
    /// este dato, un analizador de huella podria concluir que un protocolo no
    /// aparece cuando lo que ocurre es que se corto antes.
    pub longitud_en_el_cable: usize,
}

impl Trama {
    /// Direccion de origen, si la trama alcanza para leerla.
    ///
    /// Devuelve `None` en lugar de indexar a ciegas: la trama llega de la red y
    /// su longitud no es una promesa.
    #[must_use]
    pub fn origen(&self) -> Option<DireccionEnlace> {
        let bytes = self.bytes.get(6..12)?;
        let mut origen = [0u8; 6];
        origen.copy_from_slice(bytes);
        Some(origen)
    }

    /// Direccion de destino, si la trama alcanza para leerla.
    #[must_use]
    pub fn destino(&self) -> Option<DireccionEnlace> {
        let bytes = self.bytes.get(0..6)?;
        let mut destino = [0u8; 6];
        destino.copy_from_slice(bytes);
        Some(destino)
    }

    /// Indica si la trama se recorto al capturarla.
    ///
    /// No es `const fn`: `Vec::len` no esta estabilizada como tal.
    #[must_use]
    pub fn recortada(&self) -> bool {
        self.longitud_en_el_cable > self.bytes.len()
    }
}

/// Contadores de la captura.
///
/// `descartadas` es el numero de tramas que el nucleo tiro por falta de espacio
/// en el buffer del socket. **No es una metrica de rendimiento**: mientras sea
/// mayor que cero, la vista de la red esta incompleta y la huella de cualquier
/// dispositivo puede estarlo tambien.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Estadisticas {
    /// Tramas entregadas al agente.
    pub recibidas: u64,
    /// Tramas que el nucleo descarto.
    pub descartadas: u64,
}

impl Estadisticas {
    /// Indica si la vista de la red esta incompleta.
    ///
    /// Quien consuma la captura debe degradar sus conclusiones a
    /// «indeterminado» mientras esto sea cierto, en lugar de a «sin indicio»
    /// (RPT-018 §4).
    #[must_use]
    pub const fn hay_perdida(&self) -> bool {
        self.descartadas > 0
    }
}

/// Fuente de tramas de **solo lectura**.
///
/// # La ausencia es la garantia
///
/// Este trait no declara ningun metodo de envio, y ningun tipo que lo implemente
/// debe exponerlo por otra via. La pasividad exigida por RPT-002 §9.2 deja de
/// depender de que nadie escriba una linea y pasa a depender de que la linea no
/// se pueda escribir.
pub trait FuentePasiva {
    /// Espera la siguiente trama hasta agotar el plazo.
    ///
    /// Devuelve `Ok(None)` si el plazo vence sin trama, que es distinto de un
    /// fallo: una red silenciosa es un estado normal.
    ///
    /// # Errores
    ///
    /// [`ErrorCaptura::Sistema`] ante fallo del socket.
    fn siguiente(&mut self, plazo: Duration) -> Result<Option<Trama>, ErrorCaptura>;

    /// Contadores acumulados, consultados al nucleo.
    ///
    /// # Errores
    ///
    /// [`ErrorCaptura::Sistema`] si no se pueden leer.
    fn estadisticas(&self) -> Result<Estadisticas, ErrorCaptura>;
}

/// Abre una captura pasiva sobre la interfaz indicada.
///
/// # Errores
///
/// [`ErrorCaptura::PlataformaNoSoportada`] fuera de Linux;
/// [`ErrorCaptura::PrivilegiosInsuficientes`] sin `CAP_NET_RAW`;
/// [`ErrorCaptura::InterfazNoDisponible`] si el nombre no resuelve.
#[cfg(target_os = "linux")]
pub fn abrir(interfaz: &str) -> Result<impl FuentePasiva, ErrorCaptura> {
    linux::SocketPasivo::abrir(interfaz)
}

/// Abre una captura pasiva sobre la interfaz indicada.
///
/// En esta plataforma la captura **no esta soportada**: Windows exigiria Npcap
/// con licencia OEM (RPT-003 §5.4) y macOS usa BPF. Ninguna de las dos entra en
/// el alcance de RPT-018 §7.
///
/// # Errores
///
/// Siempre [`ErrorCaptura::PlataformaNoSoportada`].
#[cfg(not(target_os = "linux"))]
pub fn abrir(_interfaz: &str) -> Result<impl FuentePasiva, ErrorCaptura> {
    Err::<SinSoporte, _>(ErrorCaptura::PlataformaNoSoportada)
}

/// Marcador que satisface la firma de [`abrir`] fuera de Linux.
///
/// Nunca se construye.
#[cfg(not(target_os = "linux"))]
pub enum SinSoporte {}

#[cfg(not(target_os = "linux"))]
impl FuentePasiva for SinSoporte {
    fn siguiente(&mut self, _plazo: Duration) -> Result<Option<Trama>, ErrorCaptura> {
        Err(ErrorCaptura::PlataformaNoSoportada)
    }

    fn estadisticas(&self) -> Result<Estadisticas, ErrorCaptura> {
        Err(ErrorCaptura::PlataformaNoSoportada)
    }
}

#[cfg(test)]
mod pruebas;
