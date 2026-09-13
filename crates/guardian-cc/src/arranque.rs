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
use crate::revocacion::{
    ArchivoRevocaciones, IdentificadorClave, RegistroRevocaciones, Sucesion,
};
use eje_almacen::resumen::{Absorbedor, Resumen};

/// Magico del fichero de centinela.
pub const MAGICO_CENTINELA: &[u8; 8] = b"EJE-CEN1";

/// Version del formato de centinela.
///
/// # Las versiones anteriores se rechazan, y no es rigidez
///
/// RPT-078, PA-79. La 1 llevaba **una sola** marca de agua, la del inventario.
/// Aceptarla seria aceptar un fichero que no dice nada de la secuencia de
/// configuracion, y eso se lee como «sin establecer» — es decir, **un fichero de
/// 18 bytes escrito a mano deja pasar cualquier configuracion antigua y bien
/// firmada**. Admitir el formato viejo seria abrir por compatibilidad el mismo
/// agujero que ese campo vino a cerrar.
///
/// La 2 llevaba las dos marcas y **ninguna memoria de la clave de recuperacion**.
/// Aceptarla ahora se leeria como «este sensor nunca tuvo clave», que es
/// exactamente la lectura que un atacante con `root` quiere que hagamos despues
/// de borrarla.
///
/// # Y aqui la migracion dejo de ser hipotetica
///
/// La version 2 decia: *«se puede permitir porque no hay ningun sensor
/// desplegado; cuando lo haya, la migracion tendra que ser una operacion
/// deliberada del emisor, no una lectura tolerante del agente»*.
///
/// La 3 anclaba la de recuperacion y **no la operativa**, que es la que firma el
/// inventario y la configuracion que el sensor obedece cada dia. Aceptarla se
/// leeria como «esta clave operativa nunca se ancló», y sustituirla volveria a no
/// encender nada: es lo que paso el 1 de septiembre de 2026 reinstalando la
/// identidad de `eje-prueba` a mano, sin que el sensor dijera una palabra.
///
/// # Y aqui la migracion dejo de ser hipotetica
///
/// La version 2 decia: *«se puede permitir porque no hay ningun sensor
/// desplegado; cuando lo haya, la migracion tendra que ser una operacion
/// deliberada del emisor, no una lectura tolerante del agente»*.
///
/// Ese dia llego el 9 de septiembre de 2026 con `eje-prueba` en marcha, y la
/// frase se cumple al pie de la letra: el agente **no** migra nada. Migra
/// `eje-manifiesto migrar-centinela`, con un humano delante (PA-150, PA-146b).
///
/// La 4 llega un dia despues que la 3, y eso es un error de diseño mio: la
/// pregunta que me hice fue «como damos testigo a la clave de recuperacion» en
/// vez de «que claves merecen testigo». Con la segunda, las dos anclas habrian
/// entrado juntas y el despliegue pasaria por **una** parada en vez de dos.
pub const VERSION_CENTINELA: u16 = 4;

/// Longitud exacta del fichero de centinela.
///
/// Cada marca lleva **presencia y valor por separado**, como el grupo del socket
/// en la configuracion firmada: sin la presencia, «sin establecer» y «establecida
/// en cero» darian los mismos bytes y son cosas distintas. El ancla de
/// recuperacion sigue la misma regla: sin bandera, «sin anclar» y «anclada a
/// treinta y dos ceros» serian el mismo fichero.
const LONGITUD_CENTINELA: usize = 8 + 2 + (1 + 8) + (1 + 8) + (1 + 32) + (1 + 32);


/// Directorio volatil donde vive el socket, por omision. RPT-067, PA-120.
///
/// `/run` es `tmpfs`: se vacia en cada arranque. Eso no es un detalle de estilo,
/// es lo que **erradica por construccion** el socket huerfano — un fichero que
/// sobrevive al proceso y hace que el cliente reciba `ECONNREFUSED` sobre algo
/// que existe.
pub const DIRECTORIO_SOCKET_POR_OMISION: &str = "/run/eje-latam";

/// Nombre del socket. No es configurable a proposito.
///
/// Se puede mover el **directorio**, nunca el fichero. Si la ruta completa fuera
/// configurable, nada impediria apuntarla de vuelta al directorio de evidencia y
/// deshacer la separacion sin que ninguna comprobacion se enterase.
const NOMBRE_SOCKET: &str = "agente.sock";

/// Rutas del almacen local.
///
/// Se recibe un **directorio**, no rutas sueltas: codificar tres rutas
/// independientes invita a que una de ellas apunte a otro sitio tras un cambio
/// de configuracion.
///
/// # Por que hay dos directorios y no uno
///
/// RPT-067, PA-120. Lo persistente y lo volatil tienen ciclos de vida distintos
/// y no deben compartir sitio:
///
/// - `directorio` guarda lo que **tiene que sobrevivir** al reinicio: inventario,
///   centinela, registro de evidencia, ancla.
/// - `directorio_socket` guarda lo que **no debe sobrevivirlo**: el socket.
///
/// La consecuencia que importa no es estetica. Con los dos juntos,
/// `ReadWritePaths=/var/lib/eje-latam` autoriza a la vez la escritura de
/// evidencia y la creacion del socket, y medir el aislamiento del servicio no
/// distingue una cosa de la otra. Separados, la medicion es de una sola cosa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RutasAlmacen {
    directorio: PathBuf,
    directorio_socket: PathBuf,
}

impl RutasAlmacen {
    /// Rutas bajo el directorio de datos indicado, con el socket en
    /// [`DIRECTORIO_SOCKET_POR_OMISION`].
    #[must_use]
    pub fn nuevo(directorio: PathBuf) -> Self {
        Self {
            directorio,
            directorio_socket: PathBuf::from(DIRECTORIO_SOCKET_POR_OMISION),
        }
    }

    /// Igual, con el directorio del socket puesto a mano.
    ///
    /// Existe para el desarrollo sin privilegios: crear `/run/eje-latam` exige
    /// root, y obligar a `sudo` para levantar la consola de diagnostico haria
    /// que nadie la levantara.
    #[must_use]
    pub const fn con_directorio_socket(directorio: PathBuf, directorio_socket: PathBuf) -> Self {
        Self {
            directorio,
            directorio_socket,
        }
    }

    /// Directorio de datos.
    #[must_use]
    pub fn directorio(&self) -> &Path {
        &self.directorio
    }

    /// Directorio volatil donde se abre el socket.
    #[must_use]
    pub fn directorio_socket(&self) -> &Path {
        &self.directorio_socket
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
    /// **No** cuelga del directorio de datos: vive en el volatil (RPT-067,
    /// PA-120). RPT-002 §9.3 sigue prohibiendo el puerto TCP local — un servicio
    /// en `localhost` es alcanzable por cualquier proceso y por cualquier pagina
    /// que el usuario visite—, y esto no lo cambia: sigue siendo un socket de
    /// dominio Unix, sólo que en otro sitio.
    #[must_use]
    pub fn socket(&self) -> PathBuf {
        self.directorio_socket.join(NOMBRE_SOCKET)
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

/// Las marcas de agua de este sensor, **en un solo fichero**.
///
/// RPT-078, PA-79 paso 5.
///
/// # Por que dos contadores y no uno
///
/// El inventario y la configuracion firmada se emiten a ritmos distintos y por
/// motivos distintos. Con un contador compartido, publicar un inventario nuevo
/// subiria la marca y dejaria fuera configuraciones legitimas mas bajas — y al
/// reves. Serian dos series acopladas por casualidad de implementacion.
///
/// # Por que un solo fichero y no dos
///
/// RPT-017. Dispersar el estado de seguridad invita a que una escritura sobreviva
/// y la otra no. Aqui las dos marcas viajan en la misma escritura atomica: o
/// avanzan las dos o no avanza ninguna.
///
/// Y hay un motivo mas concreto. `aceptar_inventario` escribia el fichero
/// **entero** desde la secuencia del inventario. Con una segunda marca dentro y
/// un segundo escritor ingenuo, aceptar un inventario habria borrado la marca de
/// configuracion en silencio, dejando entrar cualquier configuracion antigua. Por
/// eso los dos `aceptar_*` releen el disco y funden, en lugar de componer el
/// fichero desde lo que traen en la mano.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Centinelas {
    /// Secuencia mas alta de inventario aceptada. RPT-012 §4.4, PA-27.
    pub inventario: Centinela,
    /// Secuencia mas alta de configuracion firmada aceptada. RPT-074 §5.
    pub configuracion: Centinela,
    /// Huella de la clave de recuperacion que este sensor **tuvo alguna vez**.
    ///
    /// PA-150.
    ///
    /// # Que compra esto, y que no
    ///
    /// **No** es resistencia a `root`: este fichero es de `root` y quien borre la
    /// clave puede borrar tambien la marca. Lo que compra es que el borrado deje
    /// de ser **silencioso**, que es lo mismo que el resto del centinela compra
    /// para las dos series (RPT-078 §5: «lo que se consigue es que revertir no
    /// sea silencioso»).
    ///
    /// Sin ella, borrar `clave-recuperacion.pub` devuelve al sensor al estado de
    /// uno recien instalado, y el operador no puede distinguir una ceremonia
    /// pendiente de una credencial suprimida.
    ///
    /// # Por que `Option` y no un enumerado como [`Centinela`]
    ///
    /// Aquel tiene comportamiento —`secuencia()`, y una comparacion con
    /// semantica de frescura—. Esto es un valor que esta o no esta, sin orden
    /// entre dos valores distintos: dos huellas que difieren no es que una sea
    /// mas nueva, es que **no son la misma clave**.
    pub huella_recuperacion: Option<IdentificadorClave>,
    /// Huella de la clave **operativa** que este sensor tuvo.
    ///
    /// PA-146b fase 2.
    ///
    /// # Por que esta llega un dia despues que la otra
    ///
    /// Porque PA-150 pregunto «como damos testigo a la clave de recuperacion» en
    /// vez de «que claves merecen testigo». La operativa es la que firma el
    /// inventario y la configuracion que el sensor obedece **cada dia**, y hasta
    /// hoy se cargaba del disco y se creia: sustituirla no encendia nada.
    ///
    /// El 1 de septiembre de 2026 se reinstalo la identidad de `eje-prueba` a
    /// mano —clave nueva, fichero sustituido— y el sensor no dijo una palabra.
    ///
    /// # Y aqui hay un quinto estado que la de recuperacion no tiene
    ///
    /// Que esta huella cambie no siempre es un ataque: puede ser una **rotacion
    /// legitima**, y lo que la distingue es un certificado de revocacion
    /// verificado que nombre a la nueva como sucesora de la anclada. Ver
    /// [`EstadoOperativa`].
    pub huella_operativa: Option<IdentificadorClave>,
}

impl Centinelas {
    /// Nada establecido. Es lo que significa que el fichero no exista.
    pub const SIN_ESTABLECER: Self = Self {
        inventario: Centinela::SinEstablecer,
        configuracion: Centinela::SinEstablecer,
        huella_recuperacion: None,
        huella_operativa: None,
    };

    /// Las mismas marcas con la del inventario sustituida.
    #[must_use]
    pub const fn con_inventario(self, marca: Centinela) -> Self {
        Self {
            inventario: marca,
            configuracion: self.configuracion,
            huella_recuperacion: self.huella_recuperacion,
            huella_operativa: self.huella_operativa,
        }
    }

    /// Las mismas marcas con la de configuracion sustituida.
    #[must_use]
    pub const fn con_configuracion(self, marca: Centinela) -> Self {
        Self {
            inventario: self.inventario,
            configuracion: marca,
            huella_recuperacion: self.huella_recuperacion,
            huella_operativa: self.huella_operativa,
        }
    }

    /// Las mismas marcas con el ancla de recuperacion sustituida.
    ///
    /// **No la llama el agente.** Ver [`EstadoRecuperacion`]: si el sensor se
    /// anclara solo al ver una clave, bastaria con borrar los dos ficheros para
    /// que anclara la del atacante, y el centinela volveria a no decir nada. La
    /// escribe `eje-manifiesto`, con un humano delante.
    #[must_use]
    pub const fn con_recuperacion(self, huella: Option<IdentificadorClave>) -> Self {
        Self {
            inventario: self.inventario,
            configuracion: self.configuracion,
            huella_recuperacion: huella,
            huella_operativa: self.huella_operativa,
        }
    }

    /// Las mismas marcas con el ancla de la clave operativa sustituida.
    ///
    /// PA-146b fase 2. Tampoco la llama el agente, y por el mismo motivo que la
    /// de recuperacion: anclar al primer uso convertiria el ataque en «borro los
    /// dos ficheros y dejo la mia».
    #[must_use]
    pub const fn con_operativa(self, huella: Option<IdentificadorClave>) -> Self {
        Self {
            inventario: self.inventario,
            configuracion: self.configuracion,
            huella_recuperacion: self.huella_recuperacion,
            huella_operativa: huella,
        }
    }

    /// Si el fichero no afirma **nada**.
    ///
    /// # Por que el ancla cuenta aqui
    ///
    /// PA-150. Hasta la version 3 esto miraba solo las dos secuencias, y el
    /// motivo era bueno: un fichero que no dice nada ya lo dice su ausencia.
    ///
    /// Con el ancla deja de ser cierto. Un sensor recien aprovisionado —clave de
    /// recuperacion anclada, sin inventario y sin configuracion— es un estado
    /// legitimo, y **el mas importante que este campo existe para poder
    /// afirmar**. Con la regla anterior su fichero se habria leido como corrupto
    /// y el aprovisionamiento no se habria podido registrar.
    #[must_use]
    pub const fn vacio(self) -> bool {
        self.inventario.secuencia().is_none()
            && self.configuracion.secuencia().is_none()
            && self.huella_recuperacion.is_none()
            && self.huella_operativa.is_none()
    }
}

/// Ancla de una clave aprovisionada: su [`IdentificadorClave`].
///
/// # Por que este identificador y no una huella propia
///
/// PA-146b fase 2, corrigiendo a PA-150. La primera version de esto era
/// `huella_de_recuperacion`, un resumen **nuevo** sobre la serializacion de la
/// clave bajo su propia etiqueta de dominio. Funcionaba para la clave de
/// recuperacion y **hacia imposible la rotacion**: los certificados nombran a la
/// sucesora con `IdentificadorClave`, que es otro resumen del mismo material, y
/// de una huella no se vuelve a la clave para recalcular la otra.
///
/// Con dos hashes distintos del mismo objeto, el ancla y el certificado no
/// pueden hablarse. Con uno, si.
///
/// Y ese uno ya existia desde RPT-016, con esta documentacion: *«se usa en lugar
/// de la clave completa porque el registro de revocaciones debe poder nombrar
/// una clave que ya no se conserva»*. Es la definicion de un ancla.
#[must_use]
pub fn ancla_de_clave(clave: &ClaveInventario) -> IdentificadorClave {
    clave.identificador()
}

/// Que dice el almacen sobre la clave de recuperacion. PA-150.
///
/// # Cuatro estados, y ninguno se puede colapsar
///
/// | Ancla | Fichero | Lectura |
/// |---|---|---|
/// | ausente | ausente | [`Self::NoAprovisionada`] — ceremonia pendiente |
/// | ausente | presente | [`Self::NoAprovisionada`] — hay clave y **nadie la ancló** |
/// | presente | ausente | [`Self::Suprimida`] — alguien borro la credencial |
/// | presente | otra huella | [`Self::NoVerifica`] — alguien puso **otra** clave |
/// | presente | misma huella | [`Self::Anclada`] |
///
/// Borrar y sustituir no son lo mismo: lo primero se remedia reponiendo el
/// fichero, lo segundo significa que hay una clave de recuperacion viva que no
/// es la nuestra, y con ella se firman certificados que este sensor creeria. Es
/// el mismo par que el inventario ya tiene partido en `Supresion` y `NoVerifica`.
///
/// # La fila segunda no es un descuido
///
/// Un fichero presente **sin** ancla se lee como no aprovisionada, no como
/// anclada. Es la negativa al «anclar al primer uso»: si el agente aceptara esa
/// clave por estar ahi, borrar los dos ficheros y dejar la propia seria un
/// ataque completo y silencioso. El ancla la escribe el administrador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoRecuperacion {
    /// No hay ancla. La ceremonia de RPT-015 §4 esta pendiente.
    NoAprovisionada,
    /// Hubo clave anclada y su fichero ya no esta.
    Suprimida,
    /// Hay clave y **no es la anclada**.
    NoVerifica,
    /// La clave presente es la anclada.
    Anclada,
}

impl EstadoRecuperacion {
    /// Deriva el estado del ancla y de lo que hay en disco.
    #[must_use]
    pub fn evaluar(
        ancla: Option<IdentificadorClave>,
        presente: Option<&ClaveInventario>,
    ) -> Self {
        match (ancla, presente) {
            (None, _) => Self::NoAprovisionada,
            (Some(_), None) => Self::Suprimida,
            (Some(anclada), Some(clave)) => {
                if ancla_de_clave(clave) == anclada {
                    Self::Anclada
                } else {
                    Self::NoVerifica
                }
            }
        }
    }

    /// Si alguien toco el almacen para llegar a este estado.
    ///
    /// `NoAprovisionada` no lo es: nadie hizo nada, y eso es justo el problema.
    #[must_use]
    pub const fn es_manipulacion(self) -> bool {
        matches!(self, Self::Suprimida | Self::NoVerifica)
    }
}

/// Que dice el almacen sobre la clave **operativa**. PA-146b fase 2.
///
/// # Cinco estados: los cuatro de la recuperacion, mas la rotacion
///
/// La operativa firma el inventario y la configuracion que el sensor obedece
/// cada dia, y hasta hoy se cargaba del disco y se creia. Sustituirla no
/// encendia nada — es lo que paso el 1 de septiembre de 2026 reinstalando la
/// identidad de `eje-prueba`, sin que el sensor dijera una palabra.
///
/// | Ancla | Fichero | Certificado | Lectura |
/// |---|---|---|---|
/// | ausente | cualquiera | — | [`Self::NoAprovisionada`] |
/// | presente | ausente | — | [`Self::Suprimida`] |
/// | presente | otra | nombra a esa otra como sucesora | [`Self::Rotada`] |
/// | presente | otra | no, o nombra a otra | [`Self::NoVerifica`] |
/// | presente | la misma | — | [`Self::Anclada`] |
///
/// # El quinto es el punto entero
///
/// Que la clave cambie **no siempre es un ataque**. Lo que distingue una
/// rotacion legitima de un secuestro es un certificado de revocacion verificado
/// —firmado por la clave de recuperacion, que vive fuera de esta maquina— que
/// nombre a la clave presente como sucesora de la anclada.
///
/// Sin ese cruce, las dos cosas se ven igual en el disco: un fichero `.pub`
/// distinto del que habia. Con el, el mismo mecanismo que acusa el secuestro
/// autoriza la rotacion, que es lo que PA-146 pedia desde el principio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoOperativa {
    /// No hay ancla. Nadie declaro cual es la identidad de este sensor.
    NoAprovisionada,
    /// Hubo clave anclada y su fichero ya no esta.
    Suprimida,
    /// Hay otra clave y **ningun certificado la autoriza**.
    NoVerifica,
    /// Hay otra clave y un certificado verificado la nombra sucesora.
    Rotada,
    /// La clave presente es la anclada.
    Anclada,
}

impl EstadoOperativa {
    /// Deriva el estado del ancla, de lo que hay en disco y de las revocaciones.
    ///
    /// # Por que el registro entra aqui y no se consulta despues
    ///
    /// Porque la pregunta «¿esta clave es legitima?» no se puede responder sin
    /// el. Pasarlo por parametro impide escribir la version que responde sin
    /// mirarlo — que es la que tenemos hoy, y que dice que si a todo.
    #[must_use]
    pub fn evaluar(
        ancla: Option<IdentificadorClave>,
        presente: Option<&ClaveInventario>,
        revocaciones: &RegistroRevocaciones,
    ) -> Self {
        let (Some(anclada), Some(clave)) = (ancla, presente) else {
            return match (ancla, presente) {
                (None, _) => Self::NoAprovisionada,
                (Some(_), None) => Self::Suprimida,
                // Inalcanzable: el patron de arriba ya cubrio los dos casos con
                // ancla y clave. Se escribe sin comodin para que anadir una
                // variante deje de compilar aqui.
                (Some(_), Some(_)) => Self::Anclada,
            };
        };

        let presente = ancla_de_clave(clave);

        if presente == anclada {
            return Self::Anclada;
        }

        // La clave cambio. Lo unico que distingue rotacion de secuestro es que
        // la autoridad de recuperacion lo haya dicho por escrito y firmado.
        match revocaciones.sucesora_de(&anclada) {
            Sucesion::Declarada(sucesora) if sucesora == presente => Self::Rotada,
            Sucesion::Declarada(_) | Sucesion::SinRevocar => Self::NoVerifica,
        }
    }

    /// Si alguien toco el almacen sin autorizacion para llegar a este estado.
    ///
    /// `Rotada` **no** lo es: cambio la clave y hay un certificado que lo
    /// autoriza. `NoAprovisionada` tampoco: nadie hizo nada, y eso es el
    /// problema.
    #[must_use]
    pub const fn es_manipulacion(self) -> bool {
        matches!(self, Self::Suprimida | Self::NoVerifica)
    }
}

/// Serializa una marca: presencia y valor.
fn escribir_marca(salida: &mut Vec<u8>, marca: Centinela) {
    match marca.secuencia() {
        Some(secuencia) => {
            salida.push(1);
            salida.extend_from_slice(&secuencia.to_be_bytes());
        }
        // Los ocho bytes van a cero y no a basura: dos ficheros que dicen lo
        // mismo tienen que ser el mismo fichero, o `analizar` no puede rechazar
        // la segunda codificacion.
        None => salida.extend_from_slice(&[0u8; 9]),
    }
}

/// Lee una marca de nueve bytes.
///
/// `None` significa que los bytes no son una marca valida, no que la marca este
/// sin establecer: eso lo dice la bandera de presencia.
fn leer_marca(bytes: &[u8]) -> Option<Centinela> {
    let (presencia, valor) = bytes.split_first()?;
    let valor: [u8; 8] = valor.try_into().ok()?;

    match *presencia {
        // Ausente **y** con el valor a cero. Un cero de presencia con valor
        // encima seria una segunda forma de escribir «sin establecer», y la
        // segunda forma es siempre por donde se cuela algo.
        0 if valor == [0u8; 8] => Some(Centinela::SinEstablecer),
        1 => Some(Centinela::Establecido(u64::from_be_bytes(valor))),
        _ => None,
    }
}

/// Serializa el centinela.
///
/// **La ausencia del fichero es [`Centinelas::SIN_ESTABLECER`]**, asi que ese
/// estado no se representa en disco: serializarlo produce bytes que su propio
/// analizador rechaza. Ver [`analizar_centinela`].
#[must_use]
pub fn serializar_centinela(centinelas: Centinelas) -> Vec<u8> {
    let mut salida = Vec::with_capacity(LONGITUD_CENTINELA);
    salida.extend_from_slice(MAGICO_CENTINELA);
    salida.extend_from_slice(&VERSION_CENTINELA.to_be_bytes());
    escribir_marca(&mut salida, centinelas.inventario);
    escribir_marca(&mut salida, centinelas.configuracion);
    escribir_ancla(&mut salida, centinelas.huella_recuperacion);
    escribir_ancla(&mut salida, centinelas.huella_operativa);
    salida
}

/// Serializa el ancla de recuperacion: presencia y valor.
fn escribir_ancla(salida: &mut Vec<u8>, huella: Option<IdentificadorClave>) {
    match huella {
        Some(identificador) => {
            salida.push(1);
            salida.extend_from_slice(identificador.resumen().bytes());
        }
        // Los treinta y dos bytes van a cero por lo mismo que los ocho de una
        // marca: dos ficheros que dicen lo mismo tienen que ser el mismo fichero.
        None => salida.extend_from_slice(&[0u8; 33]),
    }
}

/// Lee el ancla de treinta y tres bytes.
///
/// El `None` exterior significa que los bytes no son un ancla valida; el
/// interior, que no hay ancla. Colapsarlos leeria un fichero corrupto como un
/// sensor sin aprovisionar, que es la puerta que este campo cierra.
fn leer_ancla(bytes: &[u8]) -> Option<Option<IdentificadorClave>> {
    let (presencia, valor) = bytes.split_first()?;
    let valor: [u8; 32] = valor.try_into().ok()?;

    match *presencia {
        0 if valor == [0u8; 32] => Some(None),
        1 => Some(Some(IdentificadorClave::desde_resumen(Resumen::desde_bytes(
            valor,
        )))),
        _ => None,
    }
}

/// Analiza el fichero de centinela.
///
/// # Errores
///
/// [`ErrorArranque::CentinelaCorrupto`] ante longitud, magico, version o marcas
/// que no cuadren — incluido **un fichero que no afirma ninguna de las dos
/// series**: eso ya lo dice la ausencia del fichero, y admitir las dos formas
/// dejaria escribir un centinela que no dice nada donde habia uno que decia algo.
pub fn analizar_centinela(bytes: &[u8]) -> Result<Centinelas, ErrorArranque> {
    if bytes.len() != LONGITUD_CENTINELA || &bytes[..8] != MAGICO_CENTINELA {
        return Err(ErrorArranque::CentinelaCorrupto);
    }

    if u16::from_be_bytes([bytes[8], bytes[9]]) != VERSION_CENTINELA {
        return Err(ErrorArranque::CentinelaCorrupto);
    }

    let (Some(inventario), Some(configuracion), Some(huella_recuperacion), Some(huella_operativa)) = (
        leer_marca(&bytes[10..19]),
        leer_marca(&bytes[19..28]),
        leer_ancla(&bytes[28..61]),
        leer_ancla(&bytes[61..94]),
    ) else {
        return Err(ErrorArranque::CentinelaCorrupto);
    };

    let centinelas = Centinelas {
        inventario,
        configuracion,
        huella_recuperacion,
        huella_operativa,
    };

    if centinelas.vacio() {
        return Err(ErrorArranque::CentinelaCorrupto);
    }

    Ok(centinelas)
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
pub fn cargar_centinela(rutas: &RutasAlmacen) -> Result<Centinelas, ErrorArranque> {
    match leer(&rutas.centinela()) {
        Ok(bytes) => analizar_centinela(&bytes),
        Err(ErrorDisco::NoExiste { .. }) => Ok(Centinelas::SIN_ESTABLECER),
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

/// Lo que el agente averigua de su propio almacen al arrancar.
///
/// # Por que es un tipo y no una tupla mas larga
///
/// PA-146a. Hasta hoy esta funcion devolvia `(EstadoArranque, Centinelas)` y
/// **tiraba** el tercer hecho que ya tenia en la mano: si hay clave de
/// recuperacion. Anadirlo como tercer elemento de una tupla habria dejado un
/// `bool` desnudo que en el sitio de destino no dice de que habla.
pub struct Arranque {
    /// En que estado esta el inventario del almacen.
    pub estado: EstadoArranque,
    /// Las dos marcas de frescura, la del inventario y la de configuracion.
    pub centinelas: Centinelas,
    /// Que dice el almacen sobre la clave de recuperacion.
    ///
    /// PA-146a lo saco de esta funcion como booleano —«esta o no esta»—;
    /// PA-150 lo convierte en cuatro estados al darle memoria al centinela.
    /// El booleano no se equivoco: se le acabo el plazo que su propia
    /// documentacion le puso.
    ///
    /// Se saca aqui porque aqui es donde se sabe. Antes se cargaba, se usaba
    /// para las revocaciones, y el hecho de que faltara moria en esta funcion:
    /// `eje-manifiesto generar` lo avisaba por pantalla y nadie mas se enteraba
    /// nunca. Ese aviso llevaba desde el 6 de agosto sin ser un mecanismo, y el
    /// 31 de agosto se pago (RPT-092, RPT-094 §1).
    pub recuperacion: EstadoRecuperacion,
    /// Que dice el almacen sobre la clave **operativa**.
    ///
    /// PA-146b fase 2. La que firma el inventario y la configuracion, y la que
    /// hasta hoy se cargaba del disco sin que nadie comprobara su identidad.
    pub operativa: EstadoOperativa,
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
///
/// **Que el fichero de recuperacion este mal formado aborta el arranque.** Un
/// fichero ilegible no se degrada a «no esta»: eso convertiria corromper unos
/// bytes en una forma de simular una ceremonia pendiente, que es la misma puerta
/// que [`ErrorArranque::CentinelaCorrupto`] cierra para el centinela.
pub fn arrancar_con_almacen(rutas: &RutasAlmacen) -> Result<Arranque, ErrorArranque> {
    let operativa = cargar_clave(&rutas.clave_operativa(), DominioClave::Cliente)?;
    let recuperacion = cargar_clave(
        &rutas.clave_recuperacion(),
        DominioClave::ClienteRecuperacion,
    )?;

    let Some(operativa) = operativa else {
        let centinelas = cargar_centinela(rutas)?;

        // Sin clave no se puede verificar nada, pero el centinela sigue
        // distinguiendo la instalacion a medias del borrado. Si alguna vez se
        // acepto un inventario, hubo clave; que ya no este es supresion.
        //
        // Se mira la marca del INVENTARIO y no las dos: un sensor que solo tuvo
        // configuracion firmada nunca tuvo inventario, y leer su marca como
        // prueba de que lo hubo acusaria de supresion a una instalacion a medias.
        let estado = match centinelas.inventario.secuencia() {
            None => EstadoArranque::SinClaveAprovisionada,
            Some(secuencia_conocida) => EstadoArranque::Supresion { secuencia_conocida },
        };

        return Ok(Arranque {
            estado,
            centinelas,
            recuperacion: EstadoRecuperacion::evaluar(
                centinelas.huella_recuperacion,
                recuperacion.as_ref(),
            ),
            // Sin clave operativa en disco no hay rotacion posible que valorar:
            // o nunca se anclo, o estaba y ya no esta. El registro no cambia
            // ninguno de esos dos, y aqui no se ha cargado.
            operativa: EstadoOperativa::evaluar(
                centinelas.huella_operativa,
                None,
                &RegistroRevocaciones::nuevo(),
            ),
        });
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
    let (estado, centinelas) = arrancar(rutas, &operativa, recuperacion.as_ref())?;

    // PA-146b fase 2. El registro se vuelve a cargar aqui porque `arrancar` lo
    // consume por dentro y no lo devuelve, y cambiarle la firma movia diez
    // llamantes. Son dos lecturas del mismo fichero con microsegundos de
    // diferencia, ambas verificadas contra la clave de recuperacion: si
    // discreparan, alguien estaria reescribiendolo durante el arranque, y eso es
    // un problema distinto y mayor que el coste de leerlo dos veces.
    let revocaciones = cargar_revocaciones(rutas, recuperacion.as_ref())?;

    Ok(Arranque {
        estado,
        centinelas,
        recuperacion: EstadoRecuperacion::evaluar(
            centinelas.huella_recuperacion,
            recuperacion.as_ref(),
        ),
        operativa: EstadoOperativa::evaluar(
            centinelas.huella_operativa,
            Some(&operativa),
            &revocaciones,
        ),
    })
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
) -> Result<(EstadoArranque, Centinelas), ErrorArranque> {
    let centinelas = cargar_centinela(rutas)?;
    let revocaciones = cargar_revocaciones(rutas, recuperacion)?;

    let bytes = match leer(&rutas.inventario()) {
        Ok(bytes) => bytes,
        Err(ErrorDisco::NoExiste { .. }) => {
            // Aqui se decide todo. Ver la tabla del encabezado.
            let estado = match centinelas.inventario.secuencia() {
                None => EstadoArranque::PrimerArranque,
                Some(secuencia_conocida) => EstadoArranque::Supresion { secuencia_conocida },
            };
            return Ok((estado, centinelas));
        }
        Err(error) => return Err(ErrorArranque::Disco(error)),
    };

    match InventarioLocal::cargar(&bytes, operativa, centinelas.inventario, &revocaciones) {
        Ok(inventario) => Ok((EstadoArranque::Operativo(Box::new(inventario)), centinelas)),

        // El formato obsoleto se separa antes de llegar a `NoVerifica`. Sin este
        // brazo, actualizar el agente pareceria un ataque en cada sitio a la vez.
        Err(ErrorCarga::Formato(ErrorFormato::FormatoObsoleto { encontrada })) => {
            Ok((EstadoArranque::FormatoObsoleto { encontrada }, centinelas))
        }

        Err(error) => Ok((
            EstadoArranque::NoVerifica {
                detalle: error.to_string(),
            },
            centinelas,
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
/// # Por que relee el disco antes de escribir
///
/// RPT-078. El fichero de centinela lleva **dos** marcas desde el paso 5. Componer
/// el fichero entero desde la secuencia del inventario —como hacia esta funcion,
/// cuando solo habia una— borraria la marca de configuracion en silencio, y a
/// partir de ahi cualquier configuracion antigua y bien firmada entraria. El
/// merge ocurre aqui y no en quien llama, porque un parametro se puede pasar
/// caducado y esto no.
///
/// # Errores
///
/// [`ErrorArranque::Disco`] si alguna escritura falla, y
/// [`ErrorArranque::CentinelaCorrupto`] si el centinela que hay no se entiende.
/// **No se sobrescribe un centinela corrupto**: puede ser un ataque en curso, y
/// pisarlo destruiria la unica prueba de que lo hubo.
pub fn aceptar_inventario(
    rutas: &RutasAlmacen,
    bytes: &[u8],
    secuencia: u64,
) -> Result<Centinelas, ErrorArranque> {
    let centinelas = cargar_centinela(rutas)?.con_inventario(Centinela::Establecido(secuencia));

    escribir_atomico(&rutas.centinela(), &serializar_centinela(centinelas))?;
    escribir_atomico(&rutas.inventario(), bytes)?;
    Ok(centinelas)
}

/// Anota que se acepto una configuracion firmada con esta secuencia.
///
/// RPT-078, PA-79 paso 5.
///
/// # Solo la marca, y antes de obedecer
///
/// El fichero de configuracion no lo escribe el agente: lo deja el administrador
/// en `/etc/eje-latam`. Aqui solo se avanza la marca, y **antes** de aplicar los
/// parametros, por lo mismo que el centinela se escribe antes que el inventario:
/// si el proceso muere en medio, queda la marca en N y la configuracion N sin
/// aplicar. Al rearrancar se vuelve a presentar con `secuencia == aceptada`, que
/// se admite porque la comparacion es `<` y no `<=`.
///
/// Al reves —obedecer y despues anotar— habria una ventana en la que el sensor ya
/// corre con una configuracion cuya secuencia no consta, que es exactamente donde
/// vive el ataque de reversion.
///
/// # Errores
///
/// [`ErrorArranque::Disco`] o [`ErrorArranque::CentinelaCorrupto`].
pub fn aceptar_configuracion(
    rutas: &RutasAlmacen,
    secuencia: u64,
) -> Result<Centinelas, ErrorArranque> {
    let centinelas = cargar_centinela(rutas)?.con_configuracion(Centinela::Establecido(secuencia));

    escribir_atomico(&rutas.centinela(), &serializar_centinela(centinelas))?;
    Ok(centinelas)
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
