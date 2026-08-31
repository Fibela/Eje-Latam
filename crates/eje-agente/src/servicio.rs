//! Escucha local del agente.
//!
//! RPT-035, PA-41.
//!
//! # El agente pasa a escuchar, y hasta hoy no lo hacia
//!
//! `eje-captura` no tiene metodo de envio; la salida de RPT-032 es de emision
//! pura. Un socket que acepta conexiones es superficie nueva, y conviene decirlo
//! sin adornos antes de enumerar lo que la acota.
//!
//! Lo que la acota:
//!
//! - **No es una red.** Un socket de dominio Unix vive en el sistema de ficheros
//!   y no es alcanzable desde otra maquina. Ninguna pagina web puede conectarse.
//!   RPT-002 §9.3 elimino el WebSocket local justamente por eso.
//! - **Permisos `0600` por omision**, sobre el socket y sobre su directorio. Con
//!   `Acceso::Grupo` pasan a `0660` y el socket cambia de grupo, que es lo que
//!   RPT-002 §9.3 llamaba «con ACL» y no estaba implementado (PA-82). Abrirlo al
//!   grupo es una decision de despliegue explicita, nunca el valor por defecto.
//! - **La lista de permitidos de `eje-ipc` ya existe** y rechaza canal
//!   desconocido y carga excesiva antes de interpretar nada.
//! - **Los dos canales son de consulta.** Ninguno ordena contencion, y RPT-004
//!   §6.2 lo prohibe con prueba.
//!
//! Lo que **no** acota: quien ya ejecute codigo como el usuario del agente puede
//! consultar alertas. Es aceptable —ese atacante ya tiene el registro en disco—
//! y queda escrito para que nadie lo descubra en una auditoria.
//!
//! # Fuera de Unix se declara no soportado, no se finge
//!
//! `std::os::unix::net` no existe en Windows, y `eje-captura` tampoco funciona
//! alli, asi que el demonio es de Linux. Devolver un tipo que no escucha nada
//! seria peor que decirlo.

use std::path::{Path, PathBuf};
use std::time::Duration;

use eje_ipc::{Canal, componer_rechazo, componer_respuesta, descomponer_peticion, enmarcar};

/// Plazo de lectura y escritura por conexion.
///
/// Corto a proposito: una conexion que no habla no puede detener el ciclo de
/// observacion. Con un solo hilo (RPT-034 §3), un cliente lento seria un cliente
/// que apaga la vigilancia.
pub const PLAZO_CONEXION: Duration = Duration::from_millis(250);

/// Conexiones atendidas por ciclo.
///
/// Sin cota, un cliente que reconecta en bucle mantendria el agente sirviendo y
/// sin capturar. Lo que no se atiende este ciclo se atiende el siguiente.
pub const CONEXIONES_POR_CICLO: usize = 16;

/// Fallos de la escucha.
#[derive(Debug, thiserror::Error)]
pub enum ErrorEscucha {
    /// El sistema no ofrece sockets de dominio Unix.
    #[error("la escucha local no esta soportada en esta plataforma")]
    NoSoportado,

    /// Fallo del sistema de ficheros o del socket.
    #[error("{ruta}: {detalle}")]
    Entrada {
        /// Ruta implicada.
        ruta: String,
        /// Causa.
        detalle: String,
    },
}

/// Quien puede conectarse al socket.
///
/// # Por que esto no era una constante
///
/// RPT-002 §9.3 autorizo «socket Unix **con ACL**». Lo que habia era `0600`, y
/// `0600` no es una ACL: es el propietario y nadie mas. La consecuencia no se
/// vio hasta ejecutarlo (RPT-046 §11): la consola solo funciona si corre como el
/// mismo usuario que el agente, y un agente que captura tramas corre como root.
///
/// En un hospital eso significa la consola del operador con `sudo`, que no pasa
/// una revision. De ahi que el acceso pase a ser una decision de despliegue y no
/// una constante del codigo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acceso {
    /// `0600`. Agente y consola son el mismo usuario.
    ///
    /// Sigue siendo lo correcto cuando el agente no necesita privilegios, y es
    /// lo mas restrictivo que se puede pedir.
    SoloPropietario,

    /// `0660` y grupo dado. La consola corre como otro usuario del grupo.
    ///
    /// El identificador es **numerico** y no un nombre: resolver un nombre exige
    /// `getgrnam`, que no esta en la biblioteca estandar, y traer una
    /// dependencia nueva al demonio por esto es peor negocio que dejar la
    /// resolucion al empaquetado, que ya conoce el grupo que crea. Queda PA-84.
    Grupo(u32),
}

/// Lo que el servicio sabe responder.
///
/// Es un rasgo para que las pruebas ejerciten el protocolo sin abrir un socket,
/// por el mismo motivo que [`Despacho`](crate::salida::Despacho): la parte que se
/// puede equivocar en silencio es el formato.
pub trait Atiende {
    /// Responde a una peticion ya autorizada.
    ///
    /// # Errores
    ///
    /// El motivo del rechazo, en texto, para que viaje al otro extremo.
    fn responder(&mut self, canal: Canal, carga: &[u8]) -> Result<Vec<u8>, String>;
}

/// Compone la respuesta completa a una peticion en bruto.
///
/// # Por que esto es una funcion aparte del socket
///
/// Todo el protocolo cabe aqui y se prueba sin red: autorizacion, rechazo con
/// motivo, y marco de salida. El socket solo mueve bytes.
#[must_use]
pub fn atender_peticion(atiende: &mut dyn Atiende, carga: &[u8]) -> Vec<u8> {
    let cuerpo = match descomponer_peticion(carga) {
        Ok((canal, util)) => match atiende.responder(canal, util) {
            Ok(respuesta) => componer_respuesta(&respuesta)
                .unwrap_or_else(|error| componer_rechazo(&error.to_string())),
            Err(motivo) => componer_rechazo(&motivo),
        },

        // Un canal desconocido se rechaza **con motivo**. Cerrar la conexion en
        // silencio dejaria al otro extremo sin saber si el agente no entiende o
        // no esta.
        Err(error) => componer_rechazo(&error.to_string()),
    };

    // Si el cuerpo no cabe en un marco, el rechazo si cabe: nunca se devuelve
    // nada, que es lo unico inaceptable.
    enmarcar(&cuerpo).unwrap_or_else(|_| {
        enmarcar(&componer_rechazo("respuesta demasiado grande")).unwrap_or_default()
    })
}

/// Manejadores de los dos canales, sobre estado ya persistido.
///
/// # Solo lectura, y por eso no hay cerrojo
///
/// RPT-034 §4 exige que una consulta responda con lo que **ya se escribio a
/// disco**, nunca con lo que aun vive solo en memoria. Eso permite que estos
/// manejadores tomen referencias compartidas en lugar de exclusivas, y con ello
/// no hace falta ningun cerrojo — que era el argumento entero del hilo unico.
///
/// # Y por eso se puede atender A MITAD de vuelta. RPT-084, PA-136
///
/// Hasta hoy se atendia solo al final del ciclo, y eso hacia que la latencia de
/// la consola **fuera la ventana de observacion**: con el valor por omision de
/// `--tramas` y un goteo de trafico, vueltas de once segundos y tres de seis
/// canales venciendo el plazo de cinco (RPT-083 §6).
///
/// Atender entre trama y trama sirve el estado de la **vuelta anterior**, que ya
/// esta persistido. Parecia un intercambio —latencia contra frescura— y no lo es:
/// atender al final da un dato fresco que llega once segundos tarde, asi que **la
/// edad del dato que recibe el operador es la misma**. La diferencia es que uno
/// responde y el otro se cuelga.
pub struct Manejadores<'a> {
    /// Registro tal como quedo tras persistir este ciclo.
    pub registro: &'a eje_almacen::RegistroEvidencia,
    /// Condiciones vigentes, ya con el resultado de la salida.
    ///
    /// `None` **solo** en la primera vuelta, antes de que se calcule ninguna.
    /// RPT-084, PA-136: desde que se atiende a mitad de vuelta, una consulta
    /// puede llegar antes de que exista una sola condicion evaluada.
    ///
    /// No se rellena con «todo en falso». Eso diria «este sensor esta sano» sobre
    /// un sensor del que aun no se sabe nada, que es la mentira exacta que
    /// RPT-006 §4 prohibe. Se rechaza con motivo, y **solo el canal que las
    /// necesita**: las alertas ya persistidas se sirven igual.
    pub condiciones: Option<&'a eje_ipc::mensajes::Condiciones>,
    /// Inventario de lo observado, ya en la forma que viaja. RPT-090, PA-138b.
    ///
    /// `None` en la primera vuelta, por el mismo motivo que las condiciones: aun
    /// no se ha observado nada y una lista vacia diria «no hay dispositivos en
    /// esta red», que no es lo mismo que «todavia no he mirado».
    pub inventario: Option<&'a [eje_ipc::mensajes::NodoInventario]>,
    /// Ruta del segmento activo, para saber que hay archivado junto a el.
    ///
    /// PA-74. Sin esto la respuesta no puede decir donde empieza lo que entrega.
    pub evidencia: &'a std::path::Path,
    /// Estado resumido del demonio. RPT-081, PA-135.
    ///
    /// Se compone **una vez, antes del bucle**, porque sus tres campos no
    /// cambian durante la ejecucion: la version es del binario, y el perfil y el
    /// estado de arranque se fijan al arrancar. Recomponerlo en cada vuelta
    /// sugeriria que puede cambiar, y quien lo leyera acabaria preguntandose por
    /// que no cambia nunca.
    pub estado_agente: &'a eje_ipc::mensajes::EstadoAgente,
}

impl Atiende for Manejadores<'_> {
    fn responder(&mut self, canal: Canal, carga: &[u8]) -> Result<Vec<u8>, String> {
        match canal {
            // RPT-081, PA-135. Cuatro de los seis canales estaban declarados y
            // sin cablear; este es el primero que se cablea. El valor no se
            // compone aqui: llega hecho, por lo que dice `Manejadores`.
            Canal::ObtenerEstadoAgente => serde_json::to_vec(self.estado_agente)
                .map_err(|error| format!("no se pudo componer el estado del agente: {error}")),

            Canal::ConsultarAlertas => {
                let peticion: eje_ipc::mensajes::PeticionAlertas = serde_json::from_slice(carga)
                    .map_err(|error| format!("peticion de alertas ilegible: {error}"))?;

                let lote = crate::alertas::consultar(self.registro, &peticion);
                let respuesta = eje_ipc::mensajes::RespuestaAlertas {
                    primer_disponible: crate::alertas::primer_disponible(
                        self.evidencia,
                        self.registro,
                    ),
                    hay_mas: lote.hay_mas,
                    sucesos: lote.sucesos,
                };
                serde_json::to_vec(&respuesta)
                    .map_err(|error| format!("no se pudo serializar la respuesta: {error}"))
            }

            // RPT-090, PA-138b. Tercer canal cableado de los seis.
            Canal::ObtenerInventario => match self.inventario {
                Some(nodos) => serde_json::to_vec(nodos)
                    .map_err(|error| format!("no se pudo componer el inventario: {error}")),

                // Una lista vacia aqui afirmaria que la red esta desierta.
                None => Err("el sensor aun no ha completado su primera vuelta: no ha \
                             observado ningun dispositivo todavia"
                    .to_owned()),
            },

            Canal::ObtenerCondiciones => match self.condiciones {
                Some(vigentes) => serde_json::to_vec(vigentes)
                    .map_err(|error| format!("no se pudo serializar la respuesta: {error}")),

                // RPT-084, PA-136. La primera vuelta todavia no ha terminado.
                // «Todavia no se sabe» no es «no hay nada»: quien pregunte tiene
                // que poder distinguirlo, y con un objeto de trece falsos no
                // podria.
                None => Err("el sensor aun no ha completado su primera vuelta: no hay \
                     condiciones evaluadas todavia"
                    .to_owned()),
            },

            // Los otros cuatro canales pertenecen a modulos que aun no existen.
            // Se rechazan **con motivo** en lugar de devolver una lista vacia,
            // que es la mentira que RPT-006 §4 prohibe: «no hay nada» y «esto
            // todavia no lo sirve nadie» no son lo mismo.
            otro => Err(format!(
                "el canal '{}' esta declarado y aun no tiene manejador en el agente",
                otro.identificador()
            )),
        }
    }
}

/// Escucha local sobre socket de dominio Unix.
pub struct Escucha {
    #[cfg(unix)]
    oyente: std::os::unix::net::UnixListener,
    ruta: PathBuf,
}

impl Escucha {
    /// Abre la escucha en la ruta dada.
    ///
    /// # El socket huerfano se retira al abrir
    ///
    /// Sin captura de senales (RPT-034 §1), un apagado abrupto deja el fichero
    /// del socket en su sitio y `bind` falla con «direccion en uso». Retirarlo
    /// al arrancar es lo que sustituye al apagado limpio.
    ///
    /// Se retira **solo si no hay nadie escuchando**: si otro agente esta vivo
    /// sobre esa ruta, borrarlo lo dejaria sordo sin que se enterase. Se
    /// comprueba conectando.
    ///
    /// # Errores
    ///
    /// [`ErrorEscucha::NoSoportado`] fuera de Unix, [`ErrorEscucha::Entrada`]
    /// ante fallo del sistema de ficheros.
    #[cfg(unix)]
    pub fn abrir(ruta: &Path, acceso: Acceso) -> Result<Self, ErrorEscucha> {
        use std::os::unix::fs::PermissionsExt as _;
        use std::os::unix::net::{UnixListener, UnixStream};

        let fallo = |detalle: String| ErrorEscucha::Entrada {
            ruta: ruta.display().to_string(),
            detalle,
        };

        if ruta.exists() {
            if UnixStream::connect(ruta).is_ok() {
                return Err(fallo("ya hay un agente escuchando aqui".to_owned()));
            }
            std::fs::remove_file(ruta).map_err(|error| fallo(error.to_string()))?;
        }

        let oyente = UnixListener::bind(ruta).map_err(|error| fallo(error.to_string()))?;

        oyente
            .set_nonblocking(true)
            .map_err(|error| fallo(error.to_string()))?;

        // Permisos DESPUES de crear: entre `bind` y este `set_permissions` hay
        // una ventana en la que el socket existe con la mascara del proceso.
        // Se acota poniendo la mascara correcta en el directorio, que es de
        // quien llama, y queda escrito para que nadie suponga que esto basta.
        //
        // Se cierra a `0600` SIEMPRE y primero, incluso cuando se va a abrir al
        // grupo despues. El orden importa: si se pusiera `0660` antes del
        // `chown`, existiria un instante en que el socket es accesible al grupo
        // **por omision del proceso**, que no es el grupo que se pretende
        // autorizar. Cerrar y luego abrir a quien toca no tiene ese hueco.
        std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| fallo(error.to_string()))?;

        if let Acceso::Grupo(gid) = acceso {
            // Falla cerrado: si no se puede dejar el socket como se pidio, se
            // retira. Dejarlo en `0600` seria un agente que dice haber abierto
            // la escucha y una consola que nunca podra conectarse, sin que nadie
            // relacione las dos cosas.
            let deshacer = |detalle: String| {
                let _ = std::fs::remove_file(ruta);
                fallo(detalle)
            };

            std::os::unix::fs::chown(ruta, None, Some(gid)).map_err(|error| {
                deshacer(format!(
                    "no se pudo asignar el grupo {gid} al socket: {error}. \
                     Hace falta que el proceso pertenezca a ese grupo o corra como root"
                ))
            })?;

            std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o660))
                .map_err(|error| deshacer(error.to_string()))?;
        }

        Ok(Self {
            oyente,
            ruta: ruta.to_path_buf(),
        })
    }

    /// Fuera de Unix no hay escucha, y se dice.
    ///
    /// # Errores
    ///
    /// Siempre [`ErrorEscucha::NoSoportado`].
    #[cfg(not(unix))]
    pub fn abrir(ruta: &Path, acceso: Acceso) -> Result<Self, ErrorEscucha> {
        let _ = (ruta, acceso);
        Err(ErrorEscucha::NoSoportado)
    }

    /// Ruta del socket.
    #[must_use]
    pub fn ruta(&self) -> &Path {
        &self.ruta
    }

    /// Atiende las conexiones pendientes sin bloquear.
    ///
    /// Devuelve cuantas atendio. Con un solo hilo, esto va **al final del
    /// ciclo** (RPT-034 §4): una consulta responde con lo que ya se persistio,
    /// nunca con lo que aun vive solo en memoria.
    #[cfg(unix)]
    pub fn atender(&self, atiende: &mut dyn Atiende) -> usize {
        let mut atendidas = 0;

        while atendidas < CONEXIONES_POR_CICLO {
            let Ok((flujo, _)) = self.oyente.accept() else {
                // `WouldBlock` incluido: no hay nadie esperando y se sigue.
                break;
            };

            // Un fallo de una conexion no puede tumbar el ciclo. Se ignora y se
            // pasa a la siguiente: el cliente que quedo sin respuesta reintenta,
            // y la observacion no se detiene por eso.
            let _ = servir(flujo, atiende);
            atendidas += 1;
        }

        atendidas
    }

    /// Fuera de Unix no hay nada que atender.
    #[cfg(not(unix))]
    pub fn atender(&self, atiende: &mut dyn Atiende) -> usize {
        let _ = atiende;
        0
    }
}

impl Drop for Escucha {
    fn drop(&mut self) {
        // Mejor esfuerzo. Con `panic = "abort"` en release esto no corre, y por
        // eso `abrir` retira el huerfano en lugar de confiar en este destructor
        // (RPT-034 §5.3).
        let _ = std::fs::remove_file(&self.ruta);
    }
}

/// Lee una peticion de la conexion, responde y cierra.
#[cfg(unix)]
fn servir(
    mut flujo: std::os::unix::net::UnixStream,
    atiende: &mut dyn Atiende,
) -> std::io::Result<()> {
    // Los rasgos y las constantes se importan **aqui** y no en la cabecera del
    // modulo: fuera de Unix esta funcion no existe, y un `use` a nivel de fichero
    // seria un import sin usar que `-D warnings` rechaza al compilar en Windows.
    //
    // Retirarlos de la cabecera —que fue la primera correccion propuesta—
    // habria compilado en Windows y **roto en Linux**, que es la unica
    // plataforma donde este demonio corre. No se habria visto hasta PA-40.
    use std::io::{Read as _, Write as _};

    use eje_ipc::{LONGITUD_MAXIMA_MARCO, PREFIJO_LONGITUD};

    flujo.set_read_timeout(Some(PLAZO_CONEXION))?;
    flujo.set_write_timeout(Some(PLAZO_CONEXION))?;

    let mut prefijo = [0u8; PREFIJO_LONGITUD];
    flujo.read_exact(&mut prefijo)?;
    let declarados = u32::from_be_bytes(prefijo) as usize;

    // La cota va antes de reservar. Es la misma leccion que `desenmarcar`, y
    // aqui importa mas: el prefijo llega de un socket y no de un `Vec` que ya
    // esta en memoria.
    if declarados > LONGITUD_MAXIMA_MARCO {
        let _ = flujo.write_all(&atender_peticion(atiende, &[]));
        return Ok(());
    }

    let mut carga = vec![0u8; declarados];
    flujo.read_exact(&mut carga)?;

    flujo.write_all(&atender_peticion(atiende, &carga))?;
    flujo.flush()
}

#[cfg(all(test, unix))]
mod pruebas_de_acceso {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use std::os::unix::fs::MetadataExt as _;
    use std::os::unix::fs::PermissionsExt as _;

    use super::{Acceso, Escucha};

    /// Directorio propio por prueba, sin dependencias nuevas.
    fn directorio(nombre: &str) -> std::path::PathBuf {
        let ruta = std::env::temp_dir().join(format!(
            "eje-pa82-{}-{nombre}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&ruta);
        std::fs::create_dir_all(&ruta).expect("temporal");
        ruta
    }

    fn modo(ruta: &std::path::Path) -> u32 {
        // Los bits altos identifican el tipo de fichero; aqui solo interesan los
        // de permiso.
        std::fs::metadata(ruta)
            .expect("existe")
            .permissions()
            .mode()
            & 0o777
    }

    #[test]
    fn sin_grupo_el_socket_queda_solo_para_el_propietario() {
        let dir = directorio("propietario");
        let ruta = dir.join("agente.sock");

        let escucha = Escucha::abrir(&ruta, Acceso::SoloPropietario).expect("abre");

        assert_eq!(
            modo(escucha.ruta()),
            0o600,
            "sin grupo declarado no se abre a nadie mas"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn con_grupo_el_socket_queda_accesible_al_grupo_y_a_nadie_mas() {
        // El grupo se toma del propio proceso: cambiar el grupo de un fichero al
        // grupo al que ya perteneces esta permitido sin privilegios, asi que la
        // prueba corre sin root. Lo que se verifica no es el privilegio, es que
        // el modo final sea 0660 y no 0600 ni 0666.
        let dir = directorio("grupo");
        let ruta = dir.join("agente.sock");
        let gid = std::fs::metadata(&dir).expect("existe").gid();

        let escucha = Escucha::abrir(&ruta, Acceso::Grupo(gid)).expect("abre");

        assert_eq!(modo(escucha.ruta()), 0o660, "el grupo debe poder conectar");
        assert_eq!(
            std::fs::metadata(escucha.ruta()).expect("existe").gid(),
            gid,
            "el socket debe pertenecer al grupo pedido"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn si_el_grupo_no_se_puede_asignar_no_queda_socket_a_medias() {
        // Falla cerrado. Un socket que se queda en 0600 tras pedir grupo es un
        // agente que dice haber abierto la escucha y una consola que jamas
        // conecta, sin que nadie relacione las dos cosas.
        //
        // Se pide un gid al que el proceso no pertenece. Corriendo como root el
        // `chown` funcionaria y no habria fallo que observar; por eso la prueba
        // no exige que falle, exige que **si falla, no deje rastro**. Es una
        // asercion condicionada al entorno y se dice, en lugar de fingir que
        // cubre los dos casos.
        let dir = directorio("fallo");
        let ruta = dir.join("agente.sock");

        match Escucha::abrir(&ruta, Acceso::Grupo(65_533)) {
            Ok(_) => {
                // Entorno privilegiado: el cambio se permitio. `ComprobacionImposible`,
                // no `Conforme` (RPT-006 §4). No se afirma nada.
            }
            Err(_) => assert!(
                !ruta.exists(),
                "tras fallar el grupo no debe quedar el fichero del socket"
            ),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
