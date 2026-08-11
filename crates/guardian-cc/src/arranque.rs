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

use crate::almacen::{ErrorCarga, InventarioLocal};
use crate::clasificacion::DeclaracionSegmento;
use crate::clave::{ErrorClave, analizar as analizar_clave, serializar as serializar_clave};
use crate::disco::{ErrorDisco, escribir_atomico, leer};
use crate::formato::ErrorFormato;
use crate::inventario::{Centinela, ClaveInventario, DominioClave};
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

    /// Clave con la que el administrador del cliente firma inventarios.
    #[must_use]
    pub fn clave_operativa(&self) -> PathBuf {
        self.directorio.join("clave-cliente.pub")
    }

    /// Registro de evidencia de ALM-01.
    ///
    /// Cuelga del mismo directorio que el inventario y el centinela, por el
    /// mismo motivo que ellos: tres rutas independientes invitan a que una
    /// apunte a otro sitio tras un cambio de configuracion.
    #[must_use]
    pub fn evidencia(&self) -> PathBuf {
        self.directorio.join("evidencia.alm")
    }

    /// Socket de escucha local (RPT-035, PA-41).
    ///
    /// Cuelga del mismo directorio que todo lo demas. RPT-002 §9.3 prohibe el
    /// puerto TCP local: un servicio en `localhost` es alcanzable por cualquier
    /// proceso y por cualquier pagina que el usuario visite.
    #[must_use]
    pub fn socket(&self) -> PathBuf {
        self.directorio.join("agente.sock")
    }

    /// Ancla del extremo de la cadena de evidencia (RPT-033, PA-57).
    ///
    /// Fichero aparte del registro **a proposito**: si viviera dentro, alterarlo
    /// seria la misma operacion que alterar lo que ancla, y no comprobaria nada.
    #[must_use]
    pub fn ancla_evidencia(&self) -> PathBuf {
        self.directorio.join("evidencia.anc")
    }

    /// Clave de recuperacion, que solo firma certificados de revocacion.
    ///
    /// Va en fichero aparte del operativo aunque ambos sean material publico.
    /// Un solo fichero con las dos obligaria a elegir cual se usa **en el
    /// codigo**, y RPT-015 §4 separa las dos claves precisamente para que esa
    /// eleccion no exista.
    #[must_use]
    pub fn clave_recuperacion(&self) -> PathBuf {
        self.directorio.join("clave-recuperacion.pub")
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

    /// El fichero de clave aprovisionada esta mal formado.
    ///
    /// **No se degrada a «sin clave»**, por el mismo motivo que el centinela
    /// corrupto no se degrada a primer arranque: corromper un fichero seria una
    /// via para simular un estado que exime de verificar.
    #[error(transparent)]
    Clave(#[from] ErrorClave),

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
///
/// # Sobre `Debug`
///
/// Se deriva porque un estado de arranque acaba en registros y mensajes de
/// prueba, y describirlo a mano en cada sitio se desincroniza al anadir una
/// variante — que es como `FormatoObsoleto` y `SinClaveAprovisionada` estuvieron
/// a punto de quedarse sin llegar al operador (RPT-028 §2).
///
/// No expone material secreto: los marcados de un inventario no lo son, y el
/// unico campo textual —`NoVerifica::detalle`— ya se escribe para leerse.
#[derive(Debug)]
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

    /// El inventario es de una version anterior del formato.
    ///
    /// # Por que tiene estado propio
    ///
    /// Es un fichero legitimo que caduco al actualizar el agente. Sin este
    /// estado quedaria en [`Self::NoVerifica`], y **cada actualizacion rutinaria
    /// dispararia la alerta maxima de manipulacion**. La fatiga de alertas que
    /// eso produce cuesta mas que la molestia de reemitir: un operador que
    /// aprendio a ignorar esa alerta la ignorara tambien el dia que sea cierta.
    ///
    /// Se comporta como el primer arranque en cuanto a proteccion —sin marcados,
    /// la clasificacion resuelve por segmento— pero **si alerta**, porque exige
    /// una accion del administrador que nadie mas va a recordar.
    FormatoObsoleto {
        /// Version que traia el fichero.
        encontrada: u16,
    },

    /// No hay clave aprovisionada con la que verificar nada.
    ///
    /// # Por que tiene estado propio
    ///
    /// Sin clave, `InventarioLocal::cargar` **no se puede ni intentar**: no es
    /// que el inventario falle la verificacion, es que no hay con que
    /// verificarlo. Colapsarlo en [`Self::PrimerArranque`] diria que la
    /// instalacion esta completa cuando le falta la mitad, y el administrador no
    /// se enteraria hasta que emitiera un manifiesto que el agente ignora en
    /// silencio.
    ///
    /// Solo se alcanza con el centinela **sin establecer**. Si el centinela
    /// existe, alguien acepto un inventario alguna vez, luego hubo clave y
    /// ahora no: eso es [`Self::Supresion`] y no una instalacion a medias.
    SinClaveAprovisionada,
}

impl EstadoArranque {
    /// Indica si la contencion automatica puede llegar a ejecutarse.
    ///
    /// Sin marcados no hay contencion automatica sobre equipos que pudieran ser
    /// criticos, asi que [`Self::FormatoObsoleto`] se comporta como el primer
    /// arranque: la clasificacion resuelve por segmento y donde pueda haber
    /// criticos sale ambiguedad.
    #[must_use]
    pub const fn admite_contencion_automatica(&self) -> bool {
        matches!(
            self,
            Self::Operativo(_)
                | Self::PrimerArranque
                | Self::FormatoObsoleto { .. }
                | Self::SinClaveAprovisionada
        )
    }

    /// Indica si el estado exige alerta al operador.
    ///
    /// El primer arranque **no** alerta: es normal. Los demas si, pero por
    /// motivos distintos — ver [`Self::es_manipulacion`].
    #[must_use]
    pub const fn exige_alerta(&self) -> bool {
        !matches!(self, Self::Operativo(_) | Self::PrimerArranque)
    }

    /// Indica si el estado sugiere que **alguien toco el almacen**.
    ///
    /// Separado de [`Self::exige_alerta`] porque no toda alerta es un ataque.
    /// Un formato obsoleto exige accion administrativa; una supresion o una
    /// firma rota exigen respuesta a incidente. Presentarlos con la misma
    /// urgencia produce fatiga de alertas, y un operador que aprendio a ignorar
    /// esta la ignorara el dia que sea cierta.
    #[must_use]
    pub const fn es_manipulacion(&self) -> bool {
        matches!(self, Self::Supresion { .. } | Self::NoVerifica { .. })
    }

    /// Declaracion de segmento para la etiqueta VLAN observada.
    ///
    /// # Solo el estado operativo declara
    ///
    /// Los otros cuatro devuelven [`DeclaracionSegmento::NoDeclarado`], y no por
    /// comodidad: sin manifiesto verificado **nadie ha firmado que ningun
    /// segmento este limpio**. `NoDeclarado` admite criticos (RPT-009 §5), asi
    /// que la ausencia de tabla no concede contencion automatica en ningun sitio.
    ///
    /// Es la misma asimetria que rige el proveedor de marcados: la ausencia de
    /// evidencia declarativa no es evidencia de ausencia de criticidad.
    #[must_use]
    pub fn declaracion_para(&self, vlan: Option<u16>, ahora: u64) -> DeclaracionSegmento {
        match self {
            Self::Operativo(inventario) => inventario.declaracion_para(vlan, ahora),
            Self::PrimerArranque
            | Self::FormatoObsoleto { .. }
            | Self::SinClaveAprovisionada
            | Self::Supresion { .. }
            | Self::NoVerifica { .. } => DeclaracionSegmento::NoDeclarado,
        }
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
            //
            // `FormatoObsoleto` va por aqui a proposito. El fichero existe pero
            // no se puede leer con este binario, asi que no hay marcado que
            // ofrecer — y devolver error lo trataria como manipulacion, que es
            // justo la confusion que este estado existe para deshacer.
            //
            // `SinClaveAprovisionada` tambien: una instalacion a medias no es un
            // ataque. Alerta, pero por la puerta de la alerta y no por la del
            // incidente.
            Self::PrimerArranque | Self::FormatoObsoleto { .. } | Self::SinClaveAprovisionada => {
                Ok(None)
            }

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
    recuperacion: Option<&ClaveInventario>,
) -> Result<RegistroRevocaciones, ErrorArranque> {
    // Sin clave de recuperacion no hay con que verificar certificados, y un
    // certificado sin verificar no se lee. El registro queda vacio, que es el
    // mismo resultado que un fichero ausente.
    let Some(recuperacion) = recuperacion else {
        return Ok(RegistroRevocaciones::default());
    };

    match leer(&rutas.revocaciones()) {
        Ok(bytes) => Ok(ArchivoRevocaciones::analizar(&bytes, recuperacion)
            .map(|archivo| archivo.registro())
            .unwrap_or_default()),
        Err(ErrorDisco::NoExiste { .. }) => Ok(RegistroRevocaciones::default()),
        Err(error) => Err(ErrorArranque::Disco(error)),
    }
}

/// Carga una clave aprovisionada. Ausente significa `None`.
///
/// # Errores
///
/// [`ErrorArranque::Clave`] si el fichero existe y esta mal formado, o si su
/// dominio no es el que la ruta exige. [`ErrorArranque::Disco`] ante fallo de
/// lectura.
pub fn cargar_clave(
    ruta: &Path,
    esperado: DominioClave,
) -> Result<Option<ClaveInventario>, ErrorArranque> {
    match leer(ruta) {
        Ok(bytes) => Ok(Some(analizar_clave(&bytes, esperado)?)),
        Err(ErrorDisco::NoExiste { .. }) => Ok(None),
        Err(error) => Err(ErrorArranque::Disco(error)),
    }
}

/// Escribe una clave de verificacion en el almacen.
///
/// Es el paso de aprovisionamiento: se ejecuta durante la instalacion, con un
/// humano presente. Ver el §«Este fichero no esta firmado» de
/// [`crate::clave`] sobre lo que protege a estos bytes y lo que no.
///
/// # Errores
///
/// [`ErrorArranque::Disco`] si la escritura falla.
pub fn aprovisionar_clave(
    ruta: &Path,
    clave: &motor_pqc::firma_hibrida::ClaveVerificacionHibrida,
    dominio: DominioClave,
) -> Result<(), ErrorArranque> {
    escribir_atomico(ruta, &serializar_clave(clave, dominio))?;
    Ok(())
}

/// Arranca el agente leyendo tambien sus claves del almacen.
///
/// # Que anade sobre [`arrancar`]
///
/// [`arrancar`] recibe las dos claves como parametros, y hasta PA-49 nadie se
/// las daba: `eje-agente` no tenia de donde sacarlas y operaba en primer
/// arranque permanente. Esta funcion las lee de disco, que es lo que convierte
/// toda la cadena de RPT-011 en algo que un despliegue real puede usar.
///
/// # Errores
///
/// Las de [`arrancar`], mas [`ErrorArranque::Clave`] si alguno de los dos
/// ficheros de clave existe y esta mal formado.
pub fn arrancar_con_almacen(
    rutas: &RutasAlmacen,
) -> Result<(EstadoArranque, Centinela), ErrorArranque> {
    let operativa = cargar_clave(&rutas.clave_operativa(), DominioClave::Cliente)?;
    let recuperacion = cargar_clave(
        &rutas.clave_recuperacion(),
        DominioClave::ClienteRecuperacion,
    )?;

    let Some(operativa) = operativa else {
        let centinela = cargar_centinela(rutas)?;

        // Sin clave no se puede verificar nada, pero el centinela sigue
        // distinguiendo la instalacion a medias del borrado. Si alguna vez se
        // acepto un inventario, hubo clave; que ya no este es supresion.
        let estado = match centinela.secuencia() {
            None => EstadoArranque::SinClaveAprovisionada,
            Some(secuencia_conocida) => EstadoArranque::Supresion { secuencia_conocida },
        };

        return Ok((estado, centinela));
    };

    // La de recuperacion puede faltar sin que eso impida arrancar: solo sirve
    // para leer certificados de revocacion, y RPT-015 §5 ya acepta que perder el
    // registro devuelve al estado previo a la revocacion y no por debajo.
    //
    // Su ausencia se propaga como `None` y NO se sustituye por la operativa. Un
    // borrador de esta funcion hacia justo eso —envolver la publica operativa en
    // el dominio de recuperacion— y era un agujero: quien tuviera la privada
    // operativa habria podido forjar un certificado que verificase, y con el
    // bajar el centinela por `reiniciar_por` para despues reponer un inventario
    // anterior. Es el ataque de PA-27 servido por la puerta que RPT-015 §4 cerro.
    arrancar(rutas, &operativa, recuperacion.as_ref())
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
    recuperacion: Option<&ClaveInventario>,
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

        // El formato obsoleto se separa antes de llegar a `NoVerifica`. Sin este
        // brazo, actualizar el agente pareceria un ataque en cada sitio a la vez.
        Err(ErrorCarga::Formato(ErrorFormato::FormatoObsoleto { encontrada })) => {
            Ok((EstadoArranque::FormatoObsoleto { encontrada }, centinela))
        }

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
