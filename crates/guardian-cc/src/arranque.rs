//! Arranque del agente: rutas, carga y semantica del fichero ausente.
//!
//! RPT-017, PA-35.
//!
//! # El ataque que gobierna este modulo
//!
//! Sin inventario, ningun dispositivo tiene marcado. Y por RPT-009 §5 un
//! dispositivo sin marcar **en un segmento declarado limpio es contenible**.
//! Luego quien borre el fichero de inventario vuelve contenible un equipo de
//! soporte vital que estaba protegido. No hace falta clave, ni firma, ni
//! entender el formato: hace falta `del`.
//!
//! # El centinela es el testigo
//!
//! No hace falta metadato nuevo. Un agente que alguna vez acepto un inventario
//! tiene el centinela establecido; si el fichero desaparece, eso no es un primer
//! arranque sino una supresion.
//!
//! | Inventario | Centinela | Lectura |
//! |---|---|---|
//! | ausente | sin establecer | primer arranque legitimo |
//! | ausente | establecido | **supresion** |
//! | presente, no verifica | cualquiera | manipulacion |
//! | presente, verifica | — | normal |
//!
//! # El agente arranca igual
//!
//! Hace dos cosas: observar y contener. Un inventario ausente solo afecta a la
//! segunda. Negarse a arrancar apagaria tambien la observacion, y en un hospital
//! eso es quedarse sin vigilancia: un dano cierto para evitar uno hipotetico.
//!
//! La contencion automatica se deshabilita sola por el camino que ya existe —sin
//! marcados, la clasificacion resuelve por segmento y sale `Ambiguo`—, asi que
//! este modulo no anade politica: solo decide que devuelve el proveedor.

use std::path::{Path, PathBuf};

use crate::almacen::InventarioLocal;
use crate::disco::{ErrorDisco, escribir_atomico, leer};
use crate::inventario::{Centinela, ClaveInventario};
use crate::proveedores::{DireccionEnlace, ErrorProveedor, ProveedorInventario};
use crate::revocacion::{ArchivoRevocaciones, RegistroRevocaciones};

/// Magico del fichero de centinela.
pub const MAGICO_CENTINELA: &[u8; 8] = b"EJE-CEN1";

/// Version del formato de centinela.
pub const VERSION_CENTINELA: u16 = 1;

/// Longitud exacta del fichero de centinela.
const LONGITUD_CENTINELA: usize = 8 + 2 + 8;

/// Rutas del almacen local.
///
/// Se recibe un **directorio**, no rutas sueltas: codificar tres rutas
/// independientes invita a que una de ellas apunte a otro sitio tras un cambio
/// de configuracion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RutasAlmacen {
    directorio: PathBuf,
}

impl RutasAlmacen {
    /// Rutas bajo el directorio de datos indicado.
    #[must_use]
    pub const fn nuevo(directorio: PathBuf) -> Self {
        Self { directorio }
    }

    /// Directorio de datos.
    #[must_use]
    pub fn directorio(&self) -> &Path {
        &self.directorio
    }

    /// Inventario firmado.
    #[must_use]
    pub fn inventario(&self) -> PathBuf {
        self.directorio.join("inventario.inv")
    }

    /// Certificados de revocacion.
    #[must_use]
    pub fn revocaciones(&self) -> PathBuf {
        self.directorio.join("revocaciones.rev")
    }

    /// Marca de agua de frescura.
    #[must_use]
    pub fn centinela(&self) -> PathBuf {
        self.directorio.join("centinela.dat")
    }
}

/// Fallo al arrancar.
#[derive(Debug, thiserror::Error)]
pub enum ErrorArranque {
    /// El fichero de centinela esta mal formado.
    ///
    /// No se degrada a «sin establecer»: eso convertiria corromper un fichero de
    /// dieciocho bytes en una via para simular un primer arranque, que es
    /// exactamente el ataque del §2 por otra puerta.
    #[error("el fichero de centinela esta corrupto")]
    CentinelaCorrupto,

    /// Fallo de disco distinto de «no existe».
    #[error(transparent)]
    Disco(#[from] ErrorDisco),
}

/// Serializa el centinela.
///
/// **La ausencia del fichero es [`Centinela::SinEstablecer`]**, asi que ese
/// estado no se representa en disco. Un fichero que existe siempre declara una
/// secuencia; lo contrario permitiria dos codificaciones del mismo estado.
#[must_use]
pub fn serializar_centinela(secuencia: u64) -> Vec<u8> {
    let mut salida = Vec::with_capacity(LONGITUD_CENTINELA);
    salida.extend_from_slice(MAGICO_CENTINELA);
    salida.extend_from_slice(&VERSION_CENTINELA.to_be_bytes());
    salida.extend_from_slice(&secuencia.to_be_bytes());
    salida
}

/// Analiza el fichero de centinela.
///
/// # Errores
///
/// [`ErrorArranque::CentinelaCorrupto`] ante longitud, magico o version que no
/// cuadren.
pub fn analizar_centinela(bytes: &[u8]) -> Result<Centinela, ErrorArranque> {
    if bytes.len() != LONGITUD_CENTINELA || &bytes[..8] != MAGICO_CENTINELA {
        return Err(ErrorArranque::CentinelaCorrupto);
    }

    if u16::from_be_bytes([bytes[8], bytes[9]]) != VERSION_CENTINELA {
        return Err(ErrorArranque::CentinelaCorrupto);
    }

    let mut secuencia = [0u8; 8];
    secuencia.copy_from_slice(&bytes[10..18]);

    Ok(Centinela::Establecido(u64::from_be_bytes(secuencia)))
}

/// Resultado del arranque, que **es** el proveedor de inventario.
///
/// Que el estado de arranque implemente [`ProveedorInventario`] evita que quien
/// cablea tenga que acordarse de traducir cada caso: el tipo ya devuelve lo que
/// corresponde.
pub enum EstadoArranque {
    /// Inventario cargado y verificado.
    Operativo(Box<InventarioLocal>),

    /// Ni inventario ni centinela: instalacion recien hecha.
    ///
    /// Los dispositivos caen a las reglas de segmento de RPT-009 §5.
    PrimerArranque,

    /// Habia inventario —el centinela lo atestigua— y ya no esta.
    Supresion {
        /// Secuencia que el centinela conserva.
        secuencia_conocida: u64,
    },

    /// El inventario esta pero no supera la verificacion.
    NoVerifica {
        /// Motivo, para el registro forense.
        detalle: String,
    },
}

impl EstadoArranque {
    /// Indica si la contencion automatica puede llegar a ejecutarse.
    ///
    /// Solo en [`Self::Operativo`] y [`Self::PrimerArranque`]. En los dos casos
    /// de manipulacion, el proveedor devuelve error y la clasificacion resuelve
    /// en ambiguedad.
    #[must_use]
    pub const fn admite_contencion_automatica(&self) -> bool {
        matches!(self, Self::Operativo(_) | Self::PrimerArranque)
    }

    /// Indica si el estado exige alerta al operador.
    ///
    /// El primer arranque **no** alerta: es normal. La supresion si.
    #[must_use]
    pub const fn exige_alerta(&self) -> bool {
        matches!(self, Self::Supresion { .. } | Self::NoVerifica { .. })
    }
}

impl ProveedorInventario for EstadoArranque {
    fn marcado(
        &self,
        mac: &DireccionEnlace,
    ) -> Result<Option<crate::inventario::MarcadoVerificado>, ErrorProveedor> {
        match self {
            Self::Operativo(inventario) => inventario.marcado(mac),

            // Ausencia legitima: no hay marcados y no hay nada sospechoso.
            Self::PrimerArranque => Ok(None),

            // Manipulacion. RPT-010 §4 obliga a no confundirla con la ausencia:
            // la ausencia permite contener en un segmento limpio, la
            // manipulacion nunca.
            Self::Supresion { secuencia_conocida } => Err(ErrorProveedor::FirmaInvalida {
                detalle: format!(
                    "el inventario desaparecio; el centinela conserva la secuencia {secuencia_conocida}"
                ),
            }),
            Self::NoVerifica { detalle } => Err(ErrorProveedor::FirmaInvalida {
                detalle: detalle.clone(),
            }),
        }
    }
}

/// Carga el centinela del disco. Ausente significa sin establecer.
///
/// # Errores
///
/// [`ErrorArranque::CentinelaCorrupto`] o [`ErrorArranque::Disco`].
pub fn cargar_centinela(rutas: &RutasAlmacen) -> Result<Centinela, ErrorArranque> {
    match leer(&rutas.centinela()) {
        Ok(bytes) => analizar_centinela(&bytes),
        Err(ErrorDisco::NoExiste { .. }) => Ok(Centinela::SinEstablecer),
        Err(error) => Err(ErrorArranque::Disco(error)),
    }
}

/// Carga el registro de revocaciones. Ausente significa vacio.
///
/// # Por que la ausencia aqui **no** es sospechosa
///
/// A diferencia del inventario, no hay testigo equivalente al centinela, y
/// RPT-015 §5 ya acepta que perder el registro devuelve al estado previo a la
/// revocacion —no por debajo— y que el certificado se puede volver a presentar.
/// La asimetria entre ambos ficheros es deliberada.
///
/// # Errores
///
/// [`ErrorArranque::Disco`] ante fallo de lectura. Un fichero presente que no
/// verifica devuelve registro vacio y deberia alertar; se prefiere eso a impedir
/// el arranque por un fichero de revocaciones roto.
pub fn cargar_revocaciones(
    rutas: &RutasAlmacen,
    recuperacion: &ClaveInventario,
) -> Result<RegistroRevocaciones, ErrorArranque> {
    match leer(&rutas.revocaciones()) {
        Ok(bytes) => Ok(ArchivoRevocaciones::analizar(&bytes, recuperacion)
            .map(|archivo| archivo.registro())
            .unwrap_or_default()),
        Err(ErrorDisco::NoExiste { .. }) => Ok(RegistroRevocaciones::default()),
        Err(error) => Err(ErrorArranque::Disco(error)),
    }
}

/// Arranca el agente sobre el almacen indicado.
///
/// # Errores
///
/// Solo falla ante un centinela corrupto o un fallo de disco distinto de «no
/// existe». Todo lo demas se resuelve en una variante de [`EstadoArranque`]:
/// el agente arranca siempre que pueda leer su propio estado.
pub fn arrancar(
    rutas: &RutasAlmacen,
    operativa: &ClaveInventario,
    recuperacion: &ClaveInventario,
) -> Result<(EstadoArranque, Centinela), ErrorArranque> {
    let centinela = cargar_centinela(rutas)?;
    let revocaciones = cargar_revocaciones(rutas, recuperacion)?;

    let bytes = match leer(&rutas.inventario()) {
        Ok(bytes) => bytes,
        Err(ErrorDisco::NoExiste { .. }) => {
            // Aqui se decide todo. Ver la tabla del encabezado.
            let estado = match centinela.secuencia() {
                None => EstadoArranque::PrimerArranque,
                Some(secuencia_conocida) => EstadoArranque::Supresion { secuencia_conocida },
            };
            return Ok((estado, centinela));
        }
        Err(error) => return Err(ErrorArranque::Disco(error)),
    };

    match InventarioLocal::cargar(&bytes, operativa, centinela, &revocaciones) {
        Ok(inventario) => Ok((EstadoArranque::Operativo(Box::new(inventario)), centinela)),
        Err(error) => Ok((
            EstadoArranque::NoVerifica {
                detalle: error.to_string(),
            },
            centinela,
        )),
    }
}

/// Acepta un inventario nuevo y lo persiste.
///
/// # Orden de escritura
///
/// **El centinela primero.** Si el proceso muere entre ambas escrituras queda
/// centinela en N e inventario anterior en disco; al rearrancar, el inventario
/// nuevo se vuelve a presentar con `secuencia == aceptada`, que RPT-012 §4.4
/// admite porque eligio `secuencia < aceptada` y no `<=`.
///
/// Al reves —inventario primero— se actuaria sobre un inventario cuya secuencia
/// no quedo registrada, reabriendo la ventana de reversion.
///
/// # Errores
///
/// [`ErrorArranque::Disco`] si alguna escritura falla.
pub fn aceptar_inventario(
    rutas: &RutasAlmacen,
    bytes: &[u8],
    secuencia: u64,
) -> Result<Centinela, ErrorArranque> {
    escribir_atomico(&rutas.centinela(), &serializar_centinela(secuencia))?;
    escribir_atomico(&rutas.inventario(), bytes)?;
    Ok(Centinela::Establecido(secuencia))
}

/// Persiste el registro de revocaciones.
///
/// # Errores
///
/// [`ErrorArranque::Disco`] si la escritura falla.
pub fn guardar_revocaciones(
    rutas: &RutasAlmacen,
    archivo: &ArchivoRevocaciones,
) -> Result<(), ErrorArranque> {
    escribir_atomico(&rutas.revocaciones(), &archivo.serializar())?;
    Ok(())
}
