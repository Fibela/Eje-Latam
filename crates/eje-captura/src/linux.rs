//! Captura AF_PACKET en Linux.
//!
//! RPT-018 §2. **Este es el unico modulo del workspace donde se admite
//! `unsafe`**, y la lista de lugares donde ocurre esta acotada a proposito:
//!
//! 1. `socket` — abrir el descriptor.
//! 2. `if_nametoindex` — resolver el nombre de interfaz.
//! 3. `bind` — atar a la interfaz.
//! 4. `poll` — esperar con plazo.
//! 5. `recv` — leer una trama.
//! 6. `getsockopt` — leer los contadores del nucleo.
//! 7. `close` — cerrar en `Drop`.
//!
//! Siete llamadas. Cualquier ampliacion de esa lista deberia costar una
//! revision, y por eso se enumera aqui en lugar de dejarla implicita.
//!
//! # Sobre la pasividad
//!
//! Un socket AF_PACKET/SOCK_RAW **puede** transmitir a nivel de nucleo. La
//! garantia de este crate es de tipo: [`SocketPasivo`] no expone envio y el
//! descriptor no sale de aqui.
//!
//! Se intenta ademas `shutdown(SHUT_WR)` como refuerzo, pero **no se apoya la
//! garantia en el**: su soporte para sockets de paquetes no esta asegurado en
//! todas las versiones del nucleo, y prometer una barrera que quiza no exista
//! seria peor que no prometerla. El fallo de esa llamada se ignora
//! deliberadamente.

#![allow(unsafe_code)]

use std::ffi::CString;
use std::io::Error as ErrorSistema;
use std::os::raw::c_int;
use std::time::Duration;

use crate::{ErrorCaptura, Estadisticas, FuentePasiva, LONGITUD_MAXIMA_TRAMA, Trama};

/// Socket AF_PACKET de solo lectura.
pub struct SocketPasivo {
    descriptor: c_int,
    buffer: Vec<u8>,
    recibidas: u64,
    descartadas: u64,
}

/// Convierte el errno actual en un error del crate.
fn error_de_sistema(interfaz: &str) -> ErrorCaptura {
    let error = ErrorSistema::last_os_error();

    match error.raw_os_error() {
        Some(libc::EPERM | libc::EACCES) => ErrorCaptura::PrivilegiosInsuficientes {
            interfaz: interfaz.to_owned(),
        },
        Some(libc::ENODEV | libc::ENXIO) => ErrorCaptura::InterfazNoDisponible {
            interfaz: interfaz.to_owned(),
        },
        _ => ErrorCaptura::Sistema {
            detalle: error.to_string(),
        },
    }
}

impl SocketPasivo {
    /// Abre y ata el socket a la interfaz indicada.
    ///
    /// # Errores
    ///
    /// Ver [`crate::abrir`].
    pub fn abrir(interfaz: &str) -> Result<Self, ErrorCaptura> {
        let nombre = CString::new(interfaz).map_err(|_| ErrorCaptura::InterfazNoDisponible {
            interfaz: interfaz.to_owned(),
        })?;

        // ETH_P_ALL en orden de red. Se escribe explicito para no depender de
        // que libc lo exponga con ese nombre en todas las versiones.
        let protocolo = (libc::ETH_P_ALL as u16).to_be() as c_int;

        // SAFETY: `socket` no recibe punteros. Un fallo devuelve -1 y se
        // comprueba antes de usar el descriptor.
        let descriptor = unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_RAW, protocolo) };
        if descriptor < 0 {
            return Err(error_de_sistema(interfaz));
        }

        let socket = Self {
            descriptor,
            buffer: vec![0u8; LONGITUD_MAXIMA_TRAMA],
            recibidas: 0,
            descartadas: 0,
        };

        // SAFETY: `nombre` es una cadena C valida y viva durante la llamada.
        let indice = unsafe { libc::if_nametoindex(nombre.as_ptr()) };
        if indice == 0 {
            return Err(ErrorCaptura::InterfazNoDisponible {
                interfaz: interfaz.to_owned(),
            });
        }

        let mut direccion: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
        direccion.sll_family = libc::AF_PACKET as u16;
        direccion.sll_protocol = (libc::ETH_P_ALL as u16).to_be();
        direccion.sll_ifindex = indice as c_int;

        // SAFETY: `direccion` esta inicializada y su tamano se pasa exacto. El
        // puntero es valido durante la llamada.
        let atado = unsafe {
            libc::bind(
                socket.descriptor,
                std::ptr::addr_of!(direccion).cast::<libc::sockaddr>(),
                std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            )
        };
        if atado < 0 {
            return Err(error_de_sistema(interfaz));
        }

        // Refuerzo, no garantia. Ver la nota del encabezado del modulo: el
        // resultado se ignora a proposito.
        //
        // SAFETY: el descriptor es valido en este punto.
        let _ = unsafe { libc::shutdown(socket.descriptor, libc::SHUT_WR) };

        Ok(socket)
    }

    /// Consulta los contadores del nucleo y los acumula.
    ///
    /// `PACKET_STATISTICS` **vacia** los contadores al leerlos, asi que se
    /// acumulan aqui. Leerlos sin acumular haria que dos consultas seguidas
    /// dieran cero descartes y la perdida desapareciera de la vista, que es el
    /// defecto que RPT-018 §4 existe para impedir.
    fn absorber_contadores(&mut self) -> Result<(), ErrorCaptura> {
        let mut estadisticas: libc::tpacket_stats = unsafe { std::mem::zeroed() };
        let mut longitud = std::mem::size_of::<libc::tpacket_stats>() as libc::socklen_t;

        // SAFETY: el puntero apunta a una estructura viva del tamano declarado.
        let resultado = unsafe {
            libc::getsockopt(
                self.descriptor,
                libc::SOL_PACKET,
                libc::PACKET_STATISTICS,
                std::ptr::addr_of_mut!(estadisticas).cast::<libc::c_void>(),
                std::ptr::addr_of_mut!(longitud),
            )
        };

        if resultado < 0 {
            return Err(ErrorCaptura::Sistema {
                detalle: ErrorSistema::last_os_error().to_string(),
            });
        }

        self.descartadas = self
            .descartadas
            .saturating_add(u64::from(estadisticas.tp_drops));

        Ok(())
    }
}

impl FuentePasiva for SocketPasivo {
    fn siguiente(&mut self, plazo: Duration) -> Result<Option<Trama>, ErrorCaptura> {
        let mut espera = libc::pollfd {
            fd: self.descriptor,
            events: libc::POLLIN,
            revents: 0,
        };

        let milisegundos = c_int::try_from(plazo.as_millis()).unwrap_or(c_int::MAX);

        // SAFETY: se pasa un solo descriptor y el puntero es valido.
        let listo = unsafe { libc::poll(std::ptr::addr_of_mut!(espera), 1, milisegundos) };

        if listo < 0 {
            return Err(ErrorCaptura::Sistema {
                detalle: ErrorSistema::last_os_error().to_string(),
            });
        }
        if listo == 0 {
            // Red silenciosa: estado normal, no fallo.
            return Ok(None);
        }

        // MSG_TRUNC hace que `recv` devuelva la longitud REAL de la trama aunque
        // el buffer sea menor. Sin el, una trama recortada seria indistinguible
        // de una trama corta, y la huella concluiria que un protocolo no aparece
        // cuando lo que ocurre es que se corto antes.
        //
        // SAFETY: el buffer esta vivo y se pasa su longitud exacta.
        let leidos = unsafe {
            libc::recv(
                self.descriptor,
                self.buffer.as_mut_ptr().cast::<libc::c_void>(),
                self.buffer.len(),
                libc::MSG_TRUNC,
            )
        };

        if leidos < 0 {
            return Err(ErrorCaptura::Sistema {
                detalle: ErrorSistema::last_os_error().to_string(),
            });
        }

        let longitud_en_el_cable = leidos as usize;
        let conservados = longitud_en_el_cable.min(self.buffer.len());

        self.recibidas = self.recibidas.saturating_add(1);
        self.absorber_contadores()?;

        Ok(Some(Trama {
            bytes: self.buffer[..conservados].to_vec(),
            longitud_en_el_cable,
        }))
    }

    fn estadisticas(&self) -> Result<Estadisticas, ErrorCaptura> {
        Ok(Estadisticas {
            recibidas: self.recibidas,
            descartadas: self.descartadas,
        })
    }
}

impl Drop for SocketPasivo {
    fn drop(&mut self) {
        // SAFETY: el descriptor lo abrio este tipo y no se ha cerrado antes.
        unsafe {
            libc::close(self.descriptor);
        }
    }
}
